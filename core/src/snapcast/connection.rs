//! The TCP connection to snapserver's JSON-RPC port, kept up forever.
//!
//! One thread owns the connection: its session, and the socket's writer.
//! Everything it reacts to arrives on one channel: lines from a reader
//! thread, and commands from whoever holds [`Commands`].

use std::io::{self, BufRead, BufReader, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Instant;

use super::control::{Command, control};
use super::{Action, Session, Stream};
use crate::net::{Backoff, connect};

/// What the connection thread is handed.
pub(super) enum Input {
    /// A line from the socket, from connection number `connection`.
    Line {
        connection: u64,
        line: String,
    },
    /// The socket closed or failed.
    Closed {
        connection: u64,
        error: io::Error,
    },
    Command(Command),
}

/// Sends playback commands to the connection, for the stream it follows.
#[derive(Clone)]
pub struct Commands(Sender<Input>);

impl Commands {
    /// Hands `command` to the connection. Never blocks; does nothing once
    /// the connection has gone.
    pub fn send(&self, command: Command) {
        if self.0.send(Input::Command(command)).is_err() && cfg!(debug_assertions) {
            eprintln!("snapcast: not sending {command}: nothing is connected");
        }
    }
}

/// The connection thread's end of [`channel`].
pub struct Inbox {
    rx: Receiver<Input>,
    /// For reader threads. Holding it also keeps `rx` open with no
    /// `Commands` left.
    tx: Sender<Input>,
}

impl Inbox {
    /// Waits until `until` for a command, for replaying a log. Anything
    /// else that arrives is dropped.
    pub fn command(&self, until: Instant) -> Option<Command> {
        loop {
            let timeout = until.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(timeout) {
                Ok(Input::Command(command)) => return Some(command),
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return None,
            }
        }
    }
}

/// A channel for playback commands into a connection.
pub fn channel() -> (Commands, Inbox) {
    let (tx, rx) = mpsc::channel();
    (Commands(tx.clone()), Inbox { rx, tx })
}

/// Follows `client` on the snapserver at `addr` (`host:port`), calling
/// `on_stream` whenever its stream may have changed. While disconnected,
/// reconnects with backoff and reports `None`.
///
/// TCP keepalive, handled by the kernel, notices a server that vanished
/// without closing the connection.
pub fn run(addr: &str, client: &str, on_stream: impl FnMut(Option<&Stream>)) -> ! {
    let (_, inbox) = channel();
    run_with_commands(addr, client, inbox, on_stream)
}

/// As [`run`], also sending the commands that arrive in `inbox`. Commands
/// while disconnected are refused, not kept for the next connection.
pub fn run_with_commands(
    addr: &str,
    client: &str,
    inbox: Inbox,
    mut on_stream: impl FnMut(Option<&Stream>),
) -> ! {
    let mut backoff = Backoff::new();
    let mut connection = 0;
    loop {
        connection += 1;
        let result = follow(addr, client, connection, &inbox, &mut |session, action| {
            backoff.reset();
            if action == Action::Resolved && session.stream().is_none() {
                eprintln!(
                    "snapcast: client {client} not found or not connected, or its stream is missing"
                );
            }
            on_stream(session.stream());
        });
        if let Err(e) = result {
            eprintln!("snapcast: {addr}: {e}");
        }
        on_stream(None);
        let delay = backoff.next_delay();
        eprintln!("snapcast: reconnecting in {}s", delay.as_secs());
        thread::sleep(delay);
    }
}

/// One connection: runs until it fails or the server closes it. Calls
/// `changed` after each line that resolved or changed our stream.
fn follow(
    addr: &str,
    client: &str,
    connection: u64,
    inbox: &Inbox,
    changed: &mut impl FnMut(&Session, Action),
) -> io::Result<()> {
    let socket = Socket(connect(addr)?);
    eprintln!("snapcast: connected to {addr}");
    let reader = socket.0.try_clone()?;
    let tx = inbox.tx.clone();
    thread::spawn(move || read_lines(reader, connection, tx));

    let mut follow = Follow::new(client, connection, &socket.0);
    follow.get_status()?;
    loop {
        // `inbox` holds a sender, so this never fails.
        let input = inbox.rx.recv().expect("inbox sender dropped");
        follow.input(input, changed)?;
    }
}

/// Shuts the socket down when the connection ends, however it ends, so its
/// reader thread's `read()` returns and the thread exits.
struct Socket(TcpStream);

impl Drop for Socket {
    fn drop(&mut self) {
        let _ = self.0.shutdown(Shutdown::Both);
    }
}

/// Forwards `stream`'s lines into the inbox until it closes or fails, or
/// the inbox has gone.
fn read_lines(stream: TcpStream, connection: u64, inbox: Sender<Input>) {
    for line in BufReader::new(stream).lines() {
        let input = match line {
            Ok(line) => Input::Line { connection, line },
            Err(error) => Input::Closed { connection, error },
        };
        let closed = matches!(input, Input::Closed { .. });
        if inbox.send(input).is_err() || closed {
            return;
        }
    }
    let error = io::Error::new(io::ErrorKind::UnexpectedEof, "server closed the connection");
    let _ = inbox.send(Input::Closed { connection, error });
}

/// One connection's state on the connection thread, apart from its socket:
/// what it handles is testable with any writer.
pub(super) struct Follow<W> {
    session: Session,
    connection: u64,
    out: W,
    /// The last request id used, by `Server.GetStatus` and commands alike.
    id: u64,
}

impl<W: Write> Follow<W> {
    pub(super) fn new(client: &str, connection: u64, out: W) -> Self {
        Follow {
            session: Session::new(client),
            connection,
            out,
            id: 0,
        }
    }

    #[cfg(test)]
    pub(super) fn into_out(self) -> W {
        self.out
    }

    pub(super) fn get_status(&mut self) -> io::Result<()> {
        self.id += 1;
        let id = self.id;
        writeln!(
            self.out,
            r#"{{"id":{id},"jsonrpc":"2.0","method":"Server.GetStatus"}}"#
        )
    }

    /// Handles one input. An error ends the connection.
    pub(super) fn input(
        &mut self,
        input: Input,
        changed: &mut impl FnMut(&Session, Action),
    ) -> io::Result<()> {
        match input {
            // From a connection that has ended.
            Input::Line { connection, .. } | Input::Closed { connection, .. }
                if connection != self.connection => {}
            Input::Line { line, .. } => match self.session.handle(&line) {
                Ok(Action::Ignored) => {}
                Ok(Action::Refetch) => self.get_status()?,
                Ok(action) => changed(&self.session, action),
                Err(e) => eprintln!("snapcast: {e}"),
            },
            Input::Closed { error, .. } => return Err(error),
            Input::Command(command) => {
                self.id += 1;
                if let Err(refused) = control(&self.session, command, self.id, &mut self.out)? {
                    eprintln!("snapcast: not sending {command}: {refused}");
                }
            }
        }
        Ok(())
    }
}

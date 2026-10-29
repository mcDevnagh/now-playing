//! Follows Kodi's JSON-RPC notifications, reporting when playback starts,
//! and pauses Kodi when asked.
//!
//! One thread owns the connection and the socket's writer. Everything it
//! reacts to arrives on one channel: messages from a reader thread, and
//! pauses from whoever holds a [`Controller`].

use std::io::{self, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use now_playing_core::net::{Backoff, connect};
use serde_json::{Value, json};

use super::Listener;
use crate::action::Request;
use crate::app::App;

/// Follows Kodi's JSON-RPC TCP port at `address`, asking for `app` on each
/// play or resume, and pauses Kodi when `inbox` is asked to.
pub struct Kodi {
    pub address: String,
    pub app: App,
    pub inbox: Inbox,
}

impl Listener for Kodi {
    /// Kodi often isn't running, so a refused connection is normal: only the
    /// first failure of each outage is logged.
    fn run(&mut self, show: Sender<Request>) -> ! {
        let address = &self.address;
        let mut backoff = Backoff::new();
        let mut quiet = false;
        let mut connection = 0;
        loop {
            match connect(address) {
                Ok(stream) => {
                    eprintln!("kodi: connected to {address}");
                    backoff.reset();
                    connection += 1;
                    let result = follow(stream, connection, &self.inbox, &mut || {
                        eprintln!("kodi: playback started");
                        // The dispatch loop outlives the listeners.
                        let _ = show.send(Request::Show(self.app));
                    });
                    let reason = result.err().map_or("closed".to_owned(), |e| e.to_string());
                    eprintln!("kodi: {address}: disconnected ({reason}); retrying quietly");
                    quiet = true;
                }
                Err(e) if !quiet => {
                    eprintln!("kodi: {address}: {e}; retrying quietly");
                    quiet = true;
                }
                Err(_) => {}
            }
            thread::sleep(backoff.next_delay());
        }
    }
}

/// Everything the connection thread reacts to, on its one channel.
enum Input {
    /// From the reader thread: a message from connection number
    /// `connection`.
    Message { connection: u64, message: Value },
    /// From the reader thread: the socket closed or failed.
    Closed { connection: u64, error: io::Error },
    /// From the dispatch loop, through a `Controller`.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the dispatch loop doesn't pause Kodi yet")
    )]
    Pause,
}

/// What other threads hold to ask the connection to do things: for now only
/// pause. It can't fake a message.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the dispatch loop doesn't pause Kodi yet")
)]
#[derive(Clone)]
pub struct Controller(Sender<Input>);

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the dispatch loop doesn't pause Kodi yet")
)]
impl Controller {
    /// Asks the connection to pause every active player. Never blocks; does
    /// nothing once the connection thread has gone.
    pub fn pause(&self) {
        let _ = self.0.send(Input::Pause);
    }
}

/// The connection thread's end of [`channel`].
pub struct Inbox {
    rx: Receiver<Input>,
    /// For reader threads. Holding it also keeps `rx` open with no
    /// `Controller` left.
    tx: Sender<Input>,
}

impl Inbox {
    /// Drops everything waiting: pauses asked for while Kodi was closed,
    /// and anything left from an old connection.
    fn discard_queued(&self) {
        while self.rx.try_recv().is_ok() {}
    }
}

/// A channel for pauses into a connection.
pub fn channel() -> (Controller, Inbox) {
    let (tx, rx) = mpsc::channel();
    (Controller(tx.clone()), Inbox { rx, tx })
}

/// One connection: runs until it fails or Kodi closes it.
///
/// Pauses waiting in `inbox` are dropped first: Kodi is often not running,
/// and a pause from while it was closed mustn't hit it as it comes up.
fn follow(
    stream: TcpStream,
    connection: u64,
    inbox: &Inbox,
    on_start: &mut impl FnMut(),
) -> io::Result<()> {
    let socket = Socket(stream);
    inbox.discard_queued();
    let reader = socket.0.try_clone()?;
    let tx = inbox.tx.clone();
    thread::spawn(move || read(reader, connection, tx));

    let mut follow = Follow::new(connection, &socket.0);
    loop {
        // `inbox` holds a sender, so this never fails.
        let input = inbox.rx.recv().expect("inbox sender dropped");
        follow.input(input, on_start)?;
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

/// Forwards `stream`'s messages into the inbox until it closes or fails, or
/// the inbox has gone.
fn read(stream: TcpStream, connection: u64, inbox: Sender<Input>) {
    let result = read_messages(stream, |message| {
        let input = Input::Message {
            connection,
            message,
        };
        inbox.send(input).is_ok()
    });
    let error = result.err().unwrap_or_else(|| {
        io::Error::new(io::ErrorKind::UnexpectedEof, "Kodi closed the connection")
    });
    let _ = inbox.send(Input::Closed { connection, error });
}

/// Reads messages until `reader` ends, handing each to `forward` until it
/// returns false. Kodi doesn't separate its messages, so they're split by
/// parsing.
fn read_messages(reader: impl Read, mut forward: impl FnMut(Value) -> bool) -> io::Result<()> {
    let messages = serde_json::Deserializer::from_reader(BufReader::new(reader));
    for message in messages.into_iter::<Value>() {
        if !forward(message?) {
            break;
        }
    }
    Ok(())
}

/// One connection's state on the connection thread, apart from its socket:
/// what it handles is testable with any writer.
struct Follow<W> {
    connection: u64,
    out: W,
    /// The last request id used.
    id: u64,
    /// The ids of `Player.GetActivePlayers` requests not yet answered.
    asked: Vec<u64>,
}

impl<W: Write> Follow<W> {
    fn new(connection: u64, out: W) -> Self {
        Follow {
            connection,
            out,
            id: 0,
            asked: Vec::new(),
        }
    }

    #[cfg(test)]
    fn into_out(self) -> W {
        self.out
    }

    /// Handles one input. An error ends the connection.
    fn input(&mut self, input: Input, on_start: &mut impl FnMut()) -> io::Result<()> {
        match input {
            // From a connection that has ended.
            Input::Message { connection, .. } | Input::Closed { connection, .. }
                if connection != self.connection => {}
            Input::Message { message, .. } => self.message(&message, on_start)?,
            Input::Closed { error, .. } => return Err(error),
            // Which players are active is asked each time: after a
            // reconnect, or with Kodi playing since before we started, no
            // `Player.OnPlay` has told us.
            Input::Pause => {
                self.id += 1;
                let request = json!({
                    "id": self.id,
                    "jsonrpc": "2.0",
                    "method": "Player.GetActivePlayers",
                });
                writeln!(self.out, "{request}")?;
                self.asked.push(self.id);
            }
        }
        Ok(())
    }

    fn message(&mut self, message: &Value, on_start: &mut impl FnMut()) -> io::Result<()> {
        if is_start(message) {
            on_start();
            return Ok(());
        }
        // Only replies have an id.
        let id = &message["id"];
        let asked = id
            .as_u64()
            .and_then(|id| self.asked.iter().position(|&asked| asked == id))
            .map(|i| self.asked.swap_remove(i))
            .is_some();
        if let Some(error) = message.get("error") {
            eprintln!("kodi: error reply to request {id}: {error}");
            return Ok(());
        }
        // Notifications, and `Player.PlayPause` replies.
        if !asked {
            return Ok(());
        }
        let players: Vec<u64> = message["result"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|player| player["playerid"].as_u64())
            .collect();
        if players.is_empty() {
            eprintln!("kodi: nothing to pause");
        }
        for player in players {
            self.id += 1;
            writeln!(self.out, "{}", pause_request(self.id, player))?;
        }
        Ok(())
    }
}

fn is_start(message: &Value) -> bool {
    matches!(
        message["method"].as_str(),
        Some("Player.OnPlay" | "Player.OnResume")
    )
}

/// The request that pauses `player`, and the only place one is made.
///
/// Kodi has no `Player.Pause`: `Player.PlayPause` with `"play": false` sets
/// the speed to 0, and does nothing to a paused player. Without `play` it
/// would toggle.
fn pause_request(id: u64, player: u64) -> String {
    json!({
        "id": id,
        "jsonrpc": "2.0",
        "method": "Player.PlayPause",
        "params": {"playerid": player, "play": false},
    })
    .to_string()
}

#[cfg(test)]
mod tests;

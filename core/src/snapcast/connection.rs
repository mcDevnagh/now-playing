//! The TCP connection to snapserver's JSON-RPC port, kept up forever.

use std::io::{self, BufRead, BufReader, Write};
use std::thread;

use super::{Action, Session, Stream};
use crate::net::{Backoff, connect};

/// Follows `client` on the snapserver at `addr` (`host:port`), calling
/// `on_stream` whenever its stream may have changed. While disconnected,
/// reconnects with backoff and reports `None`.
///
/// Blocks in `read()` between messages. TCP keepalive, handled by the kernel,
/// notices a server that vanished without closing the connection.
pub fn run(addr: &str, client: &str, mut on_stream: impl FnMut(Option<&Stream>)) -> ! {
    let mut backoff = Backoff::new();
    loop {
        let mut session = Session::new(client);
        let result = follow(addr, &mut session, &mut |session, action| {
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
    session: &mut Session,
    changed: &mut impl FnMut(&Session, Action),
) -> io::Result<()> {
    let stream = connect(addr)?;
    eprintln!("snapcast: connected to {addr}");
    let mut writer = stream.try_clone()?;
    let mut id = 0;
    let mut get_status = || {
        id += 1;
        writeln!(
            writer,
            r#"{{"id":{id},"jsonrpc":"2.0","method":"Server.GetStatus"}}"#
        )
    };

    get_status()?;
    for line in BufReader::new(stream).lines() {
        match session.handle(&line?) {
            Ok(Action::Ignored) => {}
            Ok(Action::Refetch) => get_status()?,
            Ok(action) => changed(session, action),
            Err(e) => eprintln!("snapcast: {e}"),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "server closed the connection",
    ))
}

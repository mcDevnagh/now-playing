//! The TCP connection to snapserver's JSON-RPC port, kept up forever.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::thread;
use std::time::Duration;

use socket2::{SockRef, TcpKeepalive};

use super::{Action, Session, Stream};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Follows `client` on the snapserver at `addr` (`host:port`), calling
/// `on_stream` whenever its stream may have changed. While disconnected,
/// reconnects with backoff and reports `None`.
///
/// Blocks in `read()` between messages. TCP keepalive, handled by the kernel,
/// notices a server that vanished without closing the connection.
pub fn run(addr: &str, client: &str, mut on_stream: impl FnMut(Option<&Stream>)) -> ! {
    let mut backoff = MIN_BACKOFF;
    loop {
        let mut session = Session::new(client);
        let result = follow(addr, &mut session, &mut |session, action| {
            backoff = MIN_BACKOFF;
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
        eprintln!("snapcast: reconnecting in {}s", backoff.as_secs());
        thread::sleep(backoff);
        backoff = (backoff * 2).min(MAX_BACKOFF);
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

fn connect(addr: &str) -> io::Result<TcpStream> {
    let mut last = io::Error::new(io::ErrorKind::NotFound, "no address");
    for sockaddr in addr.to_socket_addrs()? {
        match TcpStream::connect_timeout(&sockaddr, CONNECT_TIMEOUT) {
            Ok(stream) => {
                let keepalive = TcpKeepalive::new()
                    .with_time(Duration::from_secs(60))
                    .with_interval(Duration::from_secs(10))
                    .with_retries(3);
                SockRef::from(&stream).set_tcp_keepalive(&keepalive)?;
                return Ok(stream);
            }
            Err(e) => last = e,
        }
    }
    Err(last)
}

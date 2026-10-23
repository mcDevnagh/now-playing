//! TCP connections that are kept up forever: keepalive and reconnect backoff.

use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use socket2::{SockRef, TcpKeepalive};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Connects to `addr` (`host:port`), trying each address it resolves to.
///
/// TCP keepalive, handled by the kernel, notices a peer that vanished without
/// closing the connection.
pub fn connect(addr: &str) -> io::Result<TcpStream> {
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

/// The wait before reconnecting: 1 s, doubling up to 30 s, back to 1 s once a
/// connection works.
#[derive(Debug)]
pub struct Backoff {
    delay: Duration,
}

impl Backoff {
    pub fn new() -> Self {
        Self { delay: MIN_BACKOFF }
    }

    /// Call when a connection works.
    pub fn reset(&mut self) {
        self.delay = MIN_BACKOFF;
    }

    /// The wait before the next attempt; the one after is twice as long.
    pub fn next_delay(&mut self) -> Duration {
        let delay = self.delay;
        self.delay = (delay * 2).min(MAX_BACKOFF);
        delay
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_up_to_the_cap() {
        let mut backoff = Backoff::new();
        let secs: Vec<_> = (0..7).map(|_| backoff.next_delay().as_secs()).collect();
        assert_eq!(secs, [1, 2, 4, 8, 16, 30, 30]);
    }

    #[test]
    fn backoff_resets() {
        let mut backoff = Backoff::new();
        backoff.next_delay();
        backoff.next_delay();
        backoff.reset();
        assert_eq!(backoff.next_delay(), MIN_BACKOFF);
    }
}

//! Downloading covers over HTTP(S).

use std::net::ToSocketAddrs;
use std::time::Duration;

use ureq::Agent;

const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BYTES: u64 = 20 * 1024 * 1024;

pub struct Fetcher {
    agent: Agent,
    fallback_host: String,
}

impl Fetcher {
    /// `fallback_host` replaces a URL's host when that doesn't resolve:
    /// snapserver may give art URLs with its own hostname, which a TV may
    /// not know. Pass the host of the `--server` address.
    pub fn new(fallback_host: &str) -> Fetcher {
        let agent = Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            // Covers come from the LAN or a CDN; never via a proxy from the
            // environment.
            .proxy(None)
            .build()
            .into();
        Fetcher {
            agent,
            fallback_host: fallback_host.to_owned(),
        }
    }

    pub fn fetch(&self, url: &str) -> Result<Vec<u8>, String> {
        let url = match Url::parse(url) {
            Some(parsed) if !parsed.resolves() => parsed.with_host(&self.fallback_host),
            _ => url.to_owned(),
        };
        self.get(&url).map_err(|e| format!("{url}: {e}"))
    }

    fn get(&self, url: &str) -> Result<Vec<u8>, ureq::Error> {
        self.agent
            .get(url)
            .call()?
            .body_mut()
            .with_config()
            .limit(MAX_BYTES)
            .read_to_vec()
    }
}

/// `url` with its host replaced by `host`, keeping the scheme, port and
/// path. `None` if `url` has no `scheme://`.
pub fn with_host(url: &str, host: &str) -> Option<String> {
    Url::parse(url).map(|url| url.with_host(host))
}

/// Just enough URL parsing to swap the host.
struct Url<'a> {
    scheme: &'a str,
    host: &'a str,
    port: Option<&'a str>,
    /// Path, query and fragment.
    rest: &'a str,
}

impl<'a> Url<'a> {
    fn parse(url: &'a str) -> Option<Url<'a>> {
        let (scheme, after) = url.split_once("://")?;
        let end = after.find(['/', '?', '#']).unwrap_or(after.len());
        let (authority, rest) = after.split_at(end);
        let (host, port) = match authority.rsplit_once(':') {
            // Not an IPv6 address's last group, as in `[::1]`.
            Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => {
                (host, Some(port))
            }
            _ => (authority, None),
        };
        Some(Url {
            scheme,
            host,
            port,
            rest,
        })
    }

    fn resolves(&self) -> bool {
        let default_port = if self.scheme == "https" { 443 } else { 80 };
        let port = self
            .port
            .and_then(|p| p.parse().ok())
            .unwrap_or(default_port);
        let host = self.host.trim_start_matches('[').trim_end_matches(']');
        (host, port)
            .to_socket_addrs()
            .is_ok_and(|mut addrs| addrs.next().is_some())
    }

    fn with_host(&self, host: &str) -> String {
        match self.port {
            Some(port) => format!("{}://{host}:{port}{}", self.scheme, self.rest),
            None => format!("{}://{host}{}", self.scheme, self.rest),
        }
    }
}

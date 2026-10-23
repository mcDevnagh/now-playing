//! What both binaries share: following a Snapcast client, and the TCP
//! plumbing for connections that stay up.

pub mod net;
pub mod snapcast;
pub mod state;

#[cfg(test)]
mod fixtures;

/// This machine's hostname, the default Snapcast client to follow.
pub fn hostname() -> Result<String, String> {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|name| name.trim().to_owned())
        .map_err(|e| e.to_string())
}

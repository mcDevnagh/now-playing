//! Snapserver captures shared by the unit tests.

use crate::snapcast::Server;
use serde_json::Value;

pub const META_LOG: &str = include_str!("../tests/fixtures/meta-librespot-events.log");
pub const BUILTIN_LOG: &str = include_str!("../tests/fixtures/builtin-events.log");
pub const BUILTIN_STATUS: &str = include_str!("../tests/fixtures/builtin-status.json");

/// The server status in a `Server.GetStatus` response.
pub fn status(json: &str) -> Server {
    let msg: Value = serde_json::from_str(json).unwrap();
    serde_json::from_value(msg["result"]["server"].clone()).unwrap()
}

/// The status at the start of `META_LOG`: the HTPC playing "Track A" with
/// full metadata.
pub fn meta_status() -> Server {
    status(META_LOG.lines().next().unwrap().split_once('\t').unwrap().1)
}

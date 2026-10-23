//! Snapserver captures and hand-written logs shared by the unit tests.
//!
//! Logs have one message per line, as `time<TAB>json`.

use crate::snapcast::{Action, Server, Session};
use crate::state::{State, now_playing};
use serde_json::Value;

/// Captured live with `meta_librespot.py`.
pub const META_LOG: &str = include_str!("../tests/fixtures/meta-librespot-events.log");
/// Captured live from the built-in librespot source.
pub const BUILTIN_LOG: &str = include_str!("../tests/fixtures/builtin-events.log");
pub const BUILTIN_STATUS: &str = include_str!("../tests/fixtures/builtin-status.json");
/// Hand-written: not seen live yet.
pub const GROUP_SWITCH_LOG: &str = include_str!("../tests/fixtures/group-switch.log");
/// Hand-written: not seen live yet.
pub const META_SWITCH_LOG: &str = include_str!("../tests/fixtures/meta-switch.log");
/// Hand-written: not seen live yet.
pub const SPARSE_LOG: &str = include_str!("../tests/fixtures/sparse.log");

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

/// Feeds a log to a session following the HTPC, returning the action and
/// the HTPC's state after each line. A `Refetch` is answered by the log's
/// next line.
pub fn replay(log: &str) -> Vec<(Action, State)> {
    let mut session = Session::new("HTPC");
    log.lines()
        .map(|line| {
            let (_, json) = line.split_once('\t').unwrap();
            let action = session.handle(json).unwrap();
            (action, session.stream().map_or(State::Idle, now_playing))
        })
        .collect()
}

/// Title, playing or paused, and position, for compact assertions.
pub fn summary(state: &State) -> Option<(&str, bool, Option<f64>)> {
    match state {
        State::Idle => None,
        State::Playing(t) => Some((&t.title, true, t.position)),
        State::Paused(t) => Some((&t.title, false, t.position)),
    }
}

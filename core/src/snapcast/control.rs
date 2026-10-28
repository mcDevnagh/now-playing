//! Playback commands for the stream our client follows.

use serde_json::json;

/// A remote's media key, as a `Stream.Control` command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    PlayPause,
    Next,
    Previous,
}

impl Command {
    /// Snapserver's name for it.
    fn name(self) -> &'static str {
        match self {
            Command::PlayPause => "playPause",
            Command::Next => "next",
            Command::Previous => "previous",
        }
    }
}

/// The `Stream.Control` request for `command` on `stream`.
#[cfg_attr(not(test), expect(dead_code, reason = "not sent yet"))]
pub(super) fn control_request(id: u64, stream: &str, command: Command) -> String {
    json!({
        "id": id,
        "jsonrpc": "2.0",
        "method": "Stream.Control",
        "params": {"id": stream, "command": command.name()},
    })
    .to_string()
}

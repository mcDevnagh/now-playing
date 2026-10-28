//! Playback commands for the stream our client follows.

use std::fmt;
use std::io::{self, Write};

use serde_json::json;

use super::Session;

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

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Why a command wasn't sent.
#[derive(Debug, Clone, PartialEq)]
pub enum Refused {
    /// The client has no stream: not yet, or not any more.
    NoStream,
    /// The stream doesn't report `flag`.
    NotAllowed { stream: String, flag: &'static str },
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Refused::NoStream => f.write_str("no stream to control"),
            Refused::NotAllowed { stream, flag } => {
                write!(f, "stream {stream} doesn't allow it (no {flag})")
            }
        }
    }
}

/// Writes the `Stream.Control` line for `command` to `out`, with request id
/// `id`, or says why it isn't allowed.
///
/// The target is the session's stream now. Play/pause is allowed if the
/// stream can play or pause, without guessing which the press means: the
/// stream's playback status can be stale, and if the server can't do it, its
/// error reply is logged.
pub fn control(
    session: &Session,
    command: Command,
    id: u64,
    out: &mut impl Write,
) -> io::Result<Result<(), Refused>> {
    let Some(stream) = session.stream() else {
        return Ok(Err(Refused::NoStream));
    };
    let p = &stream.properties;
    let missing = if !p.can_control {
        Some("canControl")
    } else {
        match command {
            Command::PlayPause if !(p.can_play || p.can_pause) => Some("canPlay or canPause"),
            Command::Next if !p.can_go_next => Some("canGoNext"),
            Command::Previous if !p.can_go_previous => Some("canGoPrevious"),
            _ => None,
        }
    };
    if let Some(flag) = missing {
        let stream = stream.id.clone();
        return Ok(Err(Refused::NotAllowed { stream, flag }));
    }
    writeln!(out, "{}", control_request(id, &stream.id, command))?;
    Ok(Ok(()))
}

/// The `Stream.Control` request for `command` on `stream`.
pub(super) fn control_request(id: u64, stream: &str, command: Command) -> String {
    json!({
        "id": id,
        "jsonrpc": "2.0",
        "method": "Stream.Control",
        "params": {"id": stream, "command": command.name()},
    })
    .to_string()
}

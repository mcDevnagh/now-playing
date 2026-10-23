//! Pure decisions: when Snapcast counts as having started, and whether to
//! act.

use std::fmt;

use now_playing_core::snapcast::Stream;
use now_playing_core::state::{State, now_playing};

/// Whether our client's stream is playing; `None` while it's unknown
/// (disconnected, or the client is missing).
pub fn playing(stream: Option<&Stream>) -> Option<bool> {
    stream.map(|stream| matches!(now_playing(stream), State::Playing(_)))
}

/// Finds our Snapcast client starting to play in a series of [`playing`]
/// values.
#[derive(Debug, Default)]
pub struct SnapcastEdges {
    last: Option<bool>,
}

impl SnapcastEdges {
    /// Takes the next value; true if playback just started.
    ///
    /// Only not playing → playing counts. The first value after `None` is a
    /// baseline, so a reconnect, a snapclient restart or starting mid-song
    /// doesn't count.
    pub fn update(&mut self, playing: Option<bool>) -> bool {
        let started = self.last == Some(false) && playing == Some(true);
        self.last = playing;
        started
    }
}

/// Why a trigger's actions were skipped.
#[derive(Debug, PartialEq, Eq)]
pub enum Skip {
    /// The display has no `GAMESCOPE_FOCUSED_APP`, so it isn't gamescope.
    NotGamingMode,
    /// The display couldn't be read.
    Display(String),
}

impl fmt::Display for Skip {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Skip::NotGamingMode => f.write_str("not in Gaming Mode"),
            Skip::Display(e) => write!(f, "can't read the display: {e}"),
        }
    }
}

/// The app in front, from reading `GAMESCOPE_FOCUSED_APP`, or why to skip
/// the actions.
pub fn front_app(focused: Result<Option<u32>, String>) -> Result<u32, Skip> {
    match focused {
        Ok(Some(app)) => Ok(app),
        Ok(None) => Err(Skip::NotGamingMode),
        Err(e) => Err(Skip::Display(e)),
    }
}

#[cfg(test)]
mod tests;

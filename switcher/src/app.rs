//! An app the switcher can bring to the front: its gamescope ID, and the
//! Steam game ID that launches it, if known.

use std::fmt;

/// What a listener sends when its app should come to the front.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct App {
    /// The app's gamescope ID, also its windows' `STEAM_GAME`.
    pub gamescope: u32,
    /// The Steam game ID that launches it; without it the app can't be
    /// launched.
    pub launch: Option<u64>,
}

impl fmt::Display for App {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "app {}", self.gamescope)
    }
}

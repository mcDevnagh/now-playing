//! An app the switcher can bring to the front: its gamescope ID, and the
//! Steam launch ID that starts it, if known.

use std::fmt;

/// What a listener sends when its app should come to the front.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct App {
    /// The app's gamescope ID, also its windows' `STEAM_GAME`.
    pub gamescope: u32,
    /// The Steam launch ID; without it the app can't be launched.
    pub launch: Option<u64>,
}

impl App {
    /// From a config table's `app` and `launch`. A launch ID's top 32 bits
    /// are its gamescope ID, so either is enough; if both are given they
    /// must agree.
    pub fn new(gamescope: Option<u32>, launch: Option<u64>) -> Result<Self, String> {
        let from_launch = launch.map(|launch| (launch >> 32) as u32);
        let gamescope = match (gamescope, from_launch) {
            (Some(app), Some(from_launch)) if app != from_launch => {
                return Err(format!(
                    "`app` {app} doesn't match `launch`, whose gamescope ID is {from_launch}"
                ));
            }
            (Some(app), _) | (None, Some(app)) => app,
            (None, None) => return Err("needs `app`, `launch` or both".to_owned()),
        };
        Ok(Self { gamescope, launch })
    }
}

impl fmt::Display for App {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "app {}", self.gamescope)
    }
}

#[cfg(test)]
mod tests;

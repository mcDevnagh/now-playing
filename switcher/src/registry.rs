//! What the config asks for: which listeners to start, and which actions
//! each trigger runs, in order.

use std::fmt;

use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// Music started on our Snapcast client.
    MusicStarted,
    /// Kodi started or resumed playback.
    KodiStarted,
}

impl fmt::Display for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Trigger::MusicStarted => "music started",
            Trigger::KodiStarted => "Kodi started",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Bring this gamescope ID to the front, if it's running.
    Focus(u32),
    /// Launch this Steam launch ID, if it isn't running.
    Launch(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Listener {
    Snapcast {
        server: String,
        client: Option<String>,
    },
    Kodi {
        address: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct Registry {
    pub listeners: Vec<Listener>,
    music: Vec<Action>,
    kodi: Vec<Action>,
}

impl Registry {
    pub fn new(config: &Config) -> Self {
        let mut registry = Self {
            listeners: Vec::new(),
            music: Vec::new(),
            kodi: Vec::new(),
        };
        if let Some(snapcast) = &config.snapcast {
            registry.listeners.push(Listener::Snapcast {
                server: snapcast.server.clone(),
                client: snapcast.client.clone(),
            });
            // Focus first: launching is only for when it isn't running.
            registry.music.extend(snapcast.app.map(Action::Focus));
            registry.music.extend(snapcast.launch.map(Action::Launch));
        }
        if let Some(kodi) = &config.kodi {
            registry.listeners.push(Listener::Kodi {
                address: kodi.address.clone(),
            });
            registry.kodi.push(Action::Focus(kodi.app));
        }
        registry
    }

    /// The actions for `trigger`, in the order to run them.
    pub fn actions(&self, trigger: Trigger) -> &[Action] {
        match trigger {
            Trigger::MusicStarted => &self.music,
            Trigger::KodiStarted => &self.kodi,
        }
    }
}

#[cfg(test)]
mod tests;

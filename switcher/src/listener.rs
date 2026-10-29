//! Listeners: each follows one player on its own thread, and reports when
//! that player starts.

pub mod kodi;
pub mod snapcast;
pub mod uxplay;

use std::sync::mpsc::Sender;

use now_playing_core::{hostname, snapcast as core_snapcast};

use crate::action::Request;
use crate::config::Config;
use crate::quiet::Others;

/// Follows a player, sending [`Request::PlaybackStarted`] on `requests`
/// whenever it starts. Holds everything it needs, such as addresses; the
/// rules for what counts as a start are its own.
pub trait Listener: Send {
    /// Runs forever, reconnecting as needed.
    fn run(self: Box<Self>, requests: Sender<Request>) -> !;
}

/// The listeners `config` asks for, and what their starts do to each other.
///
/// Without `quiet_others`, there's one listener per table that sets
/// `show_on_playback`, and `Others` is empty. With it, every table gets a
/// listener, since a table that doesn't show its app still names a player
/// to quiet and be quieted by; its listener's `app` is `None`.
pub fn from_config(config: &Config) -> Result<(Vec<Box<dyn Listener>>, Others), String> {
    let quiet = config.quiet_others;
    let mut listeners: Vec<Box<dyn Listener>> = Vec::new();
    let mut kodi_controller = None;
    let mut snapcast_controller = None;
    if let Some(table) = config
        .snapcast
        .as_ref()
        .filter(|t| t.show_on_playback || quiet)
    {
        let client = match &table.client {
            Some(client) => client.clone(),
            None => {
                hostname().map_err(|e| format!("no snapcast.client given and no hostname: {e}"))?
            }
        };
        let (controller, inbox) = core_snapcast::channel();
        snapcast_controller = quiet.then_some(controller);
        listeners.push(Box::new(snapcast::Snapcast {
            server: table.server.clone(),
            client,
            app: table.show_on_playback.then_some(table.app),
            inbox,
        }));
    }
    if let Some(table) = config.kodi.as_ref().filter(|t| t.show_on_playback || quiet) {
        let (controller, inbox) = kodi::channel();
        kodi_controller = quiet.then_some(controller);
        listeners.push(Box::new(kodi::Kodi {
            address: table.address.clone(),
            app: table.show_on_playback.then_some(table.app),
            inbox,
        }));
    }
    if let Some(table) = config
        .uxplay
        .as_ref()
        .filter(|t| t.show_on_playback || quiet)
    {
        listeners.push(Box::new(uxplay::Uxplay {
            window_class: table.window_class.clone(),
            gamescope: table.app.gamescope,
            app: table.show_on_playback.then_some(table.app),
        }));
    }
    let others = Others::new(kodi_controller, snapcast_controller);
    Ok((listeners, others))
}

#[cfg(test)]
mod tests;

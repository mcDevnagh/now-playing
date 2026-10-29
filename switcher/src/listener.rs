//! Listeners: each follows one player on its own thread, and reports when
//! that player starts.

pub mod kodi;
pub mod snapcast;
pub mod uxplay;

use std::sync::mpsc::Sender;

use now_playing_core::{hostname, snapcast as core_snapcast};

use crate::action::Request;
use crate::config::Config;
use crate::quiet::{Others, Player};

/// Follows a player, sending [`Request::PlaybackStarted`] on `requests`
/// whenever it starts. Holds everything it needs, such as addresses; the
/// rules for what counts as a start are its own.
pub trait Listener: Send {
    /// Runs forever, reconnecting as needed.
    fn run(self: Box<Self>, requests: Sender<Request>) -> !;
}

/// The listeners `config` asks for, and what their starts do to the
/// players.
///
/// A table gets a listener when there's something to do on its player's
/// starts, or its player may be quieted: Snapcast's always, as its starts
/// unmute our client; Kodi's if it shows on playback, or any table quiets
/// the others; UxPlay's if it shows on playback or quiets the others. A
/// listener's `app` is `None` without `show_on_playback`.
pub fn from_config(config: &Config) -> Result<(Vec<Box<dyn Listener>>, Others), String> {
    let quieting: Vec<Player> = [
        (Player::Kodi, config.kodi.as_ref().map(|t| t.quiet_others)),
        (
            Player::Snapcast,
            config.snapcast.as_ref().map(|t| t.quiet_others),
        ),
        (
            Player::Uxplay,
            config.uxplay.as_ref().map(|t| t.quiet_others),
        ),
    ]
    .into_iter()
    .filter_map(|(player, quiet)| quiet?.then_some(player))
    .collect();
    let mut listeners: Vec<Box<dyn Listener>> = Vec::new();
    let mut kodi_controller = None;
    let mut snapcast_controller = None;
    if let Some(table) = &config.snapcast {
        let client = match &table.client {
            Some(client) => client.clone(),
            None => {
                hostname().map_err(|e| format!("no snapcast.client given and no hostname: {e}"))?
            }
        };
        let (controller, inbox) = core_snapcast::channel();
        snapcast_controller = Some(controller);
        listeners.push(Box::new(snapcast::Snapcast {
            server: table.server.clone(),
            client,
            app: table.show_on_playback.then_some(table.app),
            inbox,
        }));
    }
    let kodi = config.kodi.as_ref();
    if let Some(table) = kodi.filter(|t| t.show_on_playback || !quieting.is_empty()) {
        let (controller, inbox) = kodi::channel();
        kodi_controller = Some(controller);
        listeners.push(Box::new(kodi::Kodi {
            address: table.address.clone(),
            app: table.show_on_playback.then_some(table.app),
            inbox,
        }));
    }
    let uxplay = config.uxplay.as_ref();
    if let Some(table) = uxplay.filter(|t| t.show_on_playback || t.quiet_others) {
        listeners.push(Box::new(uxplay::Uxplay {
            window_class: table.window_class.clone(),
            gamescope: table.app.gamescope,
            app: table.show_on_playback.then_some(table.app),
        }));
    }
    let others = Others::new(kodi_controller, snapcast_controller, quieting);
    Ok((listeners, others))
}

#[cfg(test)]
mod tests;

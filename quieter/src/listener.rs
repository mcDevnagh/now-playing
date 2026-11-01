//! Listeners: each follows one player on its own thread, and reports when
//! that player starts, and what else the dispatch loop acts on.

pub mod kodi;
pub mod snapcast;
pub mod uxplay;

use std::sync::mpsc::Sender;

use now_playing_core::{hostname, snapcast as core_snapcast};

use crate::action::{Dispatch, Request};
use crate::config::Config;
use crate::quiet::{Others, Player};

/// Follows a player, sending [`Request::Started`] on `requests` whenever
/// it starts, and any other requests it makes. Holds everything it needs, such as addresses; the rules for
/// what counts as a start are its own.
pub trait Listener: Send {
    /// Runs forever, reconnecting as needed.
    fn run(self: Box<Self>, requests: Sender<Request>) -> !;
}

/// The listeners `config` asks for, and the dispatch loop that acts on
/// what they report. `display` is `$DISPLAY`, where UxPlay's windows open.
///
/// A table gets a listener when there's something to do on its player's
/// starts, or its player may be quieted or kept awake: Snapcast's always,
/// as its starts unmute our client; UxPlay's if it quiets the others, or
/// keeps a Kodi with a table awake; Kodi's if any table quiets the others,
/// UxPlay's keeps it awake, or it has a screensaver.
pub fn from_config(
    config: &Config,
    display: Option<String>,
) -> Result<(Vec<Box<dyn Listener>>, Dispatch), String> {
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
            inbox,
        }));
    }
    let keep_awake =
        config.kodi.is_some() && config.uxplay.as_ref().is_some_and(|t| t.keep_kodi_awake);
    let kodi = config.kodi.as_ref();
    let screensaver = kodi.and_then(|t| t.screensaver.clone());
    if let Some(table) =
        kodi.filter(|_| !quieting.is_empty() || keep_awake || screensaver.is_some())
    {
        let (controller, inbox) = kodi::channel();
        kodi_controller = Some(controller);
        listeners.push(Box::new(kodi::Kodi {
            address: table.address.clone(),
            inbox,
        }));
    }
    let uxplay = config.uxplay.as_ref();
    if let Some(table) = uxplay.filter(|t| t.quiet_others || keep_awake) {
        let display = display.ok_or(
            "[uxplay] needs $DISPLAY, the display UxPlay's windows open on; kodi-steam passes it down",
        )?;
        listeners.push(Box::new(uxplay::Uxplay {
            window_class: table.window_class.clone(),
            display,
        }));
    }
    let others = Others::new(kodi_controller.clone(), snapcast_controller, quieting);
    let dispatch = Dispatch::new(others, kodi_controller, keep_awake, screensaver);
    Ok((listeners, dispatch))
}

#[cfg(test)]
mod tests;

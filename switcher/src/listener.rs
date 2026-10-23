//! Listeners: each follows one player on its own thread, and asks for its
//! app to be shown when that player starts.

pub mod kodi;
pub mod snapcast;

use std::sync::mpsc::Sender;

use now_playing_core::hostname;

use crate::app::App;
use crate::config::Config;

/// Follows a player, sending its [`App`] on `show` whenever the app should
/// come to the front. Holds everything it needs, such as addresses; the
/// rules for when to ask are its own.
pub trait Listener: Send {
    /// Runs forever, reconnecting as needed.
    fn run(&mut self, show: Sender<App>) -> !;
}

/// The listeners `config` asks for.
pub fn from_config(config: &Config) -> Result<Vec<Box<dyn Listener>>, String> {
    let mut listeners: Vec<Box<dyn Listener>> = Vec::new();
    if let Some(table) = &config.snapcast {
        let client = match &table.client {
            Some(client) => client.clone(),
            None => {
                hostname().map_err(|e| format!("no snapcast.client given and no hostname: {e}"))?
            }
        };
        listeners.push(Box::new(snapcast::Snapcast {
            server: table.server.clone(),
            client,
            app: table.app,
        }));
    }
    if let Some(table) = &config.kodi {
        listeners.push(Box::new(kodi::Kodi {
            address: table.address.clone(),
            app: table.app,
        }));
    }
    Ok(listeners)
}

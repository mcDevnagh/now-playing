//! Follows our Snapcast client, reporting when it starts playing, and mutes
//! or unmutes it when asked.

use std::sync::mpsc::Sender;

use now_playing_core::snapcast::{self, Inbox, Stream};
use now_playing_core::state::{State, now_playing};

use super::Listener;
use crate::action::Request;
use crate::app::App;
use crate::quiet::Player;

/// Follows `client` on the snapserver at `server`, reporting with `app`
/// whenever the client starts playing, and sending it the mutes that arrive
/// in `inbox`.
pub struct Snapcast {
    pub server: String,
    pub client: String,
    /// The app to show on a start; `None` if the table doesn't switch.
    pub app: Option<App>,
    pub inbox: Inbox,
}

impl Listener for Snapcast {
    fn run(self: Box<Self>, requests: Sender<Request>) -> ! {
        let Snapcast {
            server,
            client,
            app,
            inbox,
        } = *self;
        eprintln!("snapcast: following client {client} on {server}");
        let mut edges = SnapcastEdges::default();
        snapcast::run_with_controller(&server, &client, inbox, |stream| {
            if edges.update(playing(stream)) {
                eprintln!("snapcast: client {client} started playing");
                // The dispatch loop outlives the listeners.
                let _ = requests.send(Request::Started {
                    player: Player::Snapcast,
                    app,
                });
            }
        })
    }
}

/// Whether our client's stream is playing; `None` while it's unknown
/// (disconnected, or the client is missing).
fn playing(stream: Option<&Stream>) -> Option<bool> {
    stream.map(|stream| matches!(now_playing(stream), State::Playing(_)))
}

/// Finds our client starting to play in a series of [`playing`] values.
#[derive(Debug, Default)]
struct SnapcastEdges {
    last: Option<bool>,
}

impl SnapcastEdges {
    /// Takes the next value; true if playback just started.
    ///
    /// Only not playing → playing counts. The first value after `None` is a
    /// baseline, so a reconnect, a snapclient restart or starting mid-song
    /// doesn't count.
    fn update(&mut self, playing: Option<bool>) -> bool {
        let started = self.last == Some(false) && playing == Some(true);
        self.last = playing;
        started
    }
}

#[cfg(test)]
mod tests;

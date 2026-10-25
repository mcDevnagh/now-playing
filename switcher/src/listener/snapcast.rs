//! Follows our Snapcast client, asking for its app when it starts playing.

use std::sync::mpsc::Sender;

use now_playing_core::snapcast::{self, Stream};
use now_playing_core::state::{State, now_playing};

use super::Listener;
use crate::action::Request;
use crate::app::App;

/// Follows `client` on the snapserver at `server`, asking for `app` whenever
/// the client starts playing.
pub struct Snapcast {
    pub server: String,
    pub client: String,
    pub app: App,
}

impl Listener for Snapcast {
    fn run(&mut self, show: Sender<Request>) -> ! {
        let client = &self.client;
        eprintln!("snapcast: following client {client} on {}", self.server);
        let mut edges = SnapcastEdges::default();
        snapcast::run(&self.server, client, |stream| {
            if edges.update(playing(stream)) {
                eprintln!("snapcast: client {client} started playing");
                // The dispatch loop outlives the listeners.
                let _ = show.send(Request::Show(self.app));
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

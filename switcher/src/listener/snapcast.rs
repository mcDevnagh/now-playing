//! Follows our Snapcast client, reporting when it starts playing.

use std::sync::mpsc::Sender;
use std::thread;

use now_playing_core::snapcast;

use crate::registry::Trigger;
use crate::rules::{SnapcastEdges, playing};

/// Starts a thread that follows `client` on the snapserver at `server`,
/// sending [`Trigger::SnapcastStarted`] whenever it starts playing.
/// Reconnects forever.
pub fn spawn(server: String, client: String, triggers: Sender<Trigger>) {
    thread::spawn(move || {
        eprintln!("snapcast: following client {client} on {server}");
        let mut edges = SnapcastEdges::default();
        snapcast::run(&server, &client, |stream| {
            if edges.update(playing(stream)) {
                // The dispatch loop outlives the listeners.
                let _ = triggers.send(Trigger::SnapcastStarted);
            }
        });
    });
}

//! Follows our Snapcast client, reporting whether it's playing.

use std::sync::mpsc::Sender;
use std::thread;

use now_playing_core::snapcast;

use super::Event;
use crate::rules::playing;

/// Starts a thread that follows `client` on the snapserver at `server`,
/// sending [`Event::Music`] whenever playing changes. Reconnects forever.
pub fn spawn(server: String, client: String, events: Sender<Event>) {
    thread::spawn(move || {
        eprintln!("snapcast: following client {client} on {server}");
        let mut last = None;
        snapcast::run(&server, &client, |stream| {
            let now = playing(stream);
            if now != last {
                last = now;
                // The dispatch loop outlives the listeners.
                let _ = events.send(Event::Music(now));
            }
        });
    });
}

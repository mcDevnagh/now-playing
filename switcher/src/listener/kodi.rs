//! Follows Kodi's JSON-RPC notifications, reporting when playback starts.

use std::io::{self, BufReader, Read};
use std::sync::mpsc::Sender;
use std::thread;

use now_playing_core::net::{Backoff, connect};
use serde_json::Value;

use super::Event;

/// Starts a thread that connects to Kodi's JSON-RPC TCP port at `address`,
/// sending [`Event::KodiStarted`] on each play or resume. Reconnects forever.
///
/// Kodi often isn't running, so a refused connection is normal: only the
/// first failure of each outage is logged.
pub fn spawn(address: String, events: Sender<Event>) {
    thread::spawn(move || {
        let mut backoff = Backoff::new();
        let mut quiet = false;
        loop {
            match connect(&address) {
                Ok(stream) => {
                    eprintln!("kodi: connected to {address}");
                    backoff.reset();
                    let result = follow(stream, || {
                        // The dispatch loop outlives the listeners.
                        let _ = events.send(Event::KodiStarted);
                    });
                    let reason = result.err().map_or("closed".to_owned(), |e| e.to_string());
                    eprintln!("kodi: {address}: disconnected ({reason}); retrying quietly");
                    quiet = true;
                }
                Err(e) if !quiet => {
                    eprintln!("kodi: {address}: {e}; retrying quietly");
                    quiet = true;
                }
                Err(_) => {}
            }
            thread::sleep(backoff.next_delay());
        }
    });
}

/// Reads notifications until the connection ends, calling `on_start` for
/// each play or resume. Kodi doesn't separate its messages, so they're split
/// by parsing.
fn follow(reader: impl Read, mut on_start: impl FnMut()) -> io::Result<()> {
    let messages = serde_json::Deserializer::from_reader(BufReader::new(reader));
    for message in messages.into_iter::<Value>() {
        if is_start(&message?) {
            on_start();
        }
    }
    Ok(())
}

fn is_start(message: &Value) -> bool {
    matches!(
        message["method"].as_str(),
        Some("Player.OnPlay" | "Player.OnResume")
    )
}

#[cfg(test)]
mod tests;

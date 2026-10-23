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
mod tests {
    use super::*;

    const PLAY: &str = r#"{"jsonrpc":"2.0","method":"Player.OnPlay","params":{"data":{"item":{"id":7,"type":"movie"},"player":{"playerid":1,"speed":1}},"sender":"xbmc"}}"#;
    const PAUSE: &str = r#"{"jsonrpc":"2.0","method":"Player.OnPause","params":{"data":{"item":{"id":7,"type":"movie"},"player":{"playerid":1,"speed":0}},"sender":"xbmc"}}"#;
    const RESUME: &str = r#"{"jsonrpc":"2.0","method":"Player.OnResume","params":{"data":{"item":{"id":7,"type":"movie"},"player":{"playerid":1,"speed":1}},"sender":"xbmc"}}"#;
    const STOP: &str = r#"{"jsonrpc":"2.0","method":"Player.OnStop","params":{"data":{"end":false,"item":{"id":7,"type":"movie"}},"sender":"xbmc"}}"#;
    const VOLUME: &str = r#"{"jsonrpc":"2.0","method":"Application.OnVolumeChanged","params":{"data":{"muted":false,"volume":100},"sender":"xbmc"}}"#;

    /// How many starts `input` gives, and how reading it ended.
    fn starts(input: &str) -> (usize, io::Result<()>) {
        let mut starts = 0;
        let result = follow(input.as_bytes(), || starts += 1);
        (starts, result)
    }

    #[test]
    fn play_and_resume_start() {
        let input = [PLAY, PAUSE, RESUME, STOP].concat();
        let (count, result) = starts(&input);
        assert_eq!(count, 2);
        result.unwrap();
    }

    #[test]
    fn everything_else_is_ignored() {
        let input = [
            VOLUME,
            PAUSE,
            STOP,
            r#"{"id":1,"jsonrpc":"2.0","result":"OK"}"#,
        ]
        .concat();
        let (count, result) = starts(&input);
        assert_eq!(count, 0);
        result.unwrap();
    }

    #[test]
    fn messages_split_by_whitespace_too() {
        let input = format!("{PLAY}\n{VOLUME}\r\n  {RESUME}\n");
        let (count, result) = starts(&input);
        assert_eq!(count, 2);
        result.unwrap();
    }

    #[test]
    fn nothing_is_fine() {
        let (count, result) = starts("");
        assert_eq!(count, 0);
        result.unwrap();
    }

    /// Messages before the bad one still count; then the connection is
    /// dropped, since the stream can't be resynced.
    #[test]
    fn malformed_ends_the_connection() {
        let (count, result) = starts(&format!("{PLAY}{{\"method\": oops}}{RESUME}"));
        assert_eq!(count, 1);
        assert!(result.is_err());
    }

    #[test]
    fn cut_off_mid_message_is_an_error() {
        let (count, result) = starts(&PLAY[..PLAY.len() - 5]);
        assert_eq!(count, 0);
        assert!(result.is_err());
    }
}

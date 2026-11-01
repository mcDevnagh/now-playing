//! Follows UxPlay's video window, reporting a start when it opens: UxPlay
//! opens it when a device starts mirroring, and closes it when it stops.
//! Whether it's open is reported too, to keep Kodi awake meanwhile.

use std::sync::mpsc::Sender;
use std::thread;

use now_playing_core::net::Backoff;

use super::Listener;
use crate::action::Request;
use crate::quiet::Player;
use crate::windows;

/// Follows windows of class `window_class` on `display`, the display
/// UxPlay's windows open on, reporting whenever one opens, and whether
/// one's open.
pub struct Uxplay {
    pub window_class: String,
    pub display: String,
}

impl Listener for Uxplay {
    /// The display can go away, as gamescope restarting, so only the first
    /// failure of each outage is logged.
    fn run(self: Box<Self>, requests: Sender<Request>) -> ! {
        let Uxplay {
            window_class: class,
            display,
        } = *self;
        let mut backoff = Backoff::new();
        let mut quiet = false;
        loop {
            let mut edges = UxplayEdges::default();
            let reason = windows::follow(&display, &class, |open| {
                if edges.last.is_none() {
                    eprintln!("uxplay: following {class} windows on {display}");
                    backoff.reset();
                    quiet = false;
                }
                // The dispatch loop outlives the listeners.
                if edges.last != Some(open) {
                    let _ = requests.send(Request::Mirroring(open));
                }
                match edges.update(open) {
                    Some(true) => {
                        eprintln!("uxplay: mirroring started");
                        let _ = requests.send(Request::Started(Player::Uxplay));
                    }
                    Some(false) => eprintln!("uxplay: mirroring stopped"),
                    None => {}
                }
            });
            // Unknown until it reconnects; the window has likely gone.
            if edges.last == Some(true) {
                let _ = requests.send(Request::Mirroring(false));
            }
            if !quiet {
                eprintln!("uxplay: {display}: {reason}; retrying quietly");
                quiet = true;
            }
            thread::sleep(backoff.next_delay());
        }
    }
}

/// Finds mirroring starting and stopping in a series of "is a video window
/// open" values from one connection.
#[derive(Debug, Default)]
struct UxplayEdges {
    last: Option<bool>,
}

impl UxplayEdges {
    /// Takes the next value; `Some(true)` if mirroring just started,
    /// `Some(false)` if it just stopped.
    ///
    /// The first value is a baseline, so mirroring that was already on when
    /// the switcher (re)connected doesn't count.
    fn update(&mut self, open: bool) -> Option<bool> {
        let edge = self.last.filter(|&last| last != open).map(|_| open);
        self.last = Some(open);
        edge
    }
}

#[cfg(test)]
mod tests;

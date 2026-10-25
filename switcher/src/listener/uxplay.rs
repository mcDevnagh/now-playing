//! Follows UxPlay's video window, asking for UxPlay when it opens: UxPlay
//! opens it when a device starts mirroring, and closes it when it stops.

use std::sync::mpsc::Sender;
use std::thread;

use now_playing_core::net::Backoff;

use super::Listener;
use crate::action::Request;
use crate::app::App;
use crate::{steam, windows};

/// Follows windows of class `window_class` on UxPlay's display, asking for
/// `app` whenever one opens.
pub struct Uxplay {
    pub window_class: String,
    pub app: App,
}

impl Listener for Uxplay {
    /// UxPlay often isn't running, as outside Gaming Mode, so only the first
    /// failure of each outage is logged.
    fn run(&mut self, show: Sender<Request>) -> ! {
        let class = &self.window_class;
        // A cast means UxPlay is running, and launching a running app makes
        // Steam show an error, so it's asked for without its launch ID.
        let app = without_launch(self.app);
        let mut backoff = Backoff::new();
        let mut quiet = false;
        loop {
            let reason = match steam::display_of(app.gamescope) {
                Ok(display) => {
                    let mut edges = UxplayEdges::default();
                    let reason = windows::follow(&display, class, |open| {
                        if edges.last.is_none() {
                            eprintln!("uxplay: following {class} windows on {display}");
                            backoff.reset();
                            quiet = false;
                        }
                        match edges.update(open) {
                            Some(true) => {
                                eprintln!("uxplay: mirroring started");
                                // The dispatch loop outlives the listeners.
                                let _ = show.send(Request::Show(app));
                            }
                            Some(false) => eprintln!("uxplay: mirroring stopped"),
                            None => {}
                        }
                    });
                    format!("{display}: {reason}")
                }
                Err(e) => e,
            };
            if !quiet {
                eprintln!("uxplay: {reason}; retrying quietly");
                quiet = true;
            }
            thread::sleep(backoff.next_delay());
        }
    }
}

/// `app` without its launch ID, so showing it can't launch it.
fn without_launch(app: App) -> App {
    App {
        launch: None,
        ..app
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

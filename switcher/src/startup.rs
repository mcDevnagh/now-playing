//! Launching apps at start: waits for Steam's UI to be up in Gaming Mode,
//! then asks for each app to be launched.

use std::sync::mpsc::Sender;
use std::thread;

use now_playing_core::net::Backoff;

use crate::action::Request;
use crate::app::App;
use crate::gamescope::Gamescope;
use crate::steam;

/// Waits until gamescope is in Gaming Mode and Steam's UI answers, then
/// sends a [`Request::LaunchAtStart`] for each of `apps`. Only the first
/// reason for waiting is logged.
pub fn run(apps: Vec<App>, display: &str, requests: Sender<Request>) {
    let mut backoff = Backoff::new();
    let mut quiet = false;
    while let Err(e) = ready(display) {
        if !quiet {
            eprintln!("switcher: waiting to launch apps at start ({e})");
            quiet = true;
        }
        thread::sleep(backoff.next_delay());
    }
    eprintln!("switcher: Steam's UI is up; launching apps at start");
    for app in apps {
        // The dispatch loop outlives this thread.
        let _ = requests.send(Request::LaunchAtStart(app));
    }
}

fn ready(display: &str) -> Result<(), String> {
    match Gamescope::connect(display)?.focused_app()? {
        Some(_) => steam::ui::is_up(),
        None => Err("not in Gaming Mode".to_owned()),
    }
}

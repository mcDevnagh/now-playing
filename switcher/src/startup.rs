//! Launching apps at start: each time Gaming Mode starts, waits for Steam's
//! UI to be up, then asks for each app to be launched.

use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use crate::action::Request;
use crate::app::App;
use crate::gamescope::Gamescope;
use crate::steam;

/// How often to check whether Gaming Mode has started and Steam's UI is up.
const READY_CHECK: Duration = Duration::from_secs(2);

/// Forever: waits until gamescope is in Gaming Mode and Steam's UI answers,
/// sends a [`Request::LaunchAtStart`] for each of `apps` (with its wait to
/// come to the front), then waits for
/// Gaming Mode to end. So each Gaming Mode session gets its apps, including
/// one entered from Desktop Mode with the switcher already running.
pub fn run(apps: Vec<(App, Duration)>, display: &str, requests: Sender<Request>) -> ! {
    loop {
        let gamescope = wait_until_ready(display);
        eprintln!("switcher: Steam's UI is up; launching apps at start");
        for &(app, wait) in &apps {
            // The dispatch loop outlives this thread.
            let _ = requests.send(Request::LaunchAtStart(app, wait));
        }
        let reason = gamescope.wait_closed();
        eprintln!("switcher: left Gaming Mode ({reason}); will launch apps when it's back");
    }
}

/// Checks every [`READY_CHECK`] until Gaming Mode is up, returning the
/// connection to gamescope. Only the first reason for waiting is logged.
fn wait_until_ready(display: &str) -> Gamescope {
    let mut quiet = false;
    loop {
        match ready(display) {
            Ok(gamescope) => return gamescope,
            Err(e) if !quiet => {
                eprintln!("switcher: waiting to launch apps at start ({e})");
                quiet = true;
            }
            Err(_) => {}
        }
        thread::sleep(READY_CHECK);
    }
}

fn ready(display: &str) -> Result<Gamescope, String> {
    let gamescope = Gamescope::connect(display)?;
    match gamescope.focused_app()? {
        Some(_) => steam::ui::is_up().map(|()| gamescope),
        None => Err("not in Gaming Mode".to_owned()),
    }
}

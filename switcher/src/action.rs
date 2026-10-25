//! Showing an app: focus it if it's running, launch it if not. And
//! launching apps at start, to wait in the background.

use std::thread;
use std::time::{Duration, Instant};

use crate::app::App;
use crate::gamescope::Gamescope;
use crate::steam;

/// What the dispatch loop is asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Bring the app to the front, launching it if it isn't running.
    Show(App),
    /// Launch the app if it isn't running, then go back to Steam's home, so
    /// it waits in the background.
    LaunchAtStart(App),
}

/// What to do for an app that asked to be shown.
#[derive(Debug, PartialEq, Eq)]
enum Decision {
    /// No gamescope `GAMESCOPE_FOCUSED_APP`, as in Desktop Mode: leave the
    /// screen alone.
    NotGamingMode,
    AlreadyInFront,
    /// Running behind something else: bring it to the front.
    Focus,
    /// Not running: launch it with this launch ID.
    Launch(u64),
    /// Not running, and no launch ID to start it.
    CantLaunch,
}

/// What to do for an app to launch at start.
#[derive(Debug, PartialEq, Eq)]
enum StartDecision {
    /// No gamescope `GAMESCOPE_FOCUSED_APP`, as in Desktop Mode.
    NotGamingMode,
    /// Already running, perhaps from before the switcher restarted: leave
    /// the screen alone.
    AlreadyRunning,
    Launch(u64),
    /// Not running, and no launch ID to start it.
    CantLaunch,
}

/// How long Steam and gamescope take to act on a focus request, before
/// checking it.
const SETTLE: Duration = Duration::from_millis(200);
/// How long an app launched at start has to come to the front.
const START_TIMEOUT: Duration = Duration::from_secs(30);
/// How often to check whether it has.
const START_CHECK: Duration = Duration::from_millis(250);

/// Carries out `request` on `display`, logging what it did. With `dry_run`,
/// it only logs what it would do.
pub fn handle(request: Request, display: &str, dry_run: bool) {
    match request {
        Request::Show(app) => show(app, display, dry_run),
        Request::LaunchAtStart(app) => launch_at_start(app, display, dry_run),
    }
}

/// Brings `app` to the front on `display`.
fn show(app: App, display: &str, dry_run: bool) {
    let gamescope = match Gamescope::connect(display) {
        Ok(gamescope) => gamescope,
        Err(e) => return eprintln!("switcher: {app}: skipped ({e})"),
    };
    let decision = gamescope.focused_app().and_then(|front| {
        decide(app, front, || {
            steam::is_running(app.gamescope).map_err(|e| format!("/proc: {e}"))
        })
    });
    match decision {
        Ok(Decision::NotGamingMode) => eprintln!("switcher: {app}: skipped (not in Gaming Mode)"),
        Ok(Decision::AlreadyInFront) => eprintln!("switcher: {app}: already in front"),
        Ok(Decision::Focus) if dry_run => eprintln!("switcher: {app}: dry run: would focus it"),
        Ok(Decision::Focus) => eprintln!("switcher: {app}: {}", focus(&gamescope, app.gamescope)),
        Ok(Decision::Launch(id)) if dry_run => {
            eprintln!("switcher: {app}: dry run: would launch {id}")
        }
        Ok(Decision::Launch(id)) => match steam::ui::launch(id) {
            Ok(()) => eprintln!("switcher: {app}: launching {id}"),
            Err(e) => eprintln!("switcher: {app}: couldn't launch {id} ({e})"),
        },
        Ok(Decision::CantLaunch) => {
            eprintln!("switcher: {app}: not running, and no launch ID to start it")
        }
        Err(e) => eprintln!("switcher: {app}: skipped ({e})"),
    }
}

/// Launches `app` on `display` if it isn't running, then goes back to
/// Steam's home.
fn launch_at_start(app: App, display: &str, dry_run: bool) {
    let gamescope = match Gamescope::connect(display) {
        Ok(gamescope) => gamescope,
        Err(e) => return eprintln!("switcher: {app}: not launched at start ({e})"),
    };
    let decision = gamescope.focused_app().and_then(|front| {
        decide_start(app, front, || {
            steam::is_running(app.gamescope).map_err(|e| format!("/proc: {e}"))
        })
    });
    match decision {
        Ok(StartDecision::NotGamingMode) => {
            eprintln!("switcher: {app}: not launched at start (not in Gaming Mode)")
        }
        Ok(StartDecision::AlreadyRunning) => {
            eprintln!("switcher: {app}: already running at start")
        }
        Ok(StartDecision::Launch(id)) if dry_run => {
            eprintln!("switcher: {app}: dry run: would launch {id} at start, then go home")
        }
        Ok(StartDecision::Launch(id)) => {
            eprintln!(
                "switcher: {app}: at start: {}",
                launch_then_home(&gamescope, app.gamescope, id)
            )
        }
        Ok(StartDecision::CantLaunch) => {
            eprintln!("switcher: {app}: not launched at start (no launch ID)")
        }
        Err(e) => eprintln!("switcher: {app}: not launched at start ({e})"),
    }
}

/// Launches `app` with `launch_id`, waits for it to come to the front, then
/// shows Steam's home. Returns the outcome, to log.
fn launch_then_home(gamescope: &Gamescope, app: u32, launch_id: u64) -> String {
    if let Err(e) = steam::ui::launch(launch_id) {
        return format!("couldn't launch {launch_id} ({e})");
    }
    // Steam brings the app to the front once it's up, so going home any
    // sooner would be undone.
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        match gamescope.focused_app() {
            Ok(Some(front)) if front == app => break,
            Ok(Some(_)) if Instant::now() >= deadline => {
                return format!(
                    "launched {launch_id}, but it didn't come to the front within {} s, \
                     so not going home",
                    START_TIMEOUT.as_secs()
                );
            }
            Ok(Some(_)) => thread::sleep(START_CHECK),
            Ok(None) => return format!("launched {launch_id}, then left Gaming Mode"),
            Err(e) => {
                return format!("launched {launch_id}, but can't tell when it's up ({e})");
            }
        }
    }
    if let Err(e) = steam::ui::home() {
        return format!("launched {launch_id}, but couldn't go home ({e})");
    }
    thread::sleep(SETTLE);
    format!(
        "launched {launch_id}, {}",
        home_outcome(app, gamescope.focused_app())
    )
}

/// The outcome of going home from `app`, given the app in front afterwards.
fn home_outcome(app: u32, front: Result<Option<u32>, String>) -> String {
    match front {
        Ok(Some(front)) if front == app => "but it's still in front after going home".to_owned(),
        Ok(Some(_)) => "then went home".to_owned(),
        Ok(None) => "but left Gaming Mode".to_owned(),
        Err(e) => format!("then went home, but can't check it worked ({e})"),
    }
}

/// Focuses `app` through Steam, then checks it came to the front. Returns
/// the outcome, to log.
fn focus(gamescope: &Gamescope, app: u32) -> String {
    if let Err(e) = steam::ui::focus(app) {
        return format!("couldn't focus it ({e})");
    }
    thread::sleep(SETTLE);
    focus_outcome(app, gamescope.focused_app())
}

/// The outcome of focusing `app`, given the app in front afterwards.
fn focus_outcome(app: u32, front: Result<Option<u32>, String>) -> String {
    match front {
        Ok(Some(front)) if front == app => "switched".to_owned(),
        Ok(Some(front)) => format!("didn't switch (app {front} is still in front)"),
        Ok(None) => "didn't switch (no longer in Gaming Mode)".to_owned(),
        Err(e) => format!("focused it, but can't check it switched ({e})"),
    }
}

/// Decides what to do for `app`, given the app in front (`None` outside
/// Gaming Mode). `running` checks whether `app` is running, and is only
/// called when that matters.
fn decide(
    app: App,
    front: Option<u32>,
    running: impl FnOnce() -> Result<bool, String>,
) -> Result<Decision, String> {
    let Some(front) = front else {
        return Ok(Decision::NotGamingMode);
    };
    if front == app.gamescope {
        return Ok(Decision::AlreadyInFront);
    }
    if running()? {
        return Ok(Decision::Focus);
    }
    Ok(app.launch.map_or(Decision::CantLaunch, Decision::Launch))
}

/// Decides what to do for `app` at start, given the app in front (`None`
/// outside Gaming Mode). `running` is only called in Gaming Mode.
fn decide_start(
    app: App,
    front: Option<u32>,
    running: impl FnOnce() -> Result<bool, String>,
) -> Result<StartDecision, String> {
    if front.is_none() {
        return Ok(StartDecision::NotGamingMode);
    }
    if running()? {
        return Ok(StartDecision::AlreadyRunning);
    }
    Ok(app
        .launch
        .map_or(StartDecision::CantLaunch, StartDecision::Launch))
}

#[cfg(test)]
mod tests;

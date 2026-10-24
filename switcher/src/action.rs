//! Showing an app: focus it if it's running, launch it if not.

use std::thread;
use std::time::Duration;

use crate::app::App;
use crate::gamescope::Gamescope;
use crate::steam;

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

/// How long gamescope takes to act on a focus request, before checking it.
const SETTLE: Duration = Duration::from_millis(200);

/// Brings `app` to the front on `display`, logging what it did. With
/// `dry_run`, it only logs what it would do. Launching is a dry run so far.
pub fn show(app: App, display: &str, dry_run: bool) {
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
        Ok(Decision::Launch(id)) => eprintln!("switcher: {app}: dry run: would launch {id}"),
        Ok(Decision::CantLaunch) => {
            eprintln!("switcher: {app}: not running, and no launch ID to start it")
        }
        Err(e) => eprintln!("switcher: {app}: skipped ({e})"),
    }
}

/// Focuses `app`, then checks it came to the front. Returns the outcome, to
/// log.
fn focus(gamescope: &Gamescope, app: u32) -> String {
    if let Err(e) = gamescope.focus(app) {
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

#[cfg(test)]
mod tests;

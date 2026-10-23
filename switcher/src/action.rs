//! Showing an app: focus it if it's running, launch it if not.

use crate::app::App;
use crate::gamescope::Gamescope;

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

/// Brings `app` to the front on `display`, logging what it did. Focusing and
/// launching are dry runs so far: they're logged, not done.
pub fn show(app: App, display: &str) {
    let decision = Gamescope::connect(display).and_then(|gamescope| {
        decide(app, gamescope.focused_app()?, || {
            gamescope.is_running(app.gamescope)
        })
    });
    match decision {
        Ok(Decision::NotGamingMode) => eprintln!("switcher: {app}: skipped (not in Gaming Mode)"),
        Ok(Decision::AlreadyInFront) => eprintln!("switcher: {app}: already in front"),
        Ok(Decision::Focus) => eprintln!("switcher: {app}: dry run: would focus it"),
        Ok(Decision::Launch(id)) => eprintln!("switcher: {app}: dry run: would launch {id}"),
        Ok(Decision::CantLaunch) => {
            eprintln!("switcher: {app}: not running, and no launch ID to start it")
        }
        Err(e) => eprintln!("switcher: {app}: skipped (can't read the display: {e})"),
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

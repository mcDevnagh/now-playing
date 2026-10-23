//! What a trigger does to gamescope.

use crate::registry::Action;

/// Runs `action`, with `front` the app in front. Only dry runs exist so
/// far: they log what would happen.
pub fn run(action: Action, front: u32) {
    match action {
        Action::Focus(app) => {
            eprintln!("switcher: dry run: would focus app {app} (in front: {front})")
        }
        Action::Launch(id) => {
            eprintln!("switcher: dry run: would launch {id} if it isn't running")
        }
    }
}

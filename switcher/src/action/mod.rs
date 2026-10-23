//! What a trigger does to gamescope.

use crate::registry::Action;

/// Runs `action`. Only dry runs exist so far: they log what would happen.
pub fn run(action: Action) {
    match action {
        Action::Focus(app) => eprintln!("switcher: dry run: would focus app {app}"),
        Action::Launch(id) => {
            eprintln!("switcher: dry run: would launch {id} if it isn't running")
        }
    }
}

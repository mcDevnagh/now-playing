//! What the dispatch loop does with what the listeners report.

use crate::quiet::{Others, Player};

/// What the dispatch loop is asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// A player started: quiet the others.
    Started(Player),
}

/// Carries out `request`, logging what it did. With `dry_run`, it only logs
/// what it would do.
pub fn handle(request: Request, others: &Others, dry_run: bool) {
    match request {
        Request::Started(player) => others.started(player, dry_run),
    }
}

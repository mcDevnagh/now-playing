//! Quieting the other players when one starts, so two never play over each
//! other: Kodi is paused, and our Snapcast client is muted, its stream
//! playing on.
//!
//! Every command sets a state, never toggles one, so it's sent whatever we
//! think the player is doing: pausing something paused, or muting something
//! muted, does nothing.

use std::fmt;

use now_playing_core::snapcast;

use crate::listener::kodi;

/// A player the switcher follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Player {
    Kodi,
    Snapcast,
    Uxplay,
}

impl fmt::Display for Player {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Player::Kodi => "kodi",
            Player::Snapcast => "snapcast",
            Player::Uxplay => "uxplay",
        })
    }
}

/// One thing a start does to another player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quiet {
    PauseKodi,
    /// Sets our Snapcast client's mute: `true` mutes it, `false` unmutes it.
    MuteSnapcast(bool),
}

impl Quiet {
    /// What it is, as "pausing kodi", or with `dry_run` "pause kodi".
    fn describe(self, dry_run: bool) -> &'static str {
        match (self, dry_run) {
            (Quiet::PauseKodi, false) => "pausing kodi",
            (Quiet::PauseKodi, true) => "pause kodi",
            (Quiet::MuteSnapcast(true), false) => "muting snapcast",
            (Quiet::MuteSnapcast(true), true) => "mute snapcast",
            (Quiet::MuteSnapcast(false), false) => "unmuting snapcast",
            (Quiet::MuteSnapcast(false), true) => "unmute snapcast",
        }
    }
}

/// What a start does to the other players. Held by the dispatch loop only.
/// Empty without `quiet_others`.
#[derive(Default)]
pub struct Others {
    kodi: Option<kodi::Controller>,
    snapcast: Option<snapcast::Controller>,
}

impl Others {
    pub fn new(kodi: Option<kodi::Controller>, snapcast: Option<snapcast::Controller>) -> Self {
        Others { kodi, snapcast }
    }

    /// Whether it can reach Kodi and Snapcast.
    #[cfg(test)]
    pub fn reaches(&self) -> (bool, bool) {
        (self.kodi.is_some(), self.snapcast.is_some())
    }

    /// Quiets every player but `started`; for Snapcast, also unmutes our
    /// client. With `dry_run`, only logs.
    pub fn started(&self, started: Player, dry_run: bool) {
        let quiets = self.quiets(started);
        if quiets.is_empty() {
            return;
        }
        eprintln!("switcher: {}", describe(started, &quiets, dry_run));
        if dry_run {
            return;
        }
        for quiet in quiets {
            match quiet {
                Quiet::PauseKodi => self.kodi.iter().for_each(kodi::Controller::pause),
                Quiet::MuteSnapcast(muted) => self.snapcast.iter().for_each(|s| s.set_muted(muted)),
            }
        }
    }

    /// What starting `started` does, to the players we can reach.
    fn quiets(&self, started: Player) -> Vec<Quiet> {
        let (pause_kodi, mute_snapcast) = match started {
            Player::Kodi => (false, true),
            Player::Snapcast => (true, false),
            Player::Uxplay => (true, true),
        };
        let pause = (pause_kodi && self.kodi.is_some()).then_some(Quiet::PauseKodi);
        let mute = self
            .snapcast
            .is_some()
            .then_some(Quiet::MuteSnapcast(mute_snapcast));
        pause.into_iter().chain(mute).collect()
    }
}

/// The log line for `started` doing `quiets`.
fn describe(started: Player, quiets: &[Quiet], dry_run: bool) -> String {
    let what: Vec<_> = quiets.iter().map(|q| q.describe(dry_run)).collect();
    let what = what.join(" and ");
    match dry_run {
        false => format!("{started} started; {what}"),
        true => format!("{started} started; dry run: would {what}"),
    }
}

#[cfg(test)]
mod tests;

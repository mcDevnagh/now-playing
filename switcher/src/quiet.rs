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
    /// Mute our Snapcast client.
    MuteSnapcast,
    /// Unmute our Snapcast client.
    UnmuteSnapcast,
}

impl Quiet {
    /// What it is, as "pausing kodi", or with `dry_run` "pause kodi".
    fn describe(self, dry_run: bool) -> &'static str {
        match (self, dry_run) {
            (Quiet::PauseKodi, false) => "pausing kodi",
            (Quiet::PauseKodi, true) => "pause kodi",
            (Quiet::MuteSnapcast, false) => "muting snapcast",
            (Quiet::MuteSnapcast, true) => "mute snapcast",
            (Quiet::UnmuteSnapcast, false) => "unmuting snapcast",
            (Quiet::UnmuteSnapcast, true) => "unmute snapcast",
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
                Quiet::MuteSnapcast => self.snapcast.iter().for_each(|s| s.set_muted(true)),
                Quiet::UnmuteSnapcast => self.snapcast.iter().for_each(|s| s.set_muted(false)),
            }
        }
    }

    /// What starting `started` does, to the players we can reach.
    fn quiets(&self, started: Player) -> Vec<Quiet> {
        let quiets: &[Quiet] = match started {
            Player::Kodi => &[Quiet::MuteSnapcast],
            Player::Snapcast => &[Quiet::PauseKodi, Quiet::UnmuteSnapcast],
            Player::Uxplay => &[Quiet::PauseKodi, Quiet::MuteSnapcast],
        };
        let reaches = |quiet: &&Quiet| match quiet {
            Quiet::PauseKodi => self.kodi.is_some(),
            Quiet::MuteSnapcast | Quiet::UnmuteSnapcast => self.snapcast.is_some(),
        };
        quiets.iter().filter(reaches).copied().collect()
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

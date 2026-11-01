//! The dispatch loop: what to do with what the listeners report. It quiets
//! the other players when one starts, and keeps Kodi awake while a device
//! mirrors to UxPlay.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use crate::listener::kodi;
use crate::quiet::{Others, Player};

/// How often Kodi is sent a no-op while a device mirrors, well inside the
/// shortest idle time Kodi's screensaver can be set to, a minute.
const KEEP_AWAKE: Duration = Duration::from_secs(30);

/// What the dispatch loop is asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// A player started: quiet the others.
    Started(Player),
    /// Whether UxPlay's video window is open: sent as the listener
    /// (re)connects, then on each change.
    Mirroring(bool),
}

/// The dispatch loop's state. Held by its thread only.
pub struct Dispatch {
    others: Others,
    /// Kodi's connection, to keep it awake; `None` without a Kodi listener.
    kodi: Option<kodi::Controller>,
    /// Keep Kodi awake while mirroring: `[uxplay]`'s `keep_kodi_awake`.
    keep_awake: bool,
    /// While Kodi's kept awake: when it's next sent a no-op.
    next_noop: Option<Instant>,
}

impl Dispatch {
    pub fn new(others: Others, kodi: Option<kodi::Controller>, keep_awake: bool) -> Self {
        Dispatch {
            others,
            kodi,
            keep_awake,
            next_noop: None,
        }
    }

    #[cfg(test)]
    pub fn others(&self) -> &Others {
        &self.others
    }

    /// Whether it can keep Kodi awake.
    #[cfg(test)]
    pub fn keeps_awake(&self) -> bool {
        self.keep_awake && self.kodi.is_some()
    }

    /// Carries out `requests` forever, logging what it does. With `dry_run`,
    /// it only logs what it would do.
    pub fn run(mut self, requests: Receiver<Request>, dry_run: bool) -> ! {
        loop {
            let request = match self.next_noop {
                Some(at) => {
                    match requests.recv_timeout(at.saturating_duration_since(Instant::now())) {
                        Ok(request) => Some(request),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => unreachable!("main holds a sender"),
                    }
                }
                None => Some(requests.recv().expect("main holds a sender")),
            };
            let now = Instant::now();
            match request {
                Some(request) => self.handle(request, now, dry_run),
                None => self.tick(now, dry_run),
            }
        }
    }

    /// Carries out one request, at `now`.
    fn handle(&mut self, request: Request, now: Instant, dry_run: bool) {
        match request {
            Request::Started(player) => self.others.started(player, dry_run),
            Request::Mirroring(open) => self.mirroring(open, now, dry_run),
        }
    }

    /// Starts keeping Kodi awake as mirroring starts, at once, as Kodi may
    /// be about to go idle, or already showing its screensaver; stops as it
    /// ends.
    fn mirroring(&mut self, open: bool, now: Instant, dry_run: bool) {
        if !self.keep_awake || self.kodi.is_none() {
            return;
        }
        match (open, self.next_noop) {
            (true, None) => {
                eprintln!("switcher: mirroring; keeping kodi awake");
                self.noop(dry_run);
                self.next_noop = Some(now + KEEP_AWAKE);
            }
            (false, Some(_)) => {
                eprintln!("switcher: mirroring stopped; no longer keeping kodi awake");
                self.next_noop = None;
            }
            _ => {}
        }
    }

    /// Sends Kodi its next no-op, if one's due at `now`.
    fn tick(&mut self, now: Instant, dry_run: bool) {
        if self.next_noop.is_some_and(|at| now >= at) {
            self.noop(dry_run);
            self.next_noop = Some(now + KEEP_AWAKE);
        }
    }

    fn noop(&self, dry_run: bool) {
        match (&self.kodi, dry_run) {
            (Some(_), true) => eprintln!("switcher: dry run: would send kodi a no-op"),
            (Some(kodi), false) => kodi.noop(),
            (None, _) => {}
        }
    }
}

#[cfg(test)]
mod tests;

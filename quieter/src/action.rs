//! The dispatch loop: what to do with what the listeners report. It quiets
//! the other players when one starts, keeps Kodi awake while a device
//! mirrors to UxPlay, and follows Kodi's screensaver: in a profile that
//! switches, it loads the other profile; in any other, it runs the
//! screensaver's command until Kodi's screensaver stops.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use crate::config::Screensaver;
use crate::listener::kodi;
use crate::quiet::{Others, Player};
use crate::screensaver::{self, Child};

/// How often Kodi is sent a no-op while a device mirrors, well inside the
/// shortest idle time Kodi's screensaver can be set to, a minute.
const KEEP_AWAKE: Duration = Duration::from_secs(30);

/// What the dispatch loop is asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// A player started: quiet the others.
    Started(Player),
    /// Whether UxPlay's video window is open: sent as the listener
    /// (re)connects, then on each change.
    Mirroring(bool),
    /// Kodi's screensaver started (`true`) or stopped.
    Screensaver(bool),
    /// Kodi's current profile, by name, as asked for.
    Profile(String),
    /// The connection to Kodi dropped, as when Kodi exits or loads a
    /// profile, so its screensaver can't be followed.
    KodiDisconnected,
    /// Screensaver child `id` quit on its own, as on a key, or crashed.
    ScreensaverExited { id: u64, status: String },
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
    /// What to do as Kodi's screensaver starts: `[kodi.screensaver]`.
    screensaver: Option<Screensaver>,
    /// Whether Kodi's screensaver is up, as far as we know.
    screensaver_up: bool,
    /// The screensaver's command, while it runs.
    child: Option<Child>,
    /// The last child's id.
    last_child: u64,
    /// The profile to load once Kodi's screensaver has stopped, after
    /// waking Kodi to switch to it.
    switching: Option<String>,
    /// For children to report on; set by `run`.
    tx: Option<Sender<Request>>,
    dry_run: bool,
}

impl Dispatch {
    pub fn new(
        others: Others,
        kodi: Option<kodi::Controller>,
        keep_awake: bool,
        screensaver: Option<Screensaver>,
    ) -> Self {
        Dispatch {
            others,
            kodi,
            keep_awake,
            next_noop: None,
            screensaver,
            screensaver_up: false,
            child: None,
            last_child: 0,
            switching: None,
            tx: None,
            dry_run: false,
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

    /// Whether it follows Kodi's screensaver.
    #[cfg(test)]
    pub fn follows_screensaver(&self) -> bool {
        self.screensaver.is_some() && self.kodi.is_some()
    }

    /// Carries out `requests` forever, logging what it does. `tx` is the
    /// channel's sender, for children to report on. With `dry_run`, it only
    /// logs what it would do.
    pub fn run(mut self, tx: Sender<Request>, requests: Receiver<Request>, dry_run: bool) -> ! {
        self.tx = Some(tx);
        self.dry_run = dry_run;
        loop {
            let request = match self.next_noop {
                Some(at) => {
                    match requests.recv_timeout(at.saturating_duration_since(Instant::now())) {
                        Ok(request) => Some(request),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => unreachable!("it holds a sender"),
                    }
                }
                None => Some(requests.recv().expect("it holds a sender")),
            };
            let now = Instant::now();
            match request {
                Some(request) => self.handle(request, now),
                None => self.tick(now),
            }
        }
    }

    /// Carries out one request, at `now`.
    fn handle(&mut self, request: Request, now: Instant) {
        match request {
            Request::Started(player) => self.others.started(player, self.dry_run),
            Request::Mirroring(open) => self.mirroring(open, now),
            Request::Screensaver(true) => self.screensaver_started(),
            Request::Profile(name) => self.profile(&name),
            Request::Screensaver(false) => {
                self.screensaver_up = false;
                self.stop_child();
                self.finish_switch();
            }
            Request::KodiDisconnected => {
                self.screensaver_up = false;
                self.stop_child();
                self.switching = None;
            }
            Request::ScreensaverExited { id, status } => self.child_exited(id, &status),
        }
    }

    /// Kodi's screensaver started: with profiles that switch, asks which
    /// profile Kodi is in, to decide on its reply; otherwise starts the
    /// command.
    fn screensaver_started(&mut self) {
        self.screensaver_up = true;
        let (Some(screensaver), Some(kodi)) = (&self.screensaver, &self.kodi) else {
            return;
        };
        match screensaver.switch_to.is_empty() {
            true => self.start_child(),
            false => kodi.ask_profile(),
        }
    }

    /// Kodi's in profile `name`, as asked when its screensaver started:
    /// switches to the profile it switches to, if any, else starts the
    /// command. The config never has a profile switch to itself, as Kodi
    /// loading the profile it's in breaks it.
    ///
    /// Kodi is woken first, and the profile loaded once its screensaver
    /// has stopped: a profile loaded under the screensaver leaves Kodi
    /// black after the next profile's screensaver is woken from Kore,
    /// with keys no longer waking it.
    fn profile(&mut self, name: &str) {
        // Stopped while we asked.
        if !self.screensaver_up {
            return;
        }
        let Some(screensaver) = &self.screensaver else {
            return;
        };
        let Some(to) = screensaver.switch_to.get(name) else {
            return self.start_child();
        };
        eprintln!(
            "quieter: kodi's screensaver started in profile {name}; waking kodi to load {to}"
        );
        self.switching = Some(to.clone());
        self.noop();
    }

    /// Loads the profile being switched to, now Kodi's screensaver has
    /// stopped, if one is.
    fn finish_switch(&mut self) {
        let (Some(to), Some(kodi)) = (self.switching.take(), &self.kodi) else {
            return;
        };
        match self.dry_run {
            false => {
                eprintln!("quieter: kodi's awake; loading {to}");
                kodi.load_profile(&to);
            }
            true => eprintln!("quieter: kodi's awake; dry run: would load {to}"),
        }
    }

    /// Starts the screensaver's command, unless it's running.
    fn start_child(&mut self) {
        let (Some(screensaver), None) = (&self.screensaver, &self.child) else {
            return;
        };
        let program = &screensaver.command[0];
        if self.dry_run {
            return eprintln!("quieter: kodi's screensaver started; dry run: would run {program}");
        }
        let Some(tx) = &self.tx else { return };
        self.last_child += 1;
        match screensaver::spawn(&screensaver.command, self.last_child, tx.clone()) {
            Ok(child) => {
                eprintln!("quieter: kodi's screensaver started; running {program}");
                self.child = Some(child);
            }
            // Kodi's own screensaver stays up.
            Err(e) => {
                eprintln!("quieter: kodi's screensaver started; couldn't run {program}: {e}")
            }
        }
    }

    /// Stops the screensaver's command, if it's running.
    fn stop_child(&mut self) {
        if self.child.take().is_some() {
            eprintln!("quieter: stopping the screensaver");
        }
    }

    /// Child `id` quit on its own: if Kodi's screensaver is still up, wakes
    /// Kodi, so a key that closed the child gets back to Kodi. After a
    /// crash, Kodi's screensaver starts again after its idle time, which
    /// starts a new child.
    fn child_exited(&mut self, id: u64, status: &str) {
        if self.child.as_ref().is_none_or(|child| child.id != id) {
            return;
        }
        self.child = None;
        if self.screensaver_up {
            eprintln!("quieter: the screensaver quit ({status}); waking kodi");
            self.noop();
        } else {
            eprintln!("quieter: the screensaver quit ({status})");
        }
    }

    /// Starts keeping Kodi awake as mirroring starts, at once, as Kodi may
    /// be about to go idle, or already showing its screensaver; stops as it
    /// ends.
    fn mirroring(&mut self, open: bool, now: Instant) {
        if !self.keep_awake || self.kodi.is_none() {
            return;
        }
        match (open, self.next_noop) {
            (true, None) => {
                eprintln!("quieter: mirroring; keeping kodi awake");
                self.noop();
                self.next_noop = Some(now + KEEP_AWAKE);
            }
            (false, Some(_)) => {
                eprintln!("quieter: mirroring stopped; no longer keeping kodi awake");
                self.next_noop = None;
            }
            _ => {}
        }
    }

    /// Sends Kodi its next no-op, if one's due at `now`.
    fn tick(&mut self, now: Instant) {
        if self.next_noop.is_some_and(|at| now >= at) {
            self.noop();
            self.next_noop = Some(now + KEEP_AWAKE);
        }
    }

    fn noop(&self) {
        match (&self.kodi, self.dry_run) {
            (Some(_), true) => eprintln!("quieter: dry run: would send kodi a no-op"),
            (Some(kodi), false) => kodi.noop(),
            (None, _) => {}
        }
    }
}

#[cfg(test)]
mod tests;

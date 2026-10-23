//! Threads that watch players and report to the dispatch loop.

pub mod kodi;
pub mod snapcast;

/// What a listener reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Whether our Snapcast client is playing, sent on each change; `None`
    /// while it's unknown.
    Music(Option<bool>),
    /// Kodi started or resumed playback.
    KodiStarted,
}

//! Pure decisions: when music counts as having started.

use now_playing_core::snapcast::Stream;
use now_playing_core::state::{State, now_playing};

/// Whether our client's stream is playing; `None` while it's unknown
/// (disconnected, or the client is missing).
pub fn playing(stream: Option<&Stream>) -> Option<bool> {
    stream.map(|stream| matches!(now_playing(stream), State::Playing(_)))
}

/// Finds music starting in a series of [`playing`] values.
#[derive(Debug, Default)]
pub struct MusicEdges {
    last: Option<bool>,
}

impl MusicEdges {
    /// Takes the next value; true if music just started.
    ///
    /// Only not playing → playing counts. The first value after `None` is a
    /// baseline, so a reconnect, a snapclient restart or starting mid-song
    /// doesn't count.
    pub fn update(&mut self, playing: Option<bool>) -> bool {
        let started = self.last == Some(false) && playing == Some(true);
        self.last = playing;
        started
    }
}

#[cfg(test)]
mod tests {
    use now_playing_core::snapcast::Session;

    use super::*;

    const BUILTIN_LOG: &str = include_str!("../../core/tests/fixtures/builtin-events.log");
    const META_LOG: &str = include_str!("../../core/tests/fixtures/meta-librespot-events.log");
    const GROUP_SWITCH_LOG: &str = include_str!("../../core/tests/fixtures/group-switch.log");

    fn edges(values: &[Option<bool>]) -> Vec<bool> {
        let mut edges = MusicEdges::default();
        values.iter().map(|&value| edges.update(value)).collect()
    }

    /// Replays a log for the HTPC, returning [`playing`] after each line.
    fn replay(log: &str) -> Vec<Option<bool>> {
        let mut session = Session::new("HTPC");
        log.lines()
            .map(|line| {
                let (_, json) = line.split_once('\t').unwrap();
                session.handle(json).unwrap();
                playing(session.stream())
            })
            .collect()
    }

    fn starts(log: &str) -> usize {
        edges(&replay(log))
            .into_iter()
            .filter(|&started| started)
            .count()
    }

    #[test]
    fn stopped_to_playing_starts() {
        assert_eq!(edges(&[Some(false), Some(true)]), [false, true]);
    }

    #[test]
    fn first_value_is_a_baseline() {
        assert_eq!(edges(&[Some(true)]), [false]);
        assert_eq!(edges(&[None, Some(true)]), [false, false]);
    }

    #[test]
    fn reconnecting_mid_song_doesnt_start() {
        let values = [Some(false), Some(true), None, Some(true)];
        assert_eq!(edges(&values), [false, true, false, false]);
    }

    #[test]
    fn reconnecting_while_stopped_then_playing_starts() {
        let values = [None, Some(false), Some(true)];
        assert_eq!(edges(&values), [false, false, true]);
    }

    #[test]
    fn staying_in_a_state_doesnt_start() {
        let values = [Some(false), Some(false), Some(true), Some(true)];
        assert_eq!(edges(&values), [false, false, true, false]);
    }

    /// Playing, pause, then resume with the stale `paused` in `OnUpdate`:
    /// one start, on the resume. Track changes while playing don't count.
    #[test]
    fn meta_log_starts_once_on_resume() {
        assert_eq!(starts(META_LOG), 1);
    }

    /// Playing, idle, playing again, then track changes: one start.
    #[test]
    fn builtin_log_starts_once() {
        assert_eq!(starts(BUILTIN_LOG), 1);
    }

    /// Playing throughout, with the client disconnecting and reconnecting:
    /// the reconnect is a baseline, so no start.
    #[test]
    fn group_switch_log_doesnt_start_on_reconnect() {
        assert_eq!(starts(GROUP_SWITCH_LOG), 0);
    }
}

//! Plays a recorded log as if snapserver were sending it, to work on the
//! window without a server.

use std::thread;
use std::time::Duration;

use crate::snapcast::{Action, Session, Stream};

/// Feeds `log` (`HH:MM:SS<TAB>json` per line, as in `tests/fixtures/`) to a
/// session following `client`, keeping the log's timing, and calls
/// `on_stream` as `snapcast::run` would. Returns at the end of the log.
pub fn replay(log: &str, client: &str, mut on_stream: impl FnMut(Option<&Stream>)) {
    let mut session = Session::new(client);
    let mut last = None;
    for line in log.lines() {
        let Some((time, json)) = line.split_once('\t') else {
            continue;
        };
        let now = seconds(time);
        if let (Some(last), Some(now)) = (last, now) {
            thread::sleep(Duration::from_secs(now.saturating_sub(last)));
        }
        last = now.or(last);
        match session.handle(json) {
            Ok(Action::Resolved | Action::Changed) => on_stream(session.stream()),
            Ok(_) => {}
            Err(e) => eprintln!("replay: {e}"),
        }
    }
}

/// `HH:MM:SS` as seconds.
fn seconds(time: &str) -> Option<u64> {
    time.split(':').try_fold(0, |total, part| {
        Some(total * 60 + part.parse::<u64>().ok()?)
    })
}

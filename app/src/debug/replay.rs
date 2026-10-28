//! Plays a recorded log as if snapserver were sending it, to work on the
//! window without a server.

use std::time::{Duration, Instant};

use crate::snapcast::{self, Action, Command, Inbox, Session, Stream};

/// Feeds `log` (`HH:MM:SS<TAB>json` per line, as in `core/tests/fixtures/`)
/// to a session following `client`, keeping the log's timing, and calls
/// `on_stream` as `snapcast::run` would. Returns at the end of the log.
///
/// Commands that arrive in `inbox` meanwhile are checked against the
/// replay's stream, and the line that would be sent is logged.
pub fn replay(log: &str, client: &str, inbox: Inbox, mut on_stream: impl FnMut(Option<&Stream>)) {
    let mut session = Session::new(client);
    let mut id = 0;
    // The last timed line's time, and when it was due.
    let mut clock: Option<(u64, Instant)> = None;
    for line in log.lines() {
        let Some((time, json)) = line.split_once('\t') else {
            continue;
        };
        if let Some(now) = seconds(time) {
            let due = match clock {
                Some((last, at)) => at + Duration::from_secs(now.saturating_sub(last)),
                None => Instant::now(),
            };
            while let Some(command) = inbox.command(due) {
                id += 1;
                would_send(&session, command, id);
            }
            clock = Some((now, due));
        }
        match session.handle(json) {
            Ok(Action::Resolved | Action::Changed) => on_stream(session.stream()),
            Ok(_) => {}
            Err(e) => eprintln!("replay: {e}"),
        }
    }
}

/// Logs the line `command` would send, or why it wouldn't.
fn would_send(session: &Session, command: Command, id: u64) {
    let mut line = Vec::new();
    match snapcast::control(session, command, id, &mut line) {
        Ok(Ok(())) => eprint!("replay: would send {}", String::from_utf8_lossy(&line)),
        Ok(Err(refused)) => eprintln!("replay: not sending {command}: {refused}"),
        Err(e) => eprintln!("replay: {e}"),
    }
}

/// `HH:MM:SS` as seconds.
fn seconds(time: &str) -> Option<u64> {
    time.split(':').try_fold(0, |total, part| {
        Some(total * 60 + part.parse::<u64>().ok()?)
    })
}

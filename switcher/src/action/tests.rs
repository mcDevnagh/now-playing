use super::*;
use crate::listener::kodi::Inbox;

/// A dispatch with Kodi, and Kodi's inbox.
fn dispatch(keep_awake: bool) -> (Dispatch, Inbox) {
    let (kodi, inbox) = kodi::channel();
    let others = Others::new(Some(kodi.clone()), None, vec![]);
    (Dispatch::new(others, Some(kodi), keep_awake), inbox)
}

#[test]
fn mirroring_keeps_kodi_awake_until_it_stops() {
    let (mut dispatch, inbox) = dispatch(true);
    let start = Instant::now();
    dispatch.handle(Request::Mirroring(true), start, false);
    assert_eq!(inbox.take(), ["noop"]);
    // Not before it's due.
    dispatch.tick(start + KEEP_AWAKE / 2, false);
    assert_eq!(inbox.take(), [] as [&str; 0]);
    dispatch.tick(start + KEEP_AWAKE, false);
    assert_eq!(inbox.take(), ["noop"]);
    assert_eq!(dispatch.next_noop, Some(start + 2 * KEEP_AWAKE));
    dispatch.handle(Request::Mirroring(false), start + KEEP_AWAKE, false);
    assert_eq!(dispatch.next_noop, None);
    dispatch.tick(start + 3 * KEEP_AWAKE, false);
    assert_eq!(inbox.take(), [] as [&str; 0]);
}

/// Another window opening while one is open doesn't restart the timer.
#[test]
fn mirroring_twice_keeps_the_timer() {
    let (mut dispatch, inbox) = dispatch(true);
    let start = Instant::now();
    dispatch.handle(Request::Mirroring(true), start, false);
    dispatch.handle(Request::Mirroring(true), start + KEEP_AWAKE / 2, false);
    assert_eq!(inbox.take(), ["noop"]);
    assert_eq!(dispatch.next_noop, Some(start + KEEP_AWAKE));
}

#[test]
fn keep_kodi_awake_off_sends_nothing() {
    let (mut dispatch, inbox) = dispatch(false);
    dispatch.handle(Request::Mirroring(true), Instant::now(), false);
    assert_eq!(dispatch.next_noop, None);
    assert_eq!(inbox.take(), [] as [&str; 0]);
}

#[test]
fn a_dry_run_sends_nothing() {
    let (mut dispatch, inbox) = dispatch(true);
    let start = Instant::now();
    dispatch.handle(Request::Mirroring(true), start, true);
    dispatch.tick(start + KEEP_AWAKE, true);
    assert_eq!(inbox.take(), [] as [&str; 0]);
}

#[test]
fn starts_still_quiet() {
    let (kodi, inbox) = kodi::channel();
    let others = Others::new(Some(kodi.clone()), None, vec![Player::Uxplay]);
    let mut dispatch = Dispatch::new(others, Some(kodi), true);
    dispatch.handle(Request::Started(Player::Uxplay), Instant::now(), false);
    assert_eq!(inbox.take(), ["pause"]);
}

use std::collections::BTreeMap;
use std::sync::mpsc;

use super::*;
use crate::listener::kodi::Inbox;

/// A dispatch with Kodi, and Kodi's inbox.
fn dispatch(keep_awake: bool) -> (Dispatch, Inbox) {
    let (kodi, inbox) = kodi::channel();
    let others = Others::new(Some(kodi.clone()), None, vec![]);
    (Dispatch::new(others, Some(kodi), keep_awake, None), inbox)
}

#[test]
fn mirroring_keeps_kodi_awake_until_it_stops() {
    let (mut dispatch, inbox) = dispatch(true);
    let start = Instant::now();
    dispatch.handle(Request::Mirroring(true), start);
    assert_eq!(inbox.take(), ["noop"]);
    // Not before it's due.
    dispatch.tick(start + KEEP_AWAKE / 2);
    assert_eq!(inbox.take(), [] as [String; 0]);
    dispatch.tick(start + KEEP_AWAKE);
    assert_eq!(inbox.take(), ["noop"]);
    assert_eq!(dispatch.next_noop, Some(start + 2 * KEEP_AWAKE));
    dispatch.handle(Request::Mirroring(false), start + KEEP_AWAKE);
    assert_eq!(dispatch.next_noop, None);
    dispatch.tick(start + 3 * KEEP_AWAKE);
    assert_eq!(inbox.take(), [] as [String; 0]);
}

/// Another window opening while one is open doesn't restart the timer.
#[test]
fn mirroring_twice_keeps_the_timer() {
    let (mut dispatch, inbox) = dispatch(true);
    let start = Instant::now();
    dispatch.handle(Request::Mirroring(true), start);
    dispatch.handle(Request::Mirroring(true), start + KEEP_AWAKE / 2);
    assert_eq!(inbox.take(), ["noop"]);
    assert_eq!(dispatch.next_noop, Some(start + KEEP_AWAKE));
}

#[test]
fn keep_kodi_awake_off_sends_nothing() {
    let (mut dispatch, inbox) = dispatch(false);
    dispatch.handle(Request::Mirroring(true), Instant::now());
    assert_eq!(dispatch.next_noop, None);
    assert_eq!(inbox.take(), [] as [String; 0]);
}

#[test]
fn a_dry_run_sends_nothing() {
    let (mut dispatch, inbox) = dispatch(true);
    dispatch.dry_run = true;
    let start = Instant::now();
    dispatch.handle(Request::Mirroring(true), start);
    dispatch.tick(start + KEEP_AWAKE);
    assert_eq!(inbox.take(), [] as [String; 0]);
}

#[test]
fn starts_still_quiet() {
    let (kodi, inbox) = kodi::channel();
    let others = Others::new(Some(kodi.clone()), None, vec![Player::Uxplay]);
    let mut dispatch = Dispatch::new(others, Some(kodi), true, None);
    dispatch.handle(Request::Started(Player::Uxplay), Instant::now());
    assert_eq!(inbox.take(), ["pause"]);
}

/// A dispatch with Kodi, running `command` as the screensaver, where the
/// profiles in `switch_to` switch; Kodi's inbox; and the channel children
/// report on.
fn with_screensaver(
    command: &[&str],
    switch_to: &[(&str, &str)],
) -> (Dispatch, Inbox, Receiver<Request>) {
    let (kodi, inbox) = kodi::channel();
    let others = Others::new(Some(kodi.clone()), None, vec![]);
    let screensaver = Screensaver {
        command: command.iter().map(|&arg| arg.to_owned()).collect(),
        switch_to: switch_to
            .iter()
            .map(|&(from, to)| (from.to_owned(), to.to_owned()))
            .collect::<BTreeMap<_, _>>(),
    };
    let mut dispatch = Dispatch::new(others, Some(kodi), true, Some(screensaver));
    let (tx, rx) = mpsc::channel();
    dispatch.tx = Some(tx);
    (dispatch, inbox, rx)
}

fn handle(dispatch: &mut Dispatch, request: Request) {
    dispatch.handle(request, Instant::now());
}

/// Waits for a child's report.
fn report(rx: &Receiver<Request>) -> Request {
    rx.recv_timeout(Duration::from_secs(5)).unwrap()
}

#[test]
fn without_profiles_the_command_runs_until_the_screensaver_stops() {
    let (mut dispatch, inbox, rx) = with_screensaver(&["sleep", "30"], &[]);
    handle(&mut dispatch, Request::Screensaver(true));
    assert!(dispatch.child.is_some());
    // Not asked for: nothing switches.
    assert_eq!(inbox.take(), [] as [String; 0]);
    // A second start doesn't run a second.
    handle(&mut dispatch, Request::Screensaver(true));
    assert_eq!(dispatch.last_child, 1);
    handle(&mut dispatch, Request::Screensaver(false));
    assert!(dispatch.child.is_none());
    // Killed, so no report.
    assert!(rx.recv_timeout(Duration::from_millis(500)).is_err());
    assert_eq!(inbox.take(), [] as [String; 0]);
}

#[test]
fn a_profile_that_switches_wakes_kodi_then_loads_the_other() {
    let (mut dispatch, inbox, _rx) = with_screensaver(&["sleep", "30"], &[("Pop", "Kids")]);
    handle(&mut dispatch, Request::Screensaver(true));
    assert_eq!(inbox.take(), ["profile?"]);
    handle(&mut dispatch, Request::Profile("Pop".to_owned()));
    // Not loaded under the screensaver.
    assert_eq!(inbox.take(), ["noop"]);
    assert!(dispatch.child.is_none());
    handle(&mut dispatch, Request::Screensaver(false));
    assert_eq!(inbox.take(), ["load Kids"]);
    // Only once.
    handle(&mut dispatch, Request::Screensaver(false));
    assert_eq!(inbox.take(), [] as [String; 0]);
    // The load drops the connection.
    handle(&mut dispatch, Request::KodiDisconnected);
    assert!(!dispatch.screensaver_up);
}

/// The connection dropping before Kodi woke, as Kodi exiting, drops the
/// switch, so a later wake in a new connection doesn't load anything.
#[test]
fn losing_kodi_mid_switch_drops_it() {
    let (mut dispatch, inbox, _rx) = with_screensaver(&["sleep", "30"], &[("Pop", "Kids")]);
    handle(&mut dispatch, Request::Screensaver(true));
    handle(&mut dispatch, Request::Profile("Pop".to_owned()));
    handle(&mut dispatch, Request::KodiDisconnected);
    handle(&mut dispatch, Request::Screensaver(false));
    assert_eq!(inbox.take(), ["profile?", "noop"]);
}

#[test]
fn a_profile_that_doesnt_switch_runs_the_command() {
    let (mut dispatch, inbox, _rx) = with_screensaver(&["sleep", "30"], &[("Pop", "Kids")]);
    handle(&mut dispatch, Request::Screensaver(true));
    handle(&mut dispatch, Request::Profile("Kids".to_owned()));
    assert_eq!(inbox.take(), ["profile?"]);
    assert!(dispatch.child.is_some());
}

#[test]
fn a_profile_reply_after_the_screensaver_stopped_does_nothing() {
    let (mut dispatch, inbox, _rx) = with_screensaver(&["sleep", "30"], &[("Pop", "Kids")]);
    handle(&mut dispatch, Request::Screensaver(true));
    handle(&mut dispatch, Request::Screensaver(false));
    for profile in ["Pop", "Kids"] {
        handle(&mut dispatch, Request::Profile(profile.to_owned()));
    }
    assert_eq!(inbox.take(), ["profile?"]);
    assert!(dispatch.child.is_none());
}

#[test]
fn the_command_quitting_wakes_kodi() {
    let (mut dispatch, inbox, rx) = with_screensaver(&["true"], &[]);
    handle(&mut dispatch, Request::Screensaver(true));
    let exited = report(&rx);
    assert!(
        matches!(exited, Request::ScreensaverExited { id: 1, .. }),
        "{exited:?}"
    );
    handle(&mut dispatch, exited);
    assert!(dispatch.child.is_none());
    assert_eq!(inbox.take(), ["noop"]);
    // Kodi waking stops nothing more.
    handle(&mut dispatch, Request::Screensaver(false));
    assert_eq!(inbox.take(), [] as [String; 0]);
}

/// After a stop, an old child's report mustn't wake Kodi, or end a newer
/// child's run.
#[test]
fn an_old_childs_report_is_ignored() {
    let (mut dispatch, inbox, _rx) = with_screensaver(&["sleep", "30"], &[]);
    handle(&mut dispatch, Request::Screensaver(true));
    handle(&mut dispatch, Request::Screensaver(false));
    handle(&mut dispatch, Request::Screensaver(true));
    let old = Request::ScreensaverExited {
        id: 1,
        status: "signal: 9 (SIGKILL)".to_owned(),
    };
    handle(&mut dispatch, old);
    assert_eq!(dispatch.child.as_ref().map(|child| child.id), Some(2));
    assert_eq!(inbox.take(), [] as [String; 0]);
}

#[test]
fn losing_kodi_stops_the_command() {
    let (mut dispatch, _inbox, _rx) = with_screensaver(&["sleep", "30"], &[]);
    handle(&mut dispatch, Request::Screensaver(true));
    handle(&mut dispatch, Request::KodiDisconnected);
    assert!(dispatch.child.is_none());
}

#[test]
fn a_command_that_wont_start_leaves_kodis_screensaver_up() {
    let (mut dispatch, inbox, _rx) = with_screensaver(&["/nonexistent/now-playing"], &[]);
    handle(&mut dispatch, Request::Screensaver(true));
    assert!(dispatch.child.is_none());
    assert_eq!(inbox.take(), [] as [String; 0]);
}

#[test]
fn a_dry_run_runs_and_loads_nothing() {
    let (mut dispatch, inbox, _rx) = with_screensaver(&["sleep", "30"], &[("Pop", "Kids")]);
    dispatch.dry_run = true;
    handle(&mut dispatch, Request::Screensaver(true));
    handle(&mut dispatch, Request::Profile("Pop".to_owned()));
    handle(&mut dispatch, Request::Screensaver(false));
    handle(&mut dispatch, Request::Screensaver(true));
    handle(&mut dispatch, Request::Profile("Kids".to_owned()));
    // Asking changes nothing, so it's asked even in a dry run.
    assert_eq!(inbox.take(), ["profile?", "profile?"]);
    assert!(dispatch.child.is_none());
}

#[test]
fn without_a_screensaver_table_kodis_screensaver_is_left_alone() {
    let (mut dispatch, inbox) = dispatch(true);
    handle(&mut dispatch, Request::Screensaver(true));
    handle(&mut dispatch, Request::Profile("Pop".to_owned()));
    assert!(dispatch.child.is_none());
    assert_eq!(inbox.take(), [] as [String; 0]);
}

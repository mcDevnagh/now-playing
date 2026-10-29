use super::*;
use crate::listener::kodi::Inbox;

/// `Others` with Kodi if `kodi`, and Snapcast if `snapcast`, and Kodi's inbox.
fn others(kodi: bool, snapcast: bool) -> (Others, Inbox) {
    let (kodi_controller, inbox) = kodi::channel();
    let snapcast = snapcast.then(|| snapcast::channel().0);
    let others = Others::new(kodi.then_some(kodi_controller), snapcast);
    (others, inbox)
}

#[test]
fn kodi_starting_mutes_snapcast() {
    let (others, inbox) = others(true, true);
    assert_eq!(others.quiets(Player::Kodi), [Quiet::SetMuted(true)]);
    others.started(Player::Kodi, false);
    assert_eq!(inbox.take_pauses(), 0);
}

#[test]
fn snapcast_starting_pauses_kodi_and_unmutes_snapcast() {
    let (others, inbox) = others(true, true);
    assert_eq!(
        others.quiets(Player::Snapcast),
        [Quiet::PauseKodi, Quiet::SetMuted(false)]
    );
    others.started(Player::Snapcast, false);
    assert_eq!(inbox.take_pauses(), 1);
}

#[test]
fn uxplay_starting_pauses_kodi_and_mutes_snapcast() {
    let (others, inbox) = others(true, true);
    assert_eq!(
        others.quiets(Player::Uxplay),
        [Quiet::PauseKodi, Quiet::SetMuted(true)]
    );
    others.started(Player::Uxplay, false);
    assert_eq!(inbox.take_pauses(), 1);
}

#[test]
fn dry_run_sends_nothing() {
    let (others, inbox) = others(true, true);
    for player in [Player::Kodi, Player::Snapcast, Player::Uxplay] {
        others.started(player, true);
    }
    assert_eq!(inbox.take_pauses(), 0);
}

#[test]
fn a_missing_player_leaves_the_other_its_command() {
    let (no_kodi, _) = others(false, true);
    assert_eq!(no_kodi.quiets(Player::Uxplay), [Quiet::SetMuted(true)]);
    assert_eq!(no_kodi.quiets(Player::Snapcast), [Quiet::SetMuted(false)]);

    let (no_snapcast, inbox) = others(true, false);
    assert_eq!(no_snapcast.quiets(Player::Uxplay), [Quiet::PauseKodi]);
    assert_eq!(no_snapcast.quiets(Player::Kodi), []);
    no_snapcast.started(Player::Uxplay, false);
    assert_eq!(inbox.take_pauses(), 1);
}

#[test]
fn empty_does_nothing() {
    let others = Others::default();
    for player in [Player::Kodi, Player::Snapcast, Player::Uxplay] {
        assert_eq!(others.quiets(player), []);
    }
}

#[test]
fn log_lines() {
    assert_eq!(
        describe(Player::Kodi, &[Quiet::SetMuted(true)], false),
        "kodi started; muting snapcast"
    );
    assert_eq!(
        describe(Player::Kodi, &[Quiet::SetMuted(true)], true),
        "kodi started; dry run: would mute snapcast"
    );
    let both = [Quiet::PauseKodi, Quiet::SetMuted(false)];
    assert_eq!(
        describe(Player::Snapcast, &both, false),
        "snapcast started; pausing kodi and unmuting snapcast"
    );
    assert_eq!(
        describe(Player::Snapcast, &both, true),
        "snapcast started; dry run: would pause kodi and unmute snapcast"
    );
}

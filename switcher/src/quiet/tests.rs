use super::*;
use crate::listener::kodi::Inbox;

const ALL: [Player; 3] = [Player::Kodi, Player::Snapcast, Player::Uxplay];

/// `Others` with Kodi if `kodi`, and Snapcast if `snapcast`, where the
/// players in `quieting` quiet the others, and Kodi's inbox.
fn others(kodi: bool, snapcast: bool, quieting: &[Player]) -> (Others, Inbox) {
    let (kodi_controller, inbox) = kodi::channel();
    let snapcast = snapcast.then(|| snapcast::channel().0);
    let others = Others::new(kodi.then_some(kodi_controller), snapcast, quieting.to_vec());
    (others, inbox)
}

#[test]
fn kodi_starting_mutes_snapcast() {
    let (others, inbox) = others(true, true, &ALL);
    assert_eq!(others.quiets(Player::Kodi), [Quiet::MuteSnapcast]);
    others.started(Player::Kodi, false);
    assert_eq!(inbox.take_pauses(), 0);
}

#[test]
fn snapcast_starting_pauses_kodi_and_unmutes_snapcast() {
    let (others, inbox) = others(true, true, &ALL);
    assert_eq!(
        others.quiets(Player::Snapcast),
        [Quiet::PauseKodi, Quiet::UnmuteSnapcast]
    );
    others.started(Player::Snapcast, false);
    assert_eq!(inbox.take_pauses(), 1);
}

#[test]
fn uxplay_starting_pauses_kodi_and_mutes_snapcast() {
    let (others, inbox) = others(true, true, &ALL);
    assert_eq!(
        others.quiets(Player::Uxplay),
        [Quiet::PauseKodi, Quiet::MuteSnapcast]
    );
    others.started(Player::Uxplay, false);
    assert_eq!(inbox.take_pauses(), 1);
}

#[test]
fn only_quieting_players_quiet() {
    // Only Kodi's table quiets the others.
    let (others, inbox) = others(true, true, &[Player::Kodi]);
    assert_eq!(others.quiets(Player::Kodi), [Quiet::MuteSnapcast]);
    assert_eq!(others.quiets(Player::Uxplay), []);
    // Snapcast doesn't pause Kodi, but still unmutes.
    assert_eq!(others.quiets(Player::Snapcast), [Quiet::UnmuteSnapcast]);
    others.started(Player::Snapcast, false);
    others.started(Player::Uxplay, false);
    assert_eq!(inbox.take_pauses(), 0);
}

#[test]
fn snapcast_always_unmutes() {
    let (others, _) = others(true, true, &[]);
    assert_eq!(others.quiets(Player::Snapcast), [Quiet::UnmuteSnapcast]);
    assert_eq!(others.quiets(Player::Kodi), []);
    assert_eq!(others.quiets(Player::Uxplay), []);
}

#[test]
fn dry_run_sends_nothing() {
    let (others, inbox) = others(true, true, &ALL);
    for player in ALL {
        others.started(player, true);
    }
    assert_eq!(inbox.take_pauses(), 0);
}

#[test]
fn a_missing_player_leaves_the_other_its_command() {
    let (no_kodi, _) = others(false, true, &ALL);
    assert_eq!(no_kodi.quiets(Player::Uxplay), [Quiet::MuteSnapcast]);
    assert_eq!(no_kodi.quiets(Player::Snapcast), [Quiet::UnmuteSnapcast]);

    let (no_snapcast, inbox) = others(true, false, &ALL);
    assert_eq!(no_snapcast.quiets(Player::Uxplay), [Quiet::PauseKodi]);
    assert_eq!(no_snapcast.quiets(Player::Kodi), []);
    no_snapcast.started(Player::Uxplay, false);
    assert_eq!(inbox.take_pauses(), 1);
}

#[test]
fn empty_does_nothing() {
    let others = Others::default();
    for player in ALL {
        assert_eq!(others.quiets(player), []);
    }
}

#[test]
fn log_lines() {
    assert_eq!(
        describe(Player::Kodi, &[Quiet::MuteSnapcast], false),
        "kodi started; muting snapcast"
    );
    assert_eq!(
        describe(Player::Kodi, &[Quiet::MuteSnapcast], true),
        "kodi started; dry run: would mute snapcast"
    );
    let both = [Quiet::PauseKodi, Quiet::UnmuteSnapcast];
    assert_eq!(
        describe(Player::Snapcast, &both, false),
        "snapcast started; pausing kodi and unmuting snapcast"
    );
    assert_eq!(
        describe(Player::Snapcast, &both, true),
        "snapcast started; dry run: would pause kodi and unmute snapcast"
    );
}

use super::*;

/// How many listeners `config` gets, which players `Others` reaches, as
/// (kodi, snapcast), and which quiet the others.
fn listeners(config: &str) -> (usize, (bool, bool), Vec<Player>) {
    let (listeners, others) = from_config(&Config::parse(config).unwrap()).unwrap();
    (
        listeners.len(),
        others.reaches(),
        others.quieting().to_vec(),
    )
}

/// Kodi only launches at start; Snapcast and UxPlay show on playback.
fn tables(kodi_quiets: bool, snapcast_quiets: bool, uxplay_quiets: bool) -> String {
    format!(
        r#"
        [kodi]
        gamescope_id = 1
        steam_game_id = 10375237194288726016
        launch_at_start = true
        quiet_others = {kodi_quiets}

        [snapcast]
        gamescope_id = 2
        show_on_playback = true
        server = "s:1705"
        client = "htpc"
        quiet_others = {snapcast_quiets}

        [uxplay]
        gamescope_id = 3
        show_on_playback = true
        quiet_others = {uxplay_quiets}
        "#
    )
}

#[test]
fn without_quiet_others_kodi_needs_no_listener() {
    // Snapcast's is kept, to unmute.
    assert_eq!(
        listeners(&tables(false, false, false)),
        (2, (false, true), vec![])
    );
}

#[test]
fn any_quieting_table_gives_kodi_a_listener() {
    assert_eq!(
        listeners(&tables(false, false, true)),
        (3, (true, true), vec![Player::Uxplay])
    );
    assert_eq!(
        listeners(&tables(true, true, true)),
        (
            3,
            (true, true),
            vec![Player::Kodi, Player::Snapcast, Player::Uxplay]
        )
    );
}

#[test]
fn snapcast_always_listens() {
    let config = "[snapcast]\ngamescope_id = 2\nserver = \"s:1705\"\nclient = \"htpc\"";
    assert_eq!(listeners(config), (1, (false, true), vec![]));
}

#[test]
fn uxplay_listens_to_show_or_quiet() {
    assert_eq!(
        listeners("[uxplay]\ngamescope_id = 3"),
        (0, (false, false), vec![])
    );
    let quiets = "[uxplay]\ngamescope_id = 3\nquiet_others = true";
    assert_eq!(listeners(quiets), (1, (false, false), vec![Player::Uxplay]));
}

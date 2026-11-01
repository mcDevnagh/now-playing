use super::*;

/// How many listeners `config` gets, which players `Others` reaches, as
/// (kodi, snapcast), which quiet the others, and whether Kodi's kept awake.
fn listeners(config: &str) -> (usize, (bool, bool), Vec<Player>, bool) {
    let config = Config::parse(config).unwrap();
    let (listeners, dispatch) = from_config(&config, Some(":1".to_owned())).unwrap();
    (
        listeners.len(),
        dispatch.others().reaches(),
        dispatch.others().quieting().to_vec(),
        dispatch.keeps_awake(),
    )
}

fn tables(kodi_quiets: bool, snapcast_quiets: bool, uxplay_quiets: bool) -> String {
    format!(
        r#"
        [kodi]
        quiet_others = {kodi_quiets}

        [snapcast]
        server = "s:1705"
        client = "htpc"
        quiet_others = {snapcast_quiets}

        [uxplay]
        quiet_others = {uxplay_quiets}
        keep_kodi_awake = false
        "#
    )
}

#[test]
fn without_quiet_others_only_snapcast_listens() {
    // Snapcast's is kept, to unmute.
    assert_eq!(
        listeners(&tables(false, false, false)),
        (1, (false, true), vec![], false)
    );
}

#[test]
fn any_quieting_table_gives_kodi_a_listener() {
    assert_eq!(
        listeners(&tables(false, true, false)),
        (2, (true, true), vec![Player::Snapcast], false)
    );
    assert_eq!(
        listeners(&tables(true, true, true)),
        (
            3,
            (true, true),
            vec![Player::Kodi, Player::Snapcast, Player::Uxplay],
            false
        )
    );
}

#[test]
fn snapcast_always_listens() {
    let config = "[snapcast]\nserver = \"s:1705\"\nclient = \"htpc\"";
    assert_eq!(listeners(config), (1, (false, true), vec![], false));
}

#[test]
fn uxplay_listens_to_quiet() {
    assert_eq!(
        listeners("[uxplay]\nkeep_kodi_awake = false"),
        (0, (false, false), vec![], false)
    );
    let quiets = "[uxplay]\nquiet_others = true\nkeep_kodi_awake = false";
    assert_eq!(
        listeners(quiets),
        (1, (false, false), vec![Player::Uxplay], false)
    );
}

#[test]
fn keeping_kodi_awake_needs_both_tables() {
    // On by default.
    assert_eq!(
        listeners("[kodi]\n[uxplay]"),
        (2, (true, false), vec![], true)
    );
    // Without Kodi there's nothing to keep awake.
    assert_eq!(listeners("[uxplay]"), (0, (false, false), vec![], false));
}

#[test]
fn a_screensaver_gives_kodi_a_listener() {
    let config = "[kodi.screensaver]\ncommand = [\"now-playing\"]";
    assert_eq!(listeners(config), (1, (true, false), vec![], false));
    let config = Config::parse(config).unwrap();
    let (_, dispatch) = from_config(&config, None).unwrap();
    assert!(dispatch.follows_screensaver());
}

#[test]
fn uxplay_needs_a_display() {
    let config = Config::parse("[uxplay]\nquiet_others = true").unwrap();
    let err = from_config(&config, None).err().unwrap();
    assert!(err.contains("$DISPLAY"), "{err}");
    let config = Config::parse("[kodi]\n[uxplay]").unwrap();
    assert!(from_config(&config, None).is_err());
    // Not when it has nothing to do.
    let config = Config::parse("[uxplay]").unwrap();
    assert!(from_config(&config, None).is_ok());
}

use super::*;

/// How many listeners `config` gets, which players `Others` reaches, as
/// (kodi, snapcast), and which quiet the others.
fn listeners(config: &str) -> (usize, (bool, bool), Vec<Player>) {
    let config = Config::parse(config).unwrap();
    let (listeners, others) = from_config(&config, Some(":1".to_owned())).unwrap();
    (
        listeners.len(),
        others.reaches(),
        others.quieting().to_vec(),
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
        "#
    )
}

#[test]
fn without_quiet_others_only_snapcast_listens() {
    // Snapcast's is kept, to unmute.
    assert_eq!(
        listeners(&tables(false, false, false)),
        (1, (false, true), vec![])
    );
}

#[test]
fn any_quieting_table_gives_kodi_a_listener() {
    assert_eq!(
        listeners(&tables(false, true, false)),
        (2, (true, true), vec![Player::Snapcast])
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
    let config = "[snapcast]\nserver = \"s:1705\"\nclient = \"htpc\"";
    assert_eq!(listeners(config), (1, (false, true), vec![]));
}

#[test]
fn uxplay_listens_to_quiet() {
    assert_eq!(listeners("[uxplay]"), (0, (false, false), vec![]));
    let quiets = "[uxplay]\nquiet_others = true";
    assert_eq!(listeners(quiets), (1, (false, false), vec![Player::Uxplay]));
}

#[test]
fn uxplay_needs_a_display() {
    let config = Config::parse("[uxplay]\nquiet_others = true").unwrap();
    let err = from_config(&config, None).err().unwrap();
    assert!(err.contains("$DISPLAY"), "{err}");
    // Not when it has nothing to do.
    let config = Config::parse("[uxplay]").unwrap();
    assert!(from_config(&config, None).is_ok());
}

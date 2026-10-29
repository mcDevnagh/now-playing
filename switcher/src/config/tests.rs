use super::*;

const FULL: &str = r#"
    [kodi]
    gamescope_id = 1
    show_on_playback = true
    address = "kodi:9090"

    [snapcast]
    gamescope_id = 2457742405
    steam_game_id = 10555923251500941312
    show_on_playback = true
    server = "snapserver:1705"
    client = "TV"
    launch_at_start = true
    launch_at_start_wait = 5

    [uxplay]
    gamescope_id = 3
    steam_game_id = 12918456320
    window_class = "Mirror"
    launch_at_start = true
"#;

#[test]
fn full() {
    let config = Config::parse(FULL).unwrap();
    assert_eq!(
        config,
        Config {
            kodi: Some(Kodi {
                app: App {
                    gamescope: 1,
                    launch: None,
                },
                show_on_playback: true,
                quiet_others: false,
                address: "kodi:9090".to_owned(),
                launch_at_start: None,
            }),
            snapcast: Some(Snapcast {
                app: App {
                    gamescope: 2457742405,
                    launch: Some(10555923251500941312),
                },
                show_on_playback: true,
                quiet_others: false,
                server: "snapserver:1705".to_owned(),
                client: Some("TV".to_owned()),
                launch_at_start: Some(Duration::from_secs(5)),
            }),
            uxplay: Some(Uxplay {
                app: App {
                    gamescope: 3,
                    launch: Some(12918456320),
                },
                show_on_playback: false,
                quiet_others: false,
                window_class: "Mirror".to_owned(),
                launch_at_start: Some(Duration::from_secs(DEFAULT_LAUNCH_AT_START_WAIT)),
            }),
        }
    );
}

#[test]
fn launch_at_start() {
    assert_eq!(
        Config::parse(FULL).unwrap().launch_at_start(),
        [
            (
                App {
                    gamescope: 2457742405,
                    launch: Some(10555923251500941312),
                },
                Duration::from_secs(5)
            ),
            (
                App {
                    gamescope: 3,
                    launch: Some(12918456320),
                },
                Duration::from_secs(DEFAULT_LAUNCH_AT_START_WAIT)
            ),
        ]
    );
    let kodi =
        "[kodi]\ngamescope_id = 1\nsteam_game_id = 10375237194288726016\nlaunch_at_start = true";
    let config = Config::parse(kodi).unwrap();
    let wait = Duration::from_secs(DEFAULT_LAUNCH_AT_START_WAIT);
    assert_eq!(config.launch_at_start(), [(config.kodi.unwrap().app, wait)]);
    assert_eq!(
        Config::parse("[kodi]\ngamescope_id = 1")
            .unwrap()
            .launch_at_start(),
        []
    );
}

#[test]
fn ids_are_taken_as_given() {
    // A Steam game ID whose top 32 bits aren't the gamescope ID: neither is
    // worked out from, or checked against, the other.
    let kodi = Config::parse("[kodi]\ngamescope_id = 7\nsteam_game_id = 10375237194288726016")
        .unwrap()
        .kodi
        .unwrap();
    assert_eq!(
        kodi.app,
        App {
            gamescope: 7,
            launch: Some(10375237194288726016),
        }
    );
}

#[test]
fn minimal_uxplay_takes_the_default_window_class() {
    let config = Config::parse("[uxplay]\ngamescope_id = 3").unwrap();
    let uxplay = config.uxplay.unwrap();
    assert_eq!(uxplay.app.gamescope, 3);
    assert_eq!(uxplay.window_class, DEFAULT_UXPLAY_WINDOW_CLASS);
    assert_eq!(uxplay.launch_at_start, None);
    assert_eq!((config.kodi, config.snapcast), (None, None));
}

#[test]
fn show_on_playback_defaults_to_off() {
    let config = Config::parse("[uxplay]\ngamescope_id = 3").unwrap();
    assert!(!config.uxplay.unwrap().show_on_playback);
    let config = Config::parse("[uxplay]\ngamescope_id = 3\nshow_on_playback = true").unwrap();
    assert!(config.uxplay.unwrap().show_on_playback);
}

#[test]
fn a_table_can_do_nothing() {
    for table in ["[kodi]", "[uxplay]", "[snapcast]\nserver = \"s:1705\""] {
        let text = format!("{table}\ngamescope_id = 1\nsteam_game_id = 10375237194288726016");
        assert!(Config::parse(&text).is_ok(), "{text:?}");
    }
}

#[test]
fn minimal_kodi_takes_the_default_address() {
    let config = Config::parse("[kodi]\ngamescope_id = 1").unwrap();
    assert_eq!(config.kodi.unwrap().address, DEFAULT_KODI_ADDRESS);
    assert_eq!(config.snapcast, None);
}

#[test]
fn minimal_snapcast() {
    let snapcast = Config::parse("[snapcast]\ngamescope_id = 2\nserver = \"s:1705\"")
        .unwrap()
        .snapcast
        .unwrap();
    assert_eq!(snapcast.app.gamescope, 2);
    assert_eq!(snapcast.client, None);
}

/// Each case and a word its error must contain.
#[test]
fn errors() {
    let cases = [
        ("", "[kodi], [snapcast] or [uxplay]"),
        ("[kodi]", "gamescope_id"),
        (
            "[kodi]\nsteam_game_id = 10375237194288726016",
            "gamescope_id",
        ),
        ("[kodi]\ngamescope_id = -1", "gamescope_id"),
        ("[kodi]\ngamescope_id = 1\napp = 1", "app"),
        ("[kodi]\ngamescope_id = 1\nlaunch = 1", "launch"),
        ("[snapcast]\nserver = \"s:1705\"", "gamescope_id"),
        ("[snapcast]\ngamescope_id = 2", "server"),
        ("[snapcast]\ngamescope_id = 2\nserver = \"\"", "server"),
        (
            "[snapcast]\ngamescope_id = 4294967296\nserver = \"s:1705\"",
            "gamescope_id",
        ),
        ("[kodi]\ngamescope_id = 1\nport = 9090", "port"),
        (
            "[kodi]\ngamescope_id = 1\nshow_on_playback = 1",
            "show_on_playback",
        ),
        (
            "[kodi]\ngamescope_id = 1\nlaunch_at_start = true",
            "[kodi] `launch_at_start` needs `steam_game_id`",
        ),
        (
            "[snapcast]\ngamescope_id = 2\nserver = \"s:1705\"\nlaunch_at_start = true",
            "[snapcast] `launch_at_start` needs `steam_game_id`",
        ),
        (
            "[kodi]\ngamescope_id = 1\nlaunch_at_start = 1",
            "launch_at_start",
        ),
        ("[uxplay]", "gamescope_id"),
        (
            "[uxplay]\ngamescope_id = 3\nwindow_class = \"\"",
            "[uxplay] `window_class` is empty",
        ),
        (
            "[uxplay]\ngamescope_id = 3\nlaunch_at_start = true",
            "[uxplay] `launch_at_start` needs `steam_game_id`",
        ),
        (
            "[uxplay]\ngamescope_id = 3\nfile = \"~/.uxplay.dacp\"",
            "file",
        ),
        (
            "[kodi]\ngamescope_id = 1\nsteam_game_id = 10375237194288726016\nlaunch_at_start_wait = 5",
            "[kodi] `launch_at_start_wait` needs `launch_at_start = true`",
        ),
        (
            "[kodi]\ngamescope_id = 1\nsteam_game_id = 10375237194288726016\nlaunch_at_start = true\nlaunch_at_start_wait = -1",
            "launch_at_start_wait",
        ),
        ("[kodi]\ngamescope_id = 1\n[spotify]", "spotify"),
        ("gamescope_id = 1", "gamescope_id"),
        ("[kodi\ngamescope_id = 1", "kodi"),
    ];
    for (text, needle) in cases {
        let err = Config::parse(text).unwrap_err();
        assert!(err.contains(needle), "{text:?} gave {err:?}");
    }
}

#[test]
fn quiet_others_is_per_table_and_defaults_to_off() {
    let config = Config::parse("[kodi]\ngamescope_id = 1").unwrap();
    assert!(!config.kodi.unwrap().quiet_others);
    let text = "[kodi]\ngamescope_id = 1\nquiet_others = true\n[uxplay]\ngamescope_id = 3";
    let config = Config::parse(text).unwrap();
    assert!(config.kodi.unwrap().quiet_others);
    assert!(!config.uxplay.unwrap().quiet_others);
    for (text, needle) in [
        ("[kodi]\ngamescope_id = 1\nquiet_others = 1", "quiet_others"),
        // Not top-level any more.
        (
            "quiet_others = true\n[kodi]\ngamescope_id = 1",
            "quiet_others",
        ),
    ] {
        let err = Config::parse(text).unwrap_err();
        assert!(err.contains(needle), "{text:?} gave {err:?}");
    }
}

#[test]
fn load_names_the_file() {
    let err = Config::load(Path::new("/nonexistent/switcher.toml")).unwrap_err();
    assert!(err.starts_with("/nonexistent/switcher.toml: "), "{err}");
}

#[test]
fn default_path_prefers_xdg_config_home() {
    let path = default_path_from(Some("/xdg".into()), Some("/home/me".into()));
    assert_eq!(path.unwrap(), Path::new("/xdg/now-playing/switcher.toml"));
}

#[test]
fn default_path_falls_back_to_home() {
    let expected = Path::new("/home/me/.config/now-playing/switcher.toml");
    for xdg in [None, Some("".into()), Some("relative".into())] {
        let path = default_path_from(xdg, Some("/home/me".into()));
        assert_eq!(path.unwrap(), expected);
    }
    assert_eq!(default_path_from(None, None), None);
}

#[test]
fn example_parses() {
    let config = Config::parse(include_str!("../../switcher.example.toml")).unwrap();
    let kodi = config.kodi.unwrap();
    assert_eq!(kodi.app.gamescope, 1234567890);
    assert!(kodi.show_on_playback && kodi.quiet_others);
    let snapcast = config.snapcast.unwrap();
    assert_eq!(snapcast.app.gamescope, 2345678901);
    assert!(snapcast.show_on_playback && snapcast.quiet_others);
    let uxplay = config.uxplay.unwrap();
    assert_eq!(uxplay.app.gamescope, 3456789012);
    assert!(uxplay.show_on_playback && uxplay.quiet_others);
    assert_eq!(uxplay.window_class, "GStreamer");
}

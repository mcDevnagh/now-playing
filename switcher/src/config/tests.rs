use super::*;

const FULL: &str = r#"
    [kodi]
    quiet_others = true
    address = "kodi:9090"

    [kodi.screensaver]
    command = ["now-playing", "--screensaver"]

    [kodi.profiles."Master user"]
    switch_to = "Guest"

    [snapcast]
    server = "snapserver:1705"
    client = "TV"

    [uxplay]
    quiet_others = true
    window_class = "Mirror"
    keep_kodi_awake = false
"#;

#[test]
fn full() {
    let config = Config::parse(FULL).unwrap();
    assert_eq!(
        config,
        Config {
            kodi: Some(Kodi {
                quiet_others: true,
                address: "kodi:9090".to_owned(),
                screensaver: Some(Screensaver {
                    command: vec!["now-playing".to_owned(), "--screensaver".to_owned()],
                    switch_to: BTreeMap::from([("Master user".to_owned(), "Guest".to_owned())]),
                }),
            }),
            snapcast: Some(Snapcast {
                quiet_others: false,
                server: "snapserver:1705".to_owned(),
                client: Some("TV".to_owned()),
            }),
            uxplay: Some(Uxplay {
                quiet_others: true,
                window_class: "Mirror".to_owned(),
                keep_kodi_awake: false,
            }),
        }
    );
}

#[test]
fn minimal_uxplay_takes_the_default_window_class() {
    let config = Config::parse("[uxplay]").unwrap();
    let uxplay = config.uxplay.unwrap();
    assert_eq!(uxplay.window_class, DEFAULT_UXPLAY_WINDOW_CLASS);
    assert!(uxplay.keep_kodi_awake);
    assert_eq!((config.kodi, config.snapcast), (None, None));
}

#[test]
fn a_table_can_do_nothing() {
    for table in ["[kodi]", "[uxplay]", "[snapcast]\nserver = \"s:1705\""] {
        assert!(Config::parse(table).is_ok(), "{table:?}");
    }
}

#[test]
fn minimal_kodi_takes_the_default_address() {
    let config = Config::parse("[kodi]").unwrap();
    assert_eq!(config.kodi.unwrap().address, DEFAULT_KODI_ADDRESS);
    assert_eq!(config.snapcast, None);
}

#[test]
fn minimal_snapcast() {
    let snapcast = Config::parse("[snapcast]\nserver = \"s:1705\"")
        .unwrap()
        .snapcast
        .unwrap();
    assert_eq!(snapcast.client, None);
}

/// Each case and a word its error must contain.
#[test]
fn errors() {
    let cases = [
        ("", "[kodi], [snapcast] or [uxplay]"),
        ("[kodi]\napp = 1", "app"),
        ("[snapcast]", "server"),
        ("[snapcast]\nserver = \"\"", "server"),
        ("[kodi]\nport = 9090", "port"),
        ("[kodi]\nquiet_others = 1", "quiet_others"),
        (
            "[uxplay]\nwindow_class = \"\"",
            "[uxplay] `window_class` is empty",
        ),
        ("[uxplay]\nfile = \"~/.uxplay.dacp\"", "file"),
        ("[kodi]\n[spotify]", "spotify"),
        ("address = \"kodi:9090\"", "address"),
        ("[kodi\naddress = \"kodi:9090\"", "kodi"),
    ];
    for (text, needle) in cases {
        let err = Config::parse(text).unwrap_err();
        assert!(err.contains(needle), "{text:?} gave {err:?}");
    }
}

#[test]
fn typed_errors_point_at_the_line() {
    let err = Config::parse("[kodi]\nquiet_others = true\nport = 9090").unwrap_err();
    assert!(err.contains("line 3"), "{err}");
}

#[test]
fn an_old_config_names_every_removed_key() {
    let old = r#"
        [kodi]
        gamescope_id = 1
        show_on_playback = true
        quiet_others = true

        [snapcast]
        gamescope_id = 2
        steam_game_id = 10555923251500941312
        server = "snapserver:1705"

        [uxplay]
        launch_at_start = true
        launch_at_start_wait = 5
    "#;
    let err = Config::parse(old).unwrap_err();
    assert_eq!(
        err,
        "the switcher no longer brings apps to the front, so remove \
         kodi.gamescope_id, kodi.show_on_playback, snapcast.gamescope_id, \
         snapcast.steam_game_id, uxplay.launch_at_start, uxplay.launch_at_start_wait"
    );
}

#[test]
fn the_screensaver_is_optional_and_profiles_need_it() {
    assert_eq!(
        Config::parse("[kodi]").unwrap().kodi.unwrap().screensaver,
        None
    );
    let config = Config::parse("[kodi.screensaver]\ncommand = [\"now-playing\"]").unwrap();
    let screensaver = config.kodi.unwrap().screensaver.unwrap();
    assert!(screensaver.switch_to.is_empty());
    let cases = [
        (
            "[kodi.profiles.Pop]\nswitch_to = \"Kids\"",
            "[kodi.profiles] needs [kodi.screensaver]",
        ),
        (
            "[kodi.screensaver]\ncommand = []",
            "[kodi.screensaver] `command` needs a program to run",
        ),
        (
            "[kodi.screensaver]\ncommand = [\"\", \"--screensaver\"]",
            "[kodi.screensaver] `command` needs a program to run",
        ),
        ("[kodi.screensaver]", "command"),
        ("[kodi.screensaver]\ncommand = \"now-playing\"", "command"),
        (
            "[kodi.screensaver]\ncommand = [\"now-playing\"]\nargs = []",
            "args",
        ),
        (
            "[kodi.screensaver]\ncommand = [\"np\"]\n[kodi.profiles.Pop]\nswitch_to = \"Pop\"",
            "[kodi.profiles.\"Pop\"] `switch_to` is the profile itself",
        ),
        (
            "[kodi.screensaver]\ncommand = [\"np\"]\n[kodi.profiles.Pop]\nswitch_to = \"\"",
            "[kodi.profiles.\"Pop\"] `switch_to` is empty",
        ),
        (
            "[kodi.screensaver]\ncommand = [\"np\"]\n[kodi.profiles.Pop]\nlock = true",
            "lock",
        ),
    ];
    for (text, needle) in cases {
        let err = Config::parse(text).unwrap_err();
        assert!(err.contains(needle), "{text:?} gave {err:?}");
    }
}

#[test]
fn quiet_others_is_per_table_and_defaults_to_off() {
    let config = Config::parse("[kodi]").unwrap();
    assert!(!config.kodi.unwrap().quiet_others);
    let config = Config::parse("[kodi]\nquiet_others = true\n[uxplay]").unwrap();
    assert!(config.kodi.unwrap().quiet_others);
    assert!(!config.uxplay.unwrap().quiet_others);
    // Not top-level.
    let err = Config::parse("quiet_others = true\n[kodi]").unwrap_err();
    assert!(err.contains("quiet_others"), "{err}");
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
    assert!(kodi.quiet_others);
    let screensaver = kodi.screensaver.unwrap();
    assert_eq!(screensaver.command[1], "--screensaver");
    assert_eq!(screensaver.switch_to["Master user"], "Guest");
    assert!(config.snapcast.unwrap().quiet_others);
    let uxplay = config.uxplay.unwrap();
    assert!(uxplay.quiet_others);
    assert_eq!(uxplay.window_class, "GStreamer");
    assert!(uxplay.keep_kodi_awake);
}

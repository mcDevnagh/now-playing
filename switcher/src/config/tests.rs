use super::*;

const FULL: &str = r#"
    [kodi]
    app = 1
    address = "kodi:9090"

    [snapcast]
    app = 2457742405
    launch = 10555923251500941312
    server = "snapserver:1705"
    client = "TV"
    launch_at_start = true
    launch_at_start_wait = 5

    [uxplay]
    launch = 12918456320
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
                switch: true,
                address: "kodi:9090".to_owned(),
                launch_at_start: None,
            }),
            snapcast: Some(Snapcast {
                app: App {
                    gamescope: 2457742405,
                    launch: Some(10555923251500941312),
                },
                switch: true,
                server: "snapserver:1705".to_owned(),
                client: Some("TV".to_owned()),
                launch_at_start: Some(Duration::from_secs(5)),
            }),
            uxplay: Some(Uxplay {
                app: App {
                    gamescope: 3,
                    launch: Some(12918456320),
                },
                switch: false,
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
    let kodi = "[kodi]\nlaunch = 10375237194288726016\nlaunch_at_start = true";
    let config = Config::parse(kodi).unwrap();
    let wait = Duration::from_secs(DEFAULT_LAUNCH_AT_START_WAIT);
    assert_eq!(config.launch_at_start(), [(config.kodi.unwrap().app, wait)]);
    assert_eq!(
        Config::parse("[kodi]\napp = 1").unwrap().launch_at_start(),
        []
    );
}

#[test]
fn kodi_can_have_a_launch_id() {
    let kodi = Config::parse("[kodi]\napp = 2415673154\nlaunch = 10375237194288726016")
        .unwrap()
        .kodi
        .unwrap();
    assert_eq!(
        kodi.app,
        App {
            gamescope: 2415673154,
            launch: Some(10375237194288726016),
        }
    );
}

#[test]
fn minimal_uxplay_takes_the_default_window_class() {
    let config = Config::parse("[uxplay]\napp = 3").unwrap();
    let uxplay = config.uxplay.unwrap();
    assert_eq!(uxplay.app.gamescope, 3);
    assert_eq!(uxplay.window_class, DEFAULT_UXPLAY_WINDOW_CLASS);
    assert_eq!(uxplay.launch_at_start, None);
    assert_eq!((config.kodi, config.snapcast), (None, None));
}

#[test]
fn without_app_a_table_only_launches_at_start() {
    let config = Config::parse("[uxplay]\nlaunch = 12918456320\nlaunch_at_start = true").unwrap();
    let uxplay = config.uxplay.as_ref().unwrap();
    assert_eq!(uxplay.app.gamescope, 3);
    assert!(!uxplay.switch);
    assert_eq!(config.launch_at_start().len(), 1);
    for ids in ["app = 3", "app = 3\nlaunch = 12918456320"] {
        let uxplay = Config::parse(&format!("[uxplay]\n{ids}"))
            .unwrap()
            .uxplay
            .unwrap();
        assert!(uxplay.switch, "{ids}");
    }
}

#[test]
fn minimal_kodi_takes_the_default_address() {
    let config = Config::parse("[kodi]\napp = 1").unwrap();
    assert_eq!(config.kodi.unwrap().address, DEFAULT_KODI_ADDRESS);
    assert_eq!(config.snapcast, None);
}

#[test]
fn minimal_snapcast() {
    let snapcast = Config::parse("[snapcast]\napp = 2\nserver = \"s:1705\"")
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
        ("[kodi]", "[kodi] needs `app`, `launch`"),
        (
            "[kodi]\napp = 1\nlaunch = 10375237194288726016",
            "doesn't match",
        ),
        ("[kodi]\napp = -1", "app"),
        (
            "[snapcast]\nserver = \"s:1705\"",
            "[snapcast] needs `app`, `launch`",
        ),
        ("[snapcast]\napp = 2", "server"),
        ("[snapcast]\napp = 2\nserver = \"\"", "server"),
        ("[snapcast]\napp = 4294967296\nserver = \"s:1705\"", "app"),
        ("[kodi]\napp = 1\nport = 9090", "port"),
        (
            "[kodi]\napp = 1\nlaunch_at_start = true",
            "[kodi] `launch_at_start` needs `launch`",
        ),
        (
            "[snapcast]\napp = 2\nserver = \"s:1705\"\nlaunch_at_start = true",
            "[snapcast] `launch_at_start` needs `launch`",
        ),
        ("[kodi]\napp = 1\nlaunch_at_start = 1", "launch_at_start"),
        ("[uxplay]", "[uxplay] needs `app`, `launch`"),
        (
            "[uxplay]\napp = 3\nwindow_class = \"\"",
            "[uxplay] `window_class` is empty",
        ),
        (
            "[uxplay]\napp = 3\nlaunch_at_start = true",
            "[uxplay] `launch_at_start` needs `launch`",
        ),
        ("[uxplay]\napp = 3\nfile = \"~/.uxplay.dacp\"", "file"),
        (
            "[kodi]\nlaunch = 10375237194288726016\nlaunch_at_start_wait = 5",
            "[kodi] `launch_at_start_wait` needs `launch_at_start = true`",
        ),
        (
            "[kodi]\nlaunch = 10375237194288726016\nlaunch_at_start = true\nlaunch_at_start_wait = -1",
            "launch_at_start_wait",
        ),
        (
            "[kodi]\nlaunch = 10375237194288726016",
            "[kodi] does nothing",
        ),
        ("[uxplay]\nlaunch = 12918456320", "[uxplay] does nothing"),
        ("[kodi]\napp = 1\n[spotify]", "spotify"),
        ("app = 1", "app"),
        ("[kodi\napp = 1", "kodi"),
    ];
    for (text, needle) in cases {
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
    assert_eq!(config.kodi.unwrap().app.gamescope, 1234567890);
    assert_eq!(config.snapcast.unwrap().app.gamescope, 2345678901);
    let uxplay = config.uxplay.unwrap();
    assert_eq!(uxplay.app.gamescope, 3456789012);
    assert_eq!(uxplay.window_class, "GStreamer");
}

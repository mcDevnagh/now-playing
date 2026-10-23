use super::*;

const FULL: &str = r#"
    [kodi]
    app = 1
    address = "kodi:9090"

    [snapcast]
    app = 2
    launch = 10555923251500941312
    server = "snapserver:1705"
    client = "TV"
"#;

#[test]
fn full() {
    let config = Config::parse(FULL).unwrap();
    assert_eq!(
        config,
        Config {
            kodi: Some(Kodi {
                app: 1,
                address: "kodi:9090".to_owned(),
            }),
            snapcast: Some(Snapcast {
                app: Some(2),
                launch: Some(10555923251500941312),
                server: "snapserver:1705".to_owned(),
                client: Some("TV".to_owned()),
            }),
        }
    );
}

#[test]
fn minimal_kodi_takes_the_default_address() {
    let config = Config::parse("[kodi]\napp = 1").unwrap();
    assert_eq!(config.kodi.unwrap().address, DEFAULT_KODI_ADDRESS);
    assert_eq!(config.snapcast, None);
}

#[test]
fn minimal_snapcast() {
    for text in [
        "[snapcast]\napp = 2\nserver = \"s:1705\"",
        "[snapcast]\nlaunch = 3\nserver = \"s:1705\"",
    ] {
        let snapcast = Config::parse(text).unwrap().snapcast.unwrap();
        assert_eq!(snapcast.client, None);
    }
}

/// Each case and a word its error must contain.
#[test]
fn errors() {
    let cases = [
        ("", "[kodi] or [snapcast]"),
        ("[kodi]", "app"),
        ("[kodi]\napp = -1", "app"),
        ("[snapcast]\nserver = \"s:1705\"", "`app`, `launch`"),
        ("[snapcast]\napp = 2", "server"),
        ("[snapcast]\napp = 2\nserver = \"\"", "server"),
        ("[snapcast]\napp = 4294967296\nserver = \"s:1705\"", "app"),
        ("[kodi]\napp = 1\nport = 9090", "port"),
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

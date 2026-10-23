use super::*;
use crate::config::{Kodi, Snapcast};

fn kodi() -> Kodi {
    Kodi {
        app: 1,
        address: "kodi:9090".to_owned(),
    }
}

fn snapcast(app: Option<u32>, launch: Option<u64>) -> Snapcast {
    Snapcast {
        app,
        launch,
        server: "s:1705".to_owned(),
        client: Some("TV".to_owned()),
    }
}

fn snapcast_listener() -> Listener {
    Listener::Snapcast {
        server: "s:1705".to_owned(),
        client: Some("TV".to_owned()),
    }
}

fn kodi_listener() -> Listener {
    Listener::Kodi {
        address: "kodi:9090".to_owned(),
    }
}

#[test]
fn everything() {
    let registry = Registry::new(&Config {
        kodi: Some(kodi()),
        snapcast: Some(snapcast(Some(2), Some(3))),
    });
    assert_eq!(registry.listeners, [snapcast_listener(), kodi_listener()]);
    assert_eq!(
        registry.actions(Trigger::SnapcastStarted),
        [Action::Focus(2), Action::Launch(3)]
    );
    assert_eq!(registry.actions(Trigger::KodiStarted), [Action::Focus(1)]);
}

#[test]
fn kodi_only() {
    let registry = Registry::new(&Config {
        kodi: Some(kodi()),
        snapcast: None,
    });
    assert_eq!(registry.listeners, [kodi_listener()]);
    assert_eq!(registry.actions(Trigger::SnapcastStarted), []);
    assert_eq!(registry.actions(Trigger::KodiStarted), [Action::Focus(1)]);
}

#[test]
fn snapcast_only() {
    for (app, launch, actions) in [
        (Some(2), None, &[Action::Focus(2)][..]),
        (None, Some(3), &[Action::Launch(3)][..]),
        (Some(2), Some(3), &[Action::Focus(2), Action::Launch(3)][..]),
    ] {
        let registry = Registry::new(&Config {
            kodi: None,
            snapcast: Some(snapcast(app, launch)),
        });
        assert_eq!(registry.listeners, [snapcast_listener()]);
        assert_eq!(registry.actions(Trigger::SnapcastStarted), actions);
        assert_eq!(registry.actions(Trigger::KodiStarted), []);
    }
}

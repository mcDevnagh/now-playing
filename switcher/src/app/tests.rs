use super::*;

/// Now Playing's IDs on the HTPC.
const GAMESCOPE: u32 = 2457742405;
const LAUNCH: u64 = 10555923251500941312;

#[test]
fn launch_gives_the_gamescope_id() {
    let app = App::new(None, Some(LAUNCH)).unwrap();
    assert_eq!(
        app,
        App {
            gamescope: GAMESCOPE,
            launch: Some(LAUNCH),
        }
    );
}

#[test]
fn gamescope_alone_cant_launch() {
    let app = App::new(Some(GAMESCOPE), None).unwrap();
    assert_eq!(
        app,
        App {
            gamescope: GAMESCOPE,
            launch: None,
        }
    );
}

#[test]
fn matching_ids() {
    let app = App::new(Some(GAMESCOPE), Some(LAUNCH)).unwrap();
    assert_eq!(app.gamescope, GAMESCOPE);
}

#[test]
fn mismatched_ids() {
    let err = App::new(Some(GAMESCOPE + 1), Some(LAUNCH)).unwrap_err();
    assert!(err.contains("doesn't match"), "{err}");
}

#[test]
fn neither_id() {
    let err = App::new(None, None).unwrap_err();
    assert!(err.contains("`app`, `launch`"), "{err}");
}

use super::*;

const APP: App = App {
    gamescope: 7,
    launch: Some(7 << 32 | 0x0200_0000),
};
const NO_LAUNCH: App = App {
    gamescope: 7,
    launch: None,
};
const OTHER: u32 = 8;

fn running(answer: bool) -> impl FnOnce() -> Result<bool, String> {
    move || Ok(answer)
}

fn not_asked() -> Result<bool, String> {
    panic!("checked whether the app is running when it didn't matter")
}

#[test]
fn outside_gaming_mode_does_nothing() {
    assert_eq!(decide(APP, None, not_asked), Ok(Decision::NotGamingMode));
}

#[test]
fn in_front_does_nothing() {
    assert_eq!(
        decide(APP, Some(7), not_asked),
        Ok(Decision::AlreadyInFront)
    );
}

#[test]
fn running_behind_is_focused() {
    assert_eq!(decide(APP, Some(OTHER), running(true)), Ok(Decision::Focus));
    assert_eq!(
        decide(NO_LAUNCH, Some(OTHER), running(true)),
        Ok(Decision::Focus)
    );
}

#[test]
fn not_running_is_launched() {
    assert_eq!(
        decide(APP, Some(OTHER), running(false)),
        Ok(Decision::Launch(APP.launch.unwrap()))
    );
}

#[test]
fn not_running_without_a_launch_id_cant_launch() {
    assert_eq!(
        decide(NO_LAUNCH, Some(OTHER), running(false)),
        Ok(Decision::CantLaunch)
    );
}

/// Launching an app that is in fact running makes Steam show an error, so
/// an unknown answer must not launch.
#[test]
fn failed_running_check_is_an_error() {
    let result = decide(APP, Some(OTHER), || Err("gone".to_owned()));
    assert_eq!(result, Err("gone".to_owned()));
}

#[test]
fn focus_outcomes() {
    let cases = [
        (Ok(Some(APP.gamescope)), "switched"),
        (Ok(Some(OTHER)), "didn't switch (app 8 is still in front)"),
        (Ok(None), "didn't switch (no longer in Gaming Mode)"),
        (
            Err("display :0: gone".to_owned()),
            "focused it, but can't check it switched (display :0: gone)",
        ),
    ];
    for (front, outcome) in cases {
        assert_eq!(focus_outcome(APP.gamescope, front), outcome);
    }
}

#[test]
fn at_start_outside_gaming_mode_does_nothing() {
    assert_eq!(
        decide_start(APP, None, not_asked),
        Ok(StartDecision::NotGamingMode)
    );
}

/// A switcher restart mustn't launch it again, or pull it to the front.
#[test]
fn at_start_running_is_left_alone() {
    for front in [APP.gamescope, OTHER] {
        assert_eq!(
            decide_start(APP, Some(front), running(true)),
            Ok(StartDecision::AlreadyRunning)
        );
    }
}

#[test]
fn at_start_not_running_is_launched() {
    assert_eq!(
        decide_start(APP, Some(OTHER), running(false)),
        Ok(StartDecision::Launch(APP.launch.unwrap()))
    );
    assert_eq!(
        decide_start(NO_LAUNCH, Some(OTHER), running(false)),
        Ok(StartDecision::CantLaunch)
    );
}

#[test]
fn at_start_failed_running_check_is_an_error() {
    let result = decide_start(APP, Some(OTHER), || Err("gone".to_owned()));
    assert_eq!(result, Err("gone".to_owned()));
}

#[test]
fn home_outcomes() {
    let cases = [
        (Ok(Some(OTHER)), "then went home"),
        (
            Ok(Some(APP.gamescope)),
            "but it's still in front after going home",
        ),
        (Ok(None), "but left Gaming Mode"),
        (
            Err("display :0: gone".to_owned()),
            "then went home, but can't check it worked (display :0: gone)",
        ),
    ];
    for (front, outcome) in cases {
        assert_eq!(home_outcome(APP.gamescope, front), outcome);
    }
}

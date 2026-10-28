use super::control::control_request;
use super::*;
use crate::fixtures::{
    GO_LIBRESPOT_LOG, GROUP_SWITCH_LOG, META_GO_LIBRESPOT_LOG, META_LOG, META_SWITCH_LOG,
    SPARSE_LOG, meta_status, replay, status, summary,
};
use crate::state::{State, Track};

fn stream_id<'a>(server: &'a Server, client: &str) -> Option<&'a str> {
    server.stream_for(client).map(|s| s.id.as_str())
}

#[test]
fn client_matching() {
    let mut server = meta_status();
    assert_eq!(stream_id(&server, "HTPC"), Some("Spotify-HTPC"));
    assert_eq!(stream_id(&server, "htpc"), Some("Spotify-HTPC"));
    assert_eq!(stream_id(&server, "Kitchen"), None);
    // A connected client in a group whose stream doesn't exist.
    assert_eq!(server.groups[0].clients[0].host.name, "phone");
    server.groups[0].clients[0].connected = true;
    assert_eq!(stream_id(&server, "phone"), None);
}

#[test]
fn host_name_matches_when_id_does_not() {
    let mut server = meta_status();
    server.groups[1].clients[0].id = "living-room".into();
    assert_eq!(stream_id(&server, "htpc"), Some("Spotify-HTPC"));
}

#[test]
fn disconnected_client_has_no_stream() {
    let mut server = meta_status();
    server.groups[1].clients[0].connected = false;
    assert_eq!(stream_id(&server, "HTPC"), None);
}

#[test]
fn follows_group_stream() {
    let mut server = meta_status();
    let mut other = server.streams[0].clone();
    other.id = "Meta-HTPC".into();
    server.streams.push(other);
    server.groups[1].stream_id = "Meta-HTPC".into();
    assert_eq!(stream_id(&server, "HTPC"), Some("Meta-HTPC"));
}

#[test]
fn go_librespot_captures_follow_meta_stream() {
    for log in [GO_LIBRESPOT_LOG, META_GO_LIBRESPOT_LOG] {
        let server = status(log.lines().next().unwrap().split_once('\t').unwrap().1);
        assert_eq!(stream_id(&server, "HTPC"), Some("Meta-HTPC"));
    }
}

type Summary<'a> = Option<(&'a str, bool, Option<f64>)>;

fn playing(title: &str, position: Option<f64>) -> Summary<'_> {
    Some((title, true, position))
}

fn paused(title: &str, position: Option<f64>) -> Summary<'_> {
    Some((title, false, position))
}

#[test]
fn group_switch_and_reconnect() {
    use Action::*;
    let replayed = replay(GROUP_SWITCH_LOG);
    let got: Vec<_> = replayed.iter().map(|(a, s)| (*a, summary(s))).collect();
    assert_eq!(
        got,
        [
            (Resolved, playing("Track A", Some(10.0))),
            // another stream: not ours
            (Ignored, playing("Track A", Some(10.0))),
            // our group moves to the whole-house stream
            (Refetch, playing("Track A", Some(10.0))),
            (Resolved, playing("Track W", Some(60.0))),
            // the old stream is no longer ours
            (Ignored, playing("Track W", Some(60.0))),
            // snapclient disconnects, then comes back
            (Refetch, playing("Track W", Some(60.0))),
            (Resolved, None),
            (Refetch, None),
            (Resolved, playing("Track W", Some(80.0))),
        ]
    );
}

#[test]
fn meta_stream_switching_source() {
    use Action::*;
    let replayed = replay(META_SWITCH_LOG);
    let got: Vec<_> = replayed.iter().map(|(a, s)| (*a, summary(s))).collect();
    assert_eq!(
        got,
        [
            (Resolved, playing("Track A", Some(30.0))),
            // Spotify pauses; the meta stream forwards it
            (Ignored, playing("Track A", Some(30.0))),
            (Changed, paused("Track A", Some(40.0))),
            (Ignored, paused("Track A", Some(40.0))),
            // AirPlay starts and the meta stream switches to it
            (Ignored, paused("Track A", Some(40.0))),
            (Ignored, paused("Track A", Some(40.0))),
            (Changed, playing("Track H", None)),
            (Changed, playing("Track H", None)),
            // AirPlay stops
            (Ignored, playing("Track H", None)),
            (Changed, None),
            (Changed, None),
        ]
    );
}

#[test]
fn sparse_source() {
    let states = replay(SPARSE_LOG);
    let State::Playing(track) = &states[0].1 else {
        panic!("not playing");
    };
    assert_eq!(
        *track,
        Track {
            title: "Track S".into(),
            artists: vec![],
            album: None,
            art_url: None,
            position: None,
            duration: None,
        }
    );
    let summaries: Vec<_> = states.iter().map(|(_, state)| summary(state)).collect();
    assert_eq!(
        summaries,
        [
            Some(("Track S", true, None)),
            // no title
            None,
            Some(("Track T", true, None)),
            // idle, and no properties at all
            None,
        ]
    );
}

#[test]
fn notifications_before_first_status_are_ignored() {
    let mut session = Session::new("HTPC");
    let line = META_LOG.lines().nth(1).unwrap().split_once('\t').unwrap().1;
    assert_eq!(session.handle(line), Ok(Action::Ignored));
    assert!(session.stream().is_none());
}

#[test]
fn server_on_update_resolves() {
    let mut session = Session::new("HTPC");
    let status = META_LOG.lines().next().unwrap().split_once('\t').unwrap().1;
    let msg: serde_json::Value = serde_json::from_str(status).unwrap();
    let update = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "Server.OnUpdate",
        "params": {"server": msg["result"]["server"]},
    });
    assert_eq!(session.handle(&update.to_string()), Ok(Action::Resolved));
    assert_eq!(
        session.stream().map(|s| s.id.as_str()),
        Some("Spotify-HTPC")
    );
}

#[test]
fn bad_lines_change_nothing() {
    let mut session = Session::new("HTPC");
    let status = META_LOG.lines().next().unwrap().split_once('\t').unwrap().1;
    session.handle(status).unwrap();

    assert!(session.handle("not json").is_err());
    let error = r#"{"id":2,"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found"}}"#;
    assert!(session.handle(error).is_err());
    let bad_properties = r#"{"jsonrpc":"2.0","method":"Stream.OnProperties","params":{"id":"Spotify-HTPC","properties":{"position":"soon"}}}"#;
    assert!(session.handle(bad_properties).is_err());
    let other = r#"{"jsonrpc":"2.0","method":"Client.OnVolumeChanged","params":{"id":"HTPC","volume":{"muted":false,"percent":50}}}"#;
    assert_eq!(session.handle(other), Ok(Action::Ignored));

    let title = session
        .stream()
        .unwrap()
        .properties
        .metadata
        .title
        .as_deref();
    assert_eq!(title, Some("Track A"));
}

/// A log's lines, without their times.
fn lines(log: &str) -> impl Iterator<Item = &str> {
    log.lines().map(|line| line.split_once('\t').unwrap().1)
}

/// The log, and its last line, the captured `Stream not found` error.
fn split_error(log: &str) -> (&str, &str) {
    let (log, error) = log.trim_end().rsplit_once('\n').unwrap();
    (log, error.split_once('\t').unwrap().1)
}

fn can_everything(stream: &Stream) -> bool {
    let p = &stream.properties;
    p.can_control && p.can_play && p.can_pause && p.can_go_next && p.can_go_previous
}

#[test]
fn go_librespot_commands() {
    let d = Some(0.0020000000949949026);
    let expected = [
        (
            GO_LIBRESPOT_LOG,
            vec![
                playing("Track A", Some(0.0)),
                // play/pause, and the resume's Stream.OnUpdate still says paused
                paused("Track A", Some(0.0)),
                playing("Track A", Some(0.0)),
                // next
                playing("Track B", Some(0.0)),
            ],
        ),
        (
            META_GO_LIBRESPOT_LOG,
            vec![
                playing("Track C", Some(0.0)),
                None,
                playing("Track D", d),
                paused("Track D", d),
                paused("Track E", Some(0.0)),
                playing("Track E", Some(0.0)),
            ],
        ),
    ];
    for (log, states) in expected {
        let (log, _) = split_error(log);
        let replayed = replay(log);
        let mut got: Vec<_> = replayed.iter().map(|(_, s)| summary(s)).collect();
        got.dedup();
        assert_eq!(got, states);

        for (line, (action, _)) in lines(log).zip(&replayed).skip(1) {
            if line.contains(r#""result":"ok""#) {
                assert_eq!(*action, Action::Ignored);
            }
        }

        let mut session = Session::new("HTPC");
        for line in lines(log) {
            session.handle(line).unwrap();
            assert!(can_everything(session.stream().unwrap()));
        }
    }
}

#[test]
fn control_replies_change_nothing() {
    let (log, _) = split_error(GO_LIBRESPOT_LOG);
    let status = lines(log).next().unwrap();
    let ok = r#"{"id":2,"jsonrpc":"2.0","result":"ok"}"#;

    // Before the first status, and between status replies.
    let mut session = Session::new("HTPC");
    assert_eq!(session.handle(ok), Ok(Action::Ignored));
    assert!(session.stream().is_none());
    assert_eq!(session.handle(status), Ok(Action::Resolved));
    assert_eq!(session.handle(ok), Ok(Action::Ignored));
    assert_eq!(session.stream().unwrap().id, "Meta-HTPC");
    assert_eq!(session.handle(status), Ok(Action::Resolved));
    assert_eq!(session.stream().unwrap().id, "Meta-HTPC");
}

#[test]
fn control_error_keeps_stream() {
    for log in [GO_LIBRESPOT_LOG, META_GO_LIBRESPOT_LOG] {
        let (log, error) = split_error(log);
        let mut session = Session::new("HTPC");
        for line in lines(log) {
            session.handle(line).unwrap();
        }
        let e = session.handle(error).unwrap_err();
        assert!(e.contains("request 6"), "{e}");
        assert!(e.contains("Stream not found"), "{e}");
        assert_eq!(session.stream().unwrap().id, "Meta-HTPC");
    }
}

#[test]
fn control_request_lines() {
    assert_eq!(
        control_request(7, "Meta-HTPC", Command::PlayPause),
        r#"{"id":7,"jsonrpc":"2.0","method":"Stream.Control","params":{"command":"playPause","id":"Meta-HTPC"}}"#
    );
    assert_eq!(
        control_request(8, "Meta-HTPC", Command::Next),
        r#"{"id":8,"jsonrpc":"2.0","method":"Stream.Control","params":{"command":"next","id":"Meta-HTPC"}}"#
    );
    assert_eq!(
        control_request(9, "Meta-HTPC", Command::Previous),
        r#"{"id":9,"jsonrpc":"2.0","method":"Stream.Control","params":{"command":"previous","id":"Meta-HTPC"}}"#
    );
    // A stream id is escaped, not pasted in.
    assert_eq!(
        control_request(1, r#"a"b\c"#, Command::Next),
        r#"{"id":1,"jsonrpc":"2.0","method":"Stream.Control","params":{"command":"next","id":"a\"b\\c"}}"#
    );
}

#[test]
fn sources_without_control_cannot() {
    // META_LOG says false; the hand-written logs don't say.
    for log in [META_LOG, GROUP_SWITCH_LOG, SPARSE_LOG] {
        let (_, state) = &replay(log)[0];
        assert_ne!(*state, State::Idle);
        let mut session = Session::new("HTPC");
        session.handle(lines(log).next().unwrap()).unwrap();
        let p = &session.stream().unwrap().properties;
        assert!(
            !(p.can_control || p.can_play || p.can_pause || p.can_go_next || p.can_go_previous)
        );
    }
}

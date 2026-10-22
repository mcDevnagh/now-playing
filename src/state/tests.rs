use super::*;
use crate::snapcast::{Server, Stream};
use serde_json::Value;

const META_LOG: &str = include_str!("../../tests/fixtures/meta-librespot-events.log");
const BUILTIN_LOG: &str = include_str!("../../tests/fixtures/builtin-events.log");
const BUILTIN_STATUS: &str = include_str!("../../tests/fixtures/builtin-status.json");

/// Replays a captured log (`time<TAB>json` per line) and returns the
/// state after each line. Notifications are applied the simple way the
/// snapcast module will: `OnUpdate` replaces the stream, `OnProperties`
/// replaces its properties.
fn replay(log: &str) -> Vec<State> {
    let mut server = Server::default();
    log.lines()
        .map(|line| {
            let (_, json) = line.split_once('\t').unwrap();
            let msg: Value = serde_json::from_str(json).unwrap();
            apply(&mut server, &msg);
            now_playing(&server, "HTPC")
        })
        .collect()
}

fn apply(server: &mut Server, msg: &Value) {
    if let Some(result) = msg.get("result") {
        *server = serde_json::from_value(result["server"].clone()).unwrap();
        return;
    }
    let params = &msg["params"];
    match msg["method"].as_str().unwrap() {
        "Stream.OnUpdate" => {
            let new: Stream = serde_json::from_value(params["stream"].clone()).unwrap();
            let old = server.streams.iter_mut().find(|s| s.id == new.id).unwrap();
            *old = new;
        }
        "Stream.OnProperties" => {
            let id = params["id"].as_str().unwrap();
            let stream = server.streams.iter_mut().find(|s| s.id == id).unwrap();
            stream.properties = serde_json::from_value(params["properties"].clone()).unwrap();
        }
        other => panic!("unexpected {other}"),
    }
}

fn server(json: &str) -> Server {
    let msg: Value = serde_json::from_str(json).unwrap();
    serde_json::from_value(msg["result"]["server"].clone()).unwrap()
}

fn meta_status() -> Server {
    server(META_LOG.lines().next().unwrap().split_once('\t').unwrap().1)
}

/// Title, playing or paused, and position, for compact assertions.
fn summary(state: &State) -> Option<(&str, bool, Option<f64>)> {
    match state {
        State::Idle => None,
        State::Playing(t) => Some((&t.title, true, t.position)),
        State::Paused(t) => Some((&t.title, false, t.position)),
    }
}

#[test]
fn full_metadata() {
    let State::Playing(track) = now_playing(&meta_status(), "HTPC") else {
        panic!("not playing");
    };
    assert_eq!(
        track,
        Track {
            title: "Track A".into(),
            artists: vec!["Artist A".into()],
            album: Some("Album A".into()),
            art_url: Some("https://art.example/a.jpg".into()),
            position: Some(0.0),
            duration: Some(158.42599487304688),
        }
    );
}

#[test]
fn meta_librespot_session() {
    let states = replay(META_LOG);
    let summaries: Vec<_> = states.iter().map(summary).collect();
    assert_eq!(
        summaries,
        [
            Some(("Track A", true, Some(0.0))),
            // track change
            Some(("Track B", true, Some(0.0))),
            // pause, then the stream goes idle: still paused
            Some(("Track B", false, Some(15.586999893188477))),
            Some(("Track B", false, Some(15.586999893188477))),
            // resume: OnUpdate still carries the stale `paused`
            Some(("Track B", false, Some(15.586999893188477))),
            Some(("Track B", true, Some(15.586999893188477))),
            // seeks
            Some(("Track B", true, Some(61.582000732421875))),
            Some(("Track B", true, Some(166.7740020751953))),
            // skip, previous
            Some(("Track C", true, Some(0.0))),
            Some(("Track B", true, Some(0.0))),
            // stop
            None,
            None,
        ]
    );
}

#[test]
fn sparse_source_follows_stream_status() {
    // The built-in librespot source: no playbackStatus, no position.
    let states = replay(BUILTIN_LOG);
    let summaries: Vec<_> = states.iter().map(summary).collect();
    assert_eq!(
        summaries,
        [
            Some(("Track D", true, None)),
            // pause, resume: only the stream status changes
            None,
            Some(("Track D", true, None)),
            Some(("Track E", true, None)),
            Some(("Track F", true, None)),
            Some(("Track E", true, None)),
            None,
        ]
    );
    let State::Playing(track) = &states[2] else {
        unreachable!()
    };
    assert!(track.artists.is_empty());
    assert_eq!(track.album, None);
    assert_eq!((track.position, track.duration), (None, None));
}

#[test]
fn idle_stream_without_playback_status() {
    assert_eq!(now_playing(&server(BUILTIN_STATUS), "HTPC"), State::Idle);
}

#[test]
fn client_matching() {
    let mut server = meta_status();
    assert!(matches!(now_playing(&server, "htpc"), State::Playing(_)));
    assert_eq!(now_playing(&server, "Kitchen"), State::Idle);
    // A connected client in a group whose stream doesn't exist.
    assert_eq!(server.groups[0].clients[0].host.name, "phone");
    server.groups[0].clients[0].connected = true;
    assert_eq!(now_playing(&server, "phone"), State::Idle);
}

#[test]
fn host_name_matches_when_id_does_not() {
    let mut server = meta_status();
    server.groups[1].clients[0].id = "living-room".into();
    assert!(matches!(now_playing(&server, "htpc"), State::Playing(_)));
}

#[test]
fn disconnected_client_is_idle() {
    let mut server = meta_status();
    server.groups[1].clients[0].connected = false;
    assert_eq!(now_playing(&server, "HTPC"), State::Idle);
}

#[test]
fn follows_group_stream() {
    let mut server = meta_status();
    let mut other = server.streams[0].clone();
    other.id = "Meta-HTPC".into();
    other.properties.metadata.title = Some("Something Else".into());
    server.streams.push(other);
    server.groups[1].stream_id = "Meta-HTPC".into();
    assert_eq!(
        summary(&now_playing(&server, "HTPC")),
        Some(("Something Else", true, Some(0.0)))
    );
}

#[test]
fn missing_or_empty_fields() {
    let mut server = meta_status();
    let meta = &mut server.streams[0].properties.metadata;
    meta.album = Some(String::new());
    meta.art_url = None;
    meta.duration = None;
    let State::Playing(track) = now_playing(&server, "HTPC") else {
        panic!()
    };
    assert_eq!((track.album, track.art_url), (None, None));
    assert_eq!((track.position, track.duration), (None, None));

    server.streams[0].properties.metadata.title = Some(String::new());
    assert_eq!(now_playing(&server, "HTPC"), State::Idle);
}

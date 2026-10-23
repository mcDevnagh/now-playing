use super::*;
use crate::fixtures::{
    BUILTIN_LOG, BUILTIN_STATUS, META_LOG, meta_status, replay, status, summary,
};
use crate::snapcast::Stream;

fn states(log: &str) -> Vec<State> {
    replay(log).into_iter().map(|(_, state)| state).collect()
}

/// The HTPC's stream at the start of `META_LOG`.
fn meta_stream() -> Stream {
    meta_status().streams.remove(0)
}

#[test]
fn full_metadata() {
    let State::Playing(track) = now_playing(&meta_stream()) else {
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
    let states = states(META_LOG);
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
    let states = states(BUILTIN_LOG);
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
    let stream = &status(BUILTIN_STATUS).streams[0];
    assert_eq!(now_playing(stream), State::Idle);
}

#[test]
fn missing_or_empty_fields() {
    let mut stream = meta_stream();
    let meta = &mut stream.properties.metadata;
    meta.album = Some(String::new());
    meta.art_url = None;
    meta.duration = None;
    let State::Playing(track) = now_playing(&stream) else {
        panic!()
    };
    assert_eq!((track.album, track.art_url), (None, None));
    assert_eq!((track.position, track.duration), (None, None));

    stream.properties.metadata.title = Some(String::new());
    assert_eq!(now_playing(&stream), State::Idle);
}

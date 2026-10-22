use super::{State, Track};
use crate::snapcast::Stream;

/// What `stream` is playing. A stream with no title shows as `Idle`.
pub fn now_playing(stream: &Stream) -> State {
    let props = &stream.properties;

    let playing = match props.playback_status.as_deref() {
        Some("playing") => true,
        Some("paused") => false,
        Some("stopped") => return State::Idle,
        _ if stream.status == "playing" => true,
        _ => return State::Idle,
    };

    let meta = &props.metadata;
    let Some(title) = non_empty(&meta.title) else {
        return State::Idle;
    };
    let (position, duration) = match (props.position, meta.duration) {
        (Some(position), Some(duration)) if duration > 0.0 => (Some(position), Some(duration)),
        _ => (None, None),
    };
    let track = Track {
        title,
        artists: meta
            .artist
            .iter()
            .filter(|a| !a.is_empty())
            .cloned()
            .collect(),
        album: non_empty(&meta.album),
        art_url: non_empty(&meta.art_url),
        position,
        duration,
    };

    if playing {
        State::Playing(track)
    } else {
        State::Paused(track)
    }
}

fn non_empty(s: &Option<String>) -> Option<String> {
    s.as_deref().filter(|s| !s.is_empty()).map(str::to_owned)
}

use super::{State, Track};
use crate::snapcast::{Client, Server, Stream};

/// What `client` is playing, according to snapserver's status.
///
/// `client` is matched against client ids, first exactly, then ignoring
/// case, then against host names ignoring case. A client that is missing or
/// disconnected, or a stream with no title, shows as `Idle`.
pub fn now_playing(server: &Server, client: &str) -> State {
    let Some(stream) = find_stream(server, client) else {
        return State::Idle;
    };
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

fn find_stream<'a>(server: &'a Server, client: &str) -> Option<&'a Stream> {
    let clients = || {
        server
            .groups
            .iter()
            .flat_map(|g| g.clients.iter().map(move |c| (g, c)))
    };
    let matches: [&dyn Fn(&Client) -> bool; 3] = [
        &|c| c.id == client,
        &|c| c.id.eq_ignore_ascii_case(client),
        &|c| c.host.name.eq_ignore_ascii_case(client),
    ];
    let (group, found) = matches.iter().find_map(|m| clients().find(|(_, c)| m(c)))?;
    if !found.connected {
        return None;
    }
    server.streams.iter().find(|s| s.id == group.stream_id)
}

fn non_empty(s: &Option<String>) -> Option<String> {
    s.as_deref().filter(|s| !s.is_empty()).map(str::to_owned)
}

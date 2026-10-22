//! Prints state changes as text instead of drawing them.

use crate::snapcast::Stream;
use crate::state::{State, now_playing};

/// A callback for `snapcast::run` or `replay` that prints a line to stdout
/// whenever the state changes.
pub fn printer() -> impl FnMut(Option<&Stream>) {
    let mut shown = None;
    move |stream| {
        let state = stream.map_or(State::Idle, now_playing);
        if shown.as_ref() != Some(&state) {
            println!("{}", describe(&state));
            shown = Some(state);
        }
    }
}

/// One line, e.g. `playing: Title | Artist | Album | 1:01/4:08 | art URL`.
fn describe(state: &State) -> String {
    let (status, track) = match state {
        State::Idle => return "idle".to_owned(),
        State::Playing(track) => ("playing", track),
        State::Paused(track) => ("paused", track),
    };
    let mut line = format!("{status}: {}", track.title);
    if !track.artists.is_empty() {
        line += &format!(" | {}", track.artists.join(", "));
    }
    if let Some(album) = &track.album {
        line += &format!(" | {album}");
    }
    if let (Some(position), Some(duration)) = (track.position, track.duration) {
        line += &format!(" | {}/{}", clock(position), clock(duration));
    }
    if let Some(art) = &track.art_url {
        line += &format!(" | {art}");
    }
    line
}

fn clock(seconds: f64) -> String {
    let seconds = seconds as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

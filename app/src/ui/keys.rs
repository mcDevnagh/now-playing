//! What the remote's keys do.

use sdl3::keyboard::Keycode;

use crate::snapcast::Command;

/// The playback command a key sends, if any: only the three media keys.
pub(super) fn command_for(keycode: Option<Keycode>) -> Option<Command> {
    match keycode? {
        Keycode::MediaPlayPause => Some(Command::PlayPause),
        Keycode::MediaNextTrack => Some(Command::Next),
        Keycode::MediaPreviousTrack => Some(Command::Previous),
        _ => None,
    }
}

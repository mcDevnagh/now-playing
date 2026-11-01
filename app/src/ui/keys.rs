//! What the remote's keys do.

use sdl3::keyboard::Keycode;

use crate::snapcast::Command;

/// What a key press does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum KeyAction {
    /// Send a playback command to the stream.
    Send(Command),
    /// Quit, ending the screensaver.
    Quit,
}

/// What a key does, if anything: the three media keys send their command;
/// as a screensaver, any other key quits, even one SDL has no keycode for.
pub(super) fn action_for(keycode: Option<Keycode>, screensaver: bool) -> Option<KeyAction> {
    match command_for(keycode) {
        Some(command) => Some(KeyAction::Send(command)),
        None => screensaver.then_some(KeyAction::Quit),
    }
}

/// The playback command a key sends, if any: only the three media keys.
fn command_for(keycode: Option<Keycode>) -> Option<Command> {
    match keycode? {
        Keycode::MediaPlayPause => Some(Command::PlayPause),
        Keycode::MediaNextTrack => Some(Command::Next),
        Keycode::MediaPreviousTrack => Some(Command::Previous),
        _ => None,
    }
}

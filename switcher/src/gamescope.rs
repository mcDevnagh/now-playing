//! gamescope's X11 interface: properties on its Xwayland root window.

use std::fmt::Display;

use x11rb::connection::Connection;
use x11rb::errors::ReplyError;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt as _, GetPropertyReply, Window};
use x11rb::rust_connection::RustConnection;

const DEFAULT_DISPLAY: &str = ":0";

/// The X display gamescope's properties are on: `$DISPLAY`, else `:0`.
pub fn display() -> String {
    std::env::var("DISPLAY")
        .ok()
        .filter(|display| !display.is_empty())
        .unwrap_or_else(|| DEFAULT_DISPLAY.to_owned())
}

/// A connection to the display. Open a new one for each trigger, so a
/// gamescope restart needs no handling.
pub struct Gamescope {
    conn: RustConnection,
    root: Window,
}

impl Gamescope {
    pub fn connect(display: &str) -> Result<Self, String> {
        let (conn, screen) = RustConnection::connect(Some(display)).map_err(err(display))?;
        let root = conn.setup().roots[screen].root;
        Ok(Self { conn, root })
    }

    /// The gamescope ID of the app in front: `GAMESCOPE_FOCUSED_APP`. `None`
    /// without the property, as in Desktop Mode.
    pub fn focused_app(&self) -> Result<Option<u32>, String> {
        let Some(atom) = self.atom("GAMESCOPE_FOCUSED_APP")? else {
            return Ok(None);
        };
        let reply = self
            .conn
            .get_property(false, self.root, atom, AtomEnum::CARDINAL, 0, 1)
            .map_err(err("GAMESCOPE_FOCUSED_APP"))?
            .reply()
            .map_err(err("GAMESCOPE_FOCUSED_APP"))?;
        Ok(cardinal(&reply))
    }

    /// Whether any window belongs to `app`: its `STEAM_GAME`, set on each
    /// window Steam launched, is `app`. Looks at the root's children and,
    /// for frames, their children.
    pub fn is_running(&self, app: u32) -> Result<bool, String> {
        let Some(atom) = self.atom("STEAM_GAME")? else {
            return Ok(false);
        };
        for window in self.children(self.root)? {
            if self.steam_game(window, atom)? == Some(app) {
                return Ok(true);
            }
            for child in self.children(window)? {
                if self.steam_game(child, atom)? == Some(app) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// `window`'s children; none if it has gone.
    fn children(&self, window: Window) -> Result<Vec<Window>, String> {
        let reply = self
            .conn
            .query_tree(window)
            .map_err(err("query_tree"))?
            .reply();
        Ok(gone_is_none(reply)?.map_or_else(Vec::new, |tree| tree.children))
    }

    /// `window`'s `STEAM_GAME`; `None` without it, or if the window has gone.
    fn steam_game(&self, window: Window, atom: Atom) -> Result<Option<u32>, String> {
        let reply = self
            .conn
            .get_property(false, window, atom, AtomEnum::CARDINAL, 0, 1)
            .map_err(err("STEAM_GAME"))?
            .reply();
        Ok(gone_is_none(reply)?.and_then(|reply| cardinal(&reply)))
    }

    /// The atom called `name`; `None` if no client has created it, in which
    /// case no window has a property by that name.
    fn atom(&self, name: &str) -> Result<Option<Atom>, String> {
        let atom = self
            .conn
            .intern_atom(true, name.as_bytes())
            .map_err(err(name))?
            .reply()
            .map_err(err(name))?
            .atom;
        Ok((atom != u32::from(AtomEnum::NONE)).then_some(atom))
    }
}

/// The first value of a 32-bit CARDINAL property; `None` if it's missing.
fn cardinal(reply: &GetPropertyReply) -> Option<u32> {
    reply.value32()?.next()
}

/// A reply, or `None` if the server refused the request (as for a window
/// that closed mid-scan). A broken connection is still an error.
fn gone_is_none<T>(reply: Result<T, ReplyError>) -> Result<Option<T>, String> {
    match reply {
        Ok(reply) => Ok(Some(reply)),
        Err(ReplyError::X11Error(_)) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Prefixes an X error with what it was about.
fn err<E: Display>(what: &str) -> impl Fn(E) -> String {
    move |e| format!("{what}: {e}")
}

#[cfg(test)]
mod tests;

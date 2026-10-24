//! gamescope's X11 interface: properties on its Xwayland root window.

use std::fmt::Display;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, GetPropertyReply, PropMode, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

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
        let (conn, screen) =
            RustConnection::connect(Some(display)).map_err(err(&format!("display {display}")))?;
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

    /// Asks gamescope to bring `app` to the front, by writing its ID alone
    /// to `GAMESCOPECTRL_BASELAYER_APPID`. gamescope acts on it a moment
    /// later, and ignores it if `app` isn't running. Never write more IDs
    /// after it: if `app` isn't running, gamescope then shows nothing.
    pub fn focus(&self, app: u32) -> Result<(), String> {
        const NAME: &str = "GAMESCOPECTRL_BASELAYER_APPID";
        let atom = self
            .conn
            .intern_atom(false, NAME.as_bytes())
            .map_err(err(NAME))?
            .reply()
            .map_err(err(NAME))?
            .atom;
        self.conn
            .change_property32(
                PropMode::REPLACE,
                self.root,
                atom,
                AtomEnum::CARDINAL,
                &[app],
            )
            .map_err(err(NAME))?
            .check()
            .map_err(err(NAME))
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

/// Prefixes an X error with what it was about.
fn err<E: Display>(what: &str) -> impl Fn(E) -> String {
    move |e| format!("{what}: {e}")
}

#[cfg(test)]
mod tests;

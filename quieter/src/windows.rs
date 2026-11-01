//! Following an X display's top-level windows: whether any window of a
//! given class is mapped, as UxPlay's video window is while a device
//! mirrors to it.

use std::collections::HashSet;
use std::fmt::Display;

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, EventMask, MapState, Window,
};
use x11rb::rust_connection::RustConnection;

/// Follows whether a mapped top-level window whose `WM_CLASS` class is
/// `class` is on `display`. Calls `on_change` with the starting state, then
/// with each change. Runs until the connection fails, as when the display's
/// server exits, and returns why.
pub fn follow(display: &str, class: &str, mut on_change: impl FnMut(bool)) -> String {
    let (conn, screen) = match RustConnection::connect(Some(display)) {
        Ok(connected) => connected,
        Err(e) => return format!("display {display}: {e}"),
    };
    let root = conn.setup().roots[screen].root;
    let class = class.as_bytes();

    // Events first, then the tree, so a change between the two isn't lost.
    let events = ChangeWindowAttributesAux::new().event_mask(EventMask::SUBSTRUCTURE_NOTIFY);
    if let Err(e) = conn
        .change_window_attributes(root, &events)
        .map_err(err("select events"))
        .and_then(|cookie| cookie.check().map_err(err("select events")))
    {
        return e;
    }
    let children = match conn
        .query_tree(root)
        .map_err(err("query tree"))
        .and_then(|cookie| cookie.reply().map_err(err("query tree")))
    {
        Ok(tree) => tree.children,
        Err(e) => return e,
    };
    let mut matching = Matching::default();
    for window in children {
        if is_mapped(&conn, window) && class_of(&conn, window).as_deref() == Some(class) {
            matching.map(window);
        }
    }
    on_change(matching.any());

    loop {
        let change = match conn.wait_for_event() {
            Ok(Event::MapNotify(e)) if class_of(&conn, e.window).as_deref() == Some(class) => {
                matching.map(e.window)
            }
            Ok(Event::UnmapNotify(e)) => matching.unmap(e.window),
            Ok(Event::DestroyNotify(e)) => matching.unmap(e.window),
            Ok(_) => None,
            Err(e) => return e.to_string(),
        };
        if let Some(any) = change {
            on_change(any);
        }
    }
}

/// The mapped windows of the followed class.
#[derive(Debug, Default)]
struct Matching(HashSet<Window>);

impl Matching {
    fn any(&self) -> bool {
        !self.0.is_empty()
    }

    /// A window of the class was mapped. Returns the new state if it changed.
    fn map(&mut self, window: Window) -> Option<bool> {
        let was = self.any();
        self.0.insert(window);
        (self.any() != was).then_some(self.any())
    }

    /// A window was unmapped or destroyed, of the class or not. Returns the
    /// new state if it changed.
    fn unmap(&mut self, window: Window) -> Option<bool> {
        let was = self.any();
        self.0.remove(&window);
        (self.any() != was).then_some(self.any())
    }
}

/// Whether `window` is mapped. A window that's gone isn't.
fn is_mapped(conn: &RustConnection, window: Window) -> bool {
    conn.get_window_attributes(window)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .is_some_and(|attributes| attributes.map_state != MapState::UNMAPPED)
}

/// The class half of `window`'s `WM_CLASS`; `None` without one, or if the
/// window is gone.
fn class_of(conn: &RustConnection, window: Window) -> Option<Vec<u8>> {
    let reply = conn
        .get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256)
        .ok()?
        .reply()
        .ok()?;
    class(&reply.value).map(<[u8]>::to_vec)
}

/// The class in a `WM_CLASS` value: two NUL-terminated strings, the
/// instance then the class. Some clients leave off the last NUL.
fn class(wm_class: &[u8]) -> Option<&[u8]> {
    let mut parts = wm_class.split(|&b| b == 0);
    parts.next()?;
    parts.next().filter(|class| !class.is_empty())
}

/// Prefixes an X error with what it was about.
fn err<E: Display>(what: &str) -> impl Fn(E) -> String {
    move |e| format!("{what}: {e}")
}

#[cfg(test)]
mod tests;

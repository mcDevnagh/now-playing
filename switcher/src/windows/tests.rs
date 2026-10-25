use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use x11rb::protocol::xproto::{CreateWindowAux, PropMode, WindowClass};
use x11rb::wrapper::ConnectionExt as _;

use super::*;

#[test]
fn reads_the_class() {
    // As seen on the HTPC for UxPlay's video window.
    assert_eq!(class(b"HTPC\0GStreamer\0"), Some(&b"GStreamer"[..]));
    assert_eq!(class(b"HTPC\0GStreamer"), Some(&b"GStreamer"[..]));
}

#[test]
fn no_class() {
    assert_eq!(class(b""), None);
    assert_eq!(class(b"HTPC\0"), None);
    assert_eq!(class(b"HTPC"), None);
}

#[test]
fn reports_only_changes() {
    let mut matching = Matching::default();
    assert!(!matching.any());
    assert_eq!(matching.map(1), Some(true));
    // A second window, then it going: still one left.
    assert_eq!(matching.map(2), None);
    assert_eq!(matching.unmap(2), None);
    // A window that never matched.
    assert_eq!(matching.unmap(3), None);
    assert_eq!(matching.unmap(1), Some(false));
    // Unmapped, then destroyed: reported once.
    assert_eq!(matching.unmap(1), None);
}

#[test]
fn mapping_twice_counts_once() {
    let mut matching = Matching::default();
    assert_eq!(matching.map(1), Some(true));
    assert_eq!(matching.map(1), None);
    assert_eq!(matching.unmap(1), Some(false));
}

/// A top-level window on `conn` with `WM_CLASS` `instance`, `class`.
fn window(conn: &RustConnection, class: &str) -> Window {
    let root = conn.setup().roots[0].root;
    let window = conn.generate_id().unwrap();
    conn.create_window(
        0,
        window,
        root,
        0,
        0,
        100,
        100,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new(),
    )
    .unwrap();
    let wm_class = format!("test\0{class}\0");
    conn.change_property8(
        PropMode::REPLACE,
        window,
        AtomEnum::WM_CLASS,
        AtomEnum::STRING,
        wm_class.as_bytes(),
    )
    .unwrap();
    window
}

/// Against a real X server:
///
/// ```sh
/// Xvfb :99 &
/// X_DISPLAY=:99 cargo test -p now-playing-switcher -- --ignored
/// ```
#[test]
#[ignore = "needs an X server at $X_DISPLAY"]
fn real_x_server() {
    let display = std::env::var("X_DISPLAY").unwrap();
    let (conn, _) = RustConnection::connect(Some(&display)).unwrap();
    let sync = |conn: &RustConnection| {
        conn.sync().unwrap();
    };
    let next = |states: &mpsc::Receiver<bool>| states.recv_timeout(Duration::from_secs(5)).unwrap();

    // Mapped before following: the starting state.
    let before = window(&conn, "GStreamer");
    conn.map_window(before).unwrap();
    sync(&conn);
    let (tx, states) = mpsc::channel();
    let following = display.clone();
    thread::spawn(move || follow(&following, "GStreamer", |any| tx.send(any).unwrap()));
    assert!(next(&states));

    // Other classes don't count.
    let other = window(&conn, "now-playing-placeholder");
    conn.map_window(other).unwrap();
    // Destroyed while mapped.
    conn.destroy_window(before).unwrap();
    sync(&conn);
    assert!(!next(&states));

    // Mapped, then unmapped, while following.
    let during = window(&conn, "GStreamer");
    conn.map_window(during).unwrap();
    sync(&conn);
    assert!(next(&states));
    conn.unmap_window(during).unwrap();
    sync(&conn);
    assert!(!next(&states));
    assert!(states.recv_timeout(Duration::from_millis(200)).is_err());
}

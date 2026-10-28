use super::layout::{Style, layout, same_layout};
use super::text::{Face, Fonts};
use crate::state::Track;

const SCREEN: (f32, f32) = (1920.0, 1080.0);
const LONG_TITLE: &str =
    "The Unbearable Lightness of Being Stuck in Traffic on a Rainy Tuesday Afternoon in November";

fn track() -> Track {
    Track {
        title: "Banks Of The Ohio".into(),
        artists: vec!["Artist A".into(), "Artist B".into()],
        album: Some("Album A".into()),
        art_url: Some("https://art.example/a.jpg".into()),
        position: Some(66.0),
        duration: Some(224.0),
    }
}

fn title_face(fonts: &Fonts) -> Face<'_> {
    Style::Title.face(fonts, SCREEN.1)
}

#[test]
fn short_text_is_one_line() {
    let fonts = Fonts::load();
    assert_eq!(title_face(&fonts).wrap("Pink", 500.0, 3), ["Pink"]);
}

#[test]
fn empty_text_has_no_lines() {
    let fonts = Fonts::load();
    assert!(title_face(&fonts).wrap("  ", 500.0, 3).is_empty());
}

#[test]
fn long_text_wraps_at_words_and_ends_with_ellipsis() {
    let fonts = Fonts::load();
    let face = title_face(&fonts);
    let lines = face.wrap(LONG_TITLE, 900.0, 3);
    assert_eq!(lines.len(), 3);
    assert!(lines.iter().all(|line| face.width(line) <= 900.0));
    assert!(lines[2].ends_with('…'));
    // The first two lines break between words.
    for line in &lines[..2] {
        let next = &LONG_TITLE[LONG_TITLE.find(line.as_str()).unwrap() + line.len()..];
        assert!(next.starts_with(' '), "{line:?} breaks mid-word");
    }
}

#[test]
fn a_word_too_long_for_a_line_is_broken() {
    let fonts = Fonts::load();
    let face = title_face(&fonts);
    let lines = face.wrap("Supercalifragilisticexpialidocious", 300.0, 3);
    assert!(lines.len() > 1);
    assert!(lines.iter().all(|line| face.width(line) <= 300.0));
}

#[test]
fn ellipsize() {
    let fonts = Fonts::load();
    let face = Style::Album.face(&fonts, SCREEN.1);
    assert_eq!(face.ellipsize("Album A", 500.0), "Album A");
    let cut = face.ellipsize(LONG_TITLE, 500.0);
    assert!(cut.ends_with('…') && !cut.ends_with(" …"));
    assert!(face.width(&cut) <= 500.0);
}

#[test]
fn layout_with_art() {
    let fonts = Fonts::load();
    let (w, h) = SCREEN;
    let l = layout(&fonts, &track(), true, SCREEN);

    let cover = l.cover.expect("cover");
    assert!((cover.w - 0.62 * h).abs() < 0.01);
    assert_eq!(cover.w, cover.h);
    assert_eq!(cover.y, (h - cover.h) / 2.0);

    let styles: Vec<_> = l.lines.iter().map(|line| line.style).collect();
    assert_eq!(styles, [Style::Title, Style::Artist, Style::Album]);
    assert_eq!(l.lines[1].text, "Artist A, Artist B");
    let bar = l.bar.expect("bar");
    for line in &l.lines {
        assert_eq!(line.x, bar.x);
        assert!(line.x > cover.x + cover.w);
        assert!(line.x + line.width <= bar.x + bar.w);
    }
    assert!(bar.x + bar.w <= w);
    // The text block is vertically centred: as much room above as below.
    let above = l.lines[0].y;
    let below = h - (bar.y + bar.h);
    assert!(
        (above - below).abs() < 0.05 * h,
        "{above} above, {below} below"
    );
}

#[test]
fn layout_without_art_is_centred() {
    let fonts = Fonts::load();
    let l = layout(&fonts, &track(), false, SCREEN);
    assert_eq!(l.cover, None);
    for line in &l.lines {
        let centre = line.x + line.width / 2.0;
        assert!((centre - SCREEN.0 / 2.0).abs() < 1.0, "{line:?}");
    }
}

#[test]
fn layout_of_a_sparse_track() {
    let fonts = Fonts::load();
    let track = Track {
        title: LONG_TITLE.into(),
        artists: vec![],
        album: None,
        art_url: None,
        position: None,
        duration: None,
    };
    let l = layout(&fonts, &track, false, SCREEN);
    assert_eq!(l.bar, None);
    assert_eq!(l.lines.len(), 3);
    assert!(l.lines.iter().all(|line| line.style == Style::Title));
}

#[test]
fn only_text_changes_need_a_new_layout() {
    let a = track();
    let moved = Track {
        position: Some(100.0),
        ..track()
    };
    let new_cover = Track {
        art_url: Some("https://art.example/b.jpg".into()),
        ..track()
    };
    let renamed = Track {
        title: "Pink".into(),
        ..track()
    };
    assert!(same_layout(&a, &moved));
    assert!(same_layout(&a, &new_cover));
    assert!(!same_layout(&a, &renamed));
}

#[test]
fn media_keys_send_commands() {
    use sdl3::keyboard::Keycode;

    use super::keys::command_for;
    use crate::snapcast::Command;

    assert_eq!(
        command_for(Some(Keycode::MediaPlayPause)),
        Some(Command::PlayPause)
    );
    assert_eq!(
        command_for(Some(Keycode::MediaNextTrack)),
        Some(Command::Next)
    );
    assert_eq!(
        command_for(Some(Keycode::MediaPreviousTrack)),
        Some(Command::Previous)
    );
    for other in [
        Keycode::Escape,
        Keycode::Space,
        Keycode::AcBack,
        Keycode::MediaPlay,
    ] {
        assert_eq!(command_for(Some(other)), None, "{other}");
    }
    assert_eq!(command_for(None), None);
}

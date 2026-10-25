//! Where everything goes on screen, in pixels. Pure, so it can be tested
//! without a window.
//!
//! Proportions follow the old web page: sizes in `vh` / `vw` are hundredths
//! of the screen's height / width.

use super::text::{Face, Fonts};
use crate::state::Track;

const TITLE_LINES: usize = 3;
const TITLE_LINE_HEIGHT: f32 = 1.15;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Style {
    Title,
    Artist,
    Album,
}

impl Style {
    pub fn face<'a>(self, fonts: &'a Fonts, screen_height: f32) -> Face<'a> {
        let vh = screen_height / 100.0;
        match self {
            Style::Title => Face::new(&fonts.bold, 6.5 * vh),
            Style::Artist => Face::new(&fonts.regular, 4.0 * vh),
            Style::Album => Face::new(&fonts.regular, 3.0 * vh),
        }
    }

    /// Opacity relative to the title.
    pub fn opacity(self) -> f32 {
        match self {
            Style::Title => 1.0,
            Style::Artist => 0.85,
            Style::Album => 0.55,
        }
    }
}

/// One line of text; `x`, `y` are the top left of its rasterised bitmap.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub text: String,
    pub style: Style,
    pub x: f32,
    pub y: f32,
    pub width: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// Where the cover goes, if one is shown.
    pub cover: Option<Rect>,
    pub lines: Vec<Line>,
    /// The whole progress bar, if the track has a position and duration.
    pub bar: Option<Rect>,
}

/// The side of the cover on a screen `height` pixels high: 62vh.
pub fn cover_side(height: f32) -> f32 {
    62.0 * height / 100.0
}

/// With a cover: the cover at 62vh on the left, the text block to its
/// right. Without (the track has no art, or it failed to load): the text
/// block centred, each line centred. The text block is vertically centred
/// either way.
pub fn layout(
    fonts: &Fonts,
    track: &Track,
    with_cover: bool,
    (width, height): (f32, f32),
) -> Layout {
    let (vw, vh) = (width / 100.0, height / 100.0);
    let cover_side = cover_side(height);
    let gap = 5.0 * vw;
    let text_width = if with_cover {
        (48.0 * vw).min(width - 12.0 * vw - cover_side - gap)
    } else {
        72.0 * vw
    };

    // The text block, from y = 0; moved into place below.
    let mut lines = Vec::new();
    let mut y = 0.0;
    let mut add = |text: String, style: Style, box_height: f32, y: &mut f32| {
        let face = style.face(fonts, height);
        lines.push(Line {
            width: face.width(&text),
            text,
            style,
            x: 0.0,
            // Centre the glyphs in the line box, like CSS half-leading.
            y: *y + (box_height - face.height()) / 2.0,
        });
        *y += box_height;
    };

    let title = Style::Title.face(fonts, height);
    let title_box = TITLE_LINE_HEIGHT * 6.5 * vh;
    for text in title.wrap(&track.title, text_width, TITLE_LINES) {
        add(text, Style::Title, title_box, &mut y);
    }
    if !track.artists.is_empty() {
        y += 2.0 * vh;
        let face = Style::Artist.face(fonts, height);
        let text = face.ellipsize(&track.artists.join(", "), text_width);
        add(text, Style::Artist, face.line_height(), &mut y);
    }
    if let Some(album) = &track.album {
        y += if track.artists.is_empty() { 2.0 } else { 1.0 } * vh;
        let face = Style::Album.face(fonts, height);
        add(
            face.ellipsize(album, text_width),
            Style::Album,
            face.line_height(),
            &mut y,
        );
    }
    let mut bar = None;
    if track.position.is_some() && track.duration.is_some() {
        y += 5.0 * vh;
        bar = Some(Rect {
            x: 0.0,
            y,
            w: text_width,
            h: 0.6 * vh,
        });
        y += 0.6 * vh;
    }

    let top = (height - y) / 2.0;
    let (left, cover) = if with_cover {
        let left = (width - (cover_side + gap + text_width)) / 2.0;
        let cover = Rect {
            x: left,
            y: (height - cover_side) / 2.0,
            w: cover_side,
            h: cover_side,
        };
        (left + cover_side + gap, Some(cover))
    } else {
        ((width - text_width) / 2.0, None)
    };
    for line in &mut lines {
        let indent = if with_cover {
            0.0
        } else {
            (text_width - line.width) / 2.0
        };
        line.x = left + indent;
        line.y += top;
    }
    if let Some(bar) = &mut bar {
        bar.x = left;
        bar.y += top;
    }

    Layout { cover, lines, bar }
}

/// Whether `a` and `b` lay out the same (given the same cover or none), so
/// the text needn't be redrawn: only position and art may differ.
pub fn same_layout(a: &Track, b: &Track) -> bool {
    a.title == b.title
        && a.artists == b.artists
        && a.album == b.album
        && a.duration.is_some() == b.duration.is_some()
}

//! The track: its cover, title, artists, album and progress.
//!
//! The layout is pure, so it can be tested without a window. Sizes are in
//! hundredths of the height left inside the padding, so the whole block
//! scales with its bounds.

use std::rc::Rc;

use sdl3::pixels::Color;
use sdl3::rect::Rect as SdlRect;
use sdl3::render::{Texture, WindowCanvas};

use super::super::text::{Face, Fonts};
use super::super::texture::{self, Creator};
use super::{Bounds, alpha};
use crate::state::Track;

/// The space above and below the block, as a fraction of its bounds'
/// height.
const PADDING: f32 = 0.12;
const TITLE_LINES: usize = 1;
const TITLE_LINE_HEIGHT: f32 = 1.15;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Style {
    Title,
    Artist,
    Album,
}

impl Style {
    /// The face for this style, where `unit` is a hundredth of the height
    /// inside the padding.
    pub fn face<'a>(self, fonts: &'a Fonts, unit: f32) -> Face<'a> {
        match self {
            Style::Title => Face::new(&fonts.bold, 26.0 * unit),
            Style::Artist => Face::new(&fonts.regular, 17.0 * unit),
            Style::Album => Face::new(&fonts.regular, 13.0 * unit),
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
    pub cover: Option<Bounds>,
    pub lines: Vec<Line>,
    /// The whole progress bar, if the track has a position and duration.
    pub bar: Option<Bounds>,
}

/// The inside of `bounds`, less the padding.
fn inner(bounds: Bounds) -> Bounds {
    bounds.inset(0.0, PADDING * bounds.h)
}

/// The side of the cover in `bounds`: the height inside the padding.
pub fn cover_side(bounds: Bounds) -> f32 {
    inner(bounds).h
}

/// With a cover: the cover on the left, the text block to its right.
/// Without (the track has no art, or it failed to load): the text block on
/// the left. The text block is vertically centred either way.
pub fn layout(fonts: &Fonts, track: &Track, with_cover: bool, bounds: Bounds) -> Layout {
    let inner = inner(bounds);
    let unit = inner.h / 100.0;
    let cover_side = inner.h;
    let gap = 12.0 * unit;
    let (left, cover) = if with_cover {
        let cover = Bounds {
            w: cover_side,
            ..inner
        };
        (inner.x + cover_side + gap, Some(cover))
    } else {
        (inner.x, None)
    };
    let text_width = (inner.x + inner.w - left).max(0.0);

    // The text block, from y = 0; moved into place below.
    let mut lines = Vec::new();
    let mut y = 0.0;
    let mut add = |text: String, style: Style, box_height: f32, y: &mut f32| {
        let face = style.face(fonts, unit);
        lines.push(Line {
            width: face.width(&text),
            text,
            style,
            x: left,
            // Centre the glyphs in the line box, like CSS half-leading.
            y: *y + (box_height - face.height()) / 2.0,
        });
        *y += box_height;
    };

    let title = Style::Title.face(fonts, unit);
    let title_box = TITLE_LINE_HEIGHT * 26.0 * unit;
    for text in title.wrap(&track.title, text_width, TITLE_LINES) {
        add(text, Style::Title, title_box, &mut y);
    }
    if !track.artists.is_empty() {
        y += 4.0 * unit;
        let face = Style::Artist.face(fonts, unit);
        let text = face.ellipsize(&track.artists.join(", "), text_width);
        add(text, Style::Artist, face.line_height(), &mut y);
    }
    if let Some(album) = &track.album {
        y += if track.artists.is_empty() { 4.0 } else { 2.0 } * unit;
        let face = Style::Album.face(fonts, unit);
        add(
            face.ellipsize(album, text_width),
            Style::Album,
            face.line_height(),
            &mut y,
        );
    }
    let mut bar = None;
    if track.position.is_some() && track.duration.is_some() {
        y += 10.0 * unit;
        bar = Some(Bounds {
            x: left,
            y,
            w: text_width,
            h: 3.0 * unit,
        });
        y += 3.0 * unit;
    }

    let top = inner.y + (inner.h - y) / 2.0;
    for line in &mut lines {
        line.y += top;
    }
    if let Some(bar) = &mut bar {
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

/// A laid-out track with its text rasterised. Kept until the track's text,
/// whether a cover is shown, or the bounds change.
struct Scene<'t> {
    track: Track,
    with_cover: bool,
    bounds: Bounds,
    layout: Layout,
    lines: Vec<Texture<'t>>,
}

pub struct Metadata<'t> {
    creator: &'t Creator,
    fonts: Rc<Fonts>,
    scene: Option<Scene<'t>>,
}

impl<'t> Metadata<'t> {
    pub fn new(creator: &'t Creator, fonts: Rc<Fonts>) -> Metadata<'t> {
        Metadata {
            creator,
            fonts,
            scene: None,
        }
    }

    /// Draws `track` in `bounds`, with `cover` if given, and its progress,
    /// from 0 to 1, if known.
    pub fn draw(
        &mut self,
        canvas: &mut WindowCanvas,
        bounds: Bounds,
        track: &Track,
        cover: Option<&mut Texture>,
        progress: Option<f32>,
        opacity: f32,
    ) -> Result<(), String> {
        self.update_scene(track, cover.is_some(), bounds)?;
        let scene = self.scene.as_mut().expect("scene just updated");
        let alpha = |a: f32| alpha(a * opacity);
        bounds.clip(canvas, |canvas| {
            if let (Some(cover), Some(rect)) = (cover, scene.layout.cover) {
                cover.set_alpha_mod(alpha(1.0));
                canvas
                    .copy(cover, None, rect.sdl())
                    .map_err(|e| e.to_string())?;
            }
            for (line, texture) in scene.layout.lines.iter().zip(&mut scene.lines) {
                texture.set_alpha_mod(alpha(line.style.opacity()));
                let query = texture.query();
                let dst = SdlRect::new(
                    line.x.round() as i32,
                    line.y.round() as i32,
                    query.width,
                    query.height,
                );
                canvas.copy(texture, None, dst).map_err(|e| e.to_string())?;
            }
            if let (Some(bar), Some(progress)) = (scene.layout.bar, progress) {
                canvas.set_draw_color(Color::RGBA(255, 255, 255, alpha(0.2)));
                canvas.fill_rect(bar.sdl()).map_err(|e| e.to_string())?;
                canvas.set_draw_color(Color::RGBA(255, 255, 255, alpha(1.0)));
                let done = Bounds {
                    w: bar.w * progress,
                    ..bar
                };
                canvas.fill_rect(done.sdl()).map_err(|e| e.to_string())?;
            }
            Ok(())
        })
    }

    fn update_scene(
        &mut self,
        track: &Track,
        with_cover: bool,
        bounds: Bounds,
    ) -> Result<(), String> {
        let current = self.scene.as_ref().is_some_and(|s| {
            s.bounds == bounds && s.with_cover == with_cover && same_layout(&s.track, track)
        });
        if current {
            return Ok(());
        }
        let layout = layout(&self.fonts, track, with_cover, bounds);
        let unit = inner(bounds).h / 100.0;
        let lines = layout
            .lines
            .iter()
            .map(|line| {
                let bitmap = line.style.face(&self.fonts, unit).rasterize(&line.text);
                texture::text(self.creator, &bitmap)
            })
            .collect::<Result<_, _>>()?;
        self.scene = Some(Scene {
            track: track.clone(),
            with_cover,
            bounds,
            layout,
            lines,
        });
        Ok(())
    }
}

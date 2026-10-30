//! The parts of the screen. Each draws into a bounding box it is given, and
//! nowhere else: it could cover the whole canvas, but draws clipped to its
//! box.

pub mod background;
pub mod metadata;
pub mod time;
pub mod weather;

pub use background::Background;
pub use metadata::{Metadata, cover_side};
pub use time::{Time, until_next_minute};
pub use weather::Weather;

use sdl3::rect::Rect as SdlRect;
use sdl3::render::WindowCanvas;

/// How much of the screen's height the time and weather take, from the
/// top.
const TOP_BAND: f32 = 0.25;
/// How much of the screen's height the metadata takes, from the bottom.
const METADATA_BAND: f32 = 0.25;
/// The space between the screen's sides and the components along them, as
/// a fraction of the screen's width.
const MARGIN: f32 = 0.05;
/// The size of the text in the top band, the time's and the weather's, as
/// a fraction of their bounds' height, so the two match.
const TOP_TEXT_SIZE: f32 = 0.4;
const TOP_TEXT_OPACITY: f32 = 0.85;

/// A rectangle in pixels, from its top left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Bounds {
    /// The whole of a screen `width` by `height` pixels.
    pub fn screen((width, height): (u32, u32)) -> Bounds {
        Bounds {
            x: 0.0,
            y: 0.0,
            w: width as f32,
            h: height as f32,
        }
    }

    /// The top `fraction` of these bounds.
    pub fn top(self, fraction: f32) -> Bounds {
        Bounds {
            h: self.h * fraction,
            ..self
        }
    }

    /// The bottom `fraction` of these bounds.
    pub fn bottom(self, fraction: f32) -> Bounds {
        let h = self.h * fraction;
        Bounds {
            y: self.y + self.h - h,
            h,
            ..self
        }
    }

    /// These bounds shrunk by `dx` on the left and right, and `dy` on the
    /// top and bottom.
    pub fn inset(self, dx: f32, dy: f32) -> Bounds {
        Bounds {
            x: self.x + dx,
            y: self.y + dy,
            w: (self.w - 2.0 * dx).max(0.0),
            h: (self.h - 2.0 * dy).max(0.0),
        }
    }

    /// These bounds rounded to whole pixels, at least one each way.
    pub fn sdl(self) -> SdlRect {
        SdlRect::new(
            self.x.round() as i32,
            self.y.round() as i32,
            self.w.round().max(1.0) as u32,
            self.h.round().max(1.0) as u32,
        )
    }

    /// Runs `draw` with `canvas` clipped to these bounds, so nothing it
    /// draws lands outside them.
    fn clip(
        self,
        canvas: &mut WindowCanvas,
        draw: impl FnOnce(&mut WindowCanvas) -> Result<(), String>,
    ) -> Result<(), String> {
        canvas.set_clip_rect(self.sdl());
        let drawn = draw(canvas);
        canvas.set_clip_rect(None);
        drawn
    }
}

/// Where each component goes on the screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Regions {
    /// All of it, behind the others.
    pub background: Bounds,
    /// The left half of the top quarter.
    pub time: Bounds,
    /// The right half of the top quarter.
    pub weather: Bounds,
    /// The bottom quarter.
    pub metadata: Bounds,
}

impl Regions {
    pub fn new(size: (u32, u32)) -> Regions {
        let screen = Bounds::screen(size);
        let margin = MARGIN * screen.w;
        let top = screen.top(TOP_BAND).inset(margin, 0.0);
        let half = top.w / 2.0;
        Regions {
            background: screen,
            time: Bounds { w: half, ..top },
            weather: Bounds {
                x: top.x + half,
                w: half,
                ..top
            },
            metadata: screen.bottom(METADATA_BAND).inset(margin, 0.0),
        }
    }
}

/// An opacity from 0 to 1, as an alpha.
fn alpha(opacity: f32) -> u8 {
    (255.0 * opacity).round() as u8
}

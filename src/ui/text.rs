//! Measuring, wrapping and rasterising text with fontdue.

use fontdue::{Font, FontSettings};

const REGULAR: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const BOLD: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Bold.ttf");
const ELLIPSIS: &str = "…";

/// The bundled Noto Sans (OFL, see `assets/fonts/OFL.txt`).
pub struct Fonts {
    pub regular: Font,
    pub bold: Font,
}

impl Fonts {
    pub fn load() -> Fonts {
        let load = |bytes| Font::from_bytes(bytes, FontSettings::default()).expect("bundled font");
        Fonts {
            regular: load(REGULAR),
            bold: load(BOLD),
        }
    }
}

/// A font at one pixel size.
#[derive(Clone, Copy)]
pub struct Face<'a> {
    font: &'a Font,
    px: f32,
}

/// One line of rasterised text: coverage, one byte per pixel.
pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub alpha: Vec<u8>,
}

impl<'a> Face<'a> {
    pub fn new(font: &'a Font, px: f32) -> Face<'a> {
        Face { font, px }
    }

    /// Height of a rasterised line: ascent to descent.
    pub fn height(&self) -> f32 {
        let m = self.line_metrics();
        m.ascent - m.descent
    }

    /// Distance from one baseline to the next, as the font suggests.
    pub fn line_height(&self) -> f32 {
        self.line_metrics().new_line_size
    }

    pub fn width(&self, text: &str) -> f32 {
        let mut prev = None;
        let mut width = 0.0;
        for c in text.chars() {
            if let Some(prev) = prev {
                width += self.font.horizontal_kern(prev, c, self.px).unwrap_or(0.0);
            }
            width += self.font.metrics(c, self.px).advance_width;
            prev = Some(c);
        }
        width
    }

    /// Splits `text` into at most `max_lines` lines no wider than
    /// `max_width`, breaking between words where possible. If text is left
    /// over, the last line ends with an ellipsis.
    pub fn wrap(&self, text: &str, max_width: f32, max_lines: usize) -> Vec<String> {
        let mut lines = Vec::new();
        let mut rest = text.split_whitespace().collect::<Vec<_>>().join(" ");
        while !rest.is_empty() && max_lines > 0 {
            if lines.len() + 1 == max_lines {
                lines.push(self.ellipsize(&rest, max_width));
                break;
            }
            let end = self.fit(&rest, max_width);
            lines.push(rest[..end].trim_end().to_owned());
            rest = rest[end..].trim_start().to_owned();
        }
        lines
    }

    /// `text`, cut short with an ellipsis if wider than `max_width`.
    pub fn ellipsize(&self, text: &str, max_width: f32) -> String {
        if self.width(text) <= max_width {
            return text.to_owned();
        }
        let with_ellipsis = |end: usize| format!("{}{ELLIPSIS}", text[..end].trim_end());
        let end = text
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&end| self.width(&with_ellipsis(end)) <= max_width)
            .last()
            .unwrap_or(0);
        with_ellipsis(end)
    }

    /// Byte length of the longest start of `text` that fits in `max_width`:
    /// up to a space if any fits, otherwise mid-word (at least one char).
    fn fit(&self, text: &str, max_width: f32) -> usize {
        if self.width(text) <= max_width {
            return text.len();
        }
        let fits = |end: usize| self.width(&text[..end]) <= max_width;
        if let Some(end) = text
            .match_indices(' ')
            .map(|(i, _)| i)
            .take_while(|&i| fits(i))
            .last()
        {
            return end;
        }
        text.char_indices()
            .map(|(i, c)| i + c.len_utf8())
            .take_while(|&end| fits(end))
            .last()
            .unwrap_or_else(|| text.chars().next().map_or(0, char::len_utf8))
    }

    pub fn rasterize(&self, line: &str) -> Bitmap {
        let metrics = self.line_metrics();
        // A little slack for glyphs that overhang their advance.
        let width = self.width(line).ceil() as usize + 2;
        let height = self.height().ceil() as usize;
        let mut alpha = vec![0u8; width * height];

        let mut x = 0.0;
        let mut prev = None;
        for c in line.chars() {
            if let Some(prev) = prev {
                x += self.font.horizontal_kern(prev, c, self.px).unwrap_or(0.0);
            }
            let (glyph, coverage) = self.font.rasterize(c, self.px);
            let left = (x + glyph.xmin as f32).round() as isize;
            let top = (metrics.ascent - glyph.height as f32 - glyph.ymin as f32).round() as isize;
            for gy in 0..glyph.height {
                for gx in 0..glyph.width {
                    let (px, py) = (left + gx as isize, top + gy as isize);
                    if (0..width as isize).contains(&px) && (0..height as isize).contains(&py) {
                        let a = &mut alpha[py as usize * width + px as usize];
                        *a = a.saturating_add(coverage[gy * glyph.width + gx]);
                    }
                }
            }
            x += glyph.advance_width;
            prev = Some(c);
        }
        Bitmap {
            width,
            height,
            alpha,
        }
    }

    fn line_metrics(&self) -> fontdue::LineMetrics {
        self.font
            .horizontal_line_metrics(self.px)
            .expect("font has horizontal metrics")
    }
}

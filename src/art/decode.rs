//! Turns downloaded cover bytes into the images the screen draws.

use image::{DynamicImage, GenericImageView};

/// Covers larger than this are scaled down; the screen shows them at about
/// 670 px on a 1080p TV.
const MAX_SIDE: u32 = 1024;
/// The background is the cover shrunk to this and blurred, then drawn
/// stretched with linear filtering. Each pixel covers about 30 on a 1080p
/// TV, so the blur works out at about 60 px, as on the old web page.
const BLUR_SIDE: u32 = 64;
const BLUR_SIGMA: f32 = 2.0;
/// Corner radius as a fraction of the side: 1.5vh on a 62vh cover, as on
/// the old web page.
const CORNER: f32 = 1.5 / 62.0;

/// RGBA pixels, 4 bytes each, row by row.
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub struct Cover {
    pub url: String,
    /// Square, with rounded corners.
    pub image: Image,
    /// `BLUR_SIDE` square, for the background.
    pub blur: Image,
}

/// Decodes a JPEG or PNG cover, cropped to a centred square like CSS
/// `object-fit: cover`. Anything else is an error, and the screen then shows
/// the track without art.
pub fn decode(url: &str, bytes: &[u8]) -> Result<Cover, String> {
    let image = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let (width, height) = image.dimensions();
    let side = width.min(height);
    let mut square = image.crop_imm((width - side) / 2, (height - side) / 2, side, side);
    if side > MAX_SIDE {
        square = square.thumbnail(MAX_SIDE, MAX_SIDE);
    }
    let blur = rgba(&square.thumbnail(BLUR_SIDE, BLUR_SIDE).blur(BLUR_SIGMA));
    let mut image = rgba(&square);
    round_corners(&mut image);
    Ok(Cover {
        url: url.to_owned(),
        image,
        blur,
    })
}

fn rgba(image: &DynamicImage) -> Image {
    let buffer = image.to_rgba8();
    Image {
        width: buffer.width(),
        height: buffer.height(),
        rgba: buffer.into_raw(),
    }
}

/// Makes the corners transparent, with a one-pixel antialiased edge.
fn round_corners(image: &mut Image) {
    let (width, height) = (image.width as f32, image.height as f32);
    let radius = width.min(height) * CORNER;
    for y in 0..image.height {
        for x in 0..image.width {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // The nearest point of the rectangle shrunk by the radius: the
            // pixel itself, except in the corners.
            let (cx, cy) = (
                px.clamp(radius, width - radius),
                py.clamp(radius, height - radius),
            );
            let distance = (px - cx).hypot(py - cy);
            let coverage = (radius - distance + 0.5).clamp(0.0, 1.0);
            if coverage < 1.0 {
                let alpha = &mut image.rgba[(y * image.width + x) as usize * 4 + 3];
                *alpha = (*alpha as f32 * coverage).round() as u8;
            }
        }
    }
}

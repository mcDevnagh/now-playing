//! Uploading images and text to the GPU.

use sdl3::pixels::PixelFormat;
use sdl3::render::{BlendMode, ScaleMode, Texture, TextureCreator};
use sdl3::video::WindowContext;

use super::text::Bitmap;
use crate::art::Image;

pub type Creator = TextureCreator<WindowContext>;

pub fn rgba<'t>(creator: &'t Creator, image: &Image) -> Result<Texture<'t>, String> {
    let mut texture = creator
        .create_texture_static(PixelFormat::RGBA32, image.width, image.height)
        .map_err(|e| e.to_string())?;
    texture
        .update(None, &image.rgba, image.width as usize * 4)
        .map_err(|e| e.to_string())?;
    texture.set_blend_mode(BlendMode::Blend);
    // Linear filtering when scaled: smooths the cover, and turns the tiny
    // background into a blur.
    texture.set_scale_mode(ScaleMode::Linear);
    Ok(texture)
}

/// A white texture whose alpha is the text's coverage.
pub fn text<'t>(creator: &'t Creator, bitmap: &Bitmap) -> Result<Texture<'t>, String> {
    let image = Image {
        width: bitmap.width as u32,
        height: bitmap.height as u32,
        rgba: bitmap
            .alpha
            .iter()
            .flat_map(|&a| [255, 255, 255, a])
            .collect(),
    };
    rgba(creator, &image)
}

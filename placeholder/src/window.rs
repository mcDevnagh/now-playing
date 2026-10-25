//! The window: black, with an image centred at its own size, and deaf to
//! input.

use std::path::Path;

use sdl2::event::{Event, WindowEvent};
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Texture, TextureCreator, WindowCanvas};
use sdl2::video::WindowContext;

/// Opens the window titled `title`, and runs until SDL is asked to quit, as
/// on SIGTERM. `image`, if given, is drawn centred; if it can't be loaded,
/// that's logged and the window stays black, as the window is what matters.
pub fn run(title: &str, image: Option<&Path>) -> Result<(), String> {
    sdl2::hint::set("SDL_VIDEO_MINIMIZE_ON_FOCUS_LOSS", "0");
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    // The display's own size, for when nothing makes the window fullscreen,
    // as without a window manager.
    let mode = video.desktop_display_mode(0)?;
    let window = video
        .window(title, mode.w as u32, mode.h as u32)
        .fullscreen_desktop()
        .build()
        .map_err(|e| e.to_string())?;
    let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
    sdl.mouse().show_cursor(false);

    let creator = canvas.texture_creator();
    let texture = image.and_then(|path| {
        load(&creator, path)
            .inspect_err(|e| eprintln!("now-playing-placeholder: {}: {e}", path.display()))
            .ok()
    });

    let mut pump = sdl.event_pump()?;
    draw(&mut canvas, texture.as_ref())?;
    loop {
        match pump.wait_event() {
            Event::Quit { .. } => return Ok(()),
            Event::Window {
                win_event: WindowEvent::Exposed | WindowEvent::SizeChanged(..) | WindowEvent::Shown,
                ..
            } => draw(&mut canvas, texture.as_ref())?,
            // Input included: a remote's Back mustn't close it.
            _ => {}
        }
    }
}

/// Loads the image at `path` into a texture, at its own size.
fn load<'t>(
    creator: &'t TextureCreator<WindowContext>,
    path: &Path,
) -> Result<Texture<'t>, String> {
    let image = image::open(path).map_err(|e| e.to_string())?.into_rgba8();
    let (width, height) = image.dimensions();
    let mut texture = creator
        .create_texture_static(PixelFormatEnum::RGBA32, width, height)
        .map_err(|e| e.to_string())?;
    texture
        .update(None, image.as_raw(), width as usize * 4)
        .map_err(|e| e.to_string())?;
    // Transparent parts show the black behind.
    texture.set_blend_mode(BlendMode::Blend);
    Ok(texture)
}

/// Clears to black and draws `texture`, if any, centred at its own size.
/// The screen's size is the renderer's, in pixels, so nothing assumes a
/// resolution.
fn draw(canvas: &mut WindowCanvas, texture: Option<&Texture>) -> Result<(), String> {
    canvas.set_draw_color(Color::BLACK);
    canvas.clear();
    if let Some(texture) = texture {
        let query = texture.query();
        let to = placement(canvas.output_size()?, (query.width, query.height));
        canvas.copy(texture, None, to)?;
    }
    canvas.present();
    Ok(())
}

/// Where an image of size `image` goes on a screen of size `screen`: at its
/// own size, centred. An image larger than the screen gets negative
/// offsets, so it's cropped evenly on each side. When the difference is
/// odd, the extra pixel goes right and below, or is cropped left and above.
fn placement(screen: (u32, u32), image: (u32, u32)) -> Rect {
    let offset = |screen: u32, image: u32| {
        // Both fit in i64, and half their difference in i32.
        ((i64::from(screen) - i64::from(image)).div_euclid(2)) as i32
    };
    Rect::new(
        offset(screen.0, image.0),
        offset(screen.1, image.1),
        image.0,
        image.1,
    )
}

#[cfg(test)]
mod tests;

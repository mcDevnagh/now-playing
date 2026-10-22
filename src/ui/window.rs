//! The full-screen SDL2 window and its event loop.
//!
//! The main thread sleeps in `SDL_WaitEvent`. It wakes for input and window
//! events, for a user event pushed when a new state arrives, and once a
//! second for the progress bar, but only while playing and focused.

use std::ptr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use sdl2::event::{Event, EventSender, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect as SdlRect;
use sdl2::render::{BlendMode, Texture, TextureCreator, WindowCanvas};
use sdl2::video::WindowContext;

use super::layout::{self, Layout, Rect};
use super::text::Fonts;
use crate::state::{State, Track};

const TICK: Duration = Duration::from_secs(1);
const PAUSED_OPACITY: f32 = 0.35;
const COVER_PLACEHOLDER: Color = Color::RGB(40, 40, 40);

/// Hands states from another thread to the window.
pub struct Updates {
    tx: Sender<State>,
    events: EventSender,
    kind: u32,
    last: Option<State>,
}

impl Updates {
    /// Sends `state` and wakes the window, unless it is the same as the last.
    pub fn send(&mut self, state: State) {
        if self.last.as_ref() == Some(&state) {
            return;
        }
        self.last = Some(state.clone());
        if self.tx.send(state).is_ok() {
            let _ = self.events.push_event(Event::User {
                timestamp: 0,
                window_id: 0,
                type_: self.kind,
                code: 0,
                data1: ptr::null_mut(),
                data2: ptr::null_mut(),
            });
        }
    }
}

/// Opens the window and runs until the user quits. `source` runs on its own
/// thread and feeds states through the `Updates` it is given.
pub fn run(source: impl FnOnce(Updates) + Send + 'static) -> Result<(), String> {
    sdl2::hint::set("SDL_VIDEO_MINIMIZE_ON_FOCUS_LOSS", "0");
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let events = sdl.event()?;
    // SAFETY: SDL is initialised and this is the thread that initialised it.
    let kind = unsafe { events.register_event()? };

    let window = video
        .window("Now Playing", 1920, 1080)
        .fullscreen_desktop()
        .build()
        .map_err(|e| e.to_string())?;
    let mut canvas = window
        .into_canvas()
        .present_vsync()
        .build()
        .map_err(|e| e.to_string())?;
    canvas.set_blend_mode(BlendMode::Blend);
    sdl.mouse().show_cursor(false);

    let (tx, rx) = mpsc::channel();
    let updates = Updates {
        tx,
        events: events.event_sender(),
        kind,
        last: None,
    };
    thread::spawn(move || source(updates));

    let creator = canvas.texture_creator();
    let mut screen = Screen {
        fonts: Fonts::load(),
        creator: &creator,
        scene: None,
        state: State::Idle,
        since: Instant::now(),
    };
    event_loop(&sdl, &mut canvas, &mut screen, &rx, kind)
}

fn event_loop(
    sdl: &sdl2::Sdl,
    canvas: &mut WindowCanvas,
    screen: &mut Screen,
    rx: &Receiver<State>,
    kind: u32,
) -> Result<(), String> {
    let mut pump = sdl.event_pump()?;
    let mut focused = true;
    let mut shown = true;
    let mut dirty = true;
    // When the progress bar next needs redrawing. A deadline, not a timeout
    // per wait: other events (an air mouse's motion, say) would otherwise
    // keep restarting the wait, and the bar would never move.
    let mut next_tick = None;
    loop {
        let visible = focused && shown;
        next_tick = match next_tick {
            _ if !(visible && screen.progressing()) => None,
            Some(tick) if Instant::now() >= tick => {
                dirty = true;
                Some(Instant::now() + TICK)
            }
            Some(tick) => Some(tick),
            None => Some(Instant::now() + TICK),
        };
        if visible && dirty {
            screen.draw(canvas)?;
            dirty = false;
        }
        let event = match next_tick {
            Some(tick) => {
                let wait = tick.saturating_duration_since(Instant::now());
                pump.wait_event_timeout(wait.as_millis().max(1) as u32)
            }
            None => Some(pump.wait_event()),
        };
        // No event: the tick is due, which the top of the loop handles.
        let Some(event) = event else { continue };
        match event {
            Event::Quit { .. }
            | Event::KeyDown {
                keycode: Some(Keycode::Escape | Keycode::Backspace | Keycode::AcBack),
                ..
            } => return Ok(()),
            Event::User { type_, .. } if type_ == kind => {
                // Several may have queued; only the newest matters.
                if let Some(state) = rx.try_iter().last() {
                    screen.set_state(state);
                    dirty = true;
                }
            }
            Event::Window { win_event, .. } => match win_event {
                WindowEvent::FocusGained => (focused, dirty) = (true, true),
                WindowEvent::FocusLost => focused = false,
                WindowEvent::Shown | WindowEvent::Restored | WindowEvent::Maximized => {
                    (shown, dirty) = (true, true)
                }
                WindowEvent::Hidden | WindowEvent::Minimized => shown = false,
                WindowEvent::Exposed | WindowEvent::SizeChanged(..) => dirty = true,
                _ => {}
            },
            _ => {}
        }
    }
}

/// What's on screen, and the textures that draw it.
struct Screen<'t> {
    fonts: Fonts,
    creator: &'t TextureCreator<WindowContext>,
    scene: Option<Scene<'t>>,
    state: State,
    /// When `state` arrived, to advance the progress bar from its position.
    since: Instant,
}

/// A laid-out track with its text rasterised. Kept until the track's text,
/// or the screen size, changes.
struct Scene<'t> {
    track: Track,
    size: (u32, u32),
    layout: Layout,
    lines: Vec<Texture<'t>>,
}

impl<'t> Screen<'t> {
    fn set_state(&mut self, state: State) {
        self.state = state;
        self.since = Instant::now();
    }

    fn track(&self) -> Option<(&Track, f32)> {
        match &self.state {
            State::Idle => None,
            State::Playing(track) => Some((track, 1.0)),
            State::Paused(track) => Some((track, PAUSED_OPACITY)),
        }
    }

    /// Whether the progress bar moves on its own.
    fn progressing(&self) -> bool {
        matches!(&self.state, State::Playing(track) if track.duration.is_some())
    }

    fn draw(&mut self, canvas: &mut WindowCanvas) -> Result<(), String> {
        canvas.set_draw_color(Color::BLACK);
        canvas.clear();
        if let Some((track, opacity)) = self.track() {
            let track = track.clone();
            let progress = self.progress(&track);
            let size = canvas.output_size()?;
            let scene = self.scene(&track, size)?;
            draw_scene(canvas, scene, opacity, progress)?;
        }
        canvas.present();
        Ok(())
    }

    /// Where playback is, from 0 to 1, if known.
    fn progress(&self, track: &Track) -> Option<f32> {
        let (position, duration) = (track.position?, track.duration?);
        let elapsed = match self.state {
            State::Playing(_) => self.since.elapsed().as_secs_f64(),
            _ => 0.0,
        };
        Some(((position + elapsed) / duration).clamp(0.0, 1.0) as f32)
    }

    fn scene(&mut self, track: &Track, size: (u32, u32)) -> Result<&mut Scene<'t>, String> {
        let stale = self
            .scene
            .as_ref()
            .is_none_or(|s| s.size != size || !layout::same_layout(&s.track, track));
        if stale {
            let screen = (size.0 as f32, size.1 as f32);
            let layout = layout::layout(&self.fonts, track, screen);
            let lines = layout
                .lines
                .iter()
                .map(|line| {
                    let bitmap = line.style.face(&self.fonts, screen.1).rasterize(&line.text);
                    white_texture(self.creator, bitmap.width, bitmap.height, &bitmap.alpha)
                })
                .collect::<Result<_, _>>()?;
            self.scene = Some(Scene {
                track: track.clone(),
                size,
                layout,
                lines,
            });
        }
        Ok(self.scene.as_mut().expect("scene just built"))
    }
}

fn draw_scene(
    canvas: &mut WindowCanvas,
    scene: &mut Scene,
    opacity: f32,
    progress: Option<f32>,
) -> Result<(), String> {
    let alpha = |a: f32| (255.0 * a * opacity).round() as u8;

    if let Some(cover) = scene.layout.cover {
        let c = COVER_PLACEHOLDER;
        canvas.set_draw_color(Color::RGBA(c.r, c.g, c.b, alpha(1.0)));
        canvas.fill_rect(sdl_rect(cover))?;
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
        canvas.copy(texture, None, dst)?;
    }
    if let (Some(bar), Some(progress)) = (scene.layout.bar, progress) {
        canvas.set_draw_color(Color::RGBA(255, 255, 255, alpha(0.2)));
        canvas.fill_rect(sdl_rect(bar))?;
        canvas.set_draw_color(Color::RGBA(255, 255, 255, alpha(1.0)));
        canvas.fill_rect(sdl_rect(Rect {
            w: bar.w * progress,
            ..bar
        }))?;
    }
    Ok(())
}

/// A white texture whose alpha is `coverage`.
fn white_texture<'t>(
    creator: &'t TextureCreator<WindowContext>,
    width: usize,
    height: usize,
    coverage: &[u8],
) -> Result<Texture<'t>, String> {
    let pixels: Vec<u8> = coverage.iter().flat_map(|&a| [255, 255, 255, a]).collect();
    let mut texture = creator
        .create_texture_static(PixelFormatEnum::RGBA32, width as u32, height as u32)
        .map_err(|e| e.to_string())?;
    texture
        .update(None, &pixels, width * 4)
        .map_err(|e| e.to_string())?;
    texture.set_blend_mode(BlendMode::Blend);
    Ok(texture)
}

fn sdl_rect(r: Rect) -> SdlRect {
    SdlRect::new(
        r.x.round() as i32,
        r.y.round() as i32,
        r.w.round().max(1.0) as u32,
        r.h.round().max(1.0) as u32,
    )
}

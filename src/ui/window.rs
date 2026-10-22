//! The full-screen SDL2 window and its event loop.
//!
//! The main thread sleeps in `SDL_WaitEvent`. It wakes for input and window
//! events, for a user event pushed when a new state or cover arrives, and
//! once a second for the progress bar, but only while playing and focused.

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
use crate::art::{self, Cover, Fetcher, Image};
use crate::state::{State, Track};

const TICK: Duration = Duration::from_secs(1);
const PAUSED_OPACITY: f32 = 0.35;
/// How much the blurred background is darkened: to 35% brightness, as on
/// the old web page.
const BACKGROUND_DIM: u8 = 166;

/// What other threads hand the window.
enum Message {
    State(State),
    /// A cover URL, and its decoded images or why there are none.
    Cover(String, Result<Cover, String>),
}

/// Wakes the window's event loop from another thread.
struct Waker {
    events: EventSender,
    kind: u32,
}

impl Waker {
    fn wake(&self) {
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

/// Hands states from another thread to the window.
pub struct Updates {
    tx: Sender<Message>,
    waker: Waker,
    last: Option<State>,
}

impl Updates {
    /// Sends `state` and wakes the window, unless it is the same as the last.
    pub fn send(&mut self, state: State) {
        if self.last.as_ref() == Some(&state) {
            return;
        }
        self.last = Some(state.clone());
        if self.tx.send(Message::State(state)).is_ok() {
            self.waker.wake();
        }
    }
}

/// Opens the window and runs until the user quits. `source` runs on its own
/// thread and feeds states through the `Updates` it is given. Covers whose
/// host doesn't resolve are fetched from `art_host` instead.
pub fn run(art_host: &str, source: impl FnOnce(Updates) + Send + 'static) -> Result<(), String> {
    sdl2::hint::set("SDL_VIDEO_MINIMIZE_ON_FOCUS_LOSS", "0");
    // Linear filtering when textures are scaled: smooths the cover, and
    // turns the tiny background into a blur.
    sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "1");
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
    let waker = || Waker {
        events: events.event_sender(),
        kind,
    };
    let updates = Updates {
        tx: tx.clone(),
        waker: waker(),
        last: None,
    };
    thread::spawn(move || source(updates));
    let art = spawn_art(Fetcher::new(art_host), tx, waker());

    let creator = canvas.texture_creator();
    let mut screen = Screen {
        fonts: Fonts::load(),
        creator: &creator,
        scene: None,
        state: State::Idle,
        since: Instant::now(),
        art,
        requested: None,
        failed: None,
        cover: None,
    };
    event_loop(&sdl, &mut canvas, &mut screen, &rx, kind)
}

/// Starts the art thread. It blocks until it is sent a URL, then fetches
/// and decodes that cover and hands it to the window.
fn spawn_art(fetcher: Fetcher, tx: Sender<Message>, waker: Waker) -> Sender<String> {
    let (requests, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        while let Ok(url) = rx.recv() {
            // If more were asked for meanwhile, only the newest matters.
            let url = rx.try_iter().last().unwrap_or(url);
            let cover = fetcher
                .fetch(&url)
                .and_then(|bytes| art::decode(&url, &bytes));
            if tx.send(Message::Cover(url, cover)).is_err() {
                return;
            }
            waker.wake();
        }
    });
    requests
}

fn event_loop(
    sdl: &sdl2::Sdl,
    canvas: &mut WindowCanvas,
    screen: &mut Screen,
    rx: &Receiver<Message>,
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
                for message in rx.try_iter() {
                    screen.receive(message)?;
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
    /// Asks the art thread for a cover.
    art: Sender<String>,
    /// The newest cover URL asked for.
    requested: Option<String>,
    /// A cover URL that couldn't be fetched or decoded.
    failed: Option<String>,
    /// The newest cover that loaded. Kept, and shown, while the next loads.
    cover: Option<CoverTextures<'t>>,
}

/// A laid-out track with its text rasterised. Kept until the track's text,
/// whether a cover is shown, or the screen size changes.
struct Scene<'t> {
    track: Track,
    with_cover: bool,
    size: (u32, u32),
    layout: Layout,
    lines: Vec<Texture<'t>>,
}

struct CoverTextures<'t> {
    image: Texture<'t>,
    blur: Texture<'t>,
}

impl<'t> Screen<'t> {
    fn receive(&mut self, message: Message) -> Result<(), String> {
        match message {
            Message::State(state) => self.set_state(state),
            Message::Cover(url, cover) => self.set_cover(url, cover)?,
        }
        Ok(())
    }

    fn set_state(&mut self, state: State) {
        self.state = state;
        self.since = Instant::now();
        let url = self.track().and_then(|(track, _)| track.art_url.clone());
        if let Some(url) = url
            && self.requested.as_ref() != Some(&url)
        {
            self.requested = Some(url.clone());
            let _ = self.art.send(url);
        }
    }

    fn set_cover(&mut self, url: String, cover: Result<Cover, String>) -> Result<(), String> {
        if self.requested.as_ref() != Some(&url) {
            return Ok(()); // superseded while it loaded
        }
        match cover {
            Ok(cover) => {
                self.cover = Some(CoverTextures {
                    image: rgba_texture(self.creator, &cover.image)?,
                    blur: rgba_texture(self.creator, &cover.blur)?,
                });
                self.failed = None;
            }
            Err(e) => {
                eprintln!("now-playing: no cover: {e}");
                self.cover = None;
                self.failed = Some(url);
            }
        }
        Ok(())
    }

    fn track(&self) -> Option<(&Track, f32)> {
        match &self.state {
            State::Idle => None,
            State::Playing(track) => Some((track, 1.0)),
            State::Paused(track) => Some((track, PAUSED_OPACITY)),
        }
    }

    /// Whether `track` is shown with a cover: its own, or the previous one
    /// while its own loads. Not if it has none, or its own failed.
    fn shows_cover(&self, track: &Track) -> bool {
        track.art_url.is_some() && self.cover.is_some() && self.failed != track.art_url
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
            let with_cover = self.shows_cover(&track);
            let size = canvas.output_size()?;
            self.update_scene(&track, with_cover, size)?;
            let scene = self.scene.as_mut().expect("scene just updated");
            let cover = self.cover.as_mut().filter(|_| with_cover);
            draw_scene(canvas, scene, cover, opacity, progress)?;
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

    fn update_scene(
        &mut self,
        track: &Track,
        with_cover: bool,
        size: (u32, u32),
    ) -> Result<(), String> {
        let current = self.scene.as_ref().is_some_and(|s| {
            s.size == size && s.with_cover == with_cover && layout::same_layout(&s.track, track)
        });
        if current {
            return Ok(());
        }
        let screen = (size.0 as f32, size.1 as f32);
        let layout = layout::layout(&self.fonts, track, with_cover, screen);
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
            with_cover,
            size,
            layout,
            lines,
        });
        Ok(())
    }
}

fn draw_scene(
    canvas: &mut WindowCanvas,
    scene: &mut Scene,
    cover: Option<&mut CoverTextures>,
    opacity: f32,
    progress: Option<f32>,
) -> Result<(), String> {
    let alpha = |a: f32| (255.0 * a * opacity).round() as u8;

    if let Some(cover) = cover {
        // The tiny cover stretched over the whole screen (cropped to fill
        // it, like CSS `background-size: cover`), then darkened.
        let (width, height) = scene.size;
        let side = width.max(height);
        let offset = |length: u32| (length as i32 - side as i32) / 2;
        cover.blur.set_alpha_mod(alpha(1.0));
        let background = SdlRect::new(offset(width), offset(height), side, side);
        canvas.copy(&cover.blur, None, background)?;
        canvas.set_draw_color(Color::RGBA(0, 0, 0, BACKGROUND_DIM));
        canvas.fill_rect(None)?;

        if let Some(rect) = scene.layout.cover {
            cover.image.set_alpha_mod(alpha(1.0));
            canvas.copy(&cover.image, None, sdl_rect(rect))?;
        }
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

fn rgba_texture<'t>(
    creator: &'t TextureCreator<WindowContext>,
    image: &Image,
) -> Result<Texture<'t>, String> {
    let mut texture = creator
        .create_texture_static(PixelFormatEnum::RGBA32, image.width, image.height)
        .map_err(|e| e.to_string())?;
    texture
        .update(None, &image.rgba, image.width as usize * 4)
        .map_err(|e| e.to_string())?;
    texture.set_blend_mode(BlendMode::Blend);
    Ok(texture)
}

/// A white texture whose alpha is `coverage`.
fn white_texture<'t>(
    creator: &'t TextureCreator<WindowContext>,
    width: usize,
    height: usize,
    coverage: &[u8],
) -> Result<Texture<'t>, String> {
    let image = Image {
        width: width as u32,
        height: height as u32,
        rgba: coverage.iter().flat_map(|&a| [255, 255, 255, a]).collect(),
    };
    rgba_texture(creator, &image)
}

fn sdl_rect(r: Rect) -> SdlRect {
    SdlRect::new(
        r.x.round() as i32,
        r.y.round() as i32,
        r.w.round().max(1.0) as u32,
        r.h.round().max(1.0) as u32,
    )
}

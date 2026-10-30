//! The full-screen SDL window and its event loop.
//!
//! The main thread sleeps in `SDL_WaitEvent`. It wakes for input and window
//! events, for a user event pushed when a new state or cover arrives, and,
//! while the window is focused, when the clock or the progress bar next
//! moves: once a second while playing, otherwise at the next minute.
//! Only during a fade (`FADE`, after a change of what's on screen) does it
//! draw every frame.

use std::ptr;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Local;
use sdl3::event::{Event, EventSender, WindowEvent};
#[cfg(debug_assertions)]
use sdl3::keyboard::{Keycode, Scancode};
use sdl3::pixels::Color;
use sdl3::render::{BlendMode, Texture, WindowCanvas};

use super::components::{self, Background, Forecast, Metadata, Regions, Time, Weather, forecast};
use super::keys::command_for;
use super::text::Fonts;
use super::texture::{self, Creator};
use crate::art::{self, Cover, Fetcher};
use crate::snapcast::Controller;
use crate::state::{State, Track};
use crate::weather::{Report, Units};

const TICK: Duration = Duration::from_secs(1);
const FADE: Duration = Duration::from_millis(400);
const PAUSED_OPACITY: f32 = 0.35;

/// What other threads hand the window.
enum Message {
    State(State),
    /// A cover URL, and its decoded images or why there are none.
    Cover(String, Result<Cover, String>),
    /// The weather now and the forecast, or None if there's none to show.
    Weather(Option<Report>),
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

/// Hands the weather from another thread to the window.
pub struct WeatherUpdates {
    tx: Sender<Message>,
    waker: Waker,
    last: Option<Option<Report>>,
}

impl WeatherUpdates {
    /// Sends `weather` and wakes the window, unless it is the same as the
    /// last.
    pub fn send(&mut self, weather: Option<Report>) {
        if self.last.as_ref() == Some(&weather) {
            return;
        }
        self.last = Some(weather.clone());
        if self.tx.send(Message::Weather(weather)).is_ok() {
            self.waker.wake();
        }
    }
}

/// Opens the window and runs until the user quits. `source` runs on its own
/// thread and feeds states through the `Updates` it is given. The media keys
/// go to `controller`. Covers whose host doesn't resolve are fetched from
/// `art_host` instead. `weather`, like `source`, runs on its own thread
/// and feeds the weather through the `WeatherUpdates` it is given; its
/// temperatures are shown in `units`.
pub fn run(
    art_host: &str,
    controller: Controller,
    units: Units,
    weather: impl FnOnce(WeatherUpdates) + Send + 'static,
    source: impl FnOnce(Updates) + Send + 'static,
) -> Result<(), String> {
    sdl3::hint::set("SDL_VIDEO_MINIMIZE_ON_FOCUS_LOSS", "0");
    sdl3::hint::set("SDL_RENDER_VSYNC", "1");
    let sdl = sdl3::init().map_err(|e| e.to_string())?;
    let video = sdl.video().map_err(|e| e.to_string())?;
    let events = sdl.event().map_err(|e| e.to_string())?;
    // SAFETY: SDL is initialised and this is the thread that initialised it.
    let kind = unsafe { events.register_event() }.map_err(|e| e.to_string())?;

    // The display's own size, for when nothing makes the window fullscreen,
    // as without a window manager.
    let mode = video
        .get_primary_display()
        .and_then(|display| display.get_mode())
        .map_err(|e| e.to_string())?;
    let window = video
        .window("Now Playing", mode.w as u32, mode.h as u32)
        .fullscreen()
        .build()
        .map_err(|e| e.to_string())?;
    let mut canvas = window.into_canvas();
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
    let weather_updates = WeatherUpdates {
        tx: tx.clone(),
        waker: waker(),
        last: None,
    };
    thread::spawn(move || weather(weather_updates));
    let art = spawn_art(Fetcher::new(art_host), tx, waker());

    let creator = canvas.texture_creator();
    let size = canvas.output_size().map_err(|e| e.to_string())?;
    let fonts = Rc::new(Fonts::load());
    let mut screen = Screen {
        creator: &creator,
        background: Background,
        time: Time::new(&creator, fonts.clone()),
        weather: Weather::new(&creator, fonts.clone(), units),
        forecast: Forecast::new(&creator, fonts.clone(), units),
        metadata: Metadata::new(&creator, fonts),
        report: None,
        state: State::Idle,
        since: Instant::now(),
        art,
        requested: None,
        cover_side: cover_side(size),
        failed: None,
        cover: None,
        cover_id: 0,
        visible: false,
        fade: None,
    };
    event_loop(&sdl, &mut canvas, &mut screen, &rx, kind, &controller)
}

/// Starts the art thread. It blocks until it is sent a URL, then fetches
/// and decodes that cover and hands it to the window.
fn spawn_art(fetcher: Fetcher, tx: Sender<Message>, waker: Waker) -> Sender<(String, u32)> {
    let (requests, rx) = mpsc::channel::<(String, u32)>();
    thread::spawn(move || {
        while let Ok((url, side)) = rx.recv() {
            // If more were asked for meanwhile, only the newest matters.
            let (url, side) = rx.try_iter().last().unwrap_or((url, side));
            let cover = fetcher
                .fetch(&url)
                .and_then(|bytes| art::decode(&url, &bytes, side));
            if tx.send(Message::Cover(url, cover)).is_err() {
                return;
            }
            waker.wake();
        }
    });
    requests
}

fn event_loop(
    sdl: &sdl3::Sdl,
    canvas: &mut WindowCanvas,
    screen: &mut Screen,
    rx: &Receiver<Message>,
    kind: u32,
    controller: &Controller,
) -> Result<(), String> {
    let mut pump = sdl.event_pump().map_err(|e| e.to_string())?;
    let mut focused = true;
    let mut shown = true;
    let mut dirty = true;
    // When the clock or progress bar next needs redrawing. A deadline, not
    // a timeout per wait: other events (an air mouse's motion, say) would
    // otherwise keep restarting the wait, and the bar would never move.
    let mut next_tick = None;
    loop {
        let visible = focused && shown;
        screen.set_visible(visible);
        let fading = visible && screen.fading();
        let next = || Some(Instant::now() + screen.until_tick());
        next_tick = match next_tick {
            _ if !visible => None,
            Some(tick) if Instant::now() >= tick => {
                dirty = true;
                next()
            }
            Some(tick) => Some(tick),
            None => next(),
        };
        if visible && (dirty || fading) {
            screen.draw(canvas)?;
            dirty = false;
        }
        let event = match next_tick {
            // Drawing waits for vsync, which paces the fade.
            _ if fading => pump.poll_event(),
            Some(tick) => {
                let wait = tick.saturating_duration_since(Instant::now());
                pump.wait_event_timeout(wait.max(Duration::from_millis(1)))
            }
            None => Some(pump.wait_event()),
        };
        // No event: the tick is due or a fade goes on; the top of the loop
        // handles both.
        let Some(event) = event else { continue };
        match event {
            // No key quits: in Gaming Mode, Steam's "Exit game" closes it.
            Event::Quit { .. } => return Ok(()),
            Event::User { type_, .. } if type_ == kind => {
                for message in rx.try_iter() {
                    screen.receive(canvas, message)?;
                    dirty = true;
                }
                // Playing or pausing changes how often it ticks.
                next_tick = None;
            }
            Event::Window { win_event, .. } => match win_event {
                WindowEvent::FocusGained => (focused, dirty) = (true, true),
                WindowEvent::FocusLost => focused = false,
                WindowEvent::Shown | WindowEvent::Restored | WindowEvent::Maximized => {
                    (shown, dirty) = (true, true)
                }
                WindowEvent::Hidden | WindowEvent::Minimized => shown = false,
                WindowEvent::Exposed => dirty = true,
                WindowEvent::PixelSizeChanged(..) => {
                    screen.cancel_fade();
                    screen.resize(canvas.output_size().map_err(|e| e.to_string())?);
                    dirty = true;
                }
                _ => {}
            },
            // The snapshot being faded out may have been lost.
            Event::RenderTargetsReset { .. } | Event::RenderDeviceReset { .. } => {
                screen.cancel_fade();
                dirty = true;
            }
            // The media keys, once per press: holding Next skips one track.
            // Keys only arrive while the window has focus, so a game or Kodi
            // in front keeps them.
            Event::KeyDown {
                keycode,
                repeat: false,
                ..
            } if let Some(command) = command_for(keycode) => controller.send(command),
            // Debug builds log the keys nothing handles, to find what a
            // remote's buttons send before mapping them.
            #[cfg(debug_assertions)]
            Event::KeyDown {
                keycode,
                scancode,
                raw,
                repeat: false,
                ..
            } => log_key(keycode, scancode, raw),
            _ => {}
        }
    }
}

/// One line per key, e.g.
/// `key: keycode Escape (0x1b), scancode Escape (41), raw 9`. `raw` is the
/// platform's own code for the key, such as the X11 keycode.
#[cfg(debug_assertions)]
fn log_key(keycode: Option<Keycode>, scancode: Option<Scancode>, raw: u16) {
    let keycode = keycode.map_or("none".to_owned(), |k| format!("{k} ({:#x})", k as u32));
    let scancode = scancode.map_or("none".to_owned(), |s| format!("{s} ({})", s as i32));
    eprintln!("now-playing: key: keycode {keycode}, scancode {scancode}, raw {raw}");
}

/// What's on screen, and the textures that draw it.
struct Screen<'t> {
    creator: &'t Creator,
    background: Background,
    time: Time<'t>,
    weather: Weather<'t>,
    forecast: Forecast<'t>,
    metadata: Metadata<'t>,
    /// The weather now and the forecast, if there is any to show.
    report: Option<Report>,
    state: State,
    /// When `state` arrived, to advance the progress bar from its position.
    since: Instant,
    /// Asks the art thread for a cover, at a side in pixels.
    art: Sender<(String, u32)>,
    /// The newest cover URL asked for.
    requested: Option<String>,
    /// The side covers are asked for at: the side they're shown at.
    cover_side: u32,
    /// A cover URL that couldn't be fetched or decoded.
    failed: Option<String>,
    /// The newest cover that loaded. Kept, and shown, while the next loads.
    cover: Option<CoverTextures<'t>>,
    /// Counts covers loaded, to tell them apart.
    cover_id: u64,
    /// Whether the window can be seen. Changes while it can't don't fade.
    visible: bool,
    fade: Option<Fade<'t>>,
}

/// The screen as it was before a change, drawn over the new one with less
/// and less opacity.
struct Fade<'t> {
    from: Texture<'t>,
    start: Instant,
}

/// What a frame shows, ignoring the clock, weather and progress bar: when
/// this changes, the screen fades.
#[derive(PartialEq)]
enum Look<'a> {
    Black,
    Track {
        title: &'a str,
        artists: &'a [String],
        album: Option<&'a str>,
        paused: bool,
        /// Which cover, if one is shown.
        cover: Option<u64>,
    },
}

struct CoverTextures<'t> {
    image: Texture<'t>,
    blur: Texture<'t>,
}

impl<'t> Screen<'t> {
    /// Applies `message`, first starting a fade if it changes what's shown.
    fn receive(&mut self, canvas: &mut WindowCanvas, message: Message) -> Result<(), String> {
        if self.visible && self.changes_look(&message) {
            self.start_fade(canvas)?;
        }
        match message {
            Message::State(state) => self.set_state(state),
            Message::Cover(url, cover) => self.set_cover(url, cover)?,
            Message::Weather(report) => self.report = report,
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
            let _ = self.art.send((url, self.cover_side));
        }
    }

    /// The screen is now `size`. If covers are shown at a new size, asks
    /// for the newest one again at that size.
    fn resize(&mut self, size: (u32, u32)) {
        let side = cover_side(size);
        if side == self.cover_side {
            return;
        }
        self.cover_side = side;
        if let Some(url) = &self.requested {
            let _ = self.art.send((url.clone(), side));
        }
    }

    fn set_cover(&mut self, url: String, cover: Result<Cover, String>) -> Result<(), String> {
        if self.requested.as_ref() != Some(&url) {
            return Ok(()); // superseded while it loaded
        }
        match cover {
            Ok(cover) => {
                self.cover = Some(CoverTextures {
                    image: texture::rgba(self.creator, &cover.image)?,
                    blur: texture::rgba(self.creator, &cover.blur)?,
                });
                self.cover_id += 1;
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

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        if !visible {
            self.cancel_fade();
        }
    }

    fn fading(&self) -> bool {
        self.fade.is_some()
    }

    fn cancel_fade(&mut self) {
        self.fade = None;
    }

    fn look<'a>(&self, state: &'a State) -> Look<'a> {
        let (track, paused) = match state {
            State::Idle => return Look::Black,
            State::Playing(track) => (track, false),
            State::Paused(track) => (track, true),
        };
        Look::Track {
            title: &track.title,
            artists: &track.artists,
            album: track.album.as_deref(),
            paused,
            cover: self.shows_cover(track).then_some(self.cover_id),
        }
    }

    fn changes_look(&self, message: &Message) -> bool {
        match message {
            Message::State(state) => self.look(state) != self.look(&self.state),
            // A cover for the track on screen: it appears, replaces the
            // previous track's, or (failed) goes.
            Message::Cover(url, _) => self
                .track()
                .is_some_and(|(track, _)| track.art_url.as_ref() == Some(url)),
            // The weather changes in place, like the clock.
            Message::Weather(_) => false,
        }
    }

    /// Renders the screen as it is now into a texture, to fade out over
    /// whatever comes next. If a fade is already running, its current frame
    /// is what fades out.
    fn start_fade(&mut self, canvas: &mut WindowCanvas) -> Result<(), String> {
        let (width, height) = canvas.output_size().map_err(|e| e.to_string())?;
        let mut from = self
            .creator
            .create_texture_target(canvas.default_pixel_format(), width, height)
            .map_err(|e| e.to_string())?;
        let mut drawn = Ok(());
        canvas
            .with_texture_canvas(&mut from, |target| drawn = self.draw_frame(target))
            .map_err(|e| e.to_string())?;
        drawn?;
        from.set_blend_mode(BlendMode::Blend);
        self.fade = Some(Fade {
            from,
            start: Instant::now(),
        });
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

    /// How long until the screen next changes on its own: the progress bar
    /// moves every second while playing, and the clock every minute.
    fn until_tick(&self) -> Duration {
        match &self.state {
            State::Playing(track) if track.duration.is_some() => TICK,
            _ => components::until_next_minute(&Local::now()),
        }
    }

    fn draw(&mut self, canvas: &mut WindowCanvas) -> Result<(), String> {
        self.draw_frame(canvas)?;
        canvas.present();
        Ok(())
    }

    /// Draws the current frame, and the fading one over it, without
    /// presenting: `canvas` may be a texture.
    fn draw_frame(&mut self, canvas: &mut WindowCanvas) -> Result<(), String> {
        canvas.set_draw_color(Color::BLACK);
        canvas.clear();
        let regions = Regions::new(canvas.output_size().map_err(|e| e.to_string())?);
        let track = self
            .track()
            .map(|(track, opacity)| (track.clone(), opacity));
        let with_cover = track
            .as_ref()
            .is_some_and(|(track, _)| self.shows_cover(track));
        let progress = track.as_ref().and_then(|(track, _)| self.progress(track));
        let (blur, image) = match self.cover.as_mut().filter(|_| with_cover) {
            Some(cover) => (Some(&mut cover.blur), Some(&mut cover.image)),
            None => (None, None),
        };
        self.background.draw(canvas, regions.background, blur)?;
        if let Some((track, opacity)) = track {
            self.metadata
                .draw(canvas, regions.metadata, &track, image, progress, opacity)?;
        }
        self.time.draw(canvas, regions.time, &Local::now())?;
        let report = self.report.as_ref();
        self.weather
            .draw(canvas, regions.weather, report.map(|r| &r.current))?;
        let today = Local::now().date_naive();
        let days = report.map_or(&[][..], |r| forecast::from(&r.days, today));
        self.forecast.draw(canvas, regions.forecast, days, today)?;
        if let Some(fade) = &mut self.fade {
            let done = fade.start.elapsed().as_secs_f32() / FADE.as_secs_f32();
            if done < 1.0 {
                fade.from
                    .set_alpha_mod((255.0 * (1.0 - done)).round() as u8);
                canvas
                    .copy(&fade.from, None, None)
                    .map_err(|e| e.to_string())?;
            } else {
                self.fade = None;
            }
        }
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
}

/// The side, in pixels, of the cover on a screen of `size`.
fn cover_side(size: (u32, u32)) -> u32 {
    (components::cover_side(Regions::new(size).metadata).ceil() as u32).max(1)
}

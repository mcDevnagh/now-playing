//! Phase 0 spike: does an SDL2 window behave under gamescope when launched
//! from Steam?
//!
//! Opens a full-screen window with the cursor hidden and logs every event,
//! with a timestamp, to stderr and to `~/.local/state/now-playing-spike.log`
//! (Steam swallows stderr). Quits on `Esc` and on `SDL_QUIT`, which SDL also
//! raises for SIGINT / SIGTERM, i.e. Steam's "Exit game".
//!
//! On screen: a red frame with white corner squares (does the window cover
//! the whole 1920×1080?), and a square in the middle that blinks on the same
//! 1 Hz tick the app will use. The tick runs only while the window is shown
//! and focused, so a frozen square means the tick was paused.
//!
//! `SPIKE_MINIMIZE_ON_FOCUS_LOSS=1` restores SDL's default of minimising a
//! full-screen window when it loses focus; by default the spike turns that off,
//! as the app will.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::WindowCanvas;

const TICK_MS: u32 = 1000;

struct Log {
    start: Instant,
    file: Option<File>,
}

impl Log {
    fn open() -> Log {
        let path = std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".local/state/now-playing-spike.log"));
        let file = path.and_then(|path| {
            fs::create_dir_all(path.parent()?).ok()?;
            OpenOptions::new().create(true).append(true).open(path).ok()
        });
        Log { start: Instant::now(), file }
    }

    fn line(&mut self, msg: impl AsRef<str>) {
        let line = format!("[{:9.3}] {}", self.start.elapsed().as_secs_f64(), msg.as_ref());
        eprintln!("{line}");
        if let Some(file) = &mut self.file {
            let _ = writeln!(file, "{line}");
        }
    }
}

fn main() -> Result<(), String> {
    let mut log = Log::open();
    log.line(format!("--- start, pid {}", std::process::id()));
    for var in ["XDG_SESSION_TYPE", "WAYLAND_DISPLAY", "DISPLAY", "SDL_VIDEODRIVER", "SteamGameId"] {
        log.line(format!("env {var}={:?}", std::env::var(var).ok()));
    }

    let minimize = std::env::var("SPIKE_MINIMIZE_ON_FOCUS_LOSS").as_deref() == Ok("1");
    sdl2::hint::set(
        "SDL_VIDEO_MINIMIZE_ON_FOCUS_LOSS",
        if minimize { "1" } else { "0" },
    );

    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    log.line(format!("SDL {} driver {}", sdl2::version::version(), video.current_video_driver()));
    log.line(format!("minimize on focus loss: {minimize}"));
    match video.desktop_display_mode(0) {
        Ok(mode) => log.line(format!("display 0: {}x{} @ {} Hz", mode.w, mode.h, mode.refresh_rate)),
        Err(e) => log.line(format!("display 0: {e}")),
    }

    let window = video
        .window("Now Playing spike", 1920, 1080)
        .fullscreen_desktop()
        .build()
        .map_err(|e| e.to_string())?;
    let mut canvas = window.into_canvas().present_vsync().build().map_err(|e| e.to_string())?;
    sdl.mouse().show_cursor(false);

    let info = canvas.info();
    log.line(format!("renderer {} {:?}", info.name, info.flags));
    log.line(format!(
        "window {:?}, drawable {:?}, cursor shown {}",
        canvas.window().size(),
        canvas.window().drawable_size(),
        sdl.mouse().is_cursor_showing()
    ));

    let mut events = sdl.event_pump()?;
    let mut shown = true;
    let mut focused = true;
    let mut blink = false;
    let mut redraws = 0u64;
    let mut motions = 0u64;
    draw(&mut canvas, blink, focused);

    loop {
        let ticking = shown && focused;
        let event = if ticking {
            events.wait_event_timeout(TICK_MS)
        } else {
            Some(events.wait_event())
        };

        let Some(event) = event else {
            blink = !blink;
            draw(&mut canvas, blink, focused);
            redraws += 1;
            continue;
        };

        if let Event::MouseMotion { .. } = event {
            motions += 1;
            continue;
        }
        log.line(format!("{event:?}"));

        match event {
            Event::Quit { .. } => break,
            Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                log.line("Esc pressed");
                break;
            }
            Event::Window { win_event, .. } => match win_event {
                WindowEvent::FocusGained => focused = true,
                WindowEvent::FocusLost => focused = false,
                WindowEvent::Shown | WindowEvent::Restored | WindowEvent::Maximized => shown = true,
                WindowEvent::Hidden | WindowEvent::Minimized => shown = false,
                _ => {}
            },
            _ => {}
        }
        if (shown && focused) != ticking {
            log.line(format!("tick {}", if ticking { "off" } else { "on" }));
        }
        draw(&mut canvas, blink, focused);
        redraws += 1;
    }

    log.line(format!("--- exit after {redraws} redraws, {motions} mouse motions ignored"));
    Ok(())
}

fn draw(canvas: &mut WindowCanvas, blink: bool, focused: bool) {
    let (w, h) = canvas.output_size().unwrap_or((1920, 1080));
    canvas.set_draw_color(if focused { Color::RGB(20, 20, 30) } else { Color::RGB(40, 10, 10) });
    canvas.clear();

    canvas.set_draw_color(Color::RGB(220, 40, 40));
    for i in 0..8 {
        let _ = canvas.draw_rect(Rect::new(i, i, w - 2 * i as u32, h - 2 * i as u32));
    }

    let s = 64;
    canvas.set_draw_color(Color::WHITE);
    for (x, y) in [(0, 0), (w - s, 0), (0, h - s), (w - s, h - s)] {
        let _ = canvas.fill_rect(Rect::new(x as i32, y as i32, s, s));
    }

    if blink {
        let b = h / 6;
        let _ = canvas.fill_rect(Rect::new(((w - b) / 2) as i32, ((h - b) / 2) as i32, b, b));
    }
    canvas.present();
}

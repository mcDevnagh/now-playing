use std::time::Duration;

use chrono::{Local, TimeZone};

use super::components::forecast::{self, date_text, high_low_text};
use super::components::metadata::{Style, cover_side, layout, same_layout};
use super::components::time::{self as time_component, clock_text, until_next_minute};
use super::components::weather::{self as weather_component, temperature_text};
use super::components::{Bounds, Regions};
use super::icons::Icon;
use super::text::{Face, Fonts};
use crate::state::Track;
use crate::weather::{Condition, Day, Units, Weather};

const SCREEN: (f32, f32) = (1920.0, 1080.0);
const LONG_TITLE: &str =
    "The Unbearable Lightness of Being Stuck in Traffic on a Rainy Tuesday Afternoon in November";

fn track() -> Track {
    Track {
        title: "Banks Of The Ohio".into(),
        artists: vec!["Artist A".into(), "Artist B".into()],
        album: Some("Album A".into()),
        art_url: Some("https://art.example/a.jpg".into()),
        position: Some(66.0),
        duration: Some(224.0),
    }
}

fn metadata_bounds() -> Bounds {
    Regions::new((SCREEN.0 as u32, SCREEN.1 as u32)).metadata
}

fn title_face(fonts: &Fonts) -> Face<'_> {
    Face::new(&fonts.bold, 70.0)
}

#[test]
fn short_text_is_one_line() {
    let fonts = Fonts::load();
    assert_eq!(title_face(&fonts).wrap("Pink", 500.0, 3), ["Pink"]);
}

#[test]
fn empty_text_has_no_lines() {
    let fonts = Fonts::load();
    assert!(title_face(&fonts).wrap("  ", 500.0, 3).is_empty());
}

#[test]
fn long_text_wraps_at_words_and_ends_with_ellipsis() {
    let fonts = Fonts::load();
    let face = title_face(&fonts);
    let lines = face.wrap(LONG_TITLE, 900.0, 3);
    assert_eq!(lines.len(), 3);
    assert!(lines.iter().all(|line| face.width(line) <= 900.0));
    assert!(lines[2].ends_with('…'));
    // The first two lines break between words.
    for line in &lines[..2] {
        let next = &LONG_TITLE[LONG_TITLE.find(line.as_str()).unwrap() + line.len()..];
        assert!(next.starts_with(' '), "{line:?} breaks mid-word");
    }
}

#[test]
fn a_word_too_long_for_a_line_is_broken() {
    let fonts = Fonts::load();
    let face = title_face(&fonts);
    let lines = face.wrap("Supercalifragilisticexpialidocious", 300.0, 3);
    assert!(lines.len() > 1);
    assert!(lines.iter().all(|line| face.width(line) <= 300.0));
}

#[test]
fn ellipsize() {
    let fonts = Fonts::load();
    let face = Face::new(&fonts.regular, 32.0);
    assert_eq!(face.ellipsize("Album A", 500.0), "Album A");
    let cut = face.ellipsize(LONG_TITLE, 500.0);
    assert!(cut.ends_with('…') && !cut.ends_with(" …"));
    assert!(face.width(&cut) <= 500.0);
}

#[test]
fn regions_split_the_screen() {
    let (w, h) = SCREEN;
    let r = Regions::new((w as u32, h as u32));
    assert_eq!(r.background, Bounds::screen((w as u32, h as u32)));
    // The weather and forecast side by side in the top quarter, the
    // metadata in the bottom quarter, with the same margins at the sides.
    assert_eq!((r.weather.y, r.weather.h), (0.0, 0.25 * h));
    assert_eq!((r.forecast.y, r.forecast.h), (0.0, 0.25 * h));
    assert_eq!((r.metadata.y, r.metadata.h), (0.75 * h, 0.25 * h));
    assert_eq!(r.weather.x, r.metadata.x);
    assert_eq!(r.weather.x + r.weather.w, r.forecast.x);
    assert_eq!(r.weather.w, r.forecast.w);
    assert_eq!(r.forecast.x + r.forecast.w, r.metadata.x + r.metadata.w);
    assert!(r.weather.x > 0.0);
    assert_eq!(r.weather.x, w - (r.forecast.x + r.forecast.w));
    // The time spans the top quarter, overlapping both.
    assert_eq!(r.time.x, r.weather.x);
    assert_eq!(r.time.x + r.time.w, r.forecast.x + r.forecast.w);
    assert_eq!((r.time.y, r.time.h), (0.0, 0.25 * h));
}

fn inside(outer: Bounds, x: f32, y: f32, w: f32, h: f32) -> bool {
    x >= outer.x && y >= outer.y && x + w <= outer.x + outer.w && y + h <= outer.y + outer.h
}

#[test]
fn layout_with_art() {
    let fonts = Fonts::load();
    let bounds = metadata_bounds();
    let l = layout(&fonts, &track(), true, bounds);

    let cover = l.cover.expect("cover");
    assert_eq!(cover.w, cover.h);
    assert_eq!(cover.x, bounds.x);
    assert!((cover.w - cover_side(bounds)).abs() < 0.01);
    assert!(inside(bounds, cover.x, cover.y, cover.w, cover.h));

    let styles: Vec<_> = l.lines.iter().map(|line| line.style).collect();
    assert_eq!(styles, [Style::Title, Style::Artist, Style::Album]);
    assert_eq!(l.lines[1].text, "Artist A, Artist B");
    let bar = l.bar.expect("bar");
    assert!(bar.x > cover.x + cover.w);
    assert!(inside(bounds, bar.x, bar.y, bar.w, bar.h));
    for line in &l.lines {
        assert_eq!(line.x, bar.x);
        assert!(line.y >= bounds.y && line.y < bar.y, "{line:?}");
        assert!(line.x + line.width <= bar.x + bar.w);
    }
    // The text block is vertically centred: as much room above as below.
    let above = l.lines[0].y - bounds.y;
    let below = bounds.y + bounds.h - (bar.y + bar.h);
    assert!(
        (above - below).abs() < 0.1 * bounds.h,
        "{above} above, {below} below"
    );
}

#[test]
fn layout_without_art_is_left_aligned() {
    let fonts = Fonts::load();
    let bounds = metadata_bounds();
    let l = layout(&fonts, &track(), false, bounds);
    assert_eq!(l.cover, None);
    for line in &l.lines {
        assert_eq!(line.x, bounds.x, "{line:?}");
    }
    assert_eq!(l.bar.expect("bar").w, bounds.w);
}

#[test]
fn layout_of_a_sparse_track() {
    let fonts = Fonts::load();
    let track = Track {
        title: LONG_TITLE.into(),
        artists: vec![],
        album: None,
        art_url: None,
        position: None,
        duration: None,
    };
    let bounds = metadata_bounds();
    let l = layout(&fonts, &track, false, bounds);
    assert_eq!(l.bar, None);
    assert_eq!(l.lines.len(), 1);
    let title = &l.lines[0];
    assert_eq!(title.style, Style::Title);
    assert!(title.text.ends_with('…'));
    assert!(title.x + title.width <= bounds.x + bounds.w);
}

#[test]
fn the_clock_shows_hours_and_minutes() {
    let now = Local.with_ymd_and_hms(2026, 10, 30, 9, 5, 42).unwrap();
    assert_eq!(clock_text(&now), "9:05 am");
    assert_eq!(until_next_minute(&now), Duration::from_secs(18));
    let on_the_minute = Local.with_ymd_and_hms(2026, 10, 30, 21, 0, 0).unwrap();
    assert_eq!(clock_text(&on_the_minute), "9:00 pm");
    let midnight = Local.with_ymd_and_hms(2026, 10, 30, 0, 30, 0).unwrap();
    assert_eq!(clock_text(&midnight), "12:30 am");
    assert_eq!(until_next_minute(&on_the_minute), Duration::from_secs(60));
}

#[test]
fn only_text_changes_need_a_new_layout() {
    let a = track();
    let moved = Track {
        position: Some(100.0),
        ..track()
    };
    let new_cover = Track {
        art_url: Some("https://art.example/b.jpg".into()),
        ..track()
    };
    let renamed = Track {
        title: "Pink".into(),
        ..track()
    };
    assert!(same_layout(&a, &moved));
    assert!(same_layout(&a, &new_cover));
    assert!(!same_layout(&a, &renamed));
}

#[test]
fn media_keys_send_commands() {
    use sdl3::keyboard::Keycode;

    use super::keys::command_for;
    use crate::snapcast::Command;

    assert_eq!(
        command_for(Some(Keycode::MediaPlayPause)),
        Some(Command::PlayPause)
    );
    assert_eq!(
        command_for(Some(Keycode::MediaNextTrack)),
        Some(Command::Next)
    );
    assert_eq!(
        command_for(Some(Keycode::MediaPreviousTrack)),
        Some(Command::Previous)
    );
    for other in [
        Keycode::Escape,
        Keycode::Space,
        Keycode::AcBack,
        Keycode::MediaPlay,
    ] {
        assert_eq!(command_for(Some(other)), None, "{other}");
    }
    assert_eq!(command_for(None), None);
}

#[test]
fn every_condition_has_an_icon_and_every_icon_decodes() {
    use clap::ValueEnum;

    for &condition in Condition::value_variants() {
        for is_day in [true, false] {
            let icon = Icon::new(condition, is_day);
            assert!(Icon::ALL.contains(&icon), "{condition:?}");
        }
    }
    assert_eq!(Icon::new(Condition::Clear, false), Icon::ClearNight);
    assert_eq!(
        Icon::new(Condition::PartlyCloudy, true),
        Icon::PartlyCloudyDay
    );
    assert_eq!(Icon::new(Condition::Rain, false), Icon::Rain);
    for icon in Icon::ALL {
        let image = icon.decode().unwrap_or_else(|e| panic!("{icon:?}: {e}"));
        assert_eq!((image.width, image.height), (256, 256), "{icon:?}");
        assert_eq!(image.rgba.len(), 256 * 256 * 4);
    }
}

#[test]
fn temperatures_are_rounded_in_the_chosen_units() {
    assert_eq!(temperature_text(12.8, Units::Fahrenheit), "55°");
    assert_eq!(temperature_text(12.8, Units::Celsius), "13°");
    assert_eq!(temperature_text(-0.4, Units::Celsius), "0°");
    assert_eq!(temperature_text(-17.9, Units::Fahrenheit), "0°");
    assert_eq!(temperature_text(-12.0, Units::Celsius), "-12°");
    assert_eq!(temperature_text(-30.0, Units::Fahrenheit), "-22°");
    let f = Units::Fahrenheit;
    assert!((f.to_celsius(f.from_celsius(21.5)) - 21.5).abs() < 1e-4);
}

fn weather_bounds() -> Bounds {
    Regions::new((SCREEN.0 as u32, SCREEN.1 as u32)).weather
}

#[test]
fn weather_layout_is_left_aligned() {
    let fonts = Fonts::load();
    let bounds = weather_bounds();
    let weather = Weather {
        condition: Some(Condition::PartlyCloudy),
        is_day: false,
        temperature: 14.0,
    };
    let l = weather_component::layout(&fonts, &weather, Units::Fahrenheit, bounds);
    assert_eq!(l.text, "57°");
    let (icon, place) = l.icon.expect("icon");
    assert_eq!(icon, Icon::PartlyCloudyNight);
    assert_eq!(place.w, place.h);
    assert!(place.x + place.w < l.text_x, "{place:?} overlaps the text");
    // The icon on the left edge, at the metadata's margin.
    assert_eq!(place.x, bounds.x);
    assert!(inside(bounds, place.x, place.y, place.w, place.h));
    // On the same line as the clock: both text boxes start at the same y.
    let clock = Face::new(&fonts.regular, 0.4 * bounds.h);
    assert!((l.text_y - (bounds.y + (bounds.h - clock.height()) / 2.0)).abs() < 0.01);
}

#[test]
fn weather_without_a_condition_is_just_the_temperature() {
    let fonts = Fonts::load();
    let weather = Weather {
        condition: None,
        is_day: true,
        temperature: 20.0,
    };
    let l = weather_component::layout(&fonts, &weather, Units::Celsius, weather_bounds());
    assert_eq!(l.icon, None);
    assert_eq!(l.text, "20°");
    // On the left edge on its own.
    assert_eq!(l.text_x, weather_bounds().x);
}

fn date(day: u32) -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
}

fn day(date: chrono::NaiveDate, condition: Option<Condition>) -> Day {
    Day {
        date,
        condition,
        high: 16.1,
        low: 7.8,
    }
}

#[test]
fn the_forecast_is_todays() {
    let week: Vec<_> = (28..=31)
        .map(|d| day(date(d), Some(Condition::Rain)))
        .collect();
    assert_eq!(forecast::today(&week, date(29)).unwrap().date, date(29));
    // None for today: nothing, rather than another day's.
    assert_eq!(forecast::today(&week, date(27)), None);
    assert_eq!(forecast::today(&week[1..], date(28)), None);
}

#[test]
fn forecast_text() {
    assert_eq!(date_text(date(30)), "Fri 10/30");
    let ymd = |y, m, d| chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap();
    assert_eq!(date_text(ymd(2026, 11, 5)), "Thu 11/5");
    assert_eq!(date_text(ymd(2027, 1, 1)), "Fri 1/1");
    let cold = Day {
        high: 16.1,
        low: -0.2,
        ..day(date(30), None)
    };
    assert_eq!(high_low_text(&cold, Units::Fahrenheit), "61° / 32°");
    assert_eq!(high_low_text(&cold, Units::Celsius), "16° / 0°");
}

#[test]
fn forecast_layout() {
    let fonts = Fonts::load();
    let regions = Regions::new((SCREEN.0 as u32, SCREEN.1 as u32));
    let bounds = regions.forecast;
    let today = day(date(30), Some(Condition::PartlyCloudy));
    let l = forecast::layout(&fonts, &today, Units::Fahrenheit, bounds);
    assert_eq!(l.date.text, "Fri 10/30");
    assert_eq!(l.high_low.text, "61° / 46°");

    // The icon: a day one, the same size as the current weather's, and on
    // the same middle.
    let (icon, place) = l.icon.expect("icon");
    assert_eq!(icon, Icon::PartlyCloudyDay);
    let current = weather_component::layout(
        &fonts,
        &Weather {
            condition: Some(Condition::Clear),
            is_day: false,
            temperature: 15.0,
        },
        Units::Fahrenheit,
        regions.weather,
    );
    let current = current.icon.unwrap().1;
    assert!((place.w - current.w).abs() < 0.01);
    assert!((place.y - current.y).abs() < 0.01);
    assert!(inside(bounds, place.x, place.y, place.w, place.h));

    // The icon on the right edge, the lines right-aligned to its left.
    assert!((place.x + place.w - (bounds.x + bounds.w)).abs() < 0.01);
    let right = l.date.x + l.date.width;
    assert!((right - (l.high_low.x + l.high_low.width)).abs() < 0.01);
    assert!(right < place.x);
    // One above the other, close: the space between their digits a third
    // or so of the digits' height. Together, digits to digits, about one
    // line of the clock's digits, and centred on the icon.
    let clock_face = Face::new(&fonts.regular, 0.4 * bounds.h);
    let clock = clock_face.height();
    let (clock_top, clock_bottom) = clock_face.ink_of('0');
    let small = Face::new(&fonts.regular, l.date.height / clock * 0.4 * bounds.h);
    let (top, bottom) = small.ink_of('0');
    let digits = bottom - top;
    let space = (l.high_low.y + top) - (l.date.y + bottom);
    assert!(
        space > 0.0 && space < 0.5 * digits,
        "{space} between {digits}"
    );
    let block = (l.high_low.y + bottom) - (l.date.y + top);
    let clock_digits = clock_bottom - clock_top;
    assert!(
        (block / clock_digits - 1.0).abs() < 0.1,
        "{block} for {clock_digits}"
    );
    let middle = (l.date.y + top + l.high_low.y + bottom) / 2.0;
    assert!((middle - (place.y + place.h / 2.0)).abs() < 0.5);
    for line in [&l.date, &l.high_low] {
        assert!(line.height < clock / 2.0);
        assert!(
            inside(bounds, line.x, line.y, line.width, line.height),
            "{line:?}"
        );
    }

    // Without a condition, just the lines, right-aligned to the edge.
    let l = forecast::layout(&fonts, &day(date(30), None), Units::Fahrenheit, bounds);
    assert_eq!(l.icon, None);
    for line in [&l.date, &l.high_low] {
        assert!((line.x + line.width - (bounds.x + bounds.w)).abs() < 0.01);
    }
}

#[test]
fn the_clock_is_centred() {
    let fonts = Fonts::load();
    let regions = Regions::new((SCREEN.0 as u32, SCREEN.1 as u32));
    let bounds = regions.time;
    let face = Face::new(&fonts.regular, 0.4 * bounds.h);
    for text in ["9:05 am", "12:30 pm"] {
        let (x, y) = time_component::place(&face, text, bounds);
        let centre = x + face.width(text) / 2.0;
        assert!((centre - SCREEN.0 / 2.0).abs() < 0.01, "{text}: {centre}");
        // On the weather's line.
        let weather = weather_component::layout(
            &fonts,
            &Weather {
                condition: None,
                is_day: true,
                temperature: 15.0,
            },
            Units::Fahrenheit,
            regions.weather,
        );
        assert!((y - weather.text_y).abs() < 0.01);
    }
}

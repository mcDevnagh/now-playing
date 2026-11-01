# now-playing

A full-screen "Now Playing" screen for a [Snapcast] client. It shows the
title, artists, album, cover art and progress of whatever the client is
playing along the bottom quarter of the screen, over a blurred, darkened
copy of the cover. The current weather is always shown in the top left,
the time in the top middle, and today's forecast in the top right: the
date (e.g. `Fri 10/30`) and the high and low, then its icon. When nothing
is playing, they're all there is, on black.

It's built for a TV on a Linux HTPC, launched from Steam's Gaming Mode
(gamescope), but runs on any Linux desktop with SDL3.

A companion, [`now-playing-switcher`](#the-switcher), runs beside Kodi,
runs Now Playing as Kodi's screensaver, and quiets the other players when
one starts: Kodi, Snapcast, and [UxPlay] mirroring a device's screen.

[Snapcast]: https://github.com/badaix/snapcast
[UxPlay]: https://github.com/FDH2/UxPlay

## How it works

- It connects to snapserver's JSON-RPC port (1705) over TCP, asks for the
  server's status once, then follows notifications. It finds the stream
  its client is playing and updates as that stream's metadata and
  playback status change, including when the client moves to another
  group or stream.
- The remote's play/pause, next and previous keys control the stream
  it follows, as snapserver's `Stream.Control`. That's only sent when
  the stream says it allows it (its `can*` properties), as go-librespot
  streams, and meta streams over them, do; other sources are left alone.
  Nothing else about the server is changed.
- Metadata comes from the stream's properties. Sources that report
  playback status and position (such as a librespot stream with a
  metadata script) get a progress bar and a paused look; sources that
  don't still show what's playing.
- Cover art is fetched over HTTP(S). If the art URL's host doesn't
  resolve (snapserver may use its own hostname), the `--server` host is
  used instead.
- It redraws only when something changes, plus once a second for the
  progress bar while playing, and otherwise once a minute for the clock. It goes quiet when another window, such as
  a game, is in front. Changes fade over 400 ms.
- If the connection drops, it reconnects with backoff, up to 30 s apart.
- The weather and the daily forecast come from [Open-Meteo], every 15
  minutes, for
  `--latitude` and `--longitude`. Without them, it asks [IP.SB] once at
  start where the machine's public IP address is, and logs the city and
  coordinates it got. That's usually a nearby city, and wrong behind a
  VPN; pass the options to correct it. If fetching fails it retries,
  from 30 s up to 15 minutes apart, and after an hour without weather it
  stops showing the old one. The icons are [Breezy Weather]'s; the
  forecast always uses the day ones.
- Privacy: Open-Meteo is sent the coordinates, and, without them, IP.SB
  sees the IP address; nothing else leaves the machine for the weather.
  See [Open-Meteo's terms] and [IP.SB's privacy policy].

[Open-Meteo]: https://open-meteo.com
[IP.SB]: https://ip.sb
[Open-Meteo's terms]: https://open-meteo.com/en/terms
[IP.SB's privacy policy]: https://ip.sb/privacy-policy/

## Usage

```
now-playing [--server HOST:PORT] [--client ID] [--units UNITS]
            [--latitude DEGREES --longitude DEGREES] [--screensaver]
```

- `--server`: snapserver's JSON-RPC address. The built-in default is a
  LAN address; pass your own.
- `--client`: the Snapcast client to follow, by id or host name. Defaults
  to this machine's hostname.
- `--units`: `fahrenheit` (the default) or `celsius`, for the weather's
  temperature.
- `--latitude`, `--longitude`: where to show the weather for, in degrees
  north and east (south and west negative), given together. Defaults to
  where the machine's public IP address is.
- `--screensaver`: quit on any key but the media keys, for running as
  Kodi's screensaver.

The media keys play or pause, skip to the next track and go back to the
previous one, while the window is in front. A key that isn't sent,
because there's no stream or the stream doesn't allow it, logs one line
saying why. Holding a key sends it once.

Without `--screensaver`, no key quits it. In Gaming Mode, Steam's "Exit
game" quits it; the Steam button leaves it running in the background. With
it, any other key quits, so a key on the remote ends the screensaver, while
the media keys still control the music without waking the TV.

### Debug builds

Debug builds add three options, and a key log, that release builds leave
out:

- `--print` prints state changes instead of opening the window.
- `--fake LOG` replays a captured log from `core/tests/fixtures/` instead
  of connecting, with its original timing. A media key logs the line it
  would send to the replay's stream.
- `--fake-weather CONDITION,TEMP[,night]` shows that weather in the top
  left, and a made-up forecast starting with it, instead of fetching
  them, e.g. `partly-cloudy,57,night`: one of `clear`, `partly-cloudy`,
  `cloudy`, `rain`, `snow`, `wind`, `fog`, `haze`, `sleet`, `hail`,
  `thunder` or `thunderstorm`, the temperature in `--units`, and `night`
  for the night icons (only clear and partly cloudy have them).
- Keys the window doesn't handle are logged to stderr with their SDL
  keycode and scancode and the platform's raw code, to find what a
  remote's buttons send.

## Building

Needs Rust (2024 edition) and the development files for SDL 3.4 or newer
(`SDL3-devel` on Fedora, `sdl3` on Arch, `libsdl3-dev` on Debian).

```
cargo build --release
cargo test
```

This builds the two binaries in `target/release/`: `now-playing` and
`now-playing-switcher`. The switcher needs no system libraries.

The binary links the system libSDL3 and glibc, so build it against the
same versions as the machine it runs on. For a Fedora-based target such
as Bazzite, the `Containerfile` gives a matching build environment:

```
podman build -t now-playing-build .
podman run --rm -v "$PWD":/src:Z -e CARGO_TARGET_DIR=/src/target/fedora \
    now-playing-build cargo build --release
```

The binaries are then in `target/fedora/release/`.

## Running from Steam Gaming Mode

On the HTPC, Steam has one shortcut, Kodi, pointing at `htpc/kodi-steam`.
That script starts the switcher and UxPlay beside Kodi's Flatpak, each in
its own process group, waits on Kodi, and stops all three whichever way it
ends: Kodi exiting, or Steam's "Exit game". Started under Steam, their
windows are part of Kodi's app, so gamescope shows UxPlay's mirror over
Kodi. The commands are set at the top of the script; its own log and each
program's output go to `$XDG_STATE_HOME/now-playing/` (by default
`~/.local/state/now-playing/`).

1. Copy `kodi-steam`, `now-playing-switcher` and `now-playing` to
   `~/.local/bin/`.
2. In Desktop Mode, add `kodi-steam` to Steam as a non-Steam game. Its
   launch options go to UxPlay; UxPlay also reads `~/.config/uxplayrc`.
3. Launch it from Gaming Mode like any game.

Now Playing can also run on its own: add it to Steam as a non-Steam game,
with `--server` and `--client` in its launch options if needed.

## The switcher

`now-playing-switcher` runs beside Kodi, started by `kodi-steam`. When a
player whose table sets `quiet_others = true` starts, it quiets the
others, so two never play over each other:

- Kodi starting mutes this machine's Snapcast client.
- Snapcast starting pauses Kodi.
- A cast to UxPlay pauses Kodi and mutes the client. UxPlay itself is
  never touched.

Muting leaves the music playing, on the phone and in other rooms; pause and
play it to hear it here again, as Snapcast starting always unmutes the
client. Each command sets a state rather than toggling one, so pausing
something already paused, or muting something already muted, does nothing.

It acts when playback starts or resumes, not on a track change or a
reconnect.

While a device mirrors to UxPlay, the remote isn't touching Kodi, so Kodi
would go idle and start its screensaver over the mirror. So, with a
`[kodi]` table, the switcher sends Kodi a no-op action as mirroring starts
and every 30 s until it stops, which also ends a screensaver that's already
up. Set `keep_kodi_awake = false` under `[uxplay]` to turn it off.

### Kodi's screensaver

With a `[kodi.screensaver]` table, the switcher runs its command, Now
Playing with `--screensaver`, while Kodi's screensaver is up, and stops it
when Kodi's stops, as when playback starts or a phone shares a video.
Kodi's own screensaver, set to "Black", is underneath. The command runs
with the switcher's environment, so under `kodi-steam` its window is part
of Kodi's Steam app, and gamescope shows it over Kodi. A key on the remote
closes Now Playing, and the switcher then sends Kodi a no-op so Kodi wakes
too; if Now Playing crashes, the same happens, and Kodi's screensaver
starts it again after Kodi's idle time.

A profile can leave itself instead, as its screensaver starts: with
`[kodi.profiles."Master user"]` `switch_to = "Guest"`, Kodi going idle in
the master profile loads Guest, so master and its library stay behind the
master lock while Share to Kodi still has an unlocked profile to play to.
Guest's own screensaver then shows Now Playing after Guest's idle time.
Loading a profile stops playback; a paused movie keeps its resume point.
The switcher asks Kodi which profile it's in first, and never loads the
one already loaded, which leaves Kodi without its JSON-RPC server until it
restarts.

Kodi doesn't start its screensaver during a video, and music starting
doesn't bring Now Playing up by itself, so music over a video stays behind
it.

UxPlay reports nothing about a cast, so the switcher follows its video
window instead, on `$DISPLAY`: UxPlay opens one when mirroring starts and
closes it when mirroring stops. Audio-only AirPlay opens no window, so it
doesn't quiet anything.

Since Snapcast starting always unmutes this machine's client, a mute made
by hand, such as in Snapweb, is undone the next time the stream starts
playing.

### Setting it up

1. **Kodi:** turn on Settings → Services → Control → "Allow remote control
   from applications on this system". That opens Kodi's JSON-RPC TCP port,
   9090, which the switcher follows. For the screensaver, set Settings →
   Interface → Screensaver to "Black", with its idle time. Kodi keeps both
   per profile, so set them in each one.
2. **Config:** copy `switcher/switcher.example.toml` to
   `~/.config/now-playing/switcher.toml` and fill it in. A config from
   when the switcher brought apps to the front is refused, with an error
   naming the keys to remove.

Its log, one line per event and decision, is in
`~/.local/state/now-playing/switcher.log`.

The switcher finds UxPlay's video window by its `WM_CLASS` class,
`GStreamer`; set `window_class` if yours differs:

```
xprop WM_CLASS    # on UxPlay's display, then click the video window
```

### Usage

```
now-playing-switcher [--config PATH] [--dry-run]
```

- `--config`: the config file. Defaults to
  `$XDG_CONFIG_HOME/now-playing/switcher.toml`, else
  `~/.config/now-playing/switcher.toml`.
- `--dry-run`: log what it would do, without pausing, muting, waking
  Kodi, loading a profile or running the screensaver.

## Layout

| Path | What's there |
|---|---|
| `core/` | The `now-playing-core` crate, shared by the binaries |
| `core/src/snapcast/` | JSON-RPC types, the protocol session, the connection |
| `core/src/state/` | What to show, from a stream's properties |
| `core/src/net.rs` | TCP keepalive and reconnect backoff |
| `core/tests/fixtures/` | Captured and hand-written snapserver logs |
| `app/` | The `now-playing` crate: the full-screen app |
| `app/src/ui/` | The window, and the text and textures it draws with |
| `app/src/ui/components/` | The parts of the screen: background, time, weather, forecast and metadata |
| `app/src/weather/` | The current weather, and fetching it from Open-Meteo |
| `app/src/art/` | Fetching and decoding covers |
| `app/src/debug/` | `--print`, `--fake` and `--fake-weather`, debug builds only |
| `app/assets/fonts/` | The bundled Noto Sans fonts |
| `app/assets/weather/` | The bundled weather icons, from Breezy Weather |
| `switcher/` | The `now-playing-switcher` crate: quiets the other players |
| `switcher/src/listener/` | Following Kodi, Snapcast and UxPlay for starts |
| `switcher/src/quiet.rs` | What a start does to the other players |
| `switcher/src/action.rs` | The dispatch loop: quieting, keeping Kodi awake, its screensaver |
| `switcher/src/screensaver.rs` | Running and stopping the screensaver's command |
| `switcher/src/windows.rs` | Following a display's windows, for UxPlay's |
| `switcher/switcher.example.toml` | An example config |
| `htpc/kodi-steam` | Launches Kodi from Steam, with UxPlay and the switcher |
| `spikes/sdl-hello/` | The SDL2 spike that tested gamescope |

## License

GPL-3.0-or-later; see `LICENSE`.

The bundled Noto Sans fonts are under the SIL Open Font License 1.1; see
`app/assets/fonts/OFL.txt`.

The bundled weather icons are [Breezy Weather]'s, unchanged, under the
GNU LGPL 3.0 with additional terms; see `app/assets/weather/`.

[Breezy Weather]: https://github.com/breezy-weather/breezy-weather

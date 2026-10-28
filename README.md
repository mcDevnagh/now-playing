# now-playing

A full-screen "Now Playing" screen for a [Snapcast] client. It shows the
title, artists, album, cover art and progress of whatever the client is
playing, over a blurred, darkened copy of the cover. When nothing is
playing, the screen is black.

It's built for a TV on a Linux HTPC, launched from Steam's Gaming Mode
(gamescope), but runs on any Linux desktop with SDL3.

A companion service, [`now-playing-switcher`](#the-switcher), brings it to
the front when music starts, Kodi to the front when Kodi plays, and
[UxPlay] to the front when a device mirrors its screen to it.

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
  progress bar while playing. It goes quiet when another window, such as
  a game, is in front. Changes fade over 400 ms.
- If the connection drops, it reconnects with backoff, up to 30 s apart.

## Usage

```
now-playing [--server HOST:PORT] [--client ID]
```

- `--server`: snapserver's JSON-RPC address. The built-in default is a
  LAN address; pass your own.
- `--client`: the Snapcast client to follow, by id or host name. Defaults
  to this machine's hostname.

The media keys play or pause, skip to the next track and go back to the
previous one, while the window is in front. A key that isn't sent,
because there's no stream or the stream doesn't allow it, logs one line
saying why. Holding a key sends it once.

No key quits it. In Gaming Mode, Steam's "Exit game" quits it; the Steam
button leaves it running in the background.

### Debug builds

Debug builds add two options, and a key log, that release builds leave
out:

- `--print` prints state changes instead of opening the window.
- `--fake LOG` replays a captured log from `core/tests/fixtures/` instead
  of connecting, with its original timing. A media key logs the line it
  would send to the replay's stream.
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

This builds the three binaries in `target/release/`: `now-playing`,
`now-playing-switcher` and
[`now-playing-placeholder`](#the-placeholder-window). The switcher needs no
system libraries; the placeholder, like Now Playing, links SDL3.

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

Copy the binary to the HTPC, then in Desktop Mode add it to Steam as a
non-Steam game, with `--server` and `--client` in its launch options if
needed. Launch it from Gaming Mode like any game.

## The switcher

`now-playing-switcher` runs in the background in Steam's Gaming Mode:

- When music starts on this machine's Snapcast client, it brings Now
  Playing to the front, launching it if it isn't running.
- When Kodi starts or resumes playback, it brings Kodi to the front.
- When a device starts mirroring its screen to UxPlay, it brings UxPlay to
  the front. UxPlay has to be running to accept a connection, so it's never
  launched for a cast.
- Optionally, each time Gaming Mode starts, it launches apps in the
  background, such as Kodi, so they're ready to play: once Steam's UI is
  up, it launches each one, waits for it to come to the front, then goes
  back to Steam's home.

It acts when playback starts or resumes, not on a track change or a
reconnect. In Desktop Mode it does nothing.

UxPlay reports nothing about a cast, so the switcher follows its video
window instead: UxPlay opens one when mirroring starts and closes it when
mirroring stops. Audio-only AirPlay opens no window, so it doesn't switch.

It reads which app is in front from gamescope, and whether an app is
running from Steam's processes. To switch or launch, it asks Steam's UI,
just as Steam's own menu does, so Steam keeps track of the app in front.
(Setting gamescope's focus directly doesn't last: Steam puts its own
choice back.)

### Setting it up

1. **Kodi:** turn on Settings → Services → Control → "Allow remote control
   from applications on this system". That opens Kodi's JSON-RPC TCP port,
   9090, which the switcher follows.
2. **UxPlay:** see [Setting up UxPlay](#setting-up-uxplay) below.
3. **Steam:** turn on its remote debugging, which the switcher uses to talk
   to Steam's UI, then restart Steam:

   ```
   touch ~/.steam/steam/.cef-enable-remote-debugging
   ```

   Steam then listens on `127.0.0.1:8080`. Any program on the machine can
   control Steam's UI through it; delete the file and restart Steam to turn
   it off. Kodi's web server also uses port 8080, but on IPv6, so the two
   don't clash.
4. **Config:** copy `switcher/switcher.example.toml` to
   `~/.config/now-playing/switcher.toml` and fill it in. The switcher only
   brings an app to the front if its table gives `app`, its gamescope ID.
   To find it, bring the app to the front and run:

   ```
   DISPLAY=:0 xprop -root GAMESCOPE_FOCUSED_APP
   ```

   A non-Steam game's launch ID is its gamescope ID × 2³² + 0x02000000:

   ```
   python3 -c 'print(GAMESCOPE_ID << 32 | 0x02000000)'
   ```

   To launch an app whenever Gaming Mode starts, at login or from Desktop
   Mode, set `launch_at_start = true` in its table, with its launch ID. It
   isn't launched again if it's already running, as after a restart of the
   switcher. An app that opens no window until it's used never comes to
   the front: Steam stays on its launching screen. So after
   `launch_at_start_wait` seconds (default 30) the switcher goes home
   anyway; set it lower for such an app.

5. **Service:** copy the binary to `~/.local/bin/`, then:

   ```
   cp switcher/now-playing-switcher.service ~/.config/systemd/user/
   systemctl --user enable --now now-playing-switcher
   ```

   Its log, one line per event and decision, is in
   `journalctl --user -u now-playing-switcher`.

### Setting up UxPlay

UxPlay opens no window until a device casts to it, so Steam, which waits
for a game's window, would stay on its launching screen. Launch it through a
script that starts [`now-playing-placeholder`](#the-placeholder-window)
beside it, which gives Steam a window to show:

```sh
#!/bin/bash
# Launches UxPlay, with a placeholder window to show until a device casts.
now-playing-placeholder --image ~/.config/now-playing/airplay.png &
placeholder=$!
trap 'kill "$placeholder"' EXIT
uxplay "$@"
```

1. Save it, say as `~/.local/bin/uxplay-steam`, and make it executable.
2. In Desktop Mode, add it to Steam as a non-Steam game, with UxPlay's own
   options, such as `-n` for its name, in its launch options.
3. Launch it from Gaming Mode to find its gamescope ID, as in step 4 above,
   and give the switcher an `[uxplay]` table with it as `app`, its launch
   ID and `launch_at_start = true`, so it's running to accept a connection.

While a device mirrors, UxPlay's video window covers the placeholder; when
it stops, the placeholder shows again. The switcher finds UxPlay's display
from its process, and its video window by its `WM_CLASS` class,
`GStreamer`; set `window_class` if yours differs:

```
DISPLAY=:1 xprop WM_CLASS    # then click the video window
```

### Usage

```
now-playing-switcher [--config PATH] [--dry-run]
```

- `--config`: the config file. Defaults to
  `$XDG_CONFIG_HOME/now-playing/switcher.toml`, else
  `~/.config/now-playing/switcher.toml`.
- `--dry-run`: log what it would do, without switching or launching.

## The placeholder window

`now-playing-placeholder` is a full-screen black window for an app that
opens none until it's used, such as UxPlay. It ignores input and quits on
SIGTERM.

```
now-playing-placeholder [--image PATH] [--title TITLE]
```

- `--image`: a PNG or JPEG to draw in the middle of the screen. It's never
  scaled: an image the size of the screen fills it, a smaller one is
  centred on black, and a larger one is cropped. If it can't be loaded, the
  window stays black. No image comes with it; for UxPlay, a white AirPlay
  logo on black works well.
- `--title`: the window's title. Defaults to `AirPlay`.

## Layout

| Path | What's there |
|---|---|
| `core/` | The `now-playing-core` crate, shared by the binaries |
| `core/src/snapcast/` | JSON-RPC types, the protocol session, the connection |
| `core/src/state/` | What to show, from a stream's properties |
| `core/src/net.rs` | TCP keepalive and reconnect backoff |
| `core/tests/fixtures/` | Captured and hand-written snapserver logs |
| `app/` | The `now-playing` crate: the full-screen app |
| `app/src/ui/` | The window, text layout and drawing |
| `app/src/art/` | Fetching and decoding covers |
| `app/src/debug/` | `--print` and `--fake`, debug builds only |
| `app/assets/fonts/` | The bundled Noto Sans fonts |
| `switcher/` | The `now-playing-switcher` crate: brings players to the front |
| `switcher/src/listener/` | Following Kodi, Snapcast and UxPlay for starts |
| `switcher/src/action.rs` | Deciding whether to skip, focus or launch |
| `switcher/src/startup.rs` | Launching apps each time Gaming Mode starts |
| `switcher/src/windows.rs` | Following a display's windows, for UxPlay's |
| `switcher/src/steam/` | Steam's running apps, their displays, and its UI |
| `switcher/switcher.example.toml` | An example config |
| `switcher/now-playing-switcher.service` | The systemd user service |
| `placeholder/` | The `now-playing-placeholder` crate: a window for UxPlay |
| `spikes/sdl-hello/` | The SDL2 spike that tested gamescope |

## License

GPL-3.0-or-later; see `LICENSE`.

The bundled Noto Sans fonts are under the SIL Open Font License 1.1; see
`app/assets/fonts/OFL.txt`.

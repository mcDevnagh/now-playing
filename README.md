# now-playing

A full-screen "Now Playing" screen for a [Snapcast] client. It shows the
title, artists, album, cover art and progress of whatever the client is
playing, over a blurred, darkened copy of the cover. When nothing is
playing, the screen is black.

It's built for a TV on a Linux HTPC, launched from Steam's Gaming Mode
(gamescope), but runs on any Linux desktop with SDL2.

A companion service, [`now-playing-switcher`](#the-switcher), brings it to
the front when music starts, and Kodi to the front when Kodi plays.

[Snapcast]: https://github.com/badaix/snapcast

## How it works

- It connects to snapserver's JSON-RPC port (1705) over TCP, asks for the
  server's status once, then follows notifications. It finds the stream
  its client is playing and updates as that stream's metadata and
  playback status change, including when the client moves to another
  group or stream.
- It only reads: it never controls playback or changes the server.
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

Keys: Esc, Backspace or a remote's Back button quit. In Gaming Mode,
Steam's "Exit game" also quits; the Steam button leaves it running in
the background.

### Debug builds

Debug builds add two options that release builds leave out:

- `--print` prints state changes instead of opening the window.
- `--fake LOG` replays a captured log from `core/tests/fixtures/` instead
  of connecting, with its original timing.

## Building

Needs Rust (2024 edition) and the SDL2 development files (`SDL2-devel` on
Fedora, `sdl2` on Arch, `libsdl2-dev` on Debian).

```
cargo build --release
cargo test
```

This builds both binaries, `target/release/now-playing` and
`target/release/now-playing-switcher`. The switcher needs no system
libraries.

The binary links the system libSDL2 and glibc, so build it against the
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

It acts when playback starts or resumes, not on a track change or a
reconnect. In Desktop Mode it does nothing.

It reads which app is in front from gamescope, and whether an app is
running from Steam's processes. To switch or launch, it asks Steam's UI,
just as Steam's own menu does, so Steam keeps track of the app in front.
(Setting gamescope's focus directly doesn't last: Steam puts its own
choice back.)

### Setting it up

1. **Kodi:** turn on Settings → Services → Control → "Allow remote control
   from applications on this system". That opens Kodi's JSON-RPC TCP port,
   9090, which the switcher follows.
2. **Steam:** turn on its remote debugging, which the switcher uses to talk
   to Steam's UI, then restart Steam:

   ```
   touch ~/.steam/steam/.cef-enable-remote-debugging
   ```

   Steam then listens on `127.0.0.1:8080`. Any program on the machine can
   control Steam's UI through it; delete the file and restart Steam to turn
   it off. Kodi's web server also uses port 8080, but on IPv6, so the two
   don't clash.
3. **Config:** copy `switcher/switcher.example.toml` to
   `~/.config/now-playing/switcher.toml` and fill it in. To find an app's
   gamescope ID, bring it to the front and run:

   ```
   DISPLAY=:0 xprop -root GAMESCOPE_FOCUSED_APP
   ```

   A non-Steam game's launch ID is its gamescope ID × 2³² + 0x02000000:

   ```
   python3 -c 'print(GAMESCOPE_ID << 32 | 0x02000000)'
   ```

4. **Service:** copy the binary to `~/.local/bin/`, then:

   ```
   cp switcher/now-playing-switcher.service ~/.config/systemd/user/
   systemctl --user enable --now now-playing-switcher
   ```

   Its log, one line per event and decision, is in
   `journalctl --user -u now-playing-switcher`.

### Usage

```
now-playing-switcher [--config PATH] [--dry-run]
```

- `--config`: the config file. Defaults to
  `$XDG_CONFIG_HOME/now-playing/switcher.toml`, else
  `~/.config/now-playing/switcher.toml`.
- `--dry-run`: log what it would do, without switching or launching.

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
| `switcher/src/listener/` | Following Kodi and Snapcast for starts |
| `switcher/src/action.rs` | Deciding whether to skip, focus or launch |
| `switcher/src/steam/` | Steam's running apps, and its UI |
| `switcher/switcher.example.toml` | An example config |
| `switcher/now-playing-switcher.service` | The systemd user service |
| `spikes/sdl-hello/` | The SDL2 spike that tested gamescope |

## License

GPL-3.0-or-later; see `LICENSE`.

The bundled Noto Sans fonts are under the SIL Open Font License 1.1; see
`app/assets/fonts/OFL.txt`.

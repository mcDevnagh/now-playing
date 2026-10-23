# now-playing

A full-screen "Now Playing" screen for a [Snapcast] client. It shows the
title, artists, album, cover art and progress of whatever the client is
playing, over a blurred, darkened copy of the cover. When nothing is
playing, the screen is black.

It's built for a TV on a Linux HTPC, launched from Steam's Gaming Mode
(gamescope), but runs on any Linux desktop with SDL2.

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
cargo test --workspace
```

The binary links the system libSDL2 and glibc, so build it against the
same versions as the machine it runs on. For a Fedora-based target such
as Bazzite, the `Containerfile` gives a matching build environment:

```
podman build -t now-playing-build .
podman run --rm -v "$PWD":/src:Z -e CARGO_TARGET_DIR=/src/target/fedora \
    now-playing-build cargo build --release
```

The binary is then `target/fedora/release/now-playing`.

## Running from Steam Gaming Mode

Copy the binary to the HTPC, then in Desktop Mode add it to Steam as a
non-Steam game, with `--server` and `--client` in its launch options if
needed. Launch it from Gaming Mode like any game.

## Layout

| Path | What's there |
|---|---|
| `core/` | The `now-playing-core` crate, shared by the binaries |
| `core/src/snapcast/` | JSON-RPC types, the protocol session, the connection |
| `core/src/state/` | What to show, from a stream's properties |
| `core/src/net.rs` | TCP keepalive and reconnect backoff |
| `core/tests/fixtures/` | Captured and hand-written snapserver logs |
| `src/ui/` | The window, text layout and drawing |
| `src/art/` | Fetching and decoding covers |
| `src/debug/` | `--print` and `--fake`, debug builds only |
| `switcher/` | `now-playing-switcher`, which brings players to the front (in progress) |
| `spikes/sdl-hello/` | The SDL2 spike that tested gamescope |

## License

GPL-3.0-or-later; see `LICENSE`.

The bundled Noto Sans fonts are under the SIL Open Font License 1.1; see
`assets/fonts/OFL.txt`.

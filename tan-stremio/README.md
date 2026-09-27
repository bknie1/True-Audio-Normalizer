# tan-stremio

TAN as a [Stremio](https://www.stremio.com/) addon. Two modes:

- **`proxy`** (streamed content) - wraps an upstream stream addon (your
  debrid-configured Torrentio/Comet/etc.) and offers a **real-time
  TAN-normalized variant** of each direct-URL stream. Pick the "TAN" version of
  a movie and its audio is leveled live. This is the one to use for the movies
  you actually stream. See [Real-time proxy](#real-time-proxy-streamed-content).
- **default** (local library) - pre-normalizes **your own local video files**
  and serves each as a "TAN Normalized" stream. See
  [Local library mode](#local-library-mode).

## Real-time proxy (streamed content)

A pure Stremio addon can't touch the player's audio pipeline - it only gets to
*offer a stream URL*. The proxy works around that by making the URL it offers
point back at itself: when the player opens the "TAN" stream, a local ffmpeg
pipeline pulls the original stream, runs its audio through TAN in real time
(the `pcm-filter` stage), copies the video untouched, and remuxes to a live
MPEG-TS stream.

```
tan-stremio proxy --upstream "https://torrentio.strem.fun/<your-config>/manifest.json"
```

Then paste `http://127.0.0.1:5870/manifest.json` into Stremio's addon search
bar. Open any movie: alongside your normal streams you'll see **"TAN movie"**
entries - pick one to watch it normalized.

- `--upstream <url>` - **required, repeatable.** The install URL of a stream
  addon whose streams resolve to **direct HTTP URLs** - i.e. one configured
  with a **debrid service** (Real-Debrid/AllDebrid/Premiumize). Raw-torrent
  streams (`infoHash`, no URL) have nothing to pull, so they're skipped.
- `--profile <name>` - `movie` (default), `universal`, `music`, `speech`,
  `night`, `game`.
- `--port` / `--bind` - default `5870` / `127.0.0.1`. `--ffmpeg <path>` if
  ffmpeg isn't on PATH. Needs `curl` (in-box on Windows 10+) to query upstream.

**Verified:** end-to-end on this machine - upstream wrap, live transcode, and
TAN leveling (a quiet passage came up, a loud one came down) through a real
ffmpeg pipeline, streamed to an HTTP client. Not yet exercised inside the
Stremio UI against a live debrid account.

**Limits (v1, deliberate):** only direct-URL (debrid) streams are wrapped; the
source is read twice (once for video, once for audio - ~2x bandwidth);
surround is downmixed to stereo; and the live stream **can't seek**. A
single-read path (a stereo TAN LADSPA filter for ffmpeg) and seek support are
the obvious next steps.

## Local library mode

## Why this shape, not live audio processing

Stremio's addon protocol only ever gets to *offer a stream URL* before
playback starts - an addon has no hook into the player's live audio
pipeline, so it can't touch audio in real time the way
[tan-live](../tan-live)/[tan-tray](../tan-tray) do for desktop capture. (If
you want TAN on Stremio's actual playback audio right now, point tan-tray at
whatever output device Stremio plays through - see tan-tray's README.)

What an addon *can* do is pre-process your own files once and hand back an
alternate, already-normalized stream - the same "extra Play Version" idea as
[jellyfin-plugin-tan](../jellyfin-plugin-tan), just for Stremio instead of a
Jellyfin server. Unlike the Jellyfin plugin, this doesn't need to be a thin
client of [tan-server](../tan-server) - a Stremio addon is just a plain HTTP
server, so it's ordinary Rust linking `tan-core` directly, no second process
required.

## What it does

1. On startup, recursively scans `--library` for video files (mp4, mkv,
   avi, mov, webm, m4v).
2. For each one not already normalized, extracts its audio with `ffmpeg`,
   runs it through TAN, and muxes the result back with the *original video
   stream, copied verbatim* (no re-encode) into a cached `.mkv`. Existing
   output is left alone on later runs - delete it to force a redo. The
   original file is never modified.
3. Serves a Stremio addon manifest plus a catalog and stream resource: each
   library file appears under "TAN Local Library," and its "TAN Normalized"
   stream plays the processed copy.

Install it in Stremio by pasting the addon's manifest URL
(`http://127.0.0.1:5860/manifest.json` by default) into Stremio's addon
search bar.

## Running it

```
cargo run -p tan-stremio -- --library "D:\Movies"
```

```
usage: tan-stremio --library <dir> [--port <n>] [--bind <address>]
                    [--output <dir>] [--profile <name>] [--ffmpeg <path>]
```

- `--library <dir>` - required. Scanned recursively, once, at startup;
  restart to pick up new files.
- `--output <dir>` - where normalized copies are cached. Default:
  `<library>/.tan-cache`.
- `--profile <name>` - `universal`, `movie` (default), `music`, `speech`,
  `night`, or `game`.
- `--port` / `--bind` - default `5860` / `127.0.0.1`. Like tan-server, this
  has no authentication, so only bind elsewhere behind something that
  provides it.
- `--ffmpeg <path>` - override if `ffmpeg` isn't on PATH.

Normalization runs once at startup for anything not yet cached, not lazily
per stream request - a movie-length file can take a while to process, and
blocking an HTTP response on that would time out most Stremio clients.
Start it, let the batch finish (progress prints to the console), then open
Stremio.

## Verified vs. not

- **Compiles clean, all tests pass** (`cargo test -p tan-stremio`): the
  hashing/id scheme, JSON building, percent-decoding, and HTTP routing are
  covered against a real (if empty) library.
- **Not yet run against real ffmpeg or a real Stremio client.** This
  environment doesn't have `ffmpeg` on PATH, so the actual
  extract/normalize/mux pipeline - and whether Stremio's client actually
  displays and plays the resulting stream correctly, including seeking with
  no HTTP Range support on `/file` - hasn't been exercised end to end. That
  needs a real library, real ffmpeg, and a real Stremio install to confirm.

## Known limitations (scoped deliberately, not oversights)

- **No HTTP Range support on `/file`.** Playback should still work as a
  progressive download, but seeking within a file may be limited or
  unsupported by some players until this is added.
- **Movies only** (Stremio's `movie` type) - no series/season-episode
  structure, no music library. A flat scan treats every video file as a
  standalone title.
- **No live rescan.** New files need a restart to be picked up; nothing
  watches the filesystem.

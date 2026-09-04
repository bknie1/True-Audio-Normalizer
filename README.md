# TAN - True Audio Normalizer

TAN fixes poorly mixed audio in real time: dialogue that's too quiet, action scenes
that are too loud, and speech that gets buried under music or effects. The goal is a
single normalization engine that eventually runs system-wide on Windows, macOS,
Linux, Android, and (where the platform allows it) iOS, without the user having to
touch anything else about their audio setup.

## Status

The core engine works: perceptual (BS.1770 K-weighted) loudness metering, a two-way
gain rider anchored to the content's own baseline loudness (so overall volume always
matches the source), and a look-ahead peak limiter, with movie and music profiles.
On the bundled demo the offline mode halves a 26 dB loudness range while leaving the
loud passages within ~2.5 dB of the original. Hear it, or run it on your own files
and live browser-tab audio, at [bknie1.github.io/TAN](https://bknie1.github.io/TAN/).

Roadmap:

1. Done - WAV codec, perceptual metering, two-way AGC, look-ahead limiter, CLI, CI.
2. Next - blind dialogue detection: a lightweight real-time neural model
   (DeepFilterNet-style) so speech stays intelligible without the mastering-time
   metadata Dolby relies on.
3. Then - live system-audio adapters per platform, mobile.

## Architecture

- `tan-core` - the actual normalization engine: dynamic range compression, and later
  multiband processing plus a dialogue clarity boost. Pure Rust, no OS dependencies,
  no I/O. Operates on plain sample buffers so it's identical on every platform.
- `tan-cli` - a command-line tool that runs a WAV file through `tan-core`, for testing
  and listening to results before any live/system audio work exists.
- Future: one thin platform adapter per OS, responsible only for getting real-time
  audio into and out of `tan-core` (Windows Audio Processing Object, PipeWire filter
  node on Linux, Core Audio plugin on macOS, etc.).

## Building

```
cargo build
cargo test
```

## License

MIT, see [LICENSE](LICENSE).

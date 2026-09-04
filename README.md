# TAN - True Audio Normalizer

TAN fixes poorly mixed audio in real time: dialogue that's too quiet, action scenes
that are too loud, and speech that gets buried under music or effects. The goal is a
single normalization engine that eventually runs system-wide on Windows, macOS,
Linux, Android, and (where the platform allows it) iOS, without the user having to
touch anything else about their audio setup.

## Status

Early work in progress. Currently building `tan-core`, the portable DSP engine, with
no platform integration yet.

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

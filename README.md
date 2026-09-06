# TAN - True Audio Normalizer

TAN fixes poorly mixed audio in real time: dialogue that's too quiet, action scenes
that are too loud, and speech that gets buried under music or effects. The goal is a
single normalization engine that eventually runs system-wide on Windows, macOS,
Linux, Android, and (where the platform allows it) iOS, without the user having to
touch anything else about their audio setup.

Hear it, run it on your own files, or pipe live browser-tab audio (YouTube included)
through it at **[bknie1.github.io/True-Audio-Normalizer](https://bknie1.github.io/True-Audio-Normalizer/)**. Everything
runs client-side; nothing is uploaded.

**[Install it](INSTALL.md)** - Windows, macOS, Linux, or mobile as an installable web app.
New to audio processing or Rust? Start with **[TAN, minus the jargon](ELI5.md)** and
[GLOSSARY.md](GLOSSARY.md) for terms like FFI, WASAPI, and C ABI as they come up below.

Unlike the commercial equivalents (Dolby Volume and friends), TAN works blind - no
loudness metadata baked in at mastering time, no licensed decoder, no walled garden.
Everything here is MIT-licensed Rust with no external dependencies: the whole engine
is `std` plus math.

## Repository layout

This is a [Cargo workspace](Cargo.toml): several related crates (Rust packages)
sharing one build and one lockfile. The split is the architecture:

| Crate | What it is |
|---|---|
| [`tan-core`](tan-core/src) | The engine. Pure sample-buffer math, no I/O, no OS calls - which is why the same code runs natively, in a browser, and on live desktop audio unchanged. |
| [`tan-cli`](tan-cli/src) | Command-line tool: generates test audio, runs files through the engine, prints loudness stats. Owns the WAV codec. |
| [`tan-ffi`](tan-ffi/src) | Thin FFI (see [GLOSSARY](GLOSSARY.md)) wrapper exposing the engine as plain C-ABI functions. Builds to a native shared library (`.dll`/`.dylib`/`.so`) for other applications, or to WebAssembly for the browser portal - same source, different `cargo build --target`. All of the project's `unsafe` code lives here, and only here. |
| [`tan-live`](tan-live/src) | Windows desktop tool: captures live audio (microphone, or WASAPI loopback of whatever's playing) and plays the processed result out a second device in real time. First cut of "TAN on your actual desktop," ahead of a proper system-audio adapter. |

## How the DSP works

Audio arrives as interleaved `f32` samples in the range -1.0..1.0 (interleaved =
alternating left, right, left, right...). Each frame flows through three stages,
all in `tan-core`:

**1. Perceptual loudness metering** ([loudness.rs](tan-core/src/loudness.rs)).
Raw amplitude is a bad proxy for how loud something *sounds*: a bass rumble and
clear dialogue can have identical sample values and wildly different perceived
loudness. TAN uses the ITU-R BS.1770 "K-weighting" filter - a shelf boost in the
presence region plus a high-pass that discounts rumble - then tracks the smoothed
mean square of the filtered signal. The filters are
[biquads](tan-core/src/biquad.rs): tiny second-order recursive filters (each output
sample depends on the last two inputs and outputs) that are the workhorse of
practically all audio EQ. Coefficients come from the standard Audio EQ Cookbook
formulas.

**2. Baseline-anchored gain riding** ([lib.rs](tan-core/src/lib.rs)). The engine
tracks the content's own average loudness - following louder material quickly and
sinking toward quiet slowly, because perception anchors on the prominent level -
and levels *around* that baseline rather than toward any absolute target. That's
what keeps overall volume identical to the source. Quiet dialogue is boosted toward
the baseline, loud peaks pulled down to it. Gain changes are slew-rate limited
(capped dB-per-second) with different speeds per direction, and the cut rate scales
with overshoot so a big correction completes within ~50 ms of a loud onset - hidden
inside the onset transient, where the ear can't track level. A gate freezes gain
during silence so quiet room tone is never amplified into hiss.

**3. Look-ahead peak limiting** ([limiter.rs](tan-core/src/limiter.rs)). Output is
delayed by 8 ms while the limiter watches the *incoming* audio, so gain ramps down
before a peak arrives instead of clicking after it. The "what's the worst peak in
the next 8 ms" question is answered in constant time per sample with a monotonic
deque - a classic sliding-window-minimum data structure.

**Offline mode** ([offline.rs](tan-core/src/offline.rs)) is the same philosophy
with the reactive constraint removed: it measures the whole file first, computes a
desired gain curve, then slew-limits it forward in time *and* backward in time
(so cuts ramp down just ahead of the onset that requires them) and takes the lower
of the two curves. No surprises means no artifacts at all.

## Rust concepts on display, and where

This project doubles as a Rust learning exercise. Concepts worth studying, with the
places they appear:

- **Workspaces and crates** ([Cargo.toml](Cargo.toml)) - one repo, four packages,
  one shared `target/` build directory and lockfile.
- **Ownership and in-place mutation** - `Normalizer::process(&mut self, &mut [f32])`
  borrows the caller's buffer and processes it in place: no allocation, no copies,
  which is what a real-time audio path demands. The borrow checker proves at compile
  time that nothing else touches the buffer mid-process.
- **Slices and `chunks_exact_mut`** ([lib.rs](tan-core/src/lib.rs)) - iterating
  interleaved audio one frame (one sample per channel) at a time, with the bounds
  checks lifted out of the loop by the iterator.
- **Structs + `impl` instead of classes** - `Biquad`, `LoudnessMeter`, `Limiter`,
  `Normalizer` are plain data with associated functions. No inheritance; composition
  all the way down (`Normalizer` *contains* a meter and a limiter).
- **`Option<f32>`** ([lib.rs](tan-core/src/lib.rs)) - the baseline starts as `None`
  ("no audible material seen yet"), not as a magic sentinel value. The compiler
  forces both cases to be handled.
- **Iterators, closures, and `fold`** - peak detection, energy means, and the gain
  curve passes are iterator chains rather than index-juggling loops.
- **`VecDeque` as a monotonic queue** ([limiter.rs](tan-core/src/limiter.rs)) -
  the sliding-window minimum, amortized O(1) per sample.
- **Colocated unit tests** (`#[cfg(test)] mod tests` in every file) - tests live
  next to the code they verify and only compile for `cargo test`. The suite encodes
  behavioral promises: "steady content passes through unchanged", "level never
  slides down after an onset", "silence stays silent".
- **`f32`/`f64` boundaries** ([biquad.rs](tan-core/src/biquad.rs)) - samples are
  `f32`, but filter state is `f64` because recursive filters accumulate rounding
  error at low frequencies.
- **FFI and contained `unsafe`** ([tan-ffi/src/lib.rs](tan-ffi/src/lib.rs)) -
  plain C-ABI exports (`#[unsafe(no_mangle)] extern "C"`) passing raw pointers into
  wasm linear memory (or plain process memory, for the native build), with the
  caller on the other side. The engine itself contains zero `unsafe`; the wrapper
  quarantines all of it in one small file.
- **Real-time audio callbacks** ([tan-live/src/main.rs](tan-live/src/main.rs)) -
  `cpal` hands you a buffer and a strict time budget to fill it; anything that
  blocks (allocating, locking contended for too long) risks an audible glitch.
  `tan-live`'s current mutex-guarded ring buffer is an honest first cut, not
  the final real-time-safe design - noted in the file itself.
- **Byte-level I/O without a library** ([tan-cli/src/wav.rs](tan-cli/src/wav.rs)) -
  the WAV codec is hand-written RIFF chunk parsing using `from_le_bytes` /
  `to_le_bytes` (WAV is little-endian throughout).

## Building and using

```
cargo test
cargo build --release

# generate a deliberately badly-mixed 24s demo file
target/release/tan-cli gen demo.wav

# real-time-style processing (what live/system-wide mode sounds like)
target/release/tan-cli process demo.wav out.wav movie

# offline two-pass: artifact-free, gain ramps down ahead of loud onsets
target/release/tan-cli process demo.wav out.wav movie --two-pass
```

Profiles: `movie` (strong leveling) or `music` (gentler, preserves dynamics).

Live desktop audio (Windows only for now):

```
cargo build --release -p tan-live
target/release/tan-live.exe --list-devices
target/release/tan-live.exe --loopback --output "<a second output device>"
```

See [INSTALL.md](INSTALL.md) for the routing caveat this first cut has (it needs
two distinct audio devices until a proper system-audio adapter replaces it).

Rebuilding the browser engine after changing `tan-core`:

```
cargo build -p tan-ffi --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/tan.wasm docs/tan.wasm
```

Building the native shared library (for embedding in other applications):

```
cargo build -p tan-ffi --release
# -> target/release/tan.dll (Windows) / libtan.dylib (macOS) / libtan.so (Linux)
```

## Tutorial: running TAN alongside SteelSeries Sonar (or any audio mixer)

In plain language, because this is the question people actually hit first.

**The situation.** Today's live tool (`tan-live`) works by *capturing* sound
from one audio device and *playing* the fixed sound out of a second device. A
single headset is only one device, so on its own TAN has nowhere to sit "in the
middle" of your listening.

**Where the gaming software comes in.** Programs like SteelSeries Sonar (part of
SteelSeries GG), Voicemeeter, or VB-Audio Cable all do one thing that helps
here: they create *virtual* audio devices. A virtual device is a sound card that
exists only in software - nothing is plugged into it, but Windows treats it like
a real output. That extra device is exactly the "second device" TAN needs, so
you don't have to buy or plug in any hardware.

**How they fit together.** You end up with a short chain:

1. Your games, movies, and browser play their sound into the virtual device
   (if you already use Sonar, that virtual device is your Windows default
   output, so this part is already true).
2. `tan-live --loopback` listens to that virtual device and normalizes whatever
   is playing.
3. TAN plays the normalized result out to your real headset.
4. You listen to the headset, and TAN is now in the middle - leveling the final
   mix on its way to your ears.

**Why they don't fight.** Sonar and TAN work at different points in the chain.
Sonar mixes your separate game / chat / mic streams, applies its own EQ, and
handles your microphone; TAN only takes the finished mix Sonar produces and
evens out its loudness before it reaches you. Neither one has to be turned off
for the other to work, so you keep your Sonar setup exactly as it is.

**One thing to watch: don't make a loop.** TAN's *output* device has to be
something the mixer is not also feeding back into, or the sound will chase its
own tail. The easy answers: point TAN's output at a genuinely separate physical
device (your headset directly, while apps play into the virtual one), or use a
mixer that gives you a distinct second output (Sonar's separate stream /
monitoring output, or a second VB-Cable) and capture that.

```
tan-live --list-devices
tan-live --loopback --output "<your real headset>"
```

`--loopback` grabs whatever your default output (the virtual device) is playing;
`--output` names the real device you actually listen on.

**The short-term nature of this.** All of the above is a workaround for the
fact that TAN is not yet *inside* the Windows audio pipeline. The next major
piece of work (see below, and [`tan-apo`](tan-apo/README.md)) is an Audio
Processing Object that inserts TAN directly into the system's effect chain, the
same way Sonar and Waves attach to a device. When that lands, none of this
two-device routing is needed: TAN just sits on the endpoint alongside Sonar,
and there is nothing to wire up.

## Roadmap

1. Done - WAV codec, perceptual metering, baseline-anchored two-way AGC, look-ahead
   limiter, offline two-pass mode, CLI, wasm + interactive portal, native shared
   library, first-cut Windows live tool, tagged releases with installers, CI on
   Windows/macOS/Linux.
2. Next - a real Windows Audio Processing Object, so TAN sits in the system audio
   pipeline directly instead of needing a second device or virtual cable.
3. Then - blind dialogue detection: a lightweight real-time neural model
   (DeepFilterNet-style) so speech stays intelligible without the mastering-time
   metadata Dolby relies on.
4. Then - live system-audio adapters for macOS/Linux, mobile.

## License

MIT, see [LICENSE](LICENSE).

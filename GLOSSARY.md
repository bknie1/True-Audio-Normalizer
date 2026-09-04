# Glossary

Terms that came up building TAN, explained once here instead of re-explained
inline every time. Cross-referenced from the [README](README.md).

## Audio and DSP

**DSP (Digital Signal Processing).** Math run on a stream of numbers that
represent sound. Everything TAN does - measuring loudness, riding gain,
limiting peaks - is DSP. No AI here; it's arithmetic on samples.

**Sample.** One number representing the wave's position at one instant. TAN
works in `f32` samples from -1.0 to 1.0. 48,000 of them per second is normal
("48 kHz").

**Interleaved.** Multi-channel audio stored as alternating samples: left,
right, left, right, rather than all of the left channel followed by all of the
right. TAN's buffers are interleaved because that's how audio hardware
actually hands you the data.

**Biquad.** A tiny recursive filter: each output sample is computed from the
last two inputs and the last two outputs. Chain a few together and you can
build almost any EQ curve. TAN's [loudness meter](tan-core/src/loudness.rs)
uses two of them.

**LUFS / K-weighting / BS.1770.** LUFS is the standard unit for *perceived*
loudness (as opposed to raw amplitude), defined by the ITU-R BS.1770
recommendation. K-weighting is the filter that recommendation specifies:
it de-emphasizes bass the ear is less sensitive to and slightly boosts the
presence range, so the resulting number tracks how loud something actually
sounds rather than how big the numbers in the file are. Streaming platforms
(YouTube, Spotify) normalize content to a target LUFS value, which is part of
why TAN needed to stop using a fixed target of its own - see the baseline
section of the README.

**Compressor / limiter / AGC.** A compressor reduces gain once a signal
crosses a threshold, by some ratio. A limiter is a compressor with a very high
ratio, used to enforce a hard ceiling. AGC (automatic gain control) is the
broader term for any system that continuously adjusts gain to reach a target -
TAN's whole engine is an AGC.

**Attack / release.** How fast a compressor reacts. Attack is the speed of
reducing gain when the signal gets loud; release is the speed of letting gain
recover afterward. Too fast and you hear distortion or pumping; too slow and
loud transients get through before the compressor catches up.

**Look-ahead.** Delaying the output by a few milliseconds so the gain
computer can see a loud peak in the *input* before it reaches the *output*,
and start reducing gain in advance instead of reacting after the fact.

**Two-pass / offline processing.** Analyzing an entire file before touching
it, versus processing a live stream sample by sample as it arrives. Offline
mode can plan ahead perfectly (see the whole file, duck ahead of every onset);
live mode can only react.

## Rust and building

**Crate.** Rust's word for a package. A crate can be a library (`lib`) or an
executable (`bin`).

**Workspace.** Several crates that share one build, one dependency lock file,
and one `target/` output directory. TAN's `tan-core`, `tan-cli`, `tan-ffi`,
and `tan-live` are one workspace.

**`cargo`.** Rust's build tool and package manager, roughly npm + webpack for
Rust. `cargo build`, `cargo test`, `cargo run` are the commands you'll see
throughout this project.

**`rustc` vs. `rustup`.** `rustc` is the actual compiler. `rustup` is the tool
that installs and manages `rustc` versions and *toolchains* (see below).
You install `rustup`, and it manages `rustc` for you.

**Toolchain / target triple.** A toolchain is a specific compiler build for a
specific *target* - the combination of CPU architecture, OS, and ABI it
produces code for, written like `x86_64-pc-windows-gnu`. TAN's dev machine
uses the GNU-flavored Windows target rather than the more common MSVC one,
because it avoids needing Visual Studio's Build Tools installed - a real
tradeoff explained below.

**MSVC vs. GNU (on Windows).** Windows has two competing C/C++ toolchain
ecosystems. MSVC is Microsoft's own (comes with Visual Studio); GNU here means
[MinGW-w64](https://www.mingw-w64.org/), a Windows port of the Linux-world GCC
toolchain. Rust can target either. GNU is lighter to install (no multi-GB
Visual Studio download); MSVC is what most native Windows software - and
critically, what a future Windows Audio Processing Object would likely be
written against - expects. TAN's `tan-cli` and `tan-ffi` build fine on GNU;
`tan-live` needed one extra piece (`dlltool`, from MinGW-w64's `binutils`)
that isn't bundled with just the Rust GNU toolchain.

**Ownership / borrowing (`&mut`).** Rust's central idea: the compiler tracks,
at compile time, who is allowed to read or modify a piece of memory, and
proves no two parts of the program can step on each other's data
simultaneously. `Normalizer::process(&mut self, samples: &mut [f32])`
*borrows* the caller's buffer to modify it in place - no copy, no allocation,
and the compiler guarantees nothing else touches that buffer while it's
borrowed. This matters a lot for real-time audio, where allocating memory
inside the callback that feeds your speakers can cause an audible glitch.

**`unsafe`.** Rust code the compiler can't fully verify - usually because
it's doing something like dereferencing a raw pointer handed to it from
outside Rust. TAN keeps all of its `unsafe` code in one file
([`tan-ffi/src/lib.rs`](tan-ffi/src/lib.rs)); the DSP engine itself has none.

## Interop (getting Rust to talk to everything else)

**FFI (Foreign Function Interface).** The general mechanism by which code
written in one language can call code written in another. `tan-ffi` exists so
that JavaScript (in a browser) and, eventually, other native applications can
call into TAN's Rust engine.

**C ABI.** An ABI (Application Binary Interface) is the low-level agreement
about how function calls actually work in memory - how arguments are passed,
how the stack is laid out. Nearly every language and OS on Earth can call
functions that use the C ABI, even though C itself might be nowhere in sight -
it's the closest thing computing has to a universal plug shape. `tan-ffi`
exposes TAN's engine as plain C-ABI functions
(`extern "C"` in Rust, `#[unsafe(no_mangle)]` so the function name isn't
mangled/renamed by the compiler) specifically so anything can call it.

**WebAssembly (wasm).** A compact, portable binary format that browsers (and
some other runtimes) can run at near-native speed. `tan-ffi` compiles to
wasm for the browser and to a native shared library for everything else, from
the exact same source - the C-ABI functions are identical either way, only
the target changes.

**Shared library (`.dll` / `.dylib` / `.so`).** A compiled unit of code that
other programs load and call into at runtime, rather than being copied into
each program that uses it. `.dll` on Windows, `.dylib` on macOS, `.so` on
Linux - three file extensions for the same idea. `tan-ffi` produces one of
these per platform via cargo's `cdylib` crate type.

## Platform audio APIs

**WASAPI (Windows Audio Session API).** The audio API built into Windows.
Anything that plays or records sound on Windows eventually goes through it.

**Loopback capture.** Recording *what a device is currently playing*, rather
than recording from a microphone. This is how `tan-live` gets access to
"whatever's playing on your desktop" to process it - WASAPI supports opening
an output device in a special mode that hands you its outgoing audio as if it
were an input.

**Core Audio.** Apple's equivalent of WASAPI - the low-level audio API
underneath macOS and iOS.

**PipeWire / PulseAudio / ALSA.** Linux's audio stack, roughly low to high
level: ALSA talks directly to sound hardware; PulseAudio (and increasingly its
successor PipeWire) sits above it doing the user-space mixing, routing, and
per-app volume control that makes a desktop usable.

**Audio Processing Object (APO).** Windows' official plug-in mechanism for
inserting a real-time effect directly into the system's audio pipeline - the
same mechanism Windows' own built-in "Loudness Equalization" feature uses.
This is the eventual, proper way to make TAN run system-wide on Windows
without any of the workarounds (loopback capture, virtual cables) this project
currently uses; see the roadmap in the README.

**`cpal`.** The Rust crate `tan-live` uses to talk to WASAPI (and Core Audio,
ALSA/PulseAudio/PipeWire on other platforms) through one common interface,
so the same Rust code can enumerate devices and open audio streams across
operating systems without hand-writing bindings to each platform's native API.

# Installing TAN

Every release is published at
[github.com/bknie1/True-Audio-Normalizer/releases](https://github.com/bknie1/True-Audio-Normalizer/releases).

## Windows

```
irm https://raw.githubusercontent.com/bknie1/True-Audio-Normalizer/main/install.ps1 | iex
```

Installs to `%LOCALAPPDATA%\Programs\TAN` and adds it to your user `PATH`.
Gives you:

- `tan-cli` - process any WAV file (`tan-cli process in.wav out.wav movie --two-pass`)
- `tan-live` - process live desktop audio in real time (see below)
- `tan.dll` - the engine as a native shared library, for embedding in other
  Windows software

### `tan-live`: TAN on whatever you're actually listening to

```
tan-live --list-devices
tan-live --loopback --output "<your headphones or a second output device>"
```

`--loopback` captures whatever your default output device is currently
playing; the result plays out on a *different* device you name with
`--output`. This is a first-cut tool, not the final answer - see the caveat
below.

**Important limitation:** because it needs two separate audio devices (one to
capture from, a different one to play the processed result to), it can't yet
put TAN "in the path" of your one set of speakers or headphones without
routing help. Two ways around that today:

1. If your machine has two distinct output devices (built-in speakers plus a
   monitor's HDMI audio, for example), point `--output` at whichever one you
   aren't using for anything else, and switch your physical listening to it.
2. Install a virtual audio cable (e.g. [VB-Audio Virtual
   Cable](https://vb-audio.com/Cable/), free) as your Windows default output.
   Apps play into the cable; `tan-live --loopback --output "<your real
   headphones>"` captures the cable and forwards the processed result to your
   headphones.

The proper fix - a Windows Audio Processing Object that inserts TAN directly
into the system audio pipeline, no separate devices or virtual cables needed -
is the next major piece of work; see the [README's roadmap](README.md).

## macOS / Linux

```
curl -fsSL https://raw.githubusercontent.com/bknie1/True-Audio-Normalizer/main/install.sh | sh
```

Installs `tan-cli` to `~/.local/bin`. File processing (`process`,
`--two-pass`) works fully; there is no live system-audio tool for macOS/Linux
yet (`tan-live` is Windows-only today - see [GLOSSARY.md](GLOSSARY.md) for why
each OS needs its own audio backend work).

## Android / iOS

No native app yet. Open [bknie1.github.io/True-Audio-Normalizer](https://bknie1.github.io/True-Audio-Normalizer/)
in Chrome (Android) or Safari (iOS) and use the browser's "Add to Home
Screen" / "Install app" option - it runs the same WebAssembly engine as an
installed, full-screen app icon. It handles your own files today; live
system-wide audio on mobile is a further-out roadmap item (iOS in particular
restricts third-party apps from processing other apps' audio at all).

## Building from source

```
git clone https://github.com/bknie1/True-Audio-Normalizer
cd True-Audio-Normalizer
cargo build --release
```

See the [README](README.md) for what each crate does, and
[GLOSSARY.md](GLOSSARY.md) if any of the terminology (FFI, WASAPI, and so on)
is unfamiliar.

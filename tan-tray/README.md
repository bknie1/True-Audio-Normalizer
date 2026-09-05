# tan-tray

TAN in the system tray / menu bar. A small cross-platform (Windows, macOS,
Linux) app around the shared live engine in `tan-live`: toggle TAN on and off,
pick the capture and playback devices, and switch the movie/music profile,
without a terminal.

## What it does

It runs the same real-time pipeline as `tan-live`: capture audio, run it
through TAN, play the result out a chosen device. The tray menu lets you:

- **Enabled** - start/stop processing.
- **Profile** - Movie or Music.
- **Capture** - where the sound comes from. On Windows this lists output
  devices and captures them via WASAPI loopback (what's playing). On macOS and
  Linux there is no loopback, so it lists input devices; point it at a monitor
  or virtual source (a PulseAudio/PipeWire "Monitor of ..." on Linux, or a
  loopback device such as BlackHole on macOS).
- **Output** - which device TAN plays the processed audio to.

To avoid hearing the audio twice, capture and playback should be different
devices (for example capture your speakers and play to headphones), or route
playback through a virtual audio device.

## Building

### Linux
Needs GTK and the app-indicator/xdo dev headers in addition to ALSA:

```
sudo apt-get install -y libasound2-dev libgtk-3-dev libxdo-dev libayatana-appindicator3-dev
cargo build --release -p tan-tray
```

### macOS
```
cargo build --release -p tan-tray
```

### Windows
Builds cleanly on the **MSVC** toolchain (`stable-x86_64-pc-windows-msvc`),
which is what CI uses:

```
cargo build --release -p tan-tray
```

On the **GNU** toolchain (`stable-x86_64-pc-windows-gnu`), the tray/GUI
dependencies use `raw-dylib`, which needs a complete binutils `dlltool` (with
its helper programs) on `PATH`. The toolchain's self-contained `dlltool` is not
sufficient on its own; put a full binutils first on `PATH` (for example
`/c/cygwin64/bin` from a Cygwin install, or an MSYS2 mingw64 bin) before
building, or just build this crate with the MSVC toolchain.

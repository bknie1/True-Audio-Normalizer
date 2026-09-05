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

## Troubleshooting: hearing the original AND TAN's copy

TAN works by *capturing a copy* of what a device is playing (WASAPI loopback)
and playing the processed result out somewhere else. That copy is non-
destructive: the original device keeps right on playing to wherever it was
already headed. If that's the same place TAN's output goes, you hear both,
stacked.

**The fix is always routing, never volume/mute at the Windows level.** Muting
a device's system volume, or the app's session volume in the Windows Volume
Mixer, silences it *before* TAN's loopback capture sees it too - you'd stop
the doubling by also going deaf to the source TAN needs. The real fix is:
make the thing you're capturing stop reaching your ears *some other way*,
while leaving it fully live for TAN to read.

**Mixer/router apps (SteelSeries Sonar, Voicemeeter, Nahimic, similar):**
these already do a version of TAN's job - take an app's audio and mix it into
your headphones. To make TAN the last step instead of a second copy:

1. Point the source app (Stremio, a browser, etc.) at one of the mixer's
   virtual channels - e.g. Sonar's "Media".
2. In tan-tray: **Input** = that same channel (loopback), **Output** = your
   real headphones/speakers.
3. **In the mixer app itself, MUTE that channel - click the mute toggle, do
   not drag its volume slider to 0%.** The slider and the mute button are not
   the same thing: the slider is often just the *level* fed into a bus that's
   already been mixed into headphones, while mute cuts that specific
   contribution to the monitor mix outright, without touching the channel's
   own exposed audio device - which is exactly what TAN is reading via
   loopback. A slider at 0% did NOT stop the doubling; the mute toggle did.

If your mixer doesn't expose a clean per-channel mute (only a slider), look
for a "streamer mode" / "monitor" split, or route that channel to output
hardware you don't have speakers connected to.

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

# Jellyfin.Plugin.Tan

TAN for Jellyfin. This plugin does not run any DSP itself - it's a thin
client of a local [`tan-server`](../tan-server) instance, which does the
actual audio processing. Keeping the DSP out of the plugin means it isn't
reimplemented in C#, and the same server can back other integrations later
(a Stremio-facing proxy, anything else).

## What it does

A manual-run **Scheduled Task** ("Normalize Audio with TAN", under Dashboard
→ Scheduled Tasks → TAN) that:

1. Walks your library for movies, episodes, audio tracks, and music videos.
2. For each one, extracts its audio to WAV using Jellyfin's own bundled
   ffmpeg (`IMediaEncoder`) and POSTs it to `tan-server`'s `/normalize`
   endpoint.
3. For a **video** item, muxes the TAN-processed audio back with the
   *original video stream, copied verbatim* (no re-encode) into a new
   `<name> [TAN].mkv`. For an **audio-only** item, the processed WAV is
   written directly as `<name> [TAN].wav`.
4. Writes it next to the source file (or into a configured output folder).
   **The original is never modified or replaced.** An existing output is
   left alone on later runs; delete it to force a redo.

A **media source provider** (`TanMediaSourceProvider`) then offers that file
to Jellyfin itself: if a normalized version exists for an item, it appears
as an additional source named "TAN Normalized" in the client's **Play
Version** picker, right alongside the original - Jellyfin, not the plugin,
serves it to the player from there.

Configure the `tan-server` URL, TAN profile, and output folder from the
plugin's settings page in the Jellyfin admin dashboard (Dashboard → Plugins
→ TAN).

## Verified vs. not (being honest about it)

- **Compiles clean** against the real `Jellyfin.Controller` 10.11.11 NuGet
  package (`dotnet build`, 0 warnings, 0 errors, Debug and Release) - every
  API used here (`BasePlugin<T>`, `IHasWebPages`, `IScheduledTask`,
  `IMediaSourceProvider`, `IPluginServiceRegistrator`, `ILibraryManager`,
  `IMediaEncoder`, `MediaSourceInfo`, ...) was confirmed against the actual
  installed SDK assembly via a throwaway reflection probe before writing a
  line of it - not written from memory and hoped to be right.
- **Not yet run inside an actual Jellyfin server.** This environment has no
  running Jellyfin instance to load the plugin into, run the task against a
  real library, and confirm "TAN Normalized" actually appears and plays
  correctly in a client's Play Version picker - that's the real test, on
  your end (or your friend's).

## Building

```
dotnet build -c Release
```

## Installing (manual, no plugin repository yet)

1. Build the DLL above.
2. Copy `bin/Release/net9.0/Jellyfin.Plugin.Tan.dll` into a new folder under
   Jellyfin's plugins directory, e.g.
   `<jellyfin-data>/plugins/Tan_1.0.0.0/Jellyfin.Plugin.Tan.dll`.
3. Restart Jellyfin.
4. Dashboard → Plugins → TAN to configure the `tan-server` URL and profile.
5. Make sure `tan-server` is actually running and reachable from wherever
   Jellyfin's server process runs (same machine, by default, at
   `http://127.0.0.1:5859`).
6. Dashboard → Scheduled Tasks → TAN → "Normalize Audio with TAN" → run it.
7. Play the item; look for "TAN Normalized" in the client's Play Version
   (or audio track) picker.

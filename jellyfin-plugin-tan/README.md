# Jellyfin.Plugin.Tan

TAN for Jellyfin. This plugin does not run any DSP itself - it's a thin
client of a local [`tan-server`](../tan-server) instance, which does the
actual audio processing. Keeping the DSP out of the plugin means it isn't
reimplemented in C#, and the same server can back other integrations later
(a Stremio-facing proxy, anything else).

## What it does right now

A manual-run **Scheduled Task** ("Normalize Audio with TAN", under Dashboard
→ Scheduled Tasks → TAN) that:

1. Walks your library for movies, episodes, audio tracks, and music videos.
2. For each one, extracts its audio to WAV using Jellyfin's own bundled
   ffmpeg (`IMediaEncoder`).
3. POSTs that WAV to `tan-server`'s `/normalize` endpoint.
4. Writes the result as `<name> [TAN].wav` next to the source file (or into
   a configured output folder) - **the original is never modified or
   replaced.** An existing output is left alone on later runs; delete it to
   force a redo.

Configure the `tan-server` URL, TAN profile, and output folder from the
plugin's settings page in the Jellyfin admin dashboard (Dashboard → Plugins
→ TAN).

## What it does not do yet

The output file is not automatically wired up as a selectable alternate
audio track in Jellyfin's player. That needs deeper integration with
Jellyfin's media source / library metadata (registering the file as an
alternate version Jellyfin's player can pick), which is real, separate work
on top of this - this task is the piece that proves and runs the actual DSP
pipeline end to end; wiring its output into playback selection is the next
step, not yet done.

## Verified vs. not (being honest about it)

- **Compiles clean** against the real `Jellyfin.Controller` 10.11.11 NuGet
  package (`dotnet build`, 0 warnings, 0 errors) - every API used here
  (`BasePlugin<T>`, `IHasWebPages`, `IScheduledTask`, `ILibraryManager`,
  `IMediaEncoder`, etc.) was confirmed against the actual installed SDK
  assembly, not written from memory and hoped to be right.
- **Not yet run inside an actual Jellyfin server.** This environment has no
  running Jellyfin instance to load the plugin into, click through the admin
  UI, or execute the task against a real library - that's the real next
  test, on your end.

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

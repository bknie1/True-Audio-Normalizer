# Roadmap

A working sketch of where TAN goes next, written down because the answer got
asked directly. Not a commitment to dates - things move when they're ready.

## Just shipped

- **Feedback-loop guard** (tan-live). The most common way to break TAN was
  loopback-capturing a device and also sending the processed output back to
  that same device - including the out-of-the-box default of leaving both
  Input and Output on "System default." `start()` now refuses that
  combination outright with an explanation, instead of letting it play out
  as doubled, looping audio. Zero tan-tray changes needed; its existing
  error-surfacing already covers this.
- **tan-stremio.** A Stremio addon offering a "TAN Normalized" stream for a
  local library, the same "extra Play Version" idea as
  jellyfin-plugin-tan - scan a folder, normalize each file once via ffmpeg +
  tan-core, serve the result back into Stremio's stream picker. Plain Rust
  linking tan-core directly, no separate tan-server process needed. Compiles
  clean with full test coverage of the addon logic itself; the actual
  ffmpeg pipeline and a real Stremio client still need to be exercised
  end to end on a machine that has both.

## Next few weeks

**1. Guide the user past every other bad combination, not just the worst one.**
The feedback-loop guard catches the single most damaging case. The tray
still doesn't explain *why* a given Input/Output pairing is a bad idea
before the user picks it. Plan: surface a short compatibility hint next to
the device pickers (sample rate/channel mismatch is fine and handled, but
"this output routes back into your input" should be visible before you hit
Apply, not just after it fails).

**2. Open, optional loudness metadata as a hint - never a requirement.**
ReplayGain tags and EBU R128 loudness metadata are open, well-documented
standards, unlike Dolby's proprietary mastering-time metadata. Reading them
when present doesn't compromise the "works blind" design - it just gives
the engine a better starting baseline before its own real-time analysis
takes over. Scope: read the tag if it exists (ID3 ReplayGain, R128 loudness
atoms), use it to seed the baseline, fall back to today's cold-start
behavior when it's absent. Never require it, never write proprietary tags
of TAN's own.

**3. A real Settings window, not just a tray menu.**
The tray's flat menu doesn't have room for what people actually want to
tune: gain speeds, gate threshold, per-profile overrides. Scope for this
pass: a small native window (still no GUI framework beyond what tan-tray
already pulls in) with sliders/fields for the parameters `tan-core`
already exposes, live-previewed against the running engine. Not in scope
yet: a full custom-algorithm editor - that's bigger than a few weeks and
depends on this window existing first.

## After that

- **Custom algorithm authoring.** Once the Settings window can tune
  existing parameters, the next step is letting someone save a named
  combination of them (a "custom profile") rather than editing raw numbers
  every time. A visual node/graph editor for genuinely new signal chains is
  further out and only worth it if tuning-and-saving turns out not to be
  enough.
- **Cloud-synced settings.** Cross-platform profile sync is straightforward
  once profiles are just data (see above) - the open question is where they
  live, not the sync mechanism itself. Deferred until there's more than one
  platform's tray app to sync between.
- **TAN's own virtual audio driver.** The endgame for Windows: a signed
  virtual audio device so TAN sits system-wide instead of requiring a
  loopback + second output device dance. Gated on code-signing, which is
  its own separate task outside the engine work.

## Explicitly not now

- A visual algorithm-graph editor (see above - premature before simple
  tuning exists).
- Anything that requires proprietary mastering-time metadata to work at
  all. TAN keeps working blind by default; standards-based metadata is
  always a hint, never a dependency.

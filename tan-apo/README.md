# tan-apo — TAN as a Windows Audio Processing Object

This is the "TAN lives in the audio pipeline" path: a **System Effect APO**
that Windows inserts into an audio endpoint's stream, running TAN's leveling on
every sample the endpoint plays — no loopback, no manual routing, and it
**coexists with SteelSeries Sonar** because it's a peer effect, not a takeover.

## Why this shape (confirmed from a real machine)

Windows has a sanctioned way to add DSP to audio without replacing anyone's
driver, and Brandon's PC already runs it: **Waves MaxxAudio** processes the
Realtek output as an APO layered on via an *Extension INF*
(`dellaudioextwaves.inf`, class `Extension`) plus an APO package
(`wavesapo12de.inf`, class `AudioProcessingObject`). The Realtek base driver is
untouched. Realtek, Intel, and Waves all ship APOs through the
`AudioProcessingObject` device class (GUID
`{5989fce8-9cd0-467d-8a6a-5419e31529d4}`).

SteelSeries Sonar, meanwhile, stands up its **own virtual audio devices** and
coexists as a peer (its only PnP driver is HID). So two clean paths exist:

- **Phase 1 (this scaffold): APO + Extension INF on an existing endpoint.**
  Attach the TAN APO to a real endpoint that has a PnP hardware id we can extend
  (the Realtek output is the obvious dev target). Least code, fastest to a
  working in-pipeline TAN, coexists with everything. Endpoint-specific.
- **Phase 2 (later): TAN's own virtual audio device** (SYSVAD sample as the base
  driver) with the same APO attached to it — universal, set-TAN-as-your-output.
  Much more driver code; do it once Phase 1 proves the APO + DSP path.

## The DSP is already done

The APO does **not** reimplement any DSP. It links `tan.lib` (the static build
of `tan-ffi`, already produced by `cargo build -p tan-ffi`) and calls the
streaming C ABI:

```c
Normalizer* tan_normalizer_new(uint32_t sample_rate, uint32_t channels, uint32_t profile_id);
void        tan_normalizer_process(Normalizer* h, float* interleaved, size_t len); // in place
void        tan_normalizer_free(Normalizer* h);
```

`profile_id`: 0 = movie, 1 = music (see `tan-ffi/src/lib.rs`). `TanDsp`
(`src/TanDsp.h/.cpp`) is a tiny RAII wrapper around these three calls; that's
the only TAN-specific code the APO needs.

### Real-time caveat

An APO's `APOProcess` runs on the audio engine's real-time thread: **no
allocations, no locks, no blocking**. `Normalizer::process` is in-place and
allocation-free in steady state, but the look-ahead limiter's ring can grow on
the very first blocks. For a dev/test build this is fine (an occasional first-
block glitch); before shipping, pre-size the limiter and audit the process path
for the RT thread. Tracked, not solved.

## Building it (once the WDK is installed)

Prereqs (see `scripts/`): VS 2022 + "Desktop development with C++" workload,
the Windows 11 SDK, and the **WDK** (adds driver project templates + the WDK VS
extension). Then:

1. `cargo build -p tan-ffi --release` → `target/release/tan.lib`.
2. Build the APO DLL (a WDK "System Audio Processing Object" project) linking
   `tan.lib`. The fastest route is to start from the **WDK SwapAPO sample**
   (github.com/microsoft/Windows-driver-samples → `audio/sysvad/APO` /
   `SwapAPO`) and graft in the TAN integration:
   - Replace the sample's per-sample swap in `CSwapAPOSFX::APOProcess` with a
     single `dsp->process(pf32Buffer, frameCount * channels)` call (see
     `src/TanApoProcessing.cpp` for the exact body).
   - Create the `TanDsp` in `LockForProcess` (you have the format there:
     sample rate + channel count), destroy it in `UnlockForProcess`.
   - Keep the sample's COM/registration boilerplate; only the CLSIDs, names,
     and the process body change.
3. Test-sign and install with `scripts/install-apo.ps1` (test-signing mode must
   be on: `bcdedit /set testsigning on`, then reboot).

## Coexistence with SteelSeries

The TAN APO attaches to a physical endpoint (e.g. Realtek). Route your apps
through Sonar as you do today; whatever Sonar sends to that endpoint gets TAN'd
on the way out. TAN is an effect on the endpoint, not a replacement for Sonar,
so Sonar's mixing/virtualization keeps working. (Phase 2's virtual device lets
you instead set "TAN" as the output directly.)

## Files

- `tan_ffi.h` — C declarations for the three streaming calls.
- `src/TanDsp.h` / `src/TanDsp.cpp` — RAII wrapper around the TAN engine.
- `src/TanApoProcessing.cpp` — the exact `APOProcess` / lock / unlock bodies to
  graft into the SwapAPO-derived class.
- `inf/tan-apo.inf` — APO registration INF (skeleton; complete against the
  SwapAPO sample's INF for your endpoint's hardware id).
- `scripts/install-apo.ps1` / `uninstall-apo.ps1` — dev test-sign + install.

This scaffold will need a build round or two once the WDK is in — the COM/INF
boilerplate is best finalized against the live SwapAPO sample — but the
TAN-specific pieces (DSP wrapper, process body, ABI, install flow) are here.

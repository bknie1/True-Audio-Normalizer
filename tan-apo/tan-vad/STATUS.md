# tan-vad status / handoff

Snapshot of the TAN virtual audio device work as of 2026-09-07.

## Done (autonomous, verified)

- Forked the WDK SimpleAudioSample virtual-audio WDM driver into `tan-vad/`.
- Renamed the **deployment identity** to TanVad: binary `TanVad.sys`, service
  `TanVad`, hardware id `Root\TanVad`, catalog `TanVad.cat`. Display strings
  show the endpoint as **TAN** in the Windows playback list.
- Builds a signed package with 64-bit MSBuild:
  `x64\Release\package\` = `TanVad.sys` + signed `tanvad.cat` + `TanVad.inf`.
  (Build via `scripts\build-tan-vad.ps1`.)
- `infverif` reports the INF **VALID**.
- Dev signing is **staged**: a persistent code-signing cert `CN=TAN Dev Test
  Cert` exists in LocalMachine\My and is trusted in Root + TrustedPublisher,
  and `TanVad.sys` / `tanvad.cat` are signed with it.

## BLOCKED — needs you (firmware) or a real cert

**Secure Boot is ON** on this machine (`UEFISecureBootEnabled=1`). Windows
refuses to enable test-signing while Secure Boot is on, and a self-signed
kernel driver cannot load without test-signing. This is a firmware setting I
cannot change. Two ways forward:

1. **Dev route:** disable Secure Boot in UEFI/BIOS, then run
   `scripts\tan-vad-install.ps1` elevated. It will enable test-signing and tell
   you to reboot; after reboot, run it once more and it creates the "TAN"
   device with devcon. (Cert + signatures are already in place.)
2. **Shipping route:** get the driver signed via Microsoft Partner Center
   (attestation/WHQL). Then no Secure Boot change or test-signing is needed.
   Requires the hardware-dev account, so it is your call.

## Remaining after install (the other machine)

Once "TAN" appears as a playback device, the capture -> process -> forward path
is `tan-live` (maintained on the other machine, per the kickoff):

```
tan-live --loopback-from "TAN" --output "<your real output>"
```

The "disable filtering = forward raw" toggle you asked for is a small bypass
flag in the `tan-live` engine. That wiring is coordinated on that machine, not
from here.

## Follow-ups (nice-to-have)

- Rename internal C++ identifiers (`SimpleAudioSample` -> TanVad); cosmetic.
- Decide whether the DSP stays in `tan-live` (current plan) or moves into an
  APO attached to this virtual device.
- Optionally strip the capture (mic) endpoint to make TAN render-only.

# tan-vad — TAN as a selectable virtual audio device (Phase 2)

This is the "select TAN as your playback device, it processes, then forwards to
your real output" path — the shipping target. It is a **virtual audio render
endpoint** named **TAN**: apps play to it, and the `tan-live` loopback engine
captures it, runs TAN's leveling (with an on/off bypass for instant A/B), and
renders the result to your chosen physical output (Arctis, Sonar, speakers).

It is a fork of the WDK **SimpleAudioSample** virtual-audio WDM driver
(Windows-driver-samples/audio/simpleaudiosample, MIT). The deployment identity
is renamed to **TanVad** — binary `TanVad.sys`, service `TanVad`, hardware id
`Root\TanVad`, catalog `TanVad.cat` — and the display strings show the device
as **TAN** in the Windows playback list. Internal C++ identifiers still carry
the `SimpleAudioSample` name; renaming those is cosmetic and deferred.

## Why a virtual device (not the APO)

An APO is glued to one existing endpoint and processes in place; it cannot
present a selectable device nor forward audio elsewhere. The virtual device +
`tan-live` forwarding is the only shape that gives "pick TAN, hear it on my real
output." The DSP (`tan-core` / `tan.lib`) is shared with the APO work, so the
engine investment carries over.

## Build

Use the **64-bit** MSBuild (the WDK 26100 InfVerif task ships x64 only; 32-bit
MSBuild fails to load `x86\InfVerif.dll`):

```
& "D:\Microsoft Visual Studio\2022\Community\MSBuild\Current\Bin\amd64\MSBuild.exe" `
    tan-vad\SimpleAudioSample.sln -p:Configuration=Release -p:Platform=x64 -m
```

Output package (`x64\Release\package\`): `TanVad.sys`, a signed `tanvad.cat`,
and `TanVad.inf` (DeviceDesc "TAN (True Audio Normalizer)", speaker friendly
name "TAN", hardware id `Root\TanVad`).

## Install (dev)

Kernel driver: needs a trusted signature **and** test-signing on, so the first
install requires a reboot. Run `scripts\tan-vad-install.ps1` elevated:
1. creates/trusts a persistent dev code-signing cert,
2. signs the `.sys` and `.cat`,
3. enables test-signing (reboot, then re-run to finish),
4. creates the root device with `devcon`.

`scripts\tan-vad-uninstall.ps1` removes the device and driver package.

## Forwarding + enable/disable (tan-live)

Once "TAN" exists, forward it to your real output with the existing loopback
engine:

```
tan-live --loopback-from "TAN" --output "<your real output device>"
```

`tan-live` already does capture -> TAN process -> render-to-another-device. The
"disable filtering = forward raw" toggle is a small bypass flag in that engine.
`tan-live` is maintained on the other machine, so that wiring is coordinated
there, not edited from here.

## Remaining work

- Full identifier rename (binary/service/hardware-id -> TAN) and its own
  catalog/INF names.
- Decide whether processing stays in `tan-live` (user mode, current plan) or
  moves into an APO attached to this virtual device.
- A real code-signing cert to drop the test-signing/DisableProtectedAudioDG dev
  requirements for shipping.

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

## Reality check: the kernel driver is the wrong vehicle for daily use

Installing our own kernel driver on a normal machine requires EITHER disabling
Secure Boot (a security downgrade, not a reasonable ask) OR a Partner Center EV
signature (~$300/yr; Azure Trusted/Artifact Signing at ~$10/mo explicitly does
NOT sign drivers). So tan-vad is parked as the eventual first-party premium
package, not the path to using TAN now.

## WORKING path (done): user-mode tan-live + VB-CABLE ($0, Secure Boot ON)

Installed and verified end-to-end on this machine (2026-09-07):
- **VB-CABLE** (free, Microsoft-WHCP-signed) installed - "CABLE Input/Output"
  render+capture endpoints present and OK, Secure Boot left ON.
- `tan-live` (built here, release) runs the full chain:
  `CABLE Input (loopback) -> TAN [Movie] -> Headphones (Arctis), 48 kHz/2 ch/~200 ms`
  with no errors.

**How to use it:** set any app's playback device to **"CABLE Input"**, then run
`scripts\tan-forward.ps1` (captures CABLE Output -> TAN DSP -> your Arctis).
Enable/disable filtering = start/stop that script (a true in-engine raw-passthrough
toggle is a small tan-live change, on the other machine). `-Profile` picks the
sound (universal|movie|music|speech|night|game).

This is the recommended way to run TAN. No kernel driver, no Secure Boot change,
no signing cost - the virtual device is already Microsoft-signed.

## Parked: Partner Center attestation signing (only if distributing tan-vad)

Decision (2026-09-07): keep Secure Boot on and get the driver Microsoft-signed
via Partner Center attestation, so it installs on this (and any) machine with no
test-signing and no firmware change.

Prepared here (turn-key):
- `scripts\package-for-attestation.ps1` builds `dist\TanVad.cab` (the driver
  package under a `TanVad\` folder) - already built and verified.
- `scripts\sign-attestation-cab.ps1 -Thumbprint <EV cert>` signs that cab with
  your EV code-signing cert (prompts for the token PIN - only you can enter it).

Needs you (identity/account-bound, cannot be automated):
1. A **Microsoft Partner Center Hardware** account, established with an **EV
   code-signing certificate** (hardware token).
2. Sign the cab: `sign-attestation-cab.ps1 -Thumbprint <your EV cert thumbprint>`.
3. Upload the signed `dist\TanVad.cab` at
   https://partner.microsoft.com/dashboard/hardware -> new hardware submission
   -> choose **attestation**, select the target OS versions.
4. Download the MS-signed package Partner Center returns.
5. Install it: `pnputil /add-driver <signed>\TanVad.inf /install` (or devcon
   install with `Root\TanVad`), then `tan-vad-verify.ps1`. No test-signing, no
   reboot-to-enable-testsigning needed.

The earlier dev route (disable Secure Boot + test-signing) is still available
via `tan-vad-install.ps1` if you ever want a quick local test; the dev cert and
signed .sys/.cat from that path remain staged.

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

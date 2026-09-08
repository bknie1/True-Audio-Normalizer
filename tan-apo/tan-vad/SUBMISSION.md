# tan-vad: Partner Center attestation submission runbook (Option B)

The turn-key steps to get the TAN virtual audio driver Microsoft-signed so it
installs on any Windows 10/11 machine with Secure Boot ON (no test-signing).
Do this only when shipping the branded first-party "TAN" device is worth the
cost; the user-mode path (VB-CABLE + tan-live, see STATUS.md / tan-setup.ps1)
ships today for ~$0.

## One-time prerequisites (yours; account/identity-bound)

1. **EV code-signing certificate** (~$250-500/yr, e.g. DigiCert/Sectigo/SSL.com),
   delivered on a FIPS hardware token. Required to register the hardware
   dashboard and to sign the submission. (Azure Trusted/Artifact Signing is
   cheaper but does NOT sign drivers - confirmed, do not go down that path for
   this.)
2. **Microsoft Partner Center - Hardware program** enrollment, validated with
   that EV cert: https://partner.microsoft.com/dashboard/hardware

## Each release

1. **Build** the driver (produces the package):
   ```
   powershell -File scripts\build-tan-vad.ps1 -Clean
   ```
   -> `x64\Release\package\` = TanVad.sys, TanVad.inf, tanvad.cat.

2. **Package** the submission cab:
   ```
   powershell -File scripts\package-for-attestation.ps1
   ```
   -> `dist\TanVad.cab` (driver files under a `TanVad\` folder). Verified valid
   by infverif.

3. **Sign the cab** with your EV cert (token PIN prompt is expected):
   ```
   powershell -File scripts\sign-attestation-cab.ps1 -Thumbprint <EV-cert-thumbprint>
   ```

4. **Submit** at Partner Center -> Hardware -> "Submit new hardware":
   - Upload the signed `dist\TanVad.cab`.
   - Choose **Attestation signing** (not full WHQL/HLK - attestation needs no
     HLK test run).
   - Select target OS: Windows 11 x64 (and Windows 10 x64 if desired).
   - Submit; Microsoft returns a **signed** package (usually minutes).

5. **Download** the MS-signed package and install/ship it:
   ```
   pnputil /add-driver <signed>\TanVad.inf /install
   ```
   or create the root device with devcon (`Root\TanVad`). No test-signing, no
   Secure Boot change. Verify with `scripts\tan-vad-verify.ps1`.

## Distribution notes

- The end-user installer/GUI (wrapping install + the tan-live forwarding + the
  enable/disable toggle) lives with `tan-tray` / `packaging/` on the other
  machine; this repo dir provides the signed driver package it consumes.
- Ship `tan-live`/`tan-tray` signed too (Azure Trusted/Artifact Signing ~$10/mo
  is fine for the user-mode .exe and avoids SmartScreen; it just can't sign the
  driver).
- Even with the signed TAN device, forwarding to the user's real output is the
  same tan-live loopback path; the driver only replaces "install VB-CABLE" with
  a branded "TAN" endpoint.

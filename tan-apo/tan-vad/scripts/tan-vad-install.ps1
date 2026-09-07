<#
  tan-vad-install.ps1 - dev install for the TAN virtual audio device driver.

  This is Phase 2: a virtual render endpoint that shows up in the Windows
  playback list as "TAN". Apps play to it; the tan-live loopback engine then
  captures "TAN", runs the DSP (with an enable/disable bypass), and renders to
  your real output device. This script only installs the DEVICE; the
  capture->process->forward wiring is tan-live (see ..\README.md).

  A kernel driver must be signed by a trusted cert AND test-signing must be on
  (self-signed packages are not MS-cross-signed). Steps:
    1. Create/trust a persistent dev code-signing cert.
    2. Sign the .sys and .cat with it.
    3. Enable test-signing (needs a REBOOT the first time).
    4. Create the root virtual device with devcon.

  Run ELEVATED. If test-signing was just turned on, the script tells you to
  reboot and re-run; it is safe to re-run (idempotent).
#>
param(
    [string]$PackageDir = "$PSScriptRoot\..\x64\Release\package",
    [string]$CertCN = 'TAN Dev Test Cert'
)
$ErrorActionPreference = 'Stop'

function Require-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole(
            [Security.Principal.WindowsBuiltinRole]::Administrator)) {
        throw 'Run this in an elevated (Administrator) PowerShell.'
    }
}
Require-Admin

$sys = Join-Path $PackageDir 'TanVad.sys'
$cat = Join-Path $PackageDir 'tanvad.cat'
$inf = Join-Path $PackageDir 'TanVad.inf'
foreach ($f in $sys,$cat,$inf) { if (-not (Test-Path $f)) { throw "Missing $f - build tan-vad first (msbuild SimpleAudioSample.sln, Release x64, 64-bit MSBuild)." } }

$kitBin = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64'
$signtool = Join-Path $kitBin 'signtool.exe'
$devcon = 'C:\Program Files (x86)\Windows Kits\10\Tools\10.0.26100.0\x64\devcon.exe'

# 1. Persistent dev cert, trusted as Root + TrustedPublisher.
$cert = Get-ChildItem Cert:\LocalMachine\My | Where-Object { $_.Subject -eq "CN=$CertCN" } | Select-Object -First 1
if (-not $cert) {
    Write-Host "Creating dev cert '$CertCN'..."
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=$CertCN" `
        -CertStoreLocation Cert:\LocalMachine\My -KeyUsage DigitalSignature `
        -KeyExportPolicy Exportable -NotAfter (Get-Date).AddYears(5)
    $tmp = "$env:TEMP\tan-vad-devcert.cer"
    Export-Certificate -Cert $cert -FilePath $tmp | Out-Null
    Import-Certificate -FilePath $tmp -CertStoreLocation Cert:\LocalMachine\Root | Out-Null
    Import-Certificate -FilePath $tmp -CertStoreLocation Cert:\LocalMachine\TrustedPublisher | Out-Null
    Remove-Item $tmp -Force
}
$thumb = $cert.Thumbprint

# 2. Sign the driver + catalog.
Write-Host 'Signing .sys and .cat...'
& $signtool sign /sha1 $thumb /fd SHA256 $sys
& $signtool sign /sha1 $thumb /fd SHA256 $cat

# 3. Test-signing must be on for a self-signed KMDF package.
$ts = (bcdedit /enum '{current}' | Select-String 'testsigning\s+Yes')
if (-not $ts) {
    # Secure Boot blocks enabling test-signing; that must be turned off in UEFI first.
    $sb = $false
    try { $sb = Confirm-SecureBootUEFI } catch {}
    if ($sb) {
        Write-Host 'Secure Boot is ON, which blocks test-signing. Disable Secure Boot in UEFI/BIOS, then re-run this script.' -ForegroundColor Red
        Write-Host 'The cert is created and the driver is signed; only test-signing + the device install remain.' -ForegroundColor Yellow
        return
    }
    bcdedit /set testsigning on | Out-Null
    Write-Host 'Enabled test-signing. REBOOT now, then re-run this script to finish the install.' -ForegroundColor Yellow
    return
}

# 4. Create the root virtual device (idempotent: remove any prior instance first).
Write-Host 'Installing the TAN virtual audio device...'
& $devcon remove "ROOT\TanVad" 2>$null | Out-Null
& $devcon install "$inf" "Root\TanVad"
Write-Host 'Done. "TAN" should now appear in the Windows playback device list.' -ForegroundColor Green
Write-Host 'Next: run tan-live to forward it, e.g.:' -ForegroundColor DarkGray
Write-Host '  tan-live --loopback-from "TAN" --output "<your real output>"' -ForegroundColor DarkGray

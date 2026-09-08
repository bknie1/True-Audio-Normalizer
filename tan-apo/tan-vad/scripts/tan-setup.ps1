<#
  tan-setup.ps1 - first-run setup + launcher for the user-mode TAN path
  (the ship-to-others Option A: TAN app + a free, Microsoft-signed virtual cable,
  works on any Secure Boot machine, no kernel driver from us).

  What it does:
    1. Checks for a virtual audio cable (VB-CABLE). If missing and you pass
       -InstallCable, it downloads VB-CABLE from vb-audio.com, verifies its
       Microsoft/VB-Audio signatures, and launches the vendor installer.
    2. Starts TAN forwarding: captures the cable via loopback, runs the DSP, and
       plays to your default output (or -Output). Set the apps you want
       normalized to play to "CABLE Input"; leave everything else alone.

  Enable/disable filtering = start/stop this (Ctrl+C). -Profile picks the sound.

  NOTE: VB-CABLE is free for users to install themselves; bundling it in a
  redistributed product needs a license from VB-Audio. A shipped TAN installer
  should guide users to install it (as here), not embed it, unless licensed.
#>
param(
    [switch]$InstallCable,
    [string]$Output,                       # real output device; default = system default
    [string]$Profile = 'universal',
    [int]$LatencyMs = 200,
    [switch]$StatusOnly
)
$ErrorActionPreference = 'Stop'

$CableCapture = 'CABLE Output'   # the loopback source apps feed via "CABLE Input"
$tanLive = Join-Path $PSScriptRoot '..\..\..\target\release\tan-live.exe'

function Test-Cable {
    [bool](Get-PnpDevice -Class MEDIA -EA SilentlyContinue |
        Where-Object { $_.FriendlyName -like '*VB-Audio Virtual Cable*' -and $_.Status -eq 'OK' })
}

function Install-Cable {
    $url = 'https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip'
    $tmp = Join-Path $env:TEMP ("tan-vbcable-" + [guid]::NewGuid().ToString('N').Substring(0,8))
    New-Item -ItemType Directory -Force $tmp | Out-Null
    $zip = Join-Path $tmp 'VBCABLE.zip'
    Write-Host "Downloading VB-CABLE (free, Microsoft-signed)..."
    Invoke-WebRequest -Uri $url -OutFile $zip
    Expand-Archive $zip -DestinationPath $tmp -Force
    $setup = Join-Path $tmp 'VBCABLE_Setup_x64.exe'
    # Verify signatures before executing.
    $sig = Get-AuthenticodeSignature $setup
    if ($sig.Status -ne 'Valid') { throw "VB-CABLE setup signature not valid ($($sig.Status)) - aborting." }
    Write-Host "Signature OK: $($sig.SignerCertificate.Subject.Split(',')[0])"
    Write-Host "Launching the VB-CABLE installer. Click 'Install Driver', accept the prompt, and reboot if asked." -ForegroundColor Yellow
    Start-Process -Verb RunAs -FilePath $setup
    Write-Host "Re-run this script (without -InstallCable) once the cable is installed."
}

if ($StatusOnly) {
    Write-Host ("Virtual cable present : {0}" -f $(if (Test-Cable) {'yes'} else {'no'}))
    Write-Host ("tan-live built        : {0}" -f $(if (Test-Path $tanLive) {'yes'} else {'no (cargo build -p tan-live --release)'}))
    return
}

if (-not (Test-Cable)) {
    if ($InstallCable) { Install-Cable; return }
    Write-Host "No virtual cable found. Re-run with -InstallCable to download + install the free VB-CABLE," -ForegroundColor Yellow
    Write-Host "or install it yourself from https://vb-audio.com/Cable/ then re-run this script."
    return
}

if (-not (Test-Path $tanLive)) { throw "tan-live not built. Run: cargo build -p tan-live --release" }

$args = @('--loopback-from', $CableCapture, '--profile', $Profile, '--latency-ms', $LatencyMs)
if ($Output) { $args += @('--output', $Output) }
Write-Host "TAN is ON. Set an app's playback device to 'CABLE Input' to normalize it." -ForegroundColor Green
Write-Host "Profile: $Profile. Output: $(if ($Output) {$Output} else {'system default'}). Ctrl+C to turn TAN off (raw)."
& $tanLive @args

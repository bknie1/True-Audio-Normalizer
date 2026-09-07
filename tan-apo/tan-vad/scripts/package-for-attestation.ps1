<#
  package-for-attestation.ps1 - build the submission .cab for Microsoft Partner
  Center attestation signing (the route that installs on Secure Boot machines
  with no test-signing).

  Produces dist\TanVad.cab containing the driver package under a "TanVad\"
  folder (Partner Center preserves the cab's folder layout). This script does
  NOT sign the cab - signing needs your EV code-signing cert; use
  sign-attestation-cab.ps1 for that once you have it. Run from anywhere.
#>
param(
    [string]$PackageDir = "$PSScriptRoot\..\x64\Release\package",
    [string]$OutDir     = "$PSScriptRoot\..\dist"
)
$ErrorActionPreference = 'Stop'

$files = 'TanVad.sys','TanVad.inf','tanvad.cat'
foreach ($f in $files) { if (-not (Test-Path (Join-Path $PackageDir $f))) { throw "Missing $f in $PackageDir - build tan-vad first." } }

New-Item -ItemType Directory -Force $OutDir | Out-Null
$ddf = Join-Path $OutDir 'TanVad.ddf'
$cab = Join-Path $OutDir 'TanVad.cab'
$pkgFull = (Resolve-Path $PackageDir).Path

# DDF: place the three files under a "TanVad" folder inside the cab.
@"
.OPTION EXPLICIT
.Set CabinetNameTemplate=TanVad.cab
.Set DiskDirectoryTemplate=$OutDir
.Set CompressionType=MSZIP
.Set Cabinet=on
.Set Compress=on
.Set DestinationDir=TanVad
"$pkgFull\TanVad.sys"
"$pkgFull\TanVad.inf"
"$pkgFull\tanvad.cat"
"@ | Set-Content -Path $ddf -Encoding ASCII

Push-Location $OutDir
try { makecab /f $ddf | Out-Null } finally { Pop-Location }
Remove-Item (Join-Path $OutDir 'setup.inf'),(Join-Path $OutDir 'setup.rpt') -EA SilentlyContinue

if (Test-Path $cab) {
    Write-Host "Built $cab ($([math]::Round((Get-Item $cab).Length/1KB,1)) KB)" -ForegroundColor Green
    Write-Host 'Next: sign it with your EV cert (sign-attestation-cab.ps1), then upload at'
    Write-Host '  https://partner.microsoft.com/dashboard/hardware  ->  Submit new hardware  ->  attestation.'
} else { throw 'makecab did not produce TanVad.cab' }

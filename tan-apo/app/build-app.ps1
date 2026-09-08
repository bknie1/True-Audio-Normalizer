# Builds the TAN Control GUI and the installer (user-mode path). Free tools only.
#   1. compiles TanControl.exe with the in-box .NET Framework csc (no SDK needed)
#   2. ensures tan-live.exe is built (static CRT, so it's self-contained)
#   3. compiles dist\TAN-Setup.exe with Inno Setup (install via:
#      winget install --id JRSoftware.InnoSetup -e)
param([switch]$SkipEngine)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$repo = (Resolve-Path (Join-Path $here '..\..')).Path

# 1. GUI
$csc = "$env:WINDIR\Microsoft.NET\Framework64\v4.0.30319\csc.exe"
& $csc /nologo /target:winexe /out:"$here\TanControl.exe" /reference:System.Windows.Forms.dll /reference:System.Drawing.dll "$here\TanControl.cs"
if (-not (Test-Path "$here\TanControl.exe")) { throw "TanControl.exe build failed" }
Write-Host "Built TanControl.exe"

# 2. Engine (static CRT -> no VC++ redist dependency on end-user machines)
if (-not $SkipEngine) {
    Push-Location $repo
    try {
        $env:RUSTFLAGS = '-C target-feature=+crt-static'
        cargo build -p tan-live --release
    } finally { Pop-Location; Remove-Item Env:RUSTFLAGS -EA SilentlyContinue }
}
if (-not (Test-Path "$repo\target\release\tan-live.exe")) { throw "tan-live.exe not built" }

# 3. Installer
$iscc = "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
if (-not (Test-Path $iscc)) { $iscc = 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe' }
if (-not (Test-Path $iscc)) { throw "ISCC.exe not found. Install: winget install --id JRSoftware.InnoSetup -e" }
& $iscc "$here\TAN.iss"
Write-Host "`nInstaller: $here\dist\TAN-Setup.exe" -ForegroundColor Green

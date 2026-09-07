# Builds the TAN virtual audio device driver (Release x64) and prints the
# package path. MUST use 64-bit MSBuild: WDK 26100 ships InfVerif for x64 only,
# so 32-bit MSBuild fails the INF verification step ('x86\InfVerif.dll').
param([string]$Configuration = 'Release', [switch]$Clean)
$ErrorActionPreference = 'Stop'

$sln = Join-Path $PSScriptRoot '..\SimpleAudioSample.sln'
$candidates = @(
    'D:\Microsoft Visual Studio\2022\Community\MSBuild\Current\Bin\amd64\MSBuild.exe',
    "${env:ProgramFiles}\Microsoft Visual Studio\2022\Community\MSBuild\Current\Bin\amd64\MSBuild.exe",
    "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\amd64\MSBuild.exe"
)
$msbuild = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $msbuild) { throw "64-bit MSBuild not found. Edit this script's candidate list." }

if ($Clean) { & $msbuild $sln -t:Clean -p:Configuration=$Configuration -p:Platform=x64 -v:q -nologo | Out-Null }
& $msbuild $sln -p:Configuration=$Configuration -p:Platform=x64 -m -v:m -nologo
if ($LASTEXITCODE -ne 0) { throw "Build failed ($LASTEXITCODE)" }

$pkg = Join-Path $PSScriptRoot "..\x64\$Configuration\package"
Write-Host "`nPackage:" -ForegroundColor Green
Get-ChildItem $pkg | Select-Object Name, Length | Format-Table -AutoSize

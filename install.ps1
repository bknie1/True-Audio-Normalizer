# TAN installer for Windows.
# Usage:  irm https://raw.githubusercontent.com/bknie1/True-Audio-Normalizer/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$repo = "bknie1/True-Audio-Normalizer"
$installDir = "$env:LOCALAPPDATA\Programs\TAN"

Write-Host "Finding latest TAN release..."
$release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest"
$asset = $release.assets | Where-Object { $_.name -like "tan-windows-*.zip" } | Select-Object -First 1
if (-not $asset) {
    Write-Error "No Windows release found. Has a release been published yet?"
    exit 1
}

Write-Host "Downloading $($asset.name) ($([math]::Round($asset.size / 1MB, 1)) MB)..."
$zipPath = "$env:TEMP\tan-install.zip"
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zipPath

Write-Host "Installing to $installDir..."
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
Expand-Archive -Path $zipPath -DestinationPath $installDir -Force
Remove-Item $zipPath

$userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($userPath -notlike "*$installDir*") {
    [Environment]::SetEnvironmentVariable("PATH", "$userPath;$installDir", "User")
    Write-Host "Added $installDir to your PATH (open a new terminal for it to take effect)."
}

Write-Host ""
Write-Host "Installed. Try it:"
Write-Host "  tan-cli gen demo.wav"
Write-Host "  tan-cli process demo.wav out.wav movie --two-pass"
Write-Host "  tan-live --list-devices"

# Creates discoverable shortcuts so TAN can be managed from the Start Menu:
#   - "TAN Control (tray)"  -> starts the tray indicator
#   - "TAN Status"          -> opens the status console
# Optionally (-Startup) also drops the tray in the current user's Startup so it
# launches at logon. Per-user, no elevation needed. Run with -Remove to undo.
param([switch]$Startup, [switch]$Remove)

$scripts = $PSScriptRoot
$tray = Join-Path $scripts 'tan-apo-tray.ps1'
$ctl  = Join-Path $scripts 'tan-apo-ctl.ps1'
$programs = [Environment]::GetFolderPath('Programs')
$startupDir = [Environment]::GetFolderPath('Startup')
$lnkTray   = Join-Path $programs 'TAN Control (tray).lnk'
$lnkStatus = Join-Path $programs 'TAN Status.lnk'
$lnkStartup = Join-Path $startupDir 'TAN Control.lnk'

if ($Remove) {
    Remove-Item $lnkTray, $lnkStatus, $lnkStartup -Force -ErrorAction SilentlyContinue
    Write-Host 'Removed TAN shortcuts.'
    return
}

$ws = New-Object -ComObject WScript.Shell
function New-Lnk($path, $argLine, $desc) {
    $s = $ws.CreateShortcut($path)
    $s.TargetPath = "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe"
    $s.Arguments = $argLine
    $s.WorkingDirectory = $scripts
    $s.WindowStyle = 7  # minimized/hidden host
    $s.Description = $desc
    $s.Save()
}
New-Lnk $lnkTray   "-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File `"$tray`"" 'TAN APO tray indicator and manager'
New-Lnk $lnkStatus "-NoProfile -ExecutionPolicy Bypass -NoExit -File `"$ctl`" status" 'Show TAN APO status'
Write-Host "Start Menu shortcuts created:`n  $lnkTray`n  $lnkStatus"

if ($Startup) {
    New-Lnk $lnkStartup "-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File `"$tray`"" 'TAN APO tray (autostart)'
    Write-Host "Autostart shortcut created: $lnkStartup"
}

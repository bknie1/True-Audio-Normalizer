<#
  tan-vad-uninstall.ps1 - removes the TAN virtual audio device driver.
  Run ELEVATED. Leaves test-signing as-is unless -DisableTestSigning is passed
  (that needs a reboot to take effect).
#>
param([switch]$DisableTestSigning)
$ErrorActionPreference = 'Continue'

$devcon = 'C:\Program Files (x86)\Windows Kits\10\Tools\10.0.26100.0\x64\devcon.exe'

# Remove the device node.
& $devcon remove "ROOT\TanVad" 2>$null | Out-Null

# Remove the driver package from the store (find the oem*.inf that is ours).
$oem = pnputil /enum-drivers | Select-String -Context 0,4 'TanVad.inf' |
    ForEach-Object { $_.Context.PostContext } | Select-String 'Published Name' |
    ForEach-Object { ($_ -split ':')[1].Trim() }
foreach ($inf in ($oem | Sort-Object -Unique)) {
    if ($inf) { pnputil /delete-driver $inf /uninstall /force 2>&1 | Out-Null; Write-Host "Removed driver package $inf" }
}

if ($DisableTestSigning) {
    bcdedit /set testsigning off | Out-Null
    Write-Host 'Test-signing disabled (reboot to take effect).' -ForegroundColor Yellow
}
Write-Host 'TAN virtual audio device removed.' -ForegroundColor Green

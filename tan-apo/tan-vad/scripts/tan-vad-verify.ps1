# Post-install sanity check for the TAN virtual audio device. Read-only, no
# elevation needed. Run after tan-vad-install.ps1 (which needs Secure Boot off
# + test-signing + a reboot) to confirm the device came up.
$ErrorActionPreference = 'Continue'

Write-Host '==== TAN virtual audio device check ====' -ForegroundColor Cyan

# 1. Driver package staged in the store?
$drv = pnputil /enum-drivers 2>$null | Select-String 'TanVad.inf'
Write-Host ("  Driver package in store : {0}" -f $(if ($drv) {'yes'} else {'no'}))

# 2. Device node present?
$dev = Get-PnpDevice -EA SilentlyContinue | Where-Object { $_.InstanceId -like '*TanVad*' -or $_.FriendlyName -eq 'TAN (True Audio Normalizer)' }
if ($dev) {
    foreach ($d in $dev) { Write-Host ("  Device                  : {0} [{1}]" -f $d.FriendlyName, $d.Status) -ForegroundColor Green }
} else {
    Write-Host '  Device                  : not found (run tan-vad-install.ps1 after disabling Secure Boot + reboot)' -ForegroundColor Yellow
}

# 3. Render endpoint visible to the audio stack?
$render = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render'
$tanEp = Get-ChildItem $render -EA SilentlyContinue | ForEach-Object {
    $p = Get-ItemProperty "$($_.PSPath)\Properties" -EA SilentlyContinue
    if ($p.'{a45c254e-df1c-4efd-8020-67d146a850e0},2' -eq 'TAN') { $_.PSChildName }
}
Write-Host ("  Playback endpoint 'TAN' : {0}" -f $(if ($tanEp) {"yes ($tanEp)"} else {'not present yet'}))

# 4. Test-signing / Secure Boot context.
$sb = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\SecureBoot\State' -Name UEFISecureBootEnabled -EA SilentlyContinue).UEFISecureBootEnabled
Write-Host ("  Secure Boot             : {0}" -f $(if ($sb -eq 1) {'ON (blocks test-signing / self-signed driver load)'} else {'off'}))

Write-Host ''
Write-Host '  If the device is present, forward it with tan-live:' -ForegroundColor DarkGray
Write-Host '    tan-live --loopback-from "TAN" --output "<your real output>"' -ForegroundColor DarkGray

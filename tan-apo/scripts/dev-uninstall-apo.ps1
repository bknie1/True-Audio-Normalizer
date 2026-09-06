# Reverses dev-install-apo.ps1: detaches TAN from every render endpoint's SFX
# slot (classic and composite), removes the COM + APO registrations and the
# installed DLL, and optionally restores audiodg's signature enforcement.
# Run ELEVATED.

param(
    [switch]$ResetProtectedAudioDG,
    [switch]$KeepDll
)

$ErrorActionPreference = 'Stop'

$TAN_CLSID = '{2A746E35-39E6-4627-B243-7E732E96387F}'
$PKEY_FX_SFXClsid          = '{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},5'
$PKEY_CompositeFX_SFXClsid = '{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},13'
$RenderRoot = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render'

function Require-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole(
            [Security.Principal.WindowsBuiltinRole]::Administrator)) {
        throw 'Run this in an elevated (Administrator) PowerShell.'
    }
}
Require-Admin

# --- Detach from every render endpoint ------------------------------------
foreach ($k in Get-ChildItem $RenderRoot) {
    $fxKey = "$($k.PSPath)\FxProperties"
    if (-not (Test-Path $fxKey)) { continue }
    $fx = Get-ItemProperty $fxKey

    $composite = $fx.$PKEY_CompositeFX_SFXClsid
    if ($null -ne $composite -and @($composite) -contains $TAN_CLSID) {
        $newList = @($composite) | Where-Object { $_ -ne $TAN_CLSID }
        Set-ItemProperty -Path $fxKey -Name $PKEY_CompositeFX_SFXClsid -Value $newList -Type MultiString
        Write-Host "Removed TAN from composite SFX chain on $($k.PSChildName)"
    }

    if ($fx.$PKEY_FX_SFXClsid -eq $TAN_CLSID) {
        Remove-ItemProperty -Path $fxKey -Name $PKEY_FX_SFXClsid
        Write-Host "Removed TAN SFX from $($k.PSChildName)"
        Write-Host '  (If another SFX lived there before, restore it from the .reg backup in scripts\backup.)'
    }
}

# --- Remove registrations and the DLL -------------------------------------
Remove-Item -Path "HKLM:\SOFTWARE\Classes\AudioEngine\AudioProcessingObjects\$TAN_CLSID" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -Path "HKLM:\SOFTWARE\Classes\CLSID\$TAN_CLSID" -Recurse -Force -ErrorAction SilentlyContinue
Write-Host 'Removed COM CLSID + AudioProcessingObjects registration.'

if ($ResetProtectedAudioDG) {
    Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Audio' -Name 'DisableProtectedAudioDG' -ErrorAction SilentlyContinue
    Write-Host 'audiodg signature enforcement restored (DisableProtectedAudioDG removed).'
}

# --- Reload the audio stack (releases the DLL so it can be deleted) -------
Restart-Service -Name AudioEndpointBuilder -Force
Start-Service -Name Audiosrv -ErrorAction SilentlyContinue

if (-not $KeepDll) {
    Start-Sleep -Seconds 2
    Remove-Item 'C:\Program Files\TAN\TanApo.dll' -Force -ErrorAction SilentlyContinue
    Remove-Item 'C:\Program Files\TAN' -Force -ErrorAction SilentlyContinue
    Write-Host 'Removed C:\Program Files\TAN.'
}

Write-Host 'Done.'

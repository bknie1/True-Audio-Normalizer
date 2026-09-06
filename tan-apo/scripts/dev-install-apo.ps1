# Dev-fast install for the TAN APO: registers TanApo.dll directly in the
# registry and attaches it to one render endpoint's SFX slot. No INF, no
# catalog, no reboot; fully reversible with dev-uninstall-apo.ps1 (a .reg
# backup of the endpoint's FxProperties is written before any change).
#
# Run ELEVATED. Unless the DLL is signed with a cert Windows trusts, audiodg
# will refuse to load it; pass -AllowUnsignedApo to set the well-known dev
# override DisableProtectedAudioDG=1 (weakens audiodg's APO signature check
# machine-wide; dev machines only, undo via dev-uninstall-apo.ps1
# -ResetProtectedAudioDG).
#
# Endpoint presets resolve by endpoint description at runtime:
#   sonar-media, sonar-gaming  : SteelSeries Sonar virtual endpoints (empty
#                                SFX slot today; TAN becomes their SFX)
#   realtek-speakers           : Realtek analog out (Realtek composite chain;
#                                TAN is APPENDED after Realtek's SFX)
#   realtek-digital            : Realtek S/PDIF (composite; appended)
# Or pass the MMDevice endpoint GUID directly, e.g. {b850eed8-...}.

param(
    [Parameter(Mandatory = $true)]
    [string]$Endpoint,
    [string]$Dll = "$PSScriptRoot\..\apo\x64\Release\TanApo.dll",
    [switch]$AllowUnsignedApo,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

$TAN_CLSID = '{2A746E35-39E6-4627-B243-7E732E96387F}'
$TAN_IID   = '{DEBD06C8-2F87-49EC-B1BF-E0263C5221C1}'
$PKEY_FX_SFXClsid            = '{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},5'
$PKEY_CompositeFX_SFXClsid   = '{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},13'
$PKEY_SFX_Modes              = '{d3993a3f-99c2-4402-b5ec-a92a0367664b},5'
$MODE_DEFAULT = '{C18E2F7E-933D-4965-B7D1-1EEF228D2AF3}'
$RenderRoot = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render'

function Require-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole(
            [Security.Principal.WindowsBuiltinRole]::Administrator)) {
        throw 'Run this in an elevated (Administrator) PowerShell.'
    }
}
Require-Admin

if (-not (Test-Path $Dll)) { throw "APO DLL not found: $Dll (build TanApo.vcxproj first)" }
$Dll = (Resolve-Path $Dll).Path

# --- Resolve the endpoint -------------------------------------------------
$presets = @{
    'sonar-media'      = 'SteelSeries Sonar - Media'
    'sonar-gaming'     = 'SteelSeries Sonar - Gaming'
    'realtek-speakers' = 'Speakers|Realtek(R) Audio'
    'realtek-digital'  = 'Realtek Digital Output|Realtek(R) Audio'
}

$epGuid = $null
if ($Endpoint -match '^\{[0-9a-fA-F-]{36}\}$') {
    $epGuid = $Endpoint
} elseif ($presets.ContainsKey($Endpoint.ToLower())) {
    $wantDesc, $wantSys = $presets[$Endpoint.ToLower()] -split '\|'
    foreach ($k in Get-ChildItem $RenderRoot) {
        $p = Get-ItemProperty "$($k.PSPath)\Properties" -ErrorAction SilentlyContinue
        $desc = $p.'{a45c254e-df1c-4efd-8020-67d146a850e0},2'
        $sys  = $p.'{b3f8fa53-0004-438e-9003-51a46e139bfc},6'
        if ($desc -eq $wantDesc -and (-not $wantSys -or $sys -eq $wantSys)) { $epGuid = $k.PSChildName; break }
    }
    if (-not $epGuid) { throw "No render endpoint found matching preset '$Endpoint'." }
} else {
    throw "Endpoint must be a preset ($($presets.Keys -join ', ')) or an endpoint GUID."
}

$epKey = "$RenderRoot\$epGuid"
$fxKey = "$epKey\FxProperties"
Write-Host "Target endpoint: $epGuid"

# --- Backup ---------------------------------------------------------------
$backupDir = "$PSScriptRoot\backup"
New-Item -ItemType Directory -Force $backupDir | Out-Null
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$backup = "$backupDir\fxprops-$($epGuid.Trim('{}'))-$stamp.reg"
if (Test-Path $fxKey) {
    reg export ("HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\$epGuid\FxProperties") $backup /y | Out-Null
    Write-Host "FxProperties backed up to $backup"
} else {
    New-Item -Path $fxKey -Force | Out-Null
    Set-Content -Path "$backupDir\fxprops-$($epGuid.Trim('{}'))-$stamp.CREATED" -Value 'FxProperties key did not exist before install.' -Encoding utf8
    Write-Host 'FxProperties key created (did not exist).'
}

# --- Install the DLL + COM + APO registration -----------------------------
$installDir = 'C:\Program Files\TAN'
New-Item -ItemType Directory -Force $installDir | Out-Null
Copy-Item $Dll "$installDir\TanApo.dll" -Force
Write-Host "Copied DLL to $installDir\TanApo.dll"

$clsidKey = "HKLM:\SOFTWARE\Classes\CLSID\$TAN_CLSID"
New-Item -Path "$clsidKey\InprocServer32" -Force | Out-Null
Set-Item -Path $clsidKey -Value 'TanApoSFX Class'
Set-Item -Path "$clsidKey\InprocServer32" -Value "$installDir\TanApo.dll"
Set-ItemProperty -Path "$clsidKey\InprocServer32" -Name 'ThreadingModel' -Value 'Both'

$apoKey = "HKLM:\SOFTWARE\Classes\AudioEngine\AudioProcessingObjects\$TAN_CLSID"
New-Item -Path $apoKey -Force | Out-Null
Set-ItemProperty $apoKey 'FriendlyName' 'TanApoSFX'
Set-ItemProperty $apoKey 'Copyright' 'Copyright (c) Brandon Knieriem, MIT license'
Set-ItemProperty $apoKey 'MajorVersion' 1 -Type DWord
Set-ItemProperty $apoKey 'MinorVersion' 0 -Type DWord
Set-ItemProperty $apoKey 'Flags' 0xE -Type DWord
Set-ItemProperty $apoKey 'MinInputConnections' 1 -Type DWord
Set-ItemProperty $apoKey 'MaxInputConnections' 1 -Type DWord
Set-ItemProperty $apoKey 'MinOutputConnections' 1 -Type DWord
Set-ItemProperty $apoKey 'MaxOutputConnections' 1 -Type DWord
Set-ItemProperty $apoKey 'MaxInstances' 0xffffffff -Type DWord
Set-ItemProperty $apoKey 'NumAPOInterfaces' 1 -Type DWord
Set-ItemProperty $apoKey 'APOInterface0' $TAN_IID
Write-Host 'COM CLSID + AudioProcessingObjects registration written.'

# --- Attach to the endpoint's SFX slot ------------------------------------
$fx = Get-ItemProperty $fxKey -ErrorAction SilentlyContinue
$composite = $fx.$PKEY_CompositeFX_SFXClsid
if ($null -ne $composite) {
    $list = @($composite)
    if ($list -contains $TAN_CLSID) {
        Write-Host 'TAN already in the composite SFX list.'
    } else {
        Set-ItemProperty -Path $fxKey -Name $PKEY_CompositeFX_SFXClsid -Value ($list + $TAN_CLSID) -Type MultiString
        Write-Host "Appended TAN to composite SFX chain (now: $(($list + $TAN_CLSID) -join ', '))."
    }
} else {
    $existing = $fx.$PKEY_FX_SFXClsid
    if ($existing -and $existing -ne $TAN_CLSID -and -not $Force) {
        throw "Endpoint already has SFX $existing. Re-run with -Force to replace it (backup was taken)."
    }
    Set-ItemProperty -Path $fxKey -Name $PKEY_FX_SFXClsid -Value $TAN_CLSID
    Write-Host 'Set TAN as the endpoint SFX.'
}

# Make sure SFX streaming is enabled for DEFAULT mode.
$modes = (Get-ItemProperty $fxKey -ErrorAction SilentlyContinue).$PKEY_SFX_Modes
if ($null -eq $modes) {
    Set-ItemProperty -Path $fxKey -Name $PKEY_SFX_Modes -Value @($MODE_DEFAULT) -Type MultiString
    Write-Host 'Added SFX processing modes (DEFAULT).'
} elseif (@($modes) -notcontains $MODE_DEFAULT) {
    Set-ItemProperty -Path $fxKey -Name $PKEY_SFX_Modes -Value (@($modes) + $MODE_DEFAULT) -Type MultiString
    Write-Host 'Appended DEFAULT to SFX processing modes.'
}

# --- Unsigned-APO dev override --------------------------------------------
if ($AllowUnsignedApo) {
    Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Audio' -Name 'DisableProtectedAudioDG' -Value 1 -Type DWord
    Write-Host 'WARNING: DisableProtectedAudioDG=1 set. audiodg will load unsigned APOs (dev only).' -ForegroundColor Yellow
} else {
    Write-Host 'Note: DLL must be signed with a trusted cert or audiodg will skip it (use -AllowUnsignedApo for dev).'
}

# --- Reload the audio stack -----------------------------------------------
Write-Host 'Restarting audio services...'
Restart-Service -Name AudioEndpointBuilder -Force
Start-Service -Name Audiosrv -ErrorAction SilentlyContinue
Write-Host 'Done. Play audio through the endpoint and check that leveling engages.'
Write-Host "Verify load: tasklist /m TanApo.dll   (should list audiodg.exe once audio plays)"

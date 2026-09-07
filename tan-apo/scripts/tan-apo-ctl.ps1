<#
  tan-apo-ctl.ps1 - management console for the dev TAN APO install.

  This is the single place to see and control the TAN System Effect APO that
  the dev scripts attach to a render endpoint. It is intentionally separate
  from the shipping `tan-tray` component (a different machine/scope); this is
  the developer-side manager for the registry/dev install.

  Verbs:
    status              Show everything: registration, DLL, which endpoints
                        carry TAN, whether audiodg has loaded it, dev override.
    endpoints           List render endpoints with their current SFX/EFX.
    on   [-Endpoint X]  Enable TAN on an endpoint (default: Sonar Media).
    off  [-Endpoint X]  Disable TAN on an endpoint (leaves it attached).
    attach [-Endpoint X]  Attach TAN to an endpoint's SFX slot.
    detach [-Endpoint X]  Remove TAN from an endpoint's SFX slot.
    uninstall           Detach from ALL endpoints, unregister, remove DLL,
                        and restore audiodg signature enforcement.

  Status is read-only and runs unelevated. Every state-changing verb
  self-elevates (UAC prompt) automatically.
#>
[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet('status','endpoints','install','on','off','attach','detach','uninstall')]
    [string]$Verb = 'status',
    [string]$Endpoint = 'sonar-media',
    [string]$Dll = "$PSScriptRoot\..\apo\x64\Release\TanApo.dll",
    [switch]$Elevated  # internal: set when relaunched elevated
)

$ErrorActionPreference = 'Stop'

$TAN_CLSID = '{2A746E35-39E6-4627-B243-7E732E96387F}'
$TAN_IID   = '{DEBD06C8-2F87-49EC-B1BF-E0263C5221C1}'
$InstallDll = 'C:\Program Files\TAN\TanApo.dll'
$RenderRoot = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render'
$RenderSub  = 'SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render'
$ApoReg     = "HKLM:\SOFTWARE\Classes\AudioEngine\AudioProcessingObjects\$TAN_CLSID"

$PKEY_FX_SFXClsid          = '{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},5'
$PKEY_CompositeFX_SFXClsid = '{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},13'
$PKEY_EFXClsid             = '{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},7'
$PKEY_SFX_Modes            = '{d3993a3f-99c2-4402-b5ec-a92a0367664b},5'
$PKEY_Enable_Tan           = '{adf13a0e-662a-4e2c-a574-d9ba77ba77c2},2'
$MODE_DEFAULT = '{C18E2F7E-933D-4965-B7D1-1EEF228D2AF3}'

$Presets = @{
    'sonar-media'      = 'SteelSeries Sonar - Media'
    'sonar-gaming'     = 'SteelSeries Sonar - Gaming'
    'realtek-speakers' = 'Speakers|Realtek(R) Audio'
    'realtek-digital'  = 'Realtek Digital Output|Realtek(R) Audio'
}

function Test-Admin {
    (New-Object Security.Principal.WindowsPrincipal(
        [Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole(
        [Security.Principal.WindowsBuiltinRole]::Administrator)
}

function Invoke-Elevated {
    param([string]$V, [string]$Ep, [string]$DllPath)
    $a = @('-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`"",$V,'-Endpoint',"`"$Ep`"",'-Elevated')
    if ($DllPath) { $a += @('-Dll',"`"$DllPath`"") }
    $p = Start-Process powershell -Verb RunAs -Wait -PassThru -ArgumentList $a
    return $p.ExitCode
}

function Backup-Fx {
    param([string]$EpGuid)
    $backupDir = Join-Path $PSScriptRoot 'backup'
    New-Item -ItemType Directory -Force $backupDir | Out-Null
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $file = Join-Path $backupDir "fxprops-$($EpGuid.Trim('{}'))-$stamp.reg"
    $regPath = "HKLM\$RenderSub\$EpGuid\FxProperties"
    if (Test-Path "HKLM:\$RenderSub\$EpGuid\FxProperties") {
        reg export $regPath $file /y | Out-Null
        Write-Host "Backed up FxProperties -> $file"
    }
}

function Register-Apo {
    param([string]$DllPath)
    if (-not (Test-Path $DllPath)) { throw "APO DLL not found: $DllPath (build tan-apo\apo\TanApo.vcxproj)" }
    $DllPath = (Resolve-Path $DllPath).Path
    $dir = Split-Path $InstallDll -Parent
    New-Item -ItemType Directory -Force $dir | Out-Null
    Copy-Item $DllPath $InstallDll -Force
    $clsidKey = "HKLM:\SOFTWARE\Classes\CLSID\$TAN_CLSID"
    New-Item -Path "$clsidKey\InprocServer32" -Force | Out-Null
    Set-Item -Path $clsidKey -Value 'TanApoSFX Class'
    Set-Item -Path "$clsidKey\InprocServer32" -Value $InstallDll
    Set-ItemProperty -Path "$clsidKey\InprocServer32" -Name 'ThreadingModel' -Value 'Both'
    New-Item -Path $ApoReg -Force | Out-Null
    Set-ItemProperty $ApoReg 'FriendlyName' 'TanApoSFX'
    Set-ItemProperty $ApoReg 'Copyright' 'Copyright (c) Brandon Knieriem, MIT license'
    Set-ItemProperty $ApoReg 'MajorVersion' 1 -Type DWord
    Set-ItemProperty $ApoReg 'MinorVersion' 0 -Type DWord
    Set-ItemProperty $ApoReg 'Flags' 0xE -Type DWord
    Set-ItemProperty $ApoReg 'MinInputConnections' 1 -Type DWord
    Set-ItemProperty $ApoReg 'MaxInputConnections' 1 -Type DWord
    Set-ItemProperty $ApoReg 'MinOutputConnections' 1 -Type DWord
    Set-ItemProperty $ApoReg 'MaxOutputConnections' 1 -Type DWord
    Set-ItemProperty $ApoReg 'MaxInstances' 0xffffffff -Type DWord
    Set-ItemProperty $ApoReg 'NumAPOInterfaces' 1 -Type DWord
    Set-ItemProperty $ApoReg 'APOInterface0' $TAN_IID
    Write-Host "Registered COM CLSID + APO ($InstallDll)."
}

function Attach-Tan {
    param([string]$EpGuid)
    $fx = Get-ItemProperty "$RenderRoot\$EpGuid\FxProperties" -EA SilentlyContinue
    if (@($fx.$PKEY_CompositeFX_SFXClsid)) {
        if (@($fx.$PKEY_CompositeFX_SFXClsid) -notcontains $TAN_CLSID) {
            Set-FxValue $EpGuid $PKEY_CompositeFX_SFXClsid ([string[]](@($fx.$PKEY_CompositeFX_SFXClsid) + $TAN_CLSID)) ([Microsoft.Win32.RegistryValueKind]::MultiString)
        }
    } else {
        Set-FxValue $EpGuid $PKEY_FX_SFXClsid $TAN_CLSID ([Microsoft.Win32.RegistryValueKind]::String)
    }
    if (@((Get-ItemProperty "$RenderRoot\$EpGuid\FxProperties" -EA SilentlyContinue).$PKEY_SFX_Modes) -notcontains $MODE_DEFAULT) {
        Set-FxValue $EpGuid $PKEY_SFX_Modes ([string[]]@($MODE_DEFAULT)) ([Microsoft.Win32.RegistryValueKind]::MultiString)
    }
    Set-FxValue $EpGuid $PKEY_Enable_Tan 1 ([Microsoft.Win32.RegistryValueKind]::DWord)
}

function Get-EndpointName {
    param([string]$Guid)
    $p = Get-ItemProperty "$RenderRoot\$Guid\Properties" -ErrorAction SilentlyContinue
    $desc = $p.'{a45c254e-df1c-4efd-8020-67d146a850e0},2'
    $sys  = $p.'{b3f8fa53-0004-438e-9003-51a46e139bfc},6'
    if ($desc) { "$desc ($sys)" } else { $Guid }
}

function Resolve-Endpoint {
    param([string]$Spec)
    if ($Spec -match '^\{[0-9a-fA-F-]{36}\}$') { return $Spec }
    $key = $Spec.ToLower()
    if (-not $Presets.ContainsKey($key)) {
        throw "Unknown endpoint '$Spec'. Use a GUID or one of: $($Presets.Keys -join ', ')"
    }
    $wantDesc, $wantSys = $Presets[$key] -split '\|'
    foreach ($k in Get-ChildItem $RenderRoot) {
        $p = Get-ItemProperty "$($k.PSPath)\Properties" -ErrorAction SilentlyContinue
        $desc = $p.'{a45c254e-df1c-4efd-8020-67d146a850e0},2'
        $sys  = $p.'{b3f8fa53-0004-438e-9003-51a46e139bfc},6'
        if ($desc -eq $wantDesc -and (-not $wantSys -or $sys -eq $wantSys)) { return $k.PSChildName }
    }
    throw "No render endpoint matches preset '$Spec'."
}

# Narrow-open write: the FxProperties DACL grants Administrators SetValue only
# (not the broad ReadWriteSubTree mask Set-ItemProperty requests), so open the
# key asking for exactly RegistryRights::SetValue.
function Set-FxValue {
    param([string]$EpGuid, [string]$Name, $Value, [Microsoft.Win32.RegistryValueKind]$Kind)
    $sub = "$RenderSub\$EpGuid\FxProperties"
    $k = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey(
        $sub, [Microsoft.Win32.RegistryKeyPermissionCheck]::ReadWriteSubTree,
        [System.Security.AccessControl.RegistryRights]::SetValue)
    if ($null -eq $k) { throw "OpenSubKey(SetValue) null for $sub" }
    try { $k.SetValue($Name, $Value, $Kind) } finally { $k.Close() }
}
function Remove-FxValue {
    param([string]$EpGuid, [string]$Name)
    $sub = "$RenderSub\$EpGuid\FxProperties"
    $rights = [System.Security.AccessControl.RegistryRights]::SetValue
    $k = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey(
        $sub, [Microsoft.Win32.RegistryKeyPermissionCheck]::ReadWriteSubTree, $rights)
    if ($null -ne $k) { try { $k.DeleteValue($Name, $false) } catch {} finally { $k.Close() } }
}

function Restart-Audio {
    Restart-Service -Name AudioEndpointBuilder -Force
    Start-Service -Name Audiosrv -ErrorAction SilentlyContinue
}

function Get-TanEndpoints {
    # Returns objects for every render endpoint that currently carries TAN.
    $out = @()
    foreach ($k in Get-ChildItem $RenderRoot -ErrorAction SilentlyContinue) {
        $fx = Get-ItemProperty "$($k.PSPath)\FxProperties" -ErrorAction SilentlyContinue
        if (-not $fx) { continue }
        $slot = $null
        if (@($fx.$PKEY_CompositeFX_SFXClsid) -contains $TAN_CLSID) { $slot = 'composite-SFX' }
        elseif ($fx.$PKEY_FX_SFXClsid -eq $TAN_CLSID) { $slot = 'SFX' }
        if ($slot) {
            $enableRaw = $fx.$PKEY_Enable_Tan
            $enabled = if ($null -eq $enableRaw) { $true } else { [int]$enableRaw -ne 0 }
            $out += [PSCustomObject]@{
                Guid = $k.PSChildName; Name = Get-EndpointName $k.PSChildName
                Slot = $slot; Enabled = $enabled
            }
        }
    }
    $out
}

function Show-Status {
    Write-Host ''
    Write-Host '==== TAN APO (dev install) status ====' -ForegroundColor Cyan
    $reg = Test-Path $ApoReg
    $dll = Test-Path $InstallDll
    $override = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Audio' -Name DisableProtectedAudioDG -EA SilentlyContinue).DisableProtectedAudioDG
    Write-Host ("  APO registered      : {0}" -f $(if ($reg) {'yes (TanApoSFX)'} else {'NO'}))
    Write-Host ("  DLL present         : {0}" -f $(if ($dll) {$InstallDll} else {'NO'}))
    Write-Host ("  Unsigned dev load   : {0}" -f $(if ($override -eq 1) {'ON (DisableProtectedAudioDG=1)'} else {'off'}))

    $eps = Get-TanEndpoints
    if ($eps.Count -eq 0) {
        Write-Host '  Attached to         : (no endpoints)' -ForegroundColor Yellow
    } else {
        Write-Host '  Attached to         :'
        foreach ($e in $eps) {
            $state = if ($e.Enabled) { 'ENABLED' } else { 'disabled' }
            $color = if ($e.Enabled) { 'Green' } else { 'Yellow' }
            Write-Host ("     - {0}  [{1}, {2}]" -f $e.Name, $e.Slot, $state) -ForegroundColor $color
        }
    }

    $adg = Get-Process audiodg -EA SilentlyContinue
    if ($adg) {
        $loaded = (tasklist /m TanApo.dll 2>&1 | Out-String) -match 'audiodg'
        $msg = if ($loaded) { 'LOADED in audiodg.exe (TAN is live)' } else { 'not loaded (no active stream on a TAN endpoint yet)' }
        $c = if ($loaded) { 'Green' } else { 'Gray' }
        Write-Host ("  Runtime             : audiodg PID {0}, DLL {1}" -f $adg.Id, $msg) -ForegroundColor $c
    } else {
        Write-Host '  Runtime             : audiodg not running (no audio active)'
    }
    Write-Host ''
    Write-Host '  Manage: tan-apo-ctl.ps1 <on|off|attach|detach|uninstall> [-Endpoint sonar-media|<guid>]' -ForegroundColor DarkGray
    Write-Host ''
}

function Show-Endpoints {
    Write-Host ''
    Write-Host 'Render endpoints (active/plugged first):' -ForegroundColor Cyan
    foreach ($k in Get-ChildItem $RenderRoot) {
        $state = (Get-ItemProperty $k.PSPath -EA SilentlyContinue).DeviceState
        $fx = Get-ItemProperty "$($k.PSPath)\FxProperties" -EA SilentlyContinue
        $sfx = $fx.$PKEY_FX_SFXClsid; $comp = $fx.$PKEY_CompositeFX_SFXClsid; $efx = $fx.$PKEY_EFXClsid
        $tan = (@($comp) -contains $TAN_CLSID) -or ($sfx -eq $TAN_CLSID)
        if ($state -eq 1 -or $tan) {
            $tag = if ($tan) { '  <-- TAN here' } else { '' }
            Write-Host ("  {0}{1}" -f (Get-EndpointName $k.PSChildName), $tag) -ForegroundColor $(if($tan){'Green'}else{'Gray'})
        }
    }
    Write-Host ''
}

# ---- Dispatch ------------------------------------------------------------
if ($Verb -eq 'status')    { Show-Status; return }
if ($Verb -eq 'endpoints') { Show-Endpoints; return }

# State-changing verbs need admin; self-elevate if needed.
if (-not (Test-Admin)) {
    Write-Host "Elevating for '$Verb'..." -ForegroundColor DarkGray
    $dllArg = if ($Verb -eq 'install') { $Dll } else { '' }
    $code = Invoke-Elevated -V $Verb -Ep $Endpoint -DllPath $dllArg
    Write-Host "(elevated $Verb exited $code)"
    Show-Status
    return
}

switch ($Verb) {
    'uninstall' {
        foreach ($k in Get-ChildItem $RenderRoot) {
            $fx = Get-ItemProperty "$($k.PSPath)\FxProperties" -EA SilentlyContinue
            if (-not $fx) { continue }
            if (@($fx.$PKEY_CompositeFX_SFXClsid) -contains $TAN_CLSID) {
                $new = @($fx.$PKEY_CompositeFX_SFXClsid) | Where-Object { $_ -ne $TAN_CLSID }
                Set-FxValue $k.PSChildName $PKEY_CompositeFX_SFXClsid ([string[]]$new) ([Microsoft.Win32.RegistryValueKind]::MultiString)
                Write-Host "Removed TAN from composite SFX on $(Get-EndpointName $k.PSChildName)"
            }
            if ($fx.$PKEY_FX_SFXClsid -eq $TAN_CLSID) {
                Remove-FxValue $k.PSChildName $PKEY_FX_SFXClsid
                Write-Host "Removed TAN SFX from $(Get-EndpointName $k.PSChildName)"
            }
            Remove-FxValue $k.PSChildName $PKEY_Enable_Tan
        }
        Remove-Item $ApoReg -Recurse -Force -EA SilentlyContinue
        Remove-Item "HKLM:\SOFTWARE\Classes\CLSID\$TAN_CLSID" -Recurse -Force -EA SilentlyContinue
        Remove-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Audio' -Name DisableProtectedAudioDG -EA SilentlyContinue
        Restart-Audio
        Start-Sleep 2
        Remove-Item $InstallDll -Force -EA SilentlyContinue
        Remove-Item 'C:\Program Files\TAN' -Force -EA SilentlyContinue
        Write-Host 'TAN fully uninstalled and audiodg signature enforcement restored.' -ForegroundColor Green
    }
    'install' {
        $ep = Resolve-Endpoint $Endpoint
        Backup-Fx $ep
        Register-Apo $Dll
        Attach-Tan $ep
        Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Audio' -Name 'DisableProtectedAudioDG' -Value 1 -Type DWord
        Write-Host 'Unsigned-APO dev override set (DisableProtectedAudioDG=1).'
        Restart-Audio
        Write-Host "Installed + attached + enabled TAN on $(Get-EndpointName $ep)" -ForegroundColor Green
    }
    default {
        $ep = Resolve-Endpoint $Endpoint
        switch ($Verb) {
            'attach' {
                Backup-Fx $ep
                Attach-Tan $ep
                Restart-Audio
                Write-Host "Attached + enabled TAN on $(Get-EndpointName $ep)" -ForegroundColor Green
            }
            'detach' {
                $fx = Get-ItemProperty "$RenderRoot\$ep\FxProperties" -EA SilentlyContinue
                if (@($fx.$PKEY_CompositeFX_SFXClsid) -contains $TAN_CLSID) {
                    $new = @($fx.$PKEY_CompositeFX_SFXClsid) | Where-Object { $_ -ne $TAN_CLSID }
                    Set-FxValue $ep $PKEY_CompositeFX_SFXClsid ([string[]]$new) ([Microsoft.Win32.RegistryValueKind]::MultiString)
                }
                if ($fx.$PKEY_FX_SFXClsid -eq $TAN_CLSID) { Remove-FxValue $ep $PKEY_FX_SFXClsid }
                Restart-Audio
                Write-Host "Detached TAN from $(Get-EndpointName $ep)" -ForegroundColor Green
            }
            'on'  { Set-FxValue $ep $PKEY_Enable_Tan 1 ([Microsoft.Win32.RegistryValueKind]::DWord); Restart-Audio; Write-Host "TAN ENABLED on $(Get-EndpointName $ep)" -ForegroundColor Green }
            'off' { Set-FxValue $ep $PKEY_Enable_Tan 0 ([Microsoft.Win32.RegistryValueKind]::DWord); Restart-Audio; Write-Host "TAN disabled on $(Get-EndpointName $ep)" -ForegroundColor Yellow }
        }
    }
}

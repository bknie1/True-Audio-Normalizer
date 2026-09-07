<#
  tan-apo-tray.ps1 - a lightweight system-tray indicator + manager for the dev
  TAN APO. Green dot = TAN is loaded and live in audiodg; yellow = attached but
  idle or disabled; gray = not attached. Right-click for enable/disable,
  attach/detach, status, and uninstall. Runs unelevated; state changes shell
  out to tan-apo-ctl.ps1, which raises its own UAC prompt.

  This is a developer indicator for the registry/dev install, separate from the
  shipping `tan-tray` component. Launch hidden via "TAN Control.cmd".
#>
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$ctl = Join-Path $PSScriptRoot 'tan-apo-ctl.ps1'
$TAN_CLSID = '{2A746E35-39E6-4627-B243-7E732E96387F}'
$RenderRoot = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render'
$PKEY_FX_SFXClsid          = '{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},5'
$PKEY_CompositeFX_SFXClsid = '{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},13'
$PKEY_Enable_Tan           = '{adf13a0e-662a-4e2c-a574-d9ba77ba77c2},2'

function New-DotIcon([System.Drawing.Color]$c) {
    $bmp = New-Object System.Drawing.Bitmap 16,16
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::Transparent)
    $brush = New-Object System.Drawing.SolidBrush $c
    $g.FillEllipse($brush, 2, 2, 12, 12)
    $g.Dispose(); $brush.Dispose()
    [System.Drawing.Icon]::FromHandle($bmp.GetHicon())
}
$icoGreen  = New-DotIcon ([System.Drawing.Color]::FromArgb(60,200,90))
$icoYellow = New-DotIcon ([System.Drawing.Color]::FromArgb(230,180,40))
$icoGray   = New-DotIcon ([System.Drawing.Color]::FromArgb(150,150,150))

function Get-TanState {
    $attached = $false; $enabled = $false; $names = @()
    foreach ($k in Get-ChildItem $RenderRoot -ErrorAction SilentlyContinue) {
        $fx = Get-ItemProperty "$($k.PSPath)\FxProperties" -ErrorAction SilentlyContinue
        if (-not $fx) { continue }
        $here = (@($fx.$PKEY_CompositeFX_SFXClsid) -contains $TAN_CLSID) -or ($fx.$PKEY_FX_SFXClsid -eq $TAN_CLSID)
        if ($here) {
            $attached = $true
            $p = Get-ItemProperty "$($k.PSPath)\Properties" -ErrorAction SilentlyContinue
            $names += $p.'{a45c254e-df1c-4efd-8020-67d146a850e0},2'
            $en = $fx.$PKEY_Enable_Tan
            if ($null -eq $en -or [int]$en -ne 0) { $enabled = $true }
        }
    }
    $loaded = $false
    if (Get-Process audiodg -ErrorAction SilentlyContinue) {
        $loaded = ((tasklist /m TanApo.dll 2>&1 | Out-String) -match 'audiodg')
    }
    [PSCustomObject]@{ Attached = $attached; Enabled = $enabled; Loaded = $loaded; Names = ($names -join ', ') }
}

function Run-Ctl([string]$verb) {
    Start-Process powershell -ArgumentList @(
        '-NoProfile','-ExecutionPolicy','Bypass','-NoExit','-File',"`"$ctl`"",$verb,'-Endpoint','sonar-media')
}

$ni = New-Object System.Windows.Forms.NotifyIcon
$ni.Visible = $true
$ni.Icon = $icoGray
$ni.Text = 'TAN APO'

$menu = New-Object System.Windows.Forms.ContextMenuStrip
$hdr = $menu.Items.Add('TAN: (checking...)'); $hdr.Enabled = $false
[void]$menu.Items.Add('-')
$miEnable = $menu.Items.Add('Enable on Sonar Media');  $miEnable.add_Click({ Run-Ctl 'on' })
$miDisable = $menu.Items.Add('Disable on Sonar Media'); $miDisable.add_Click({ Run-Ctl 'off' })
$miAttach = $menu.Items.Add('Attach to Sonar Media');   $miAttach.add_Click({ Run-Ctl 'attach' })
$miDetach = $menu.Items.Add('Detach from Sonar Media'); $miDetach.add_Click({ Run-Ctl 'detach' })
[void]$menu.Items.Add('-')
$miStatus = $menu.Items.Add('Show status console');     $miStatus.add_Click({ Run-Ctl 'status' })
$miUninstall = $menu.Items.Add('Uninstall TAN');        $miUninstall.add_Click({
    if ([System.Windows.Forms.MessageBox]::Show('Remove the TAN APO from all endpoints and unregister it?','TAN',[System.Windows.Forms.MessageBoxButtons]::YesNo) -eq 'Yes') { Run-Ctl 'uninstall' }
})
[void]$menu.Items.Add('-')
$miExit = $menu.Items.Add('Exit tray'); $miExit.add_Click({
    $ni.Visible = $false; $timer.Stop(); [System.Windows.Forms.Application]::Exit()
})
$ni.ContextMenuStrip = $menu

function Refresh {
    $s = Get-TanState
    if (-not $s.Attached) {
        $ni.Icon = $icoGray; $ni.Text = 'TAN: not attached'
        $hdr.Text = 'TAN: not attached'
    } elseif ($s.Loaded -and $s.Enabled) {
        $ni.Icon = $icoGreen; $ni.Text = "TAN: LIVE on $($s.Names)"
        $hdr.Text = "TAN: LIVE ($($s.Names))"
    } else {
        $ni.Icon = $icoYellow
        $state = if (-not $s.Enabled) { 'attached, disabled' } else { 'attached, idle (no stream)' }
        $ni.Text = "TAN: $state"
        $hdr.Text = "TAN: $state - $($s.Names)"
    }
}

$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 4000
$timer.add_Tick({ Refresh })
$timer.Start()
Refresh

$ni.add_MouseClick({ param($s,$e) if ($e.Button -eq [System.Windows.Forms.MouseButtons]::Left) { Refresh; $ni.ShowBalloonTip(2000,'TAN APO',$ni.Text,[System.Windows.Forms.ToolTipIcon]::Info) } })

[System.Windows.Forms.Application]::Run()
$ni.Dispose()

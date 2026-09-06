# Remove the dev TAN APO. Run elevated. Finds the installed oemNNN.inf that
# matches tan-apo and deletes it, forcing removal even if in use.
$ErrorActionPreference = "Stop"

$id = [Security.Principal.WindowsIdentity]::GetCurrent()
if (-not (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole(
        [Security.Principal.WindowsBuiltinRole]::Administrator)) {
    throw "Run this in an elevated (Administrator) PowerShell."
}

# Find the published oem*.inf that came from tan-apo.inf.
$published = (pnputil /enum-drivers | Out-String) -split "Published Name:" |
    Where-Object { $_ -match "tan-apo\.inf" } |
    ForEach-Object { ($_ -split "`n")[0].Trim() }

if (-not $published) {
    Write-Host "No installed tan-apo driver package found."
} else {
    foreach ($oem in $published) {
        Write-Host "Deleting $oem ..."
        pnputil /delete-driver $oem /uninstall /force
    }
    Write-Host "Removed. Restart the audio service: net stop audiosrv; net start audiosrv"
}

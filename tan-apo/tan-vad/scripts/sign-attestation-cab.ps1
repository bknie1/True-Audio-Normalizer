<#
  sign-attestation-cab.ps1 - sign the attestation submission cab with your EV
  code-signing certificate. Partner Center requires the uploaded cab to be
  signed by the EV cert tied to your Hardware account.

  Pass the EV cert's thumbprint (from certmgr / the token software). If your EV
  cert is on a hardware token (most are), signtool will prompt for the token
  PIN - that is expected and only you can enter it.
#>
param(
    [Parameter(Mandatory = $true)][string]$Thumbprint,
    [string]$Cab = "$PSScriptRoot\..\dist\TanVad.cab",
    [string]$TimestampUrl = 'http://timestamp.digicert.com'
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path $Cab)) { throw "Cab not found: $Cab (run package-for-attestation.ps1 first)." }
$signtool = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe'

& $signtool sign /sha1 $Thumbprint /fd SHA256 /tr $TimestampUrl /td SHA256 $Cab
& $signtool verify /pa /v $Cab
Write-Host "Signed $Cab. Upload it at https://partner.microsoft.com/dashboard/hardware" -ForegroundColor Green

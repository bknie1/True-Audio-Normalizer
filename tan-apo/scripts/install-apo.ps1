# Dev install for the TAN APO. Test-signs the built DLL + INF and installs
# them. Requires: an elevated (admin) PowerShell, test-signing mode ON
# (bcdedit /set testsigning on, then reboot), and the WDK/SDK on PATH
# (run from a "Developer Command Prompt for VS 2022" or the x64 Native Tools).
#
# Assumes you've already built:
#   - tan.lib   : cargo build -p tan-ffi --release   (repo target\release\tan.lib)
#   - tan-apo.dll: the WDK APO project, linking tan.lib
# Point $ApoDll / $Inf at those outputs.

param(
    [string]$ApoDll = "$PSScriptRoot\..\build\tan-apo.dll",
    [string]$Inf    = "$PSScriptRoot\..\inf\tan-apo.inf",
    [string]$CertCN = "TAN Dev Test Cert"
)

$ErrorActionPreference = "Stop"

function Require-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole(
            [Security.Principal.WindowsBuiltinRole]::Administrator)) {
        throw "Run this in an elevated (Administrator) PowerShell."
    }
}
Require-Admin

# 1. A reusable self-signed code-signing cert in the local machine store.
$cert = Get-ChildItem Cert:\LocalMachine\My | Where-Object { $_.Subject -eq "CN=$CertCN" } | Select-Object -First 1
if (-not $cert) {
    Write-Host "Creating test cert '$CertCN'..."
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=$CertCN" `
        -CertStoreLocation Cert:\LocalMachine\My -KeyUsage DigitalSignature `
        -KeyExportPolicy Exportable -NotAfter (Get-Date).AddYears(5)
    # Trust it as a root + trusted publisher so Windows accepts the signature.
    $tmp = "$env:TEMP\tan-devcert.cer"
    Export-Certificate -Cert $cert -FilePath $tmp | Out-Null
    Import-Certificate -FilePath $tmp -CertStoreLocation Cert:\LocalMachine\Root | Out-Null
    Import-Certificate -FilePath $tmp -CertStoreLocation Cert:\LocalMachine\TrustedPublisher | Out-Null
    Remove-Item $tmp -Force
}

# 2. Sign the DLL, then build + sign a catalog for the INF package.
$signtool = (Get-Command signtool.exe -ErrorAction SilentlyContinue)?.Source
if (-not $signtool) { throw "signtool.exe not on PATH - open a Developer Command Prompt / x64 Native Tools." }
$thumb = $cert.Thumbprint

Write-Host "Signing $ApoDll ..."
& $signtool sign /sha1 $thumb /fd SHA256 /t http://timestamp.digicert.com "$ApoDll"

$pkgDir = Split-Path $Inf -Parent
Copy-Item $ApoDll $pkgDir -Force
Write-Host "Building catalog for $pkgDir ..."
& inf2cat /driver:"$pkgDir" /os:10_X64 /verbose
& $signtool sign /sha1 $thumb /fd SHA256 /t http://timestamp.digicert.com "$pkgDir\tan-apo.cat"

# 3. Install the driver package.
Write-Host "Installing INF ..."
pnputil /add-driver "$Inf" /install

Write-Host "Done. If TAN is an Extension on an endpoint, restart the audio"
Write-Host "service to reload effects:  net stop audiosrv; net start audiosrv"
Write-Host "(or replug/disable-enable the endpoint in Sound settings)."

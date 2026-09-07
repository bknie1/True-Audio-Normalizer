# Runs TAN over the PROVEN user-mode path: capture a virtual audio device via
# WASAPI loopback, run the TAN DSP, and play to your real output. This is the
# recommended way to use TAN on a normal (Secure Boot ON) machine - no kernel
# driver, no signing, $0. Requires:
#   - tan-live built:  cargo build -p tan-live --release
#   - a virtual audio device as the capture source (default: "CABLE Output"
#     from the free, Microsoft-signed VB-CABLE; or pass a Sonar channel).
# Set the apps you want normalized to play to that virtual device (e.g.
# "CABLE Input"), then run this. Stop it (Ctrl+C) to go back to raw audio.
param(
    [string]$From    = 'CABLE Output',                           # capture source (virtual device)
    [string]$Output  = 'Headphones (3- Arctis Nova Pro Wireless)', # where TAN plays the result
    [string]$Profile = 'movie',                                  # universal|movie|music|speech|night|game
    [int]$LatencyMs  = 200
)
$exe = Join-Path $PSScriptRoot '..\..\..\target\release\tan-live.exe'
if (-not (Test-Path $exe)) { throw "tan-live not built. Run: cargo build -p tan-live --release" }
Write-Host "TAN: capturing '$From' -> [$Profile] -> '$Output'. Ctrl+C to stop (raw)." -ForegroundColor Cyan
& $exe --loopback-from $From --output $Output --profile $Profile --latency-ms $LatencyMs

# Builds tan_ladspa.dll - the stereo TAN LADSPA plugin for ffmpeg's `ladspa`
# filter. Links the MSVC static build of tan-ffi (tan.lib), so build that first:
#   cargo build -p tan-ffi --release --target x86_64-pc-windows-msvc
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$lib = Join-Path $here '..\..\target\x86_64-pc-windows-msvc\release\tan.lib'
if (-not (Test-Path $lib)) { throw "tan.lib missing: $lib (cargo build -p tan-ffi --release --target x86_64-pc-windows-msvc)" }
$vcvars = 'D:\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat'
if (-not (Test-Path $vcvars)) { throw "vcvars64.bat not found: $vcvars" }

# Rust's std pulls in these system libs; link them alongside tan.lib.
$syslibs = 'ws2_32.lib userenv.lib ntdll.lib bcrypt.lib synchronization.lib advapi32.lib kernel32.lib'
$cl = "cl /nologo /LD /O2 /I `"$here`" `"$here\tan_ladspa.c`" `"$lib`" $syslibs " +
      "/Fe:`"$here\tan_ladspa.dll`" /Fo:`"$here\tan_ladspa.obj`" /link /DEF:`"$here\tan_ladspa.def`""

# A .def so ladspa_descriptor is exported with a clean name.
Set-Content -Path (Join-Path $here 'tan_ladspa.def') -Value "EXPORTS`r`n    ladspa_descriptor" -Encoding ascii

& cmd /c "`"$vcvars`" && $cl"
if (Test-Path "$here\tan_ladspa.dll") {
    # ffmpeg's ladspa filter looks for a `.so`; a PE loads fine under that name.
    Copy-Item "$here\tan_ladspa.dll" "$here\tan_ladspa.so" -Force
    Write-Host "Built $here\tan_ladspa.dll (+ tan_ladspa.so for ffmpeg)" -ForegroundColor Green
} else {
    throw "build failed"
}

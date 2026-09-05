; Inno Setup script for the TAN Windows installer.
;
; Built in CI (see .github/workflows/release.yml) with:
;   ISCC.exe /DAppVersion=<x.y.z> /DDistDir=<...\dist> /DOutDir=<...> tan-setup.iss
;
; DistDir must contain the release binaries (tan-tray.exe, tan-live.exe,
; tan-cli.exe, tan.dll) and the docs (README.md, ELI5.md, LICENSE).

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef DistDir
  #define DistDir "..\..\dist"
#endif
#ifndef OutDir
  #define OutDir "."
#endif

[Setup]
; A stable AppId keeps upgrades/uninstalls tied to one product across versions.
AppId={{7A3C1E44-2B9F-4D6A-9E21-5B8C0F3A7D12}
AppName=TAN
AppVersion={#AppVersion}
AppPublisher=Brandon Knieriem
AppPublisherURL=https://github.com/bknie1/True-Audio-Normalizer
DefaultDirName={autopf}\TAN
DefaultGroupName=TAN
DisableProgramGroupPage=yes
LicenseFile={#DistDir}\LICENSE
OutputDir={#OutDir}
OutputBaseFilename=tan-setup-{#AppVersion}
Compression=lzma
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\tan-tray.exe
; tan-tray is a windowless tray app, so Restart Manager's usual "detect and
; auto-close running apps" flow can't signal it to shut down gracefully -
; that's what was hanging and then failing with Access Denied on Ignore.
; tan-tray holds a mutex of this exact name for its whole lifetime (see
; hold_app_mutex() in tan-tray/src/main.rs); AppMutex is Inno's reliable,
; poll-based "is it still running" check, and turning off the RM-based
; auto-close avoids the two mechanisms fighting each other.
AppMutex=TANTrayAppMutex
CloseApplications=no

[Files]
Source: "{#DistDir}\tan-tray.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\tan-server.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\tan-live.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\tan-cli.exe";  DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\tan.dll";      DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\README.md";    DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\ELI5.md";      DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\LICENSE";      DestDir: "{app}"; Flags: ignoreversion

[Tasks]
Name: "startup"; Description: "Start TAN in the system tray when I sign in"; GroupDescription: "Startup:"

[Icons]
Name: "{group}\TAN (tray)";     Filename: "{app}\tan-tray.exe"
Name: "{group}\Uninstall TAN";  Filename: "{uninstallexe}"
Name: "{userstartup}\TAN";      Filename: "{app}\tan-tray.exe"; Tasks: startup

[Run]
Filename: "{app}\tan-tray.exe"; Description: "Launch TAN now"; Flags: nowait postinstall skipifsilent

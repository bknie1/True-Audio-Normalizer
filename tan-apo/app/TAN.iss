; Inno Setup script for TAN (user-mode path). Builds TAN-Setup.exe: a per-user
; installer (no admin needed) that installs the TAN Control GUI + the tan-live
; engine + helper scripts and creates shortcuts. The GUI detects and offers to
; install the free VB-CABLE virtual device at first run.
;
; Build:  "%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" TAN.iss
; Output: dist\TAN-Setup.exe   (unsigned; users get a SmartScreen "More info ->
;         Run anyway" until the exe is code-signed - a paid step, deferred.)

#define AppName "TAN - True Audio Normalizer"
#define AppVersion "0.0.6"

[Setup]
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Brandon Knieriem
DefaultDirName={autopf}\TAN
DefaultGroupName=TAN
DisableProgramGroupPage=yes
OutputDir=dist
OutputBaseFilename=TAN-Setup
Compression=lzma2
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
WizardStyle=modern
UninstallDisplayName={#AppName}

[Files]
Source: "TanControl.exe";                       DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\target\release\tan-live.exe";     DestDir: "{app}"; Flags: ignoreversion
Source: "..\tan-vad\scripts\tan-setup.ps1";      DestDir: "{app}"; Flags: ignoreversion
Source: "..\tan-vad\scripts\tan-forward.ps1";    DestDir: "{app}"; Flags: ignoreversion
Source: "README.txt";                            DestDir: "{app}"; Flags: isreadme

[Icons]
Name: "{group}\TAN Control";   Filename: "{app}\TanControl.exe"
Name: "{group}\Uninstall TAN"; Filename: "{uninstallexe}"
Name: "{autodesktop}\TAN Control"; Filename: "{app}\TanControl.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Run]
Filename: "{app}\TanControl.exe"; Description: "Launch TAN Control now"; Flags: nowait postinstall skipifsilent

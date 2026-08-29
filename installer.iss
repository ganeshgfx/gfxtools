[Setup]
AppName=GFX Tools
AppVersion=0.1.0
DefaultDirName={localappdata}\GFXTools
DisableProgramGroupPage=yes
OutputBaseFilename=GFXTools_Installer
Compression=lzma
SolidCompression=yes
PrivilegesRequired=lowest
UninstallDisplayIcon={app}\gfx-tools.exe
SetupIconFile=ico\main.ico

[Files]
Source: "target\release\gfx-tools.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "plugin\*"; DestDir: "{userappdata}\Adobe\CEP\extensions\GFXTools"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "target\release\gfx-tools.exe"; DestDir: "{userappdata}\Adobe\CEP\extensions\GFXTools\bin"; Flags: ignoreversion

[Registry]
Root: HKCU; Subkey: "Software\Adobe\CSXS.10"; ValueType: string; ValueName: "PlayerDebugMode"; ValueData: "1"
Root: HKCU; Subkey: "Software\Adobe\CSXS.11"; ValueType: string; ValueName: "PlayerDebugMode"; ValueData: "1"
Root: HKCU; Subkey: "Software\Adobe\CSXS.12"; ValueType: string; ValueName: "PlayerDebugMode"; ValueData: "1"
Root: HKCU; Subkey: "Software\Adobe\CSXS.13"; ValueType: string; ValueName: "PlayerDebugMode"; ValueData: "1"
Root: HKCU; Subkey: "Software\Adobe\CSXS.14"; ValueType: string; ValueName: "PlayerDebugMode"; ValueData: "1"
Root: HKCU; Subkey: "Software\Adobe\CSXS.15"; ValueType: string; ValueName: "PlayerDebugMode"; ValueData: "1"
Root: HKCU; Subkey: "Software\Adobe\CSXS.16"; ValueType: string; ValueName: "PlayerDebugMode"; ValueData: "1"

[Run]
Filename: "{app}\gfx-tools.exe"; Parameters: "install"; Flags: runhidden

[UninstallRun]
Filename: "{app}\gfx-tools.exe"; Parameters: "uninstall"; Flags: runhidden

[UninstallDelete]
Type: filesandordirs; Name: "{userappdata}\Adobe\CEP\extensions\GFXTools"
Type: filesandordirs; Name: "{app}"

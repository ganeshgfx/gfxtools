[Setup]
AppName=GFX Tools
AppVersion=0.1.0
DefaultDirName={localappdata}\GFXTools
DefaultGroupName=GFX Tools
DisableProgramGroupPage=yes
OutputBaseFilename=GFXTools_Installer
Compression=lzma
SolidCompression=yes
PrivilegesRequired=lowest
UninstallDisplayIcon={app}\gfx-tools.exe
SetupIconFile=ico\main.ico

[Icons]
; Start Menu — GFX Tools folder
Name: "{userprograms}\GFX Tools\Settings";    Filename: "{app}\gfx-tools.exe"; Parameters: "--settings";    IconFilename: "{app}\gfx-tools.exe"; Comment: "Open GFX Tools settings"
Name: "{userprograms}\GFX Tools\Diagnostics"; Filename: "{app}\gfx-tools.exe"; Parameters: "--diagnostics"; IconFilename: "{app}\gfx-tools.exe"; Comment: "Check yt-dlp / FFmpeg / gallery-dl installation"
Name: "{userprograms}\GFX Tools\Uninstall GFX Tools"; Filename: "{uninstallexe}"; Comment: "Remove GFX Tools"

[Files]
Source: "target\release\gfx-tools.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "scripts\download-deps.ps1"; DestDir: "{app}"; Flags: ignoreversion
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
Filename: "powershell.exe"; Parameters: "-ExecutionPolicy Bypass -File ""{app}\download-deps.ps1"" -InstallDir ""{app}"""; Flags: runhidden; StatusMsg: "Downloading yt-dlp, FFmpeg, gallery-dl (this may take a moment)..."

[UninstallRun]
Filename: "{app}\gfx-tools.exe"; Parameters: "uninstall"; Flags: runhidden

[UninstallDelete]
Type: filesandordirs; Name: "{userappdata}\Adobe\CEP\extensions\GFXTools"
Type: filesandordirs; Name: "{app}"

[Code]
// Auto-uninstall any previous installation before installing the new version.
// Looks for unins000.exe at the fixed install location and runs it /SILENT.
function InitializeSetup(): Boolean;
var
  UninsExe: String;
  ResultCode: Integer;
begin
  UninsExe := ExpandConstant('{localappdata}\GFXTools\unins000.exe');
  if FileExists(UninsExe) then begin
    if MsgBox('A previous version of GFX Tools was detected. Would you like to uninstall it before continuing? (Recommended)', mbConfirmation, MB_YESNO) = idYes then begin
      Exec(UninsExe, '/SILENT /NORESTART', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    end;
  end;
  Result := True;
end;

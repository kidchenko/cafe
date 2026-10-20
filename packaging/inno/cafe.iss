; Inno Setup script for Cafe.
;   iscc /DAppVersion=x.y.z packaging\inno\cafe.iss
; Expects target\release\cafe.exe to already be built (build.ps1 does both).

#define AppName      "Cafe"
#ifndef AppVersion
  #define AppVersion "0.0.1"
#endif
#define AppPublisher "kidchenko"
#define AppURL       "https://github.com/kidchenko/cafe"
#define AppExe       "cafe.exe"

[Setup]
; Never reuse this GUID for another product: it is how Windows matches
; upgrades and uninstalls, and winget's ProductCode is derived from it.
AppId={{CBD8B671-D528-47A2-9E6A-A61E7CDC5937}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
DisableDirPage=auto
LicenseFile=..\..\LICENSE
OutputDir=..\..\dist
OutputBaseFilename=Cafe-Setup-{#AppVersion}
SetupIconFile=..\..\assets\icon.ico
UninstallDisplayIcon={app}\{#AppExe}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; The app itself never needs elevation; only Program Files does.
PrivilegesRequired=admin
; Close a running instance on upgrade/uninstall instead of hitting a locked exe.
CloseApplications=yes
CloseApplicationsFilter=*.exe
; [Tasks] and [Code] touch HKCU on purpose; see the notes there.
UsedUserAreasWarning=no
MinVersion=10.0

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
; Ticked by default: a keep-awake tool you forget to start after a reboot is
; the one that lets the machine sleep mid-download. Written to HKCU under the
; same value name the app's own "Run at login" toggle uses, so the two agree.
Name: "autostart"; Description: "Start {#AppName} when I sign in"; \
  GroupDescription: "Startup:"

[Files]
Source: "..\..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{group}\Uninstall {#AppName}"; Filename: "{uninstallexe}"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; \
  ValueType: string; ValueName: "{#AppName}"; ValueData: """{app}\{#AppExe}"""; \
  Tasks: autostart; Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#AppExe}"; Description: "Launch {#AppName}"; \
  Flags: nowait postinstall skipifsilent

[UninstallRun]
; Quitting releases the keep-awake request; the OS would drop it on process
; exit anyway, so a hard kill is safe here.
Filename: "{sys}\taskkill.exe"; Parameters: "/f /im {#AppExe}"; \
  Flags: runhidden; RunOnceId: "KillCafe"

[UninstallDelete]
Type: files; Name: "{userappdata}\{#AppName}\cafe.log"
Type: files; Name: "{userappdata}\{#AppName}\cafe.log.1"

[Code]
// The app's own "Run at login" toggle writes the same HKCU value; clear it
// even if the installer task was never ticked. Under UAC an admin elevating
// their own account keeps the same HKCU, which is the normal case.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RegDeleteValue(HKEY_CURRENT_USER,
      'Software\Microsoft\Windows\CurrentVersion\Run', '{#AppName}');
    // Only succeeds once config.json is gone: settings survive a reinstall.
    RemoveDir(ExpandConstant('{userappdata}\{#AppName}'));
    RemoveDir(ExpandConstant('{app}'));
  end;
end;

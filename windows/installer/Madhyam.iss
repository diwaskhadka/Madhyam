; Madhyam setup (Inno Setup 6). A regular Windows installer: wizard, Start menu and optional
; desktop shortcut, uninstaller in Settings > Apps. Installs for the current user without an admin
; prompt (or for all users when run as admin). Re-running a newer setup upgrades in place.
; Playlists and settings live in %LOCALAPPDATA%\Madhyam and are never touched by setup
; (the app moves Madhyam 0.4.x's %LOCALAPPDATA%\Madhyam there on first start).
;
; Build: iscc /DAppVersion=0.4.11 /DSourceDir=<publish folder> /DOutputDir=<dir> Madhyam.iss

; Display name from branding/brand.ini (shared by every platform).
#define AppName ReadIni(AddBackslash(SourcePath) + "..\..\branding\brand.ini", "brand", "name", "Madhyam")
; Fixed from 0.9.0 on: the exe name (shortcuts point at it). Before 0.9.0 it was Madhyam.exe.
#define AppExe "Madhyam.exe"

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\publish\Madhyam"
#endif
#ifndef OutputDir
  #define OutputDir "."
#endif

[Setup]
; Never change AppId: setup uses it to find the installed copy to upgrade. It is Madhyam
; 0.4.x's id, so 0.9.0 upgrades those installs (in their old folder) instead of adding a second app.
AppId={{6B0E4C4D-7E2A-4C1B-9B57-2E7F0C1A9D41}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Diwas Khadka
AppPublisherURL=https://github.com/diwaskhadka/Madhyam
VersionInfoVersion={#AppVersion}
; New installs only; an upgrade keeps the folder it was installed in (UsePreviousAppDir).
DefaultDirName={autopf}\Madhyam
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
SetupIconFile=..\Madhyam.Windows\Assets\app.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
OutputDir={#OutputDir}
OutputBaseFilename=Madhyam-Setup-{#AppVersion}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Shortcuts:"

[InstallDelete]
; Drop files of the previous version (e.g. removed VLC plugins) before copying the new ones.
Type: filesandordirs; Name: "{app}\libvlc"
; Upgrading from Madhyam 0.4.x: remove the old exe and shortcuts, so one app is left.
Type: files; Name: "{app}\Madhyam.exe"
Type: files; Name: "{autoprograms}\Madhyam.lnk"
Type: files; Name: "{autodesktop}\Madhyam.lnk"

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Excludes: "README-FIRST.txt"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "Start {#AppName}"; Flags: nowait postinstall skipifsilent

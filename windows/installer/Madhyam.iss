; Madhyam setup (Inno Setup 6). A regular Windows installer: wizard, Start menu and optional
; desktop shortcut, uninstaller in Settings > Apps. Installs for the current user without an admin
; prompt (or for all users when run as admin). Re-running a newer setup upgrades in place.
; Playlists and settings live in %LOCALAPPDATA%\Madhyam and are never touched by setup.
;
; Build: iscc /DAppVersion=0.4.11 /DSourceDir=<publish folder> /DOutputDir=<dir> Madhyam.iss

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
AppId={{6B0E4C4D-7E2A-4C1B-9B57-2E7F0C1A9D41}
AppName=Madhyam
AppVersion={#AppVersion}
AppVerName=Madhyam {#AppVersion}
AppPublisher=Diwas Khadka
AppPublisherURL=https://github.com/diwaskhadka/Madhyam
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\Madhyam
DefaultGroupName=Madhyam
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
SetupIconFile=..\Madhyam.Windows\Assets\Madhyam.ico
UninstallDisplayIcon={app}\Madhyam.exe
UninstallDisplayName=Madhyam
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

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Excludes: "README-FIRST.txt"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Madhyam"; Filename: "{app}\Madhyam.exe"
Name: "{autodesktop}\Madhyam"; Filename: "{app}\Madhyam.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\Madhyam.exe"; Description: "Start Madhyam"; Flags: nowait postinstall skipifsilent

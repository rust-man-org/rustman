; Rustman Windows installer (Inno Setup 6).
;
; Why this exists: the Windows release ships a bare `rustman.exe` in a zip.
; A zip alone has no Start Menu entry, uninstall entry, or optional desktop
; shortcut. This script builds a setup.exe that installs those, matching what
; packaging/linux/install-desktop-entry.sh does for the Linux tarball.
;
; Local build (after `cargo build --release`):
;   "%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" packaging\windows\rustman.iss
;   or:  "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" packaging\windows\rustman.iss
;
; CI overrides version and arch:
;   ISCC.exe /DMyAppVersion=0.4.0 /DMyArch=x86_64 packaging\windows\rustman.iss
;
; Keep MyAppVersion in sync with Cargo.toml when building by hand.

#ifndef MyAppVersion
  #define MyAppVersion "0.4.0"
#endif
#ifndef MyArch
  #define MyArch "x86_64"
#endif

#define MyAppName "Rustman"
#define MyAppPublisher "animeshchaudhri"
#define MyAppURL "https://github.com/rust-man-org/rustman"
#define MyAppExeName "rustman.exe"

[Setup]
; Stable across releases so Windows upgrades the same install.
AppId={{A8E3C9F1-4B2D-4E7A-9C1F-6D8E2A5B0F34}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
LicenseFile=..\..\LICENSE
OutputDir=..\..\dist
OutputBaseFilename=Rustman-{#MyAppVersion}-setup-{#MyArch}
SetupIconFile=..\..\public\icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; Per-user install by default; the dialog can elevate for all users.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
DisableProgramGroupPage=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

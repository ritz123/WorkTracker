; Inno Setup script for the Windows release.
;
; Installs WorkTracker.exe for the current user, adds a Start menu shortcut,
; and offers a desktop shortcut. AppId stays fixed so a newer setup upgrades
; the same installation instead of creating a second copy. The build passes
; the release version with /DAppVersion=MAJOR.MINOR.PATCH, and the program
; file is stage\WorkTracker.exe next to this script.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{8F4C2A91-6D3E-4B70-9A15-2C7E5B84D0F6}
AppName=Work Tracker
AppVersion={#AppVersion}
AppPublisher=Biplab Sarkar
DefaultDirName={autopf}\Work Tracker
DefaultGroupName=Work Tracker
DisableProgramGroupPage=yes
OutputDir=output
OutputBaseFilename=WorkTracker-{#AppVersion}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
CloseApplications=yes
UninstallDisplayName=Work Tracker
; Shown on the setup file itself. The installed shortcut uses the icon
; embedded in WorkTracker.exe, which is the same clock.
SetupIconFile=..\icon\worktracker.ico
UninstallDisplayIcon={app}\WorkTracker.exe

[Files]
Source: "stage\WorkTracker.exe"; DestDir: "{app}"; DestName: "WorkTracker.exe"; Flags: ignoreversion

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"

[Icons]
Name: "{autoprograms}\Work Tracker"; Filename: "{app}\WorkTracker.exe"
Name: "{autodesktop}\Work Tracker"; Filename: "{app}\WorkTracker.exe"; Tasks: desktopicon

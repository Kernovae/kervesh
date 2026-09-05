#define AppVersion "0.1.0"
[Setup]
AppName=Kervesh
AppVersion={#AppVersion}
AppPublisher=Kernovae
DefaultDirName={localappdata}\Programs\Kervesh
DefaultGroupName=Kervesh
PrivilegesRequired=lowest
OutputDir=..\..\artifacts
OutputBaseFilename=kervesh-{#AppVersion}-windows-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile=..\..\LICENSE
[Files]
Source: "..\..\target\release\kervesh.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"
[Icons]
Name: "{group}\Kervesh"; Filename: "{app}\kervesh.exe"
[Run]
Filename: "{app}\kervesh.exe"; Description: "Open Kervesh"; Flags: nowait postinstall skipifsilent

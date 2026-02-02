[Setup]
AppName=Nvidia
AppVersion=1.0
DefaultDirName={autopf}\Nvidia
DefaultGroupName=Nvidia
OutputDir=installer
OutputBaseFilename=Nvidia-Setup
Compression=lzma2
SolidCompression=yes
PrivilegesRequired=admin
SetupIconFile=icon.ico
UninstallDisplayIcon={app}\interview_helper.exe

[Files]
Source: "target\release\interview_helper.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "cloudflared.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "config.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "profile.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "config.yml"; DestDir: "{app}"; Flags: ignoreversion
Source: "credentials.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "static\*"; DestDir: "{app}\static"; Flags: ignoreversion recursesubdirs
Source: "start_hidden.bat"; DestDir: "{app}"; Flags: ignoreversion
Source: "start.vbs"; DestDir: "{app}"; DestName: "Nvidia.vbs"; Flags: ignoreversion
Source: "stop.bat"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Nvidia"; Filename: "wscript.exe"; Parameters: """{app}\Nvidia.vbs"""; WorkingDir: "{app}"; IconFilename: "{app}\interview_helper.exe"
Name: "{group}\Stop Nvidia"; Filename: "{app}\stop.bat"; WorkingDir: "{app}"
Name: "{autodesktop}\Nvidia"; Filename: "wscript.exe"; Parameters: """{app}\Nvidia.vbs"""; WorkingDir: "{app}"; IconFilename: "{app}\interview_helper.exe"

[Run]
Filename: "wscript.exe"; Parameters: """{app}\Nvidia.vbs"""; Description: "Launch Nvidia"; Flags: postinstall nowait skipifsilent

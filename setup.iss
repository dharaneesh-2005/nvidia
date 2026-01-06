[Setup]
AppName=Interview Helper
AppVersion=1.0
DefaultDirName={autopf}\InterviewHelper
DefaultGroupName=Interview Helper
OutputDir=installer
OutputBaseFilename=InterviewHelper-Setup
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
Source: "start.vbs"; DestDir: "{app}"; DestName: "InterviewHelper.vbs"; Flags: ignoreversion
Source: "stop.bat"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Interview Helper"; Filename: "wscript.exe"; Parameters: """{app}\InterviewHelper.vbs"""; WorkingDir: "{app}"; IconFilename: "{app}\interview_helper.exe"
Name: "{group}\Stop Interview Helper"; Filename: "{app}\stop.bat"; WorkingDir: "{app}"
Name: "{autodesktop}\Interview Helper"; Filename: "wscript.exe"; Parameters: """{app}\InterviewHelper.vbs"""; WorkingDir: "{app}"; IconFilename: "{app}\interview_helper.exe"

[Run]
Filename: "wscript.exe"; Parameters: """{app}\InterviewHelper.vbs"""; Description: "Launch Interview Helper"; Flags: postinstall nowait skipifsilent

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
Source: "add_custom_domain.bat"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Nvidia"; Filename: "wscript.exe"; Parameters: """{app}\Nvidia.vbs"""; WorkingDir: "{app}"; IconFilename: "{app}\interview_helper.exe"
Name: "{group}\Stop Nvidia"; Filename: "{app}\stop.bat"; WorkingDir: "{app}"
Name: "{autodesktop}\Nvidia"; Filename: "wscript.exe"; Parameters: """{app}\Nvidia.vbs"""; WorkingDir: "{app}"; IconFilename: "{app}\interview_helper.exe"

[Run]
Filename: "wscript.exe"; Parameters: """{app}\Nvidia.vbs"""; Description: "Launch Nvidia"; Flags: postinstall nowait skipifsilent

[Code]
var
  DomainPage: TInputOptionWizardPage;
  CustomSubdomainPage: TInputQueryWizardPage;
  SelectedDomain: String;

procedure InitializeWizard;
begin
  // Create domain selection page
  DomainPage := CreateInputOptionPage(wpSelectDir,
    'Domain Configuration', 'Choose your subdomain for pinmypic.online',
    'Select how you want to access your application:',
    True, False);
  
  DomainPage.Add('Helper subdomain (helper.pinmypic.online)');
  DomainPage.Add('Custom subdomain');
  
  DomainPage.Values[0] := True; // Default to helper subdomain
  
  // Create custom subdomain input page
  CustomSubdomainPage := CreateInputQueryPage(DomainPage.ID,
    'Custom Subdomain', 'Enter your custom subdomain',
    'Enter the subdomain name (without .pinmypic.online):');
  
  CustomSubdomainPage.Add('Subdomain name:', False);
  CustomSubdomainPage.Values[0] := 'myapp';
end;

function ShouldSkipPage(PageID: Integer): Boolean;
begin
  // Skip custom subdomain page if not selected
  if PageID = CustomSubdomainPage.ID then
    Result := not DomainPage.Values[1]
  else
    Result := False;
end;

function GetSelectedDomain(): String;
var
  CustomSub: String;
begin
  if DomainPage.Values[0] then
    Result := 'helper.pinmypic.online'
  else if DomainPage.Values[1] then
  begin
    CustomSub := Trim(CustomSubdomainPage.Values[0]);
    if CustomSub = '' then
      Result := 'helper.pinmypic.online'
    else
      Result := CustomSub + '.pinmypic.online';
  end
  else
    Result := 'helper.pinmypic.online';
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ConfigContent: TArrayOfString;
  ConfigFile: String;
  DomainFile: String;
  I: Integer;
  ResultCode: Integer;
begin
  if CurStep = ssPostInstall then
  begin
    SelectedDomain := GetSelectedDomain();
    
    // Update config.yml
    ConfigFile := ExpandConstant('{app}\config.yml');
    SetArrayLength(ConfigContent, 7);
    ConfigContent[0] := 'tunnel: interview-helper';
    ConfigContent[1] := 'credentials-file: credentials.json';
    ConfigContent[2] := '';
    ConfigContent[3] := 'ingress:';
    ConfigContent[4] := '  - hostname: ' + SelectedDomain;
    ConfigContent[5] := '    service: http://localhost:5000';
    ConfigContent[6] := '  - service: http_status:404';
    
    SaveStringsToFile(ConfigFile, ConfigContent, False);
    
    // Save domain to domain.txt
    DomainFile := ExpandConstant('{app}\domain.txt');
    SetArrayLength(ConfigContent, 1);
    ConfigContent[0] := SelectedDomain;
    SaveStringsToFile(DomainFile, ConfigContent, False);
    
    // If custom domain selected, add DNS route
    if (SelectedDomain <> 'helper.pinmypic.online') then
    begin
      if MsgBox('Would you like to add the DNS route for ' + SelectedDomain + ' to Cloudflare now?' + #13#10 + #13#10 +
                'This requires cloudflared to be configured with your Cloudflare account.' + #13#10 + #13#10 +
                'You can also do this later by running add_custom_domain.bat', 
                mbConfirmation, MB_YESNO) = IDYES then
      begin
        Exec(ExpandConstant('{app}\cloudflared.exe'), 
             'tunnel route dns interview-helper ' + SelectedDomain, 
             ExpandConstant('{app}'), 
             SW_SHOW, 
             ewWaitUntilTerminated, 
             ResultCode);
        
        if ResultCode = 0 then
          MsgBox('DNS route added successfully!' + #13#10 + #13#10 +
                 'Your app will be accessible at:' + #13#10 +
                 'https://' + SelectedDomain, mbInformation, MB_OK)
        else
          MsgBox('DNS route setup failed.' + #13#10 + #13#10 +
                 'Please run add_custom_domain.bat after installation to complete setup.', 
                 mbError, MB_OK);
      end;
    end
    else
    begin
      // Show success message for helper subdomain
      MsgBox('Domain configured successfully!' + #13#10 + #13#10 +
             'Your app will be accessible at:' + #13#10 +
             'https://' + SelectedDomain, mbInformation, MB_OK);
    end;
  end;
end;

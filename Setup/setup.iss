#define AppEnName "EyeNurse"
#define OutputPath="..\EyeNurse.Client\bin\Release\"
#define ProcessName="EyeNurse"
#define AppVersion=GetFileVersion("..\EyeNurse.Client\bin\Release\EyeNurse.exe")

[Setup]
AppName={cm:AppName}                                
AppId={#AppEnName}
AppVerName={cm:AppName} v{#AppVersion}
DefaultDirName={pf}\{#AppEnName}
DefaultGroupName={cm:AppName}
CloseApplications=force
UninstallDisplayIcon={app}\EyeNurse.exe
VersionInfoDescription={cm:Description}
VersionInfoProductName={cm:AppName}
OutputDir={#SourcePath}
OutputBaseFilename={#AppEnName}Setup_{#AppVersion}

[Languages]
Name: en; MessagesFile: "compiler:Default.isl"
Name: chs; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[Messages]
en.BeveledLabel=English
chs.BeveledLabel=中文

[CustomMessages]
en.Description=Eye care for PC user
en.AppName={#AppEnName}
en.LaunchApp="Launch application"
chs.Description=保护重度电脑使用者的氪金*眼
chs.AppName=眼睛护士
chs.LaunchApp=启动程序

[Code]
procedure TaskKill(FileName: String);
var
  ResultCode: Integer;
begin
    Exec(ExpandConstant('taskkill.exe'), '/f /im ' + '"' + FileName + '"', '', SW_HIDE,
     ewWaitUntilTerminated, ResultCode);
end;

[Files]
Source:{#OutputPath}*.exe;DestDir: "{app}" ;Flags: ignoreversion; BeforeInstall: TaskKill('{#ProcessName}.exe')
Source:{#OutputPath}*.dll;DestDir: "{app}"
Source:{#OutputPath}Resources\* ;DestDir: "{app}\Resources"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{group}\{cm:AppName}"; Filename: "{app}\EyeNurse.exe"
Name: "{group}\{cm:UninstallProgram,{cm:AppName}}"; Filename: "{uninstallexe}"

[Tasks]
; The following task doesn't do anything and is only meant to show [CustomMessages] usage
; Name: mytask; Description: "{cm:Description}"

[Run]
Filename: "{app}\EyeNurse.EXE"; Description: {cm:LaunchApp}; Flags: nowait postinstall skipifsilent 

[UninstallRun]
Filename: "{cmd}"; Parameters: "/C ""taskkill /im {#ProcessName}.exe /f /t"

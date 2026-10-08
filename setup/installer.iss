; v3 安装包脚本（一致性差异 #8 / 清单 J4；结构对齐 v2 src/setup/installer.iss）。
; 由 Inno Setup（ISCC.exe）编译；输入 = setup/publish/（compile.ps1 暂存），输出 = setup/dist/。

#define MyAppVersion GetFileVersion(".\publish\eyenurse.exe")
#define MyAppName "EyeNurse"
#define MyAppNameVersion "EyeNurse3"
#define MyAppPublisher "Giantapp Studio"
#define MyAppURL "https://www.giantapp.cn"
#define MyAppExeName "eyenurse.exe"
#define OutputDir=".\dist"
#define ProductNameEn="Eye Nurse 3"
#define ProductNameZh="眼睛护士3"

[Setup]
; v3 独立 AppId（与 v2 安装包互不覆盖；生成一次后不得更改）
AppId={{7E4F2A10-93C8-4B5D-A6E1-2C0D8B9F13A6}
AppName={cm:AppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} v{#MyAppVersion}
VersionInfoVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppNameVersion}
DisableProgramGroupPage=yes
; 与 v2 一致：按用户安装（PrivilegesRequired=lowest）
PrivilegesRequired=lowest
OutputDir={#OutputDir}
OutputBaseFilename={#MyAppName} Installer v{#MyAppVersion}
VersionInfoDescription={#MyAppName} Installer v{#MyAppVersion}
VersionInfoProductName={#MyAppName} Installer v{#MyAppVersion}
SetupIconFile=..\assets\logo.ico
Compression=lzma
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[CustomMessages]
english.AppName={#ProductNameEn}
chinesesimplified.AppName={#ProductNameZh}

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}";

[Files]
Source: "publish\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "publish\*"; DestDir: "{app}"; Excludes: "*.pdb"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{cm:AppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{cm:AppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{cmd}"; Parameters: "/C ""taskkill /im {#MyAppExeName} /f /t"; RunOnceId: "killprocss";

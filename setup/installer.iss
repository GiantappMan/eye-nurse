; v3 安装包脚本（一致性差异 #8 / 清单 J4；结构对齐 v2 src/setup/installer.iss）。
; 由 Inno Setup（ISCC.exe）编译；输入 = setup/publish/（compile.ps1 暂存），输出 = setup/dist/。
; WebView2 Runtime 检测/引导见 [Code]（清单 J4：无 WebView2 时设置窗与休息遮罩页均
; 无法渲染，干净机安装前必须引导）。

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
; WebView2 Runtime 引导文案（检测/下载/安装逻辑见 [Code]）
english.WebView2Missing=Eye Nurse 3 requires the Microsoft WebView2 Runtime, which was not detected on this computer.%n%nDownload and install it now? (internet connection required)
chinesesimplified.WebView2Missing=眼睛护士3 需要 Microsoft WebView2 Runtime，未在本机检测到。%n%n是否现在下载并安装？（需要联网）
english.WebView2MissingExit=Setup will now exit. Please install the WebView2 Runtime and run Setup again.
chinesesimplified.WebView2MissingExit=安装程序将退出。请先安装 WebView2 Runtime 后重新运行安装程序。
english.WebView2DownloadFailed=Failed to download the WebView2 Runtime installer. Please check your network connection, or install it manually from:%n%nhttps://developer.microsoft.com/microsoft-edge/webview2/
chinesesimplified.WebView2DownloadFailed=下载 WebView2 Runtime 安装器失败。请检查网络连接，或从以下地址手动安装：%n%nhttps://developer.microsoft.com/microsoft-edge/webview2/
english.WebView2InstallFailed=The WebView2 Runtime still was not detected after the installer ran. Eye Nurse 3 cannot run without it. Setup will now exit.
chinesesimplified.WebView2InstallFailed=运行安装器后仍未检测到 WebView2 Runtime，眼睛护士3 无法在没有它的情况下运行。安装程序将退出。

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

[Code]
// —— WebView2 Runtime 检测与引导（清单 J4 / 架构 §13）——
// 检测：Evergreen Runtime 的注册表指纹（EdgeUpdate\Clients\{F3017226-…} 的 pv 值，
// 非空且非占位 0.0.0.0），覆盖 per-user（HKCU）与 machine（HKLM 原生视图 +
// WOW6432Node，与 64/32 位注册表视图无关地全覆盖）。
// 引导：缺失 → 询问用户 → 经 ServerXMLHTTP 下载官方 Evergreen Bootstrapper
// （fwlink 2124703）到 {tmp} 并执行（不静默：bootstrapper 自带进度 UI；本安装包
// PrivilegesRequired=lowest，无提权时 bootstrapper 自动走 per-user 安装）→ 装完
// 复检，仍缺失则终止安装。
const
  WebView2ClientId = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';
  WebView2BootstrapperUrl = 'https://go.microsoft.com/fwlink/p/?LinkId=2124703';

function WebView2PvAt(const RootKey: Integer; const SubKey: String): String;
var
  Pv: String;
begin
  Result := '';
  if RegQueryStringValue(RootKey, SubKey, 'pv', Pv) then
    if (Pv <> '') and (Pv <> '0.0.0.0') then
      Result := Pv;
end;

function WebView2RuntimeVersion: String;
begin
  Result := WebView2PvAt(HKCU, 'Software\Microsoft\EdgeUpdate\Clients\' + WebView2ClientId);
  if Result = '' then
    Result := WebView2PvAt(HKLM, 'SOFTWARE\Microsoft\EdgeUpdate\Clients\' + WebView2ClientId);
  if Result = '' then
    Result := WebView2PvAt(HKLM, 'SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\' + WebView2ClientId);
end;

function DownloadToFile(const Url, Dest: String): Boolean;
var
  Http, Stream: Variant;
begin
  Result := False;
  try
    Http := CreateOleObject('Msxml2.ServerXMLHTTP.6.0');
    Http.Open('GET', Url, False);
    Http.Send('');
    if Http.Status <> 200 then
      Exit;
    Stream := CreateOleObject('ADODB.Stream');
    Stream.Type := 1; // adTypeBinary
    Stream.Open;
    Stream.Write(Http.ResponseBody);
    Stream.SaveToFile(Dest, 2); // adSaveCreateOverWrite
    Stream.Close;
    Result := True;
  except
    on E: Exception do
      Result := False; // 网络/COM 异常一律按下载失败处理（下方给出手动安装指引后终止）
  end;
end;

function InitializeSetup(): Boolean;
var
  Bootstrapper: String;
  ResultCode: Integer;
begin
  Result := True;
  if WebView2RuntimeVersion <> '' then
    Exit;

  if MsgBox(ExpandConstant('{cm:WebView2Missing}'), mbConfirmation, MB_YESNO) = IDNO then
  begin
    MsgBox(ExpandConstant('{cm:WebView2MissingExit}'), mbError, MB_OK);
    Result := False;
    Exit;
  end;

  Bootstrapper := ExpandConstant('{tmp}\MicrosoftEdgeWebview2Setup.exe');
  if not DownloadToFile(WebView2BootstrapperUrl, Bootstrapper) then
  begin
    MsgBox(ExpandConstant('{cm:WebView2DownloadFailed}'), mbError, MB_OK);
    Result := False;
    Exit;
  end;

  if not Exec(Bootstrapper, '/install', '', SW_SHOW, ewWaitUntilTerminated, ResultCode) or
     (WebView2RuntimeVersion = '') then
  begin
    MsgBox(ExpandConstant('{cm:WebView2InstallFailed}'), mbError, MB_OK);
    Result := False;
  end;
end;

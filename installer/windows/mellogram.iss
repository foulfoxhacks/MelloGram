; MelloGram Windows installer (Inno Setup 6).
;
; Built in CI after `cargo tauri build`:
;   ISCC.exe /DAppVersion=0.1.0 /DSourceDir=<repo>\target\release installer\windows\mellogram.iss
;
; - Per-user install by default (no admin prompt); "Install for all users" is
;   offered on the first page and elevates only then.
; - Branded wizard art lives in installer\windows\art (regenerate with
;   branding/make_assets.py).
; - Microsoft WebView2 is installed only when it is missing, using the small
;   official bootstrapper bundled next to this script by CI.

#ifndef AppVersion
  #define AppVersion "0.0.0-dev"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\target\release"
#endif

#define AppName "MelloGram"
#define AppExe "MelloGram.exe"
#define AppPublisher "FoulFoxHacks"
#define AppURL "https://mellogram.foulfoxhacks.dev"

[Setup]
; Never change AppId: it links upgrades and the uninstaller to earlier installs.
AppId={{6F1D3B0E-6C8A-4F7E-9D2B-5A4E2C9B7D11}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/faq/
AppUpdatesURL={#AppURL}/download/
AppCopyright=Copyright (C) FoulFoxHacks. Licensed under GPL-3.0-or-later.
VersionInfoVersion={#AppVersion}
VersionInfoCompany={#AppPublisher}
VersionInfoDescription={#AppName} Setup
VersionInfoProductName={#AppName}

DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
DisableDirPage=auto
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763

OutputDir=Output
OutputBaseFilename=MelloGram-Setup-x64
SetupIconFile=..\..\app\src-tauri\icons\icon.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
Compression=lzma2/ultra64
SolidCompression=yes
LZMAUseSeparateProcess=yes

WizardStyle=modern
WizardSizePercent=110
WizardImageFile=art\wizard-side-100.bmp,art\wizard-side-150.bmp,art\wizard-side-200.bmp
WizardSmallImageFile=art\wizard-small-100.bmp,art\wizard-small-150.bmp,art\wizard-small-200.bmp
WizardImageStretch=no

CloseApplications=yes
RestartApplications=no
ChangesAssociations=no
ShowLanguageDialog=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Messages]
WelcomeLabel1=Welcome to MelloGram
WelcomeLabel2=This will install [name/ver], the customizable Telegram client, on your computer.%n%nYou sign in with your normal Telegram account. Plugins, themes and wallpapers are all built in.
FinishedHeadingLabel=MelloGram is ready
FinishedLabel=MelloGram has been installed. Sign in with your Telegram account, then press Ctrl+, to make it yours.

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Shortcuts:"

[Files]
Source: "{#SourceDir}\mellogram.exe"; DestDir: "{app}"; DestName: "{#AppExe}"; Flags: ignoreversion
Source: "MicrosoftEdgeWebview2Setup.exe"; DestDir: "{tmp}"; Flags: dontcopy

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; Comment: "The customizable Telegram client"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "Launch MelloGram"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Remove the launch-at-login entry MelloGram may have created.
Filename: "{sys}\reg.exe"; Parameters: "delete ""HKCU\Software\Microsoft\Windows\CurrentVersion\Run"" /v ""MelloGram"" /f"; Flags: runhidden; RunOnceId: "RemoveAutostart"

[Code]
const
  WebView2Key = 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';

function WebView2Installed: Boolean;
var
  Version: String;
begin
  Result :=
    (RegQueryStringValue(HKLM64, WebView2Key, 'pv', Version) and (Version <> '') and (Version <> '0.0.0.0')) or
    (RegQueryStringValue(HKLM32, WebView2Key, 'pv', Version) and (Version <> '') and (Version <> '0.0.0.0')) or
    (RegQueryStringValue(HKCU, WebView2Key, 'pv', Version) and (Version <> '') and (Version <> '0.0.0.0'));
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ResultCode: Integer;
begin
  Result := '';
  if not WebView2Installed then
  begin
    WizardForm.PreparingLabel.Caption := 'Installing Microsoft WebView2 (needed by MelloGram)...';
    ExtractTemporaryFile('MicrosoftEdgeWebview2Setup.exe');
    if not Exec(ExpandConstant('{tmp}\MicrosoftEdgeWebview2Setup.exe'), '/silent /install', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then
      if not WebView2Installed then
        Result := 'Microsoft WebView2 could not be installed (code ' + IntToStr(ResultCode) + '). Connect to the internet and run Setup again, or install WebView2 from https://developer.microsoft.com/microsoft-edge/webview2/.';
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataDir: String;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    DataDir := ExpandConstant('{userappdata}\app.mellogram.desktop');
    if DirExists(DataDir) then
      if MsgBox('Also delete your MelloGram settings, themes and Telegram session on this computer?' + #13#10 + #13#10 +
                'Choose No to keep them for a future install.', mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
        DelTree(DataDir, True, True, True);
  end;
end;

; Instalador de FinanZillo escritorio (Inno Setup 6).
; Compilar desde desktop/:  ISCC installer\finanzillo.iss
; Requiere antes:           cargo build --release

#define AppName "FinanZillo"
; La versión puede venir de fuera (ISCC /DAppVersion=2.1.0), como hace la release.
#ifndef AppVersion
  #define AppVersion "2.0.0"
#endif
#define AppExe "finanzillo.exe"

[Setup]
AppId={{6B0E2C4A-8F4D-4E7B-9C1A-F1A2C3D4E5F6}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=FinanZillo
AppPublisherURL=https://github.com/adrmoralf/FinanZillo
LicenseFile=..\..\LICENSE
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
; Instalación solo para el usuario actual: no pide permisos de administrador.
PrivilegesRequired=lowest
OutputDir=..\target\instalador
OutputBaseFilename=FinanZillo-{#AppVersion}-instalador
SetupIconFile=..\assets\finanzillo.ico
UninstallDisplayIcon={app}\{#AppExe}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
; Avisa al Explorador al terminar: refresca los iconos en caché (si no, tras
; cambiar el icono seguía viéndose el antiguo en accesos directos).
ChangesAssociations=yes

[Languages]
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "escritorio"; Description: "Crear un acceso directo en el escritorio"; GroupDescription: "Accesos directos:"
Name: "arranque"; Description: "Abrir FinanZillo al iniciar Windows"; GroupDescription: "Inicio:"; Flags: unchecked

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\fonts\OFL-Archivo.txt"; DestDir: "{app}\licencias"; Flags: ignoreversion
Source: "..\assets\fonts\OFL-JetBrainsMono.txt"; DestDir: "{app}\licencias"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: escritorio
Name: "{userstartup}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: arranque

[Run]
Filename: "{app}\{#AppExe}"; Description: "Abrir FinanZillo"; Flags: nowait postinstall skipifsilent

; Los datos (%APPDATA%\FinanZillo: archivo cifrado y copias) NO se borran al
; desinstalar, a propósito: son del usuario.

Unicode True
!include "MUI2.nsh"
!include "x64.nsh"

!ifndef PACKAGE
!error "PACKAGE must point to the staged obs-live-production directory"
!endif
!ifndef OUTPUT
!error "OUTPUT must name the installer executable"
!endif

Name "Live Production SRT Publisher for OBS 33 (x64)"
OutFile "${OUTPUT}"
RequestExecutionLevel admin
InstallDir "$COMMONFILES64\obs-studio\plugins\obs-live-production"
SetCompressor /SOLID lzma
ShowInstDetails show

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "This plugin requires Windows x64 and OBS Studio 33 x64."
    Abort
  ${EndIf}
  SetShellVarContext all
  StrCpy $INSTDIR "$APPDATA\obs-studio\plugins\obs-live-production"
  nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -Command "if(Get-Process obs64 -ErrorAction SilentlyContinue){exit 1}"'
  Pop $0
  Pop $1
  ${If} $0 != 0
    MessageBox MB_OK|MB_ICONSTOP "Close OBS Studio before installing this plugin."
    Abort
  ${EndIf}
FunctionEnd

Section "Live Production SRT Publisher"
  SetOutPath "$INSTDIR"
  File /r "${PACKAGE}/*"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  SetRegView 64
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\OBS-Live-Production" "DisplayName" "Live Production SRT Publisher for OBS 33 (x64)"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\OBS-Live-Production" "UninstallString" '$\"$INSTDIR\uninstall.exe$\"'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\OBS-Live-Production" "InstallLocation" "$INSTDIR"
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\OBS-Live-Production" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\OBS-Live-Production" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext all
  Delete "$INSTDIR\bin\64bit\obs-live-production.dll"
  Delete "$INSTDIR\bin\64bit\obs-live-production-srt.exe"
  Delete "$INSTDIR\bin\64bit\srt.dll"
  RMDir "$INSTDIR\bin\64bit"
  RMDir "$INSTDIR\bin"
  RMDir /r "$INSTDIR\data"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  SetRegView 64
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\OBS-Live-Production"
SectionEnd

; DockX installer — per-user, no admin rights needed.
Target amd64-unicode
SetCompressor /SOLID lzma
!include "MUI2.nsh"
!include "LogicLib.nsh"

!define APP "DockX"
!ifndef VERSION
  !define VERSION "2.0.1"
!endif
!define PUBLISHER "X44TO"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\DockX"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"

Name "${APP} ${VERSION}"
OutFile "..\dist\DockX-Setup.exe"
InstallDir "$LOCALAPPDATA\Programs\DockX"
InstallDirRegKey HKCU "${UNINST_KEY}" "InstallLocation"
RequestExecutionLevel user
BrandingText "DockX ${VERSION} · by X44TO"
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1058 "ProductName" "DockX"
VIAddVersionKey /LANG=1058 "CompanyName" "X44TO"
VIAddVersionKey /LANG=1058 "FileDescription" "Інсталятор DockX"
VIAddVersionKey /LANG=1058 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1058 "LegalCopyright" "X44TO · MIT"

!define MUI_ICON "..\res\dockx.ico"
!define MUI_UNICON "..\res\dockx.ico"
!define MUI_ABORTWARNING
!define MUI_WELCOMEFINISHPAGE_BITMAP "..\brand\installer-side.bmp"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "..\brand\installer-side.bmp"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_RIGHT
!define MUI_HEADERIMAGE_BITMAP "..\brand\installer-header.bmp"
!define MUI_HEADERIMAGE_UNBITMAP "..\brand\installer-header.bmp"
!define MUI_WELCOMEPAGE_TITLE "DockX ${VERSION}"
!define MUI_WELCOMEPAGE_TEXT "Док у стилі macOS і Dynamic Island для Windows 10 та 11.$\r$\n$\r$\nВстановлення не потребує прав адміністратора. Якщо DockX уже запущений, інсталятор закриє його і збереже твої налаштування.$\r$\n$\r$\nНатисни «Далі», щоб продовжити."
!define MUI_FINISHPAGE_RUN "$INSTDIR\DockX.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Запустити DockX"
!define MUI_FINISHPAGE_LINK "Сторінка проєкту на GitHub"
!define MUI_FINISHPAGE_LINK_LOCATION "https://github.com/hatogg/DockX"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Ukrainian"

!macro CloseDockX
  ; ask the running DockX to save and quit, then make sure it is gone
  StrCpy $1 0
  ${Do}
    FindWindow $0 "DockXWindow" "DockX"
    ${If} $0 == 0
      ${Break}
    ${EndIf}
    SendMessage $0 ${WM_CLOSE} 0 0 /TIMEOUT=2000
    Sleep 400
    IntOp $1 $1 + 1
  ${LoopUntil} $1 >= 6
  nsExec::Exec 'taskkill /IM DockX.exe /F'
  Pop $0
  Sleep 200
!macroend

Section "DockX" SecMain
  SectionIn RO
  !insertmacro CloseDockX
  SetOutPath "$INSTDIR"
  File "..\dist\DockX.exe"
  File "/oname=README.md" "..\docs\README-app.md"
  File "..\LICENSE"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\DockX"
  CreateShortcut "$SMPROGRAMS\DockX\DockX.lnk" "$INSTDIR\DockX.exe" "" "$INSTDIR\DockX.exe" 0
  CreateShortcut "$SMPROGRAMS\DockX\Видалити DockX.lnk" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "DockX"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\DockX.exe"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "URLInfoAbout" "https://github.com/hatogg/DockX"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" 1600
SectionEnd

Section "Ярлик на столі" SecDesktop
  CreateShortcut "$DESKTOP\DockX.lnk" "$INSTDIR\DockX.exe" "" "$INSTDIR\DockX.exe" 0
SectionEnd

Section "Автозапуск" SecAutostart
  WriteRegStr HKCU "${RUN_KEY}" "DockX" '"$INSTDIR\DockX.exe"'
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} "Сама програма: док і Dynamic Island, ярлики в меню «Пуск», запис у «Програми та компоненти»."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "Ярлик DockX на робочому столі."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecAutostart} "DockX стартуватиме після входу в Windows. Можна змінити в налаштуваннях програми."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  !insertmacro CloseDockX
  Delete "$INSTDIR\DockX.exe"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\DockX\DockX.lnk"
  Delete "$SMPROGRAMS\DockX\Видалити DockX.lnk"
  RMDir "$SMPROGRAMS\DockX"
  Delete "$DESKTOP\DockX.lnk"
  DeleteRegValue HKCU "${RUN_KEY}" "DockX"
  DeleteRegKey HKCU "${UNINST_KEY}"
  IfSilent done
  MessageBox MB_YESNO|MB_ICONQUESTION "Видалити також налаштування, справи, полицю й кеш текстів пісень?$\r$\n(%APPDATA%\DockX)" IDNO done
  RMDir /r "$APPDATA\DockX"
  done:
SectionEnd

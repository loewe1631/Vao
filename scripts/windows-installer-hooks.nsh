; Startup belongs to this bundle and installation path. Never enable it on install.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    Push $0
    ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${BUNDLEID}"
    ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --autostart'
      DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${BUNDLEID}"
      DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "${BUNDLEID}"
    ${EndIf}
    Pop $0
  ${EndIf}
!macroend

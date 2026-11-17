; NSIS hooks of the Decka installer (Tauri "installerHooks").
;
; Windows Firewall: the phone has to reach the agent over Wi-Fi. A rule for the program (any port) on private AND public
; networks is added, because a Wi-Fi hotspot (of the PC or of another phone) is a "public" network for Windows. The change needs
; administrator rights, so Windows asks once (UAC); if it is refused, the installation goes on and the hint in the Phone tab
; shows the same command later.

!macro NSIS_HOOK_POSTINSTALL
  ExecShellWait "runas" "$SYSDIR\cmd.exe" '/c netsh advfirewall firewall delete rule name="Decka" & netsh advfirewall firewall add rule name="Decka" dir=in action=allow program="$INSTDIR\${MAINBINARYNAME}.exe" enable=yes profile=private,public' SW_HIDE
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ExecShellWait "runas" "$SYSDIR\cmd.exe" '/c netsh advfirewall firewall delete rule name="Decka"' SW_HIDE
!macroend

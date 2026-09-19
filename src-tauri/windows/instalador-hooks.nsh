; NodeFlow · lo que la plantilla del instalador NO garantiza por sí sola.
;
; Tauri crea el acceso directo del escritorio en dos momentos: la casilla «Crear un acceso directo
; en el escritorio» de la última página (instalación normal) y, si el instalador corre en modo
; silencio o pasivo, al final de la sección de instalación.
;
; Lo que NO hace es tocarlo cuando el instalador viene con `/UPDATE` —que es exactamente como lo
; lanza la app cuando se actualiza sola—: ahí la plantilla sale sin crear ni corregir nada. Por eso
; un acceso directo perdido no volvía nunca, y uno que apuntaba a un binario viejo se quedaba
; apuntando a la nada. Este hook deja el acceso directo en su lugar, apuntando al ejecutable
; instalado, sin pisarlo si ya está bien.

!include LogicLib.nsh

!macro NSIS_HOOK_POSTINSTALL
  Push $0
  !insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  Pop $0
  ${If} $0 <> 1
    DetailPrint "Dejando el acceso directo en el escritorio…"
    Delete "$DESKTOP\${PRODUCTNAME}.lnk"
    CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
  ${EndIf}
  Pop $0
!macroend

; Al desinstalar, la plantilla borra el acceso directo sólo si apunta al ejecutable instalado: el
; que hubiera quedado apuntando a un binario viejo sobrevive como un ícono que no abre nada.
!macro NSIS_HOOK_PREUNINSTALL
  Delete "$DESKTOP\${PRODUCTNAME}.lnk"
!macroend

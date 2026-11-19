; Lo que hace el instalador de winshotx además de lo de Tauri. Tauri mete estas macros en su
; plantilla de NSIS (`bundle > windows > nsis > installerHooks` en `tauri.conf.json`).

; Antes de borrar nada: devolverle a Windows Impr Pant y Win+Mayús+S si winshotx se las quitó.
; Sin esto, quien desinstala con Win+Mayús+S cogida se queda sin Win+S (la búsqueda), y con Impr
; Pant cogida la tecla ya no abre la Herramienta de Recortes. Lo hace el propio winshotx.exe
; (`teclas_windows.rs`), por el mismo camino que apagar los interruptores en los ajustes.
;
; Solo al desinstalar de verdad: actualizar no devuelve nada, o la versión nueva arrancaría con
; los interruptores puestos y las teclas otra vez en manos de Windows. En la plantilla de Tauri
; (CLI 2.11.4) hay dos caminos que llegan aquí sin ser una desinstalación:
; - `/UPDATE`, que pone `$UpdateMode` a 1. Lo pasa el actualizador de la app al instalador nuevo
;   (tauri-plugin-updater 2.10.1, `updater.rs`), y la plantilla se lo pasa a este desinstalador
;   si llega a llamarlo.
; - El instalador de otra versión (o de la misma en modo pasivo) que quita esta copia antes de
;   poner la suya: llama a este desinstalador con `_?=$INSTDIR`, y con eso NSIS no se copia a la
;   carpeta temporal y corre desde `$INSTDIR` («_?= ... stops the uninstaller from copying itself
;   to the temporary directory and running from there», manual de NSIS, apartado 3.2.2).
;   Desinstalar desde Configuración, el Panel de control o winget no pasa `_?=`, NSIS se copia a
;   la carpeta temporal, y ahí `$EXEDIR` ya no es `$INSTDIR`.
;
; El Explorador NO se reinicia: Win+S vuelve al cerrar sesión, y si hay alguien delante
; (no `/S` ni `/P`) winshotx se lo dice con un aviso. Reiniciarlo desde aquí cerraría sin
; preguntar las carpetas que tenga abiertas quien desinstala, y desde un winget con permisos de
; administrador `taskkill /IM explorer.exe` alcanzaría a los Exploradores de otras sesiones y
; volvería a abrir el suyo como administrador.
!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
  ${AndIf} $EXEDIR != $INSTDIR
    ; Primero, que winshotx no siga abierto: si quien desinstala dice que no lo cierra, se para
    ; aquí sin haber devuelto nada. Es la misma pregunta que Tauri hace justo después.
    !insertmacro CheckIfAppIsRunning "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
    ${If} ${Silent}
    ${OrIf} $PassiveMode = 1
      ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --restore-windows-keys'
    ${Else}
      ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --restore-windows-keys --tell'
    ${EndIf}
  ${EndIf}
!macroend

' NodeFlow · Guardado automático SIN ventanas.
' La tarea de Windows llama a este archivo con wscript: el proceso corre oculto
' (sin consola visible) y cada 10 minutos revisa si hay cambios; si los hay, corre
' los chequeos y recién entonces guarda y sube. Si no hay cambios, no hace nada.
'
' Por qué existe: antes la tarea llamaba a bash.exe directamente y eso abría una
' terminal cada vez. WScript.Shell.Run con estilo de ventana 0 la mantiene oculta.
Set sh = CreateObject("WScript.Shell")
q = Chr(34)
bash = "C:\Program Files\Git\usr\bin\bash.exe"
script = "bash /c/Users/tomas/Desktop/NodeFlow/nodeflow-desktop/scripts/auto.sh"
cmd = q & bash & q & " -lc " & q & script & q
sh.Run cmd, 0, False

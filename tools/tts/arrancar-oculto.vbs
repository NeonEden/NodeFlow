' NodeFlow · Arranca el servidor de voz local (Kokoro) SIN ventanas.
' Se puede poner en la carpeta de Inicio de Windows para que la voz esté siempre lista.
Set sh = CreateObject("WScript.Shell")
q = Chr(34)
base = "C:\Users\tomas\Desktop\NodeFlow\nodeflow-desktop\tools\tts"
sh.CurrentDirectory = base
sh.Run q & base & "\.venv\Scripts\python.exe" & q & " " & q & base & "\servidor.py" & q, 0, False

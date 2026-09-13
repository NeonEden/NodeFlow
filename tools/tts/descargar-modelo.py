"""Baja el modelo de Kokoro (~340 MB) a esta carpeta. Se corre una sola vez."""
import os
import urllib.request

DIR = os.path.dirname(os.path.abspath(__file__))
BASE = "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0"
ARCHIVOS = {"kokoro-v1.0.onnx": "modelo (~310 MB)", "voices-v1.0.bin": "voces (~26 MB)"}

for nombre, que in ARCHIVOS.items():
    destino = os.path.join(DIR, nombre)
    if os.path.exists(destino) and os.path.getsize(destino) > 1024 * 1024:
        print(f"ya está: {nombre}")
        continue
    print(f"bajando {nombre} ({que})…")
    urllib.request.urlretrieve(f"{BASE}/{nombre}", destino)
    print(f"  listo: {os.path.getsize(destino)/1024/1024:.1f} MB")
print("modelo completo")

"""
Servidor de voz local (Kokoro TTS) para NodeFlow.

Escucha en 127.0.0.1:8125 y expone POST /decir  {texto, voz?, velocidad?}  → devuelve WAV.

Por qué existe: el bucle hablado necesita una voz que responda sin cuotas, sin latencia de red y sin
depender de un servicio de terceros. Kokoro son 82M de parámetros: corre en tu placa y es gratis.
NodeFlow lo llama desde su backend; el texto nunca sale de la máquina.

Uso:
    .venv/Scripts/python servidor.py            # queda escuchando
    curl -X POST http://127.0.0.1:8125/decir -d '{"texto":"Listo, armé la arquitectura."}' -o salida.wav
"""
import io
import json
import os
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

DIR = os.path.dirname(os.path.abspath(__file__))
MODELO = os.path.join(DIR, "kokoro-v1.0.onnx")
VOCES = os.path.join(DIR, "voices-v1.0.bin")

# Voces en español de Kokoro v1.0: ef_dora (femenina), em_alex (masculina), em_santa.
VOZ = os.environ.get("NF_TTS_VOZ", "ef_dora")
IDIOMA = os.environ.get("NF_TTS_IDIOMA", "es")
PUERTO = int(os.environ.get("NF_TTS_PUERTO", "8125"))


def cargar():
    if not (os.path.exists(MODELO) and os.path.exists(VOCES)):
        print("Faltan los archivos del modelo. Corré:  python descargar-modelo.py", file=sys.stderr)
        sys.exit(2)
    from kokoro_onnx import Kokoro

    print(f"cargando Kokoro… ({os.path.basename(MODELO)})", flush=True)
    return Kokoro(MODELO, VOCES)


KOKORO = cargar()


class Handler(BaseHTTPRequestHandler):
    def _json(self, codigo, cuerpo):
        datos = json.dumps(cuerpo).encode("utf-8")
        self.send_response(codigo)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(datos)))
        self.end_headers()
        self.wfile.write(datos)

    def do_GET(self):
        if self.path == "/estado":
            self._json(200, {"ok": True, "modelo": os.path.basename(MODELO), "voz": VOZ, "idioma": IDIOMA,
                             "voces_es": ["ef_dora", "em_alex", "em_santa"]})
        else:
            self.send_error(404)

    def do_POST(self):
        if self.path != "/decir":
            self.send_error(404)
            return
        try:
            largo = int(self.headers.get("Content-Length", 0))
            pedido = json.loads(self.rfile.read(largo) or b"{}")
        except Exception as e:
            self._json(400, {"ok": False, "error": f"pedido inválido: {e}"})
            return
        texto = (pedido.get("texto") or "").strip()
        if not texto:
            self._json(400, {"ok": False, "error": "falta `texto`"})
            return
        try:
            import soundfile as sf

            muestras, sr = KOKORO.create(
                texto,
                voice=pedido.get("voz") or VOZ,
                speed=float(pedido.get("velocidad", 1.0)),
                lang=pedido.get("idioma") or IDIOMA,
            )
            buf = io.BytesIO()
            sf.write(buf, muestras, sr, format="WAV")
            audio = buf.getvalue()
        except Exception as e:
            self._json(500, {"ok": False, "error": f"no pude sintetizar: {e}"})
            return
        self.send_response(200)
        self.send_header("Content-Type", "audio/wav")
        self.send_header("Content-Length", str(len(audio)))
        self.send_header("X-Muestras", str(len(muestras)))
        self.send_header("X-Frecuencia", str(sr))
        self.end_headers()
        self.wfile.write(audio)

    def log_message(self, *a):  # silencio: no ensuciamos la salida
        pass


if __name__ == "__main__":
    print(f"voz lista en http://127.0.0.1:{PUERTO} · voz={VOZ} · idioma={IDIOMA}", flush=True)
    HTTPServer(("127.0.0.1", PUERTO), Handler).serve_forever()

#!/usr/bin/env bash
# NodeFlow · Arma el paquete de la voz local que la app descarga bajo demanda.
#
# Por qué existe: la voz de calidad (Kokoro) son ~405 MB entre runtime y modelo, y no tiene por qué
# viajar en el instalador. El instalador son 4,4 MB; la voz es opcional y se baja desde el panel.
#
# Qué produce: `nodeflow-voz-local.zip` — Python embebible + `Lib/site-packages` + `servidor.py` +
# las voces, **sin el modelo** (ese lo baja la app de HuggingFace, con URL estable).
#
# Uso:
#   bash scripts/empaquetar-voz.sh              # arma el ZIP y muestra tamaño y sha256
#   bash scripts/empaquetar-voz.sh --publicar   # además lo sube al release (gh release upload)
#
# Después de publicar, el sha256 **tiene que** coincidir con `ZIP_SHA256` en
# `src-tauri/src/voz_local.rs`: es lo que hace que un paquete a medias no se despliegue nunca.

set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TTS="$REPO/tools/tts"
VENV="$TTS/.venv"
PY_VERSION="3.11.9"                       # la misma serie que el venv (ABI de 3.11)
WORK="$LOCALAPPDATA/Temp/nf-voz-build"
ZIP="$LOCALAPPDATA/Temp/nodeflow-voz-local.zip"
RELEASE="v0.3.6"

echo "── paquete de la voz local ────────────────────────────────────────────────"

if [ ! -f "$VENV/Scripts/python.exe" ]; then
  echo "  ✗ no encuentro el venv de desarrollo en $VENV"
  echo "    creálo con: python -m venv .venv && .venv/Scripts/pip install kokoro-onnx soundfile"
  exit 1
fi
if [ ! -f "$TTS/voices-v1.0.bin" ]; then
  echo "  ✗ falta $TTS/voices-v1.0.bin (las voces de Kokoro)"
  exit 1
fi

rm -rf "$WORK"
mkdir -p "$WORK"

# 1) Python embebible: el intérprete, sin instalación y sin registro.
if [ ! -f "$LOCALAPPDATA/Temp/py-embed.zip" ]; then
  echo "  · bajando Python $PY_VERSION embebible…"
  python -c "
import urllib.request as u
u.urlretrieve('https://www.python.org/ftp/python/$PY_VERSION/python-$PY_VERSION-embed-amd64.zip', r'$LOCALAPPDATA\Temp\py-embed.zip')
"
fi
python -c "
import zipfile
zipfile.ZipFile(r'$LOCALAPPDATA\Temp\py-embed.zip').extractall(r'$WORK')
"

# 2) Los paquetes, tal como quedaron instalados en el venv.
#    OJO: van también los `.dist-info`. Sin ellos `kokoro_onnx` no arranca: consulta su propia
#    versión con `importlib.metadata` y explota con PackageNotFoundError (medido, fue el único
#    error del primer armado).
cp -r "$VENV/Lib/site-packages" "$WORK/Lib/site-packages"
rm -rf "$WORK/Lib/site-packages/pip" "$WORK/Lib/site-packages/setuptools"
find "$WORK/Lib/site-packages" -name "__pycache__" -type d -prune -exec rm -rf {} + 2>/dev/null

# 3) El intérprete tiene que ver `site-packages` (el embebible no lo hace solo).
printf 'python311.zip\n.\nLib\\site-packages\nimport site\n' > "$WORK/python311._pth"

# 4) El servidor y las voces. El modelo NO viaja: lo baja la app.
cp "$TTS/servidor.py" "$WORK/servidor.py"
cp "$TTS/voices-v1.0.bin" "$WORK/voices-v1.0.bin"

# 5) Empaquetar.
rm -f "$ZIP"
python -c "
import zipfile, pathlib
work = pathlib.Path(r'$WORK')
zip_path = pathlib.Path(r'$ZIP')
n = 0
with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED, compresslevel=6) as z:
    for f in sorted(work.rglob('*')):
        if f.is_file():
            z.write(f, arcname=str(f.relative_to(work)))
            n += 1
print(f'  · {n} archivos empaquetados')
"

if [ ! -f "$ZIP" ]; then echo "  ✗ el ZIP no se creó"; exit 1; fi

BYTES=$(stat -c %s "$ZIP")
SHA=$(sha256sum "$ZIP" | cut -d' ' -f1)
echo "  ✓ $ZIP"
echo "    tamaño: $BYTES bytes ($((BYTES/1024/1024)) MB)"
echo "    sha256: $SHA"
echo
echo "  Comprobá que este sha256 sea el de ZIP_SHA256 en src-tauri/src/voz_local.rs:"
grep -n 'pub const ZIP_SHA256' "$REPO/src-tauri/src/voz_local.rs" | sed 's/^/    /'

if [ "${1:-}" = "--publicar" ]; then
  echo
  echo "  · subiendo al release $RELEASE…"
  gh release upload "$RELEASE" "$ZIP" --clobber && echo "  ✓ publicado"
  echo "  · el asset tarda unos segundos en propagarse; verificá con:"
  echo "    gh api repos/NeonEden/NodeFlow/releases/tags/$RELEASE --jq '.assets[] | select(.name==\"nodeflow-voz-local.zip\") | \"\\(.size) · \\(.digest)\"'"
fi

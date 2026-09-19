#!/usr/bin/env bash
# NodeFlow · arma el instalador de Windows (el .exe que se le da a otra persona).
#
#   bash scripts/instalador.sh              → compila y deja el instalador listo, sin instalar nada
#   bash scripts/instalador.sh --instalar   → además lo corre, con el asistente a la vista
#   bash scripts/instalador.sh --silencioso → además lo instala sin ventanas (para verificar)
#
# Qué hace distinto de `instalar.sh` (que sirve para el ciclo de desarrollo):
#   · regenera la marca del instalador (íconos y las dos imágenes NSIS) desde icons/icon.png;
#   · compila con `tauri build`, o sea CON empaquetado y firma del updater;
#   · deja el .exe instalable en src-tauri/target/release/bundle/nsis/ con su .sig al lado.
#
# El instalador resultante se comporta como el de cualquier programa: asistente con la marca de la
# app, selector de carpeta, licencia, accesos directos en el escritorio y en el menú inicio, entrada
# en «Aplicaciones instaladas» con ícono, editor, versión y tamaño, y desinstalador propio.

set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO" || exit 1

MODO="${1:-}"
case "$MODO" in
  ""|--instalar|--silencioso) ;;
  *) echo "uso: scripts/instalador.sh [--instalar|--silencioso]"; exit 2 ;;
esac

VERSION=$(grep -m1 '"version"' src-tauri/tauri.conf.json | sed -E 's/.*"version": "([^"]+)".*/\1/')
SETUP="$REPO/src-tauri/target/release/bundle/nsis/NodeFlow_${VERSION}_x64-setup.exe"

echo "[1/4] marca del instalador (íconos + imágenes NSIS)…"
python scripts/instalador-imagenes.py || exit 1

echo "[2/4] compilando (frontend + release + empaquetado + firma)…"
# La clave privada del updater nunca está en el repo: vive en el perfil del usuario. Sin ella, el
# instalador sale sin firma y una app ya instalada rechazaría la actualización.
ENV_FIRMA="${NODEFLOW_FIRMA:-$HOME/.tauri/nodeflow-signing.env}"
if [ -f "$ENV_FIRMA" ]; then
  set -a; . "$ENV_FIRMA"; set +a
  echo "  firma: $(basename "$ENV_FIRMA")"
else
  echo "  AVISO: sin $ENV_FIRMA el instalador sale SIN firma (el updater no va a poder aplicarlo)."
fi
node_modules/.bin/tauri build || exit 1

echo "[3/4] instalador…"
[ -f "$SETUP" ] || { echo "✗ no encuentro $SETUP"; exit 1; }
printf '  %s\n  %s  (%s)\n' "$SETUP" "$(sha256sum "$SETUP" | cut -c1-32)…" "$(du -h "$SETUP" | cut -f1)"
[ -f "$SETUP.sig" ] && echo "  firma del updater: $(basename "$SETUP").sig" || echo "  ⚠ sin firma"

case "$MODO" in
  --instalar)
    echo "[4/4] abriendo el asistente…"
    # Se lanza el .exe directo (no desde la consola) para que el asistente no quede colgado del
    # terminal: la instalación termina en su propia ventana.
    cmd //c start "" "$(cygpath -w "$SETUP" 2>/dev/null || echo "$SETUP")" >/dev/null 2>&1 || "$SETUP"
    ;;
  --silencioso)
    echo "[4/4] instalando en silencio…"
    "$SETUP" /S
    sleep 8
    ls -la "$LOCALAPPDATA/NodeFlow/"
    powershell -NoProfile -Command "Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\NodeFlow' | Select-Object DisplayName,DisplayVersion,Publisher,DisplayIcon | Format-List | Out-String" 2>/dev/null
    ;;
  *)
    echo "[4/4] listo. Para instalarlo: \"$SETUP\"  (o bash scripts/instalador.sh --instalar)"
    ;;
esac

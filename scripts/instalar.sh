#!/usr/bin/env bash
# NodeFlow · compilar e instalar la app local de una sola pasada.
#
#   bash scripts/instalar.sh
#
# Hace lo mismo que se hacía a mano y que se olvidaba un paso: compila el frontend, compila el release
# **sin bundle ni firma** (alcanza para la app local), comprueba que la interfaz esté embebida, cierra
# la app, respalda el binario anterior, copia el nuevo, relanza y verifica.
#
# Nunca instala un binario que no embeba la interfaz: ese es el que deja la ventana en blanco.

set -u
cd "$(dirname "$0")/.." || exit 1
REPO="$(pwd)"
# El instalado se llama igual que el que instala el instalador NSIS (`NodeFlow.exe`, el target del
# paquete): así el acceso directo y la entrada de «Aplicaciones instaladas» siguen siendo válidos
# después de una pasada de desarrollo.
EXE_DEST="$LOCALAPPDATA/NodeFlow/NodeFlow.exe"

echo "[1/5] frontend…"
npm run build || exit 1

echo "[2/5] release (sin bundle, sin firma)…"
node_modules/.bin/tauri build --no-bundle 2>&1 | tail -3 || exit 1

EMBEBIDA=$(grep -a -c "assets/index" src-tauri/target/release/NodeFlow.exe || true)
if [ "${EMBEBIDA:-0}" = "0" ]; then
  echo "✗ el binario NO embebe la interfaz: no lo instalo (dejaría la ventana en blanco)."
  exit 1
fi

echo "[3/5] cerrando la app…"
powershell -NoProfile -Command "Get-Process NodeFlow -ErrorAction SilentlyContinue | ForEach-Object { \$_.CloseMainWindow() | Out-Null }" >/dev/null 2>&1
sleep 5
if tasklist 2>/dev/null | grep -qiE "^NodeFlow\.exe"; then
  echo "  (sigue viva: la cierro por la fuerza)"
  powershell -NoProfile -Command "Stop-Process -Name NodeFlow -Force" >/dev/null 2>&1
  sleep 2
fi

echo "[4/5] instalando…"
cp "$EXE_DEST" "$EXE_DEST.bak-$(date +%d%m-%H%M)" 2>/dev/null || true
cp "$REPO/src-tauri/target/release/NodeFlow.exe" "$EXE_DEST" || exit 1
sha256sum "$REPO/src-tauri/target/release/NodeFlow.exe" "$EXE_DEST"

# El servidor MCP viaja con la app: sin esto, el instalado a mano no lo encuentra.
if [ -f "$REPO/mcp-server/nodeflow_mcp.py" ]; then
  mkdir -p "$(dirname "$EXE_DEST")/mcp-server"
  cp "$REPO/mcp-server/nodeflow_mcp.py" "$(dirname "$EXE_DEST")/mcp-server/nodeflow_mcp.py"
fi

# Los accesos directos los crea el instalador NSIS, no este camino: si se perdieron, la app queda
# instalada y sana pero sin ícono a la vista. Esto los repone (idempotente).
powershell -NoProfile -ExecutionPolicy Bypass -File "$REPO/scripts/acceso-directo.ps1" 2>/dev/null || true

echo "[5/5] relanzando…"
powershell -NoProfile -Command "Start-Process \"\$env:LOCALAPPDATA\NodeFlow\NodeFlow.exe\"" >/dev/null 2>&1
sleep 8
tasklist 2>/dev/null | grep -iE "^NodeFlow\.exe" || echo "  ¡ojo! no arrancó: mirá el log en %LOCALAPPDATA%\\com.nodeflow.desktop\\logs"

bash "$REPO/scripts/verificar-app.sh" || true

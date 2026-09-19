#!/usr/bin/env bash
# NodeFlow · ¿Por qué la ventana está vacía? — diagnóstico de una pasada.
#
# Uso: scripts/verificar-app.sh
#
# Responde la pregunta que costó dos incidentes: el binario que se está abriendo, ¿trae la interfaz
# EMBEBIDA (build de producción) o la pide por HTTP al servidor de Vite (build de desarrollo)?
# Un build de desarrollo lanzado sin Vite deja la ventana con ERR_CONNECTION_REFUSED: no hay error
# en el log, el backend responde, los tests pasan… y parece que «la app no anda» sin causa.
#
# Detectores (todos medidos, ninguno adivinado):
#   1. Embed: un build de producción lleva la UI dentro del .exe → aparecen las claves de assets
#      («/assets/»). Un build de desarrollo no las tiene (0) y su único camino es Vite.
#   2. Vite: si el binario es de desarrollo, tiene que responder localhost:5173 (acá Vite escucha
#      SÓLO en IPv6, así que 127.0.0.1 da «conexión rechazada» aunque esté vivo).
#   3. Backend: /api/health en 127.0.0.1:37371. Si responde y la ventana está vacía, falta el frontend.
#   4. dist al día: si src/ es más nuevo que dist/, el próximo build embebe una interfaz vieja.

set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO" || exit 1

API="127.0.0.1:37371"
PY=python
BIN="NodeFlow.exe"   # el ejecutable instalado: el target del paquete se llama así desde
                     # que dejó de llamarse `app.exe` (lo nombra el instalador).

# ── helpers ───────────────────────────────────────────────────────────────────────────────────
embebe_ui() { # ¿el binario lleva la interfaz adentro?
  [ -f "$1" ] || { echo "no-existe"; return; }
  # Sin `|| echo 0`: grep -c ya imprime 0 cuando no hay coincidencias, y el `||` metía un "0\n0"
  # que rompía la comparación numérica de más abajo.
  local n
  n=$(grep -a -c "/assets/" "$1" 2>/dev/null)
  if [ "${n:-0}" -gt 0 ]; then echo "si($n)"; else echo "NO(0)"; fi
}

http_ok() { # $1 = url → imprime el código o el error, sin curl (bloqueado por el egress de esta máquina)
  $PY -c "
import sys, urllib.request
try:
    r = urllib.request.urlopen(sys.argv[1], timeout=4)
    print('200' if r.status == 200 else str(r.status))
except Exception as e:
    print(type(e).__name__)
" "$1" 2>/dev/null
}

echo "── binarios ──────────────────────────────────────────────────────────────"
for f in "src-tauri/target/release/$BIN" "src-tauri/target/debug/$BIN"; do
  [ -f "$f" ] || continue
  printf '  %-38s ui embebida: %-8s fecha: %s\n' "$f" "$(embebe_ui "$f")" "$(date -r "$f" '+%d/%m %H:%M')"
done
INSTALADO="$LOCALAPPDATA/NodeFlow/$BIN"
if [ -f "$INSTALADO" ]; then
  printf '  %-38s ui embebida: %-8s fecha: %s\n' "instalado ($INSTALADO)" \
    "$(embebe_ui "$INSTALADO")" "$(date -r "$INSTALADO" '+%d/%m %H:%M')"
  # Mismo inodo = alguien enlazó/copió el binario de compilación encima del instalado: el
  # "acceso directo del usuario" pasa a depender de lo que haya en target/.
  for f in "src-tauri/target/release/$BIN"; do
    [ -f "$f" ] || continue
    if [ "$(stat -c '%i' "$f" 2>/dev/null)" = "$(stat -c '%i' "$INSTALADO" 2>/dev/null)" ]; then
      echo "  ⚠ el instalado y $f son EL MISMO archivo (hardlink)"
    fi
  done
else
  echo "  (no hay app instalada en $LOCALAPPDATA/NodeFlow/)"
fi

# Un acceso directo no alcanza con que exista: tiene que apuntar al ejecutable instalado. El que
# quedó apuntando a un binario viejo (una copia a mano, un nombre anterior) es un ícono que no abre
# nada. Y si falta, la app anda pero no hay por dónde abrirla.
ACC="$USERPROFILE/Desktop/NodeFlow.lnk"
if [ -f "$ACC" ]; then
  APUNTA="$(powershell -NoProfile -Command "(New-Object -ComObject WScript.Shell).CreateShortcut('$ACC').TargetPath" 2>/dev/null | tr -d '\r\n')"
  # Se compara por nombre de archivo: la ruta de acá viene con barras mezcladas (MSYS) y la del
  # acceso directo con barras invertidas, así que el texto nunca va a coincidir aunque sea el mismo.
  if [ "$(basename "$APUNTA")" = "$BIN" ]; then
    echo "  acceso directo del escritorio: sí → $(basename "$APUNTA")"
  else
    echo "  ⚠ el acceso directo apunta a «${APUNTA:-nada}», y el instalado es $INSTALADO"
  fi
else
  echo "  ⚠ NO hay acceso directo en el escritorio (reponélo: powershell -File scripts/acceso-directo.ps1)"
fi

# Un programa instalado de verdad aparece en «Aplicaciones instaladas» con su desinstalador.
REG="$(powershell -NoProfile -Command "if (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\NodeFlow' -ErrorAction SilentlyContinue) { 'si' } else { 'no' }" 2>/dev/null | tr -d '\r\n')"
if [ "$REG" = "si" ]; then
  echo "  en «Aplicaciones instaladas»: sí (con desinstalador)"
else
  echo "  ⚠ no figura en «Aplicaciones instaladas»: se instaló a mano, no con el instalador"
fi

echo "── servidores ────────────────────────────────────────────────────────────"
printf '  Vite      http://localhost:5173   %s   (IPv6 [::1]: %s)\n' \
  "$(http_ok "http://localhost:5173/")" "$(http_ok "http://[::1]:5173/")"
printf '  Backend   http://%s/api/health  %s\n' "$API" "$(http_ok "http://$API/api/health")"

echo "── dist vs src ───────────────────────────────────────────────────────────"
if [ -f dist/index.html ]; then
  MAS_NUEVO_SRC=$(find src src-tauri/src src-tauri/specs -type f -newer dist/index.html 2>/dev/null | wc -l)
  if [ "$MAS_NUEVO_SRC" -gt 0 ]; then
    echo "  ⚠ $MAS_NUEVO_SRC archivo(s) de código más nuevos que dist/ — el próximo build embebe UI vieja (corré: npm run build)"
  else
    echo "  dist/ al día"
  fi
else
  echo "  falta dist/ (corré: npm run build)"
fi

echo "── veredicto ─────────────────────────────────────────────────────────────"
VITE="$(http_ok "http://[::1]:5173/")"
UI_REL="$(embebe_ui "src-tauri/target/release/$BIN")"
UI_INS="$(embebe_ui "$INSTALADO")"
if [ "$UI_INS" = "no-existe" ] || [ "$UI_INS" = "NO(0)" ]; then
  if [ "$VITE" = "200" ]; then
    echo "  El binario instalado NO embebe la interfaz y Vite SÍ está corriendo: la ventana debería verse."
    echo "  Si quedó en ERR_CONNECTION_REFUSED, apretá «Actualizar» en la ventana (o abrilo de nuevo)."
  else
    echo "  ✗ El binario instalado es de DESARROLLO (sin UI embebida) y Vite NO responde."
    echo "    Esa es la causa de la ventana con ERR_CONNECTION_REFUSED. Elegí una:"
    echo "      a) desarrollo:  npm run dev:web    (en background) y reabrí la app"
    echo "      b) instalable:  npm run build && node_modules/.bin/tauri build && bash scripts/release.sh --solo-publicar"
  fi
elif [ "$UI_REL" = "NO(0)" ] && [ "$UI_INS" != "NO(0)" ]; then
  echo "  La app instalada está sana (UI embebida). Ojo con target/release/app.exe: es un build de"
  echo "  desarrollo y sólo funciona con Vite corriendo — no lo copies sobre el instalado."
else
  echo "  La app instalada embebe la interfaz: se abre sin Vite ni esta sesión."
fi

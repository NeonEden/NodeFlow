#!/usr/bin/env bash
# Delegar una tarea de código a un worker-CLI: terreno aparte, worker con tope de tiempo,
# y los árbitros corridos por vos (no por el worker).
#
# Uso:  bash scripts/delegar.sh <tema> <archivo-de-pedido> [worker]
#       worker = opencode | claude | agy        (por defecto: opencode)
#
# Variables útiles:
#   NF_MODELO   modelo para opencode   (def: opencode/nemotron-3-ultra-free, gratis)
#   NF_TOPE     segundos máximos       (def: 900)
#
# Qué deja: un worktree ../nf-<tema> con la rama agente/<tema>, los árbitros corridos y el log del
# worker. NO commitea, NO pushea y NO abre PR: eso lo decide la persona (ver AGENTS.md).
set -uo pipefail

tema="${1:?falta el tema (ej: warnings)}"
pedido="${2:?falta el archivo de pedido (ver references/pedido.md de la skill)}"
worker="${3:-opencode}"

[ -f "$pedido" ] || { echo "no existe el pedido: $pedido"; exit 1; }

repo="$(git rev-parse --show-toplevel)" || exit 1
raiz="$(dirname "$repo")"
wt="$raiz/nf-$tema"
rama="agente/$tema"
log="${LOCALAPPDATA:-/tmp}/Temp/nf-$tema"
tope="${NF_TOPE:-900}"

# --- 1. terreno aparte (nunca sobre main) -------------------------------------------------------
if [ -d "$wt" ]; then
  echo "== el worktree ya existe: $wt (rama $rama)"
else
  git -C "$repo" worktree add "$wt" -b "$rama" main || { echo "no se pudo crear el worktree"; exit 1; }
fi

# El contrato tiene que estar en el commit: git worktree add parte de main.
[ -f "$wt/AGENTS.md" ] || echo "!! ATENCIÓN: no hay AGENTS.md en el worktree (¿sin commitear?). El worker va a trabajar sin contrato."

# --- 2. calentar el build: sin esto el worker se queda sin pasos compilando ---------------------
if [ ! -d "$wt/src-tauri/target" ]; then
  echo "== calentando el build (una vez por worktree) =="
  ( cd "$wt/src-tauri" && cargo build --lib >/dev/null 2>&1 )
fi

# node_modules por enlace (junction), no copia: así los árbitros del frontend corren en el worktree.
# OJO AL BORRAR: hay que quitar el enlace ANTES de `git worktree remove`, porque un `rm -rf` que
# siga el junction borra el node_modules del repo principal. Ver el bloque de cierre.
if [ ! -e "$wt/node_modules" ]; then
  powershell -NoProfile -Command "New-Item -ItemType Junction -Path '$wt\node_modules' -Target '$repo\node_modules' | Out-Null" 2>/dev/null
fi

# --- 3. lanzar el worker ------------------------------------------------------------------------
echo "== worker: $worker (tope ${tope}s) · log: $log.json =="
case "$worker" in
  opencode)
    modelo="${NF_MODELO:-opencode/nemotron-3-ultra-free}"
    ( cd "$wt" && timeout "$tope" opencode run --format json -m "$modelo" --title "$tema" \
        "$(cat "$pedido")" > "$log.json" 2>&1 ) ;;
  claude)
    ( cd "$wt" && timeout "$tope" claude -p "$(cat "$pedido")" --output-format json \
        --allowedTools "Read" "Edit" "Write" "Bash(git diff:*)" "Bash(git status:*)" \
        > "$log.json" 2>&1 ) ;;
  agy)
    # --mode accept-edits: puede editar sin preguntar. --sandbox: sin acceso libre a la terminal.
    # Sin --dangerously-skip-permissions, el modo headless AUTO-RECHAZA toda herramienta que necesite
    # permiso (medido: «a tool required the "command" permission that headless mode cannot prompt for»)
    # y el worker termina sin salida. Corre en un worktree descartable y los árbitros son el juez;
    # la alternativa fina es una allow-rule en su settings.json.
    ( cd "$wt" && timeout "$tope" agy -p "$(cat "$pedido")" --output-format json \
        --mode accept-edits --sandbox --dangerously-skip-permissions > "$log.json" 2>&1 ) ;;
  *)
    echo "worker desconocido: $worker (opencode|claude|agy)"; exit 2 ;;
esac
echo "   worker terminó (exit $?)"

# --- 4. árbitros: los corre el orquestador, no el worker ----------------------------------------
echo
echo "== ¿tocó algo? =="
sucio="$(git -C "$wt" status --porcelain)"
if [ -z "$sucio" ]; then
  echo "!! el worker no editó nada. Revisá $log.json antes de re-pedir (aviso típico: se quedó sin pasos)."
  exit 3
fi
echo "$sucio"
git -C "$wt" diff --stat

echo
echo "== árbitro 1 · build sin avisos de código =="
( cd "$wt/src-tauri" && cargo build --lib 2>&1 \
    | grep -E "^warning: (unused|method|struct|field|associated|function|variable)|^error" \
    || echo "sin avisos de código" )

echo
echo "== árbitro 2 · tests =="
( cd "$wt/src-tauri" && cargo test --lib 2>&1 | tail -3 )

# El frontend sólo se verifica si el diff lo tocó: compilar tipos siempre cuesta ~15 s y no dice nada
# de un cambio en Rust.
if git -C "$wt" status --porcelain | grep -qE "^\s*\S+\s+src/|\.(tsx|ts|css)\b"; then
  echo
  echo "== árbitro 3 · tipos del frontend (el diff toca src/) =="
  ( cd "$wt" && npx tsc --noEmit && echo "tipos OK" )
  echo "   (npm run build queda a criterio: escribe dist/ dentro del worktree)"
  # Los tests de frontend son el árbitro fuerte: si el proyecto define un script `test`, se corre.
  if grep -q '"test"' "$wt/package.json" 2>/dev/null; then
    echo
    echo "== árbitro 4 · tests de frontend =="
    ( cd "$wt" && npm test 2>&1 | tail -8 )
  fi
else
  echo
  echo "== árbitro 3 · frontend: no aplica (el diff no toca src/) =="
fi

# --- 5. cierre (lo decide la persona) -----------------------------------------------------------
echo
echo "== rama lista: $rama en $wt =="
echo "   commit + PR (vos, no el worker):"
echo "     git -C \"$wt\" add -A && git -C \"$wt\" commit -F <mensaje>"
echo "     git -C \"$wt\" push -u origin $rama && gh pr create --base main --head $rama --fill"
echo "   descartar (¡quitá el enlace de node_modules primero!):"
echo "     cmd //c rmdir '$(cygpath -w "$wt/node_modules" 2>/dev/null || echo "$wt/node_modules")'"
echo "     git worktree remove --force \"$wt\" && git branch -D $rama"

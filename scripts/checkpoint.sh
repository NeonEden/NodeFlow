#!/usr/bin/env bash
# NodeFlow · Punto de guardado
#
# Uso:
#   scripts/checkpoint.sh              # verifica y guarda una vez
#   scripts/checkpoint.sh --watch      # repite cada NF_CHECKPOINT_MIN minutos (5 por defecto)
#   scripts/checkpoint.sh "mensaje"    # usa ese mensaje de commit
#
# Qué hace: si no hay cambios, termina sin hacer ruido. Si hay, corre los chequeos
# (TypeScript + tests de Rust) y SOLO si pasan commitea y empuja. Si algo falla, no guarda:
# deja el motivo en el log. Es la red de seguridad: no se pierde trabajo y no se sube roto.

set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO" || exit 1
LOG="$REPO/.git/checkpoint.log"
MIN="${NF_CHECKPOINT_MIN:-5}"

anotar() { printf '[%s] %s\n' "$(date '+%F %T')" "$1" >> "$LOG"; echo "$1"; }

verificar() {
  if ! npx --no-install tsc --noEmit >/tmp/nf-tsc.log 2>&1; then
    anotar "NO guardado: tsc falló → $(tail -3 /tmp/nf-tsc.log | tr '\n' ' ')"
    return 1
  fi
  if ! cargo test --manifest-path src-tauri/Cargo.toml --lib 2>&1 | grep -q "test result: ok"; then
    anotar "NO guardado: los tests de Rust fallaron"
    return 1
  fi
  return 0
}

guardar() {
  local cambios
  cambios="$(git status --porcelain | wc -l | tr -d ' ')"
  [ "$cambios" = "0" ] && return 0

  verificar || return 1

  local resumen
  resumen="$(git status --porcelain | awk '{print $2}' | head -6 | tr '\n' ' ')"
  local mensaje="${1:-chore(checkpoint): $cambios archivo(s) · $(date '+%F %H:%M')}"
  git add -A
  git commit -q -m "$mensaje

Verificado antes de guardar: tsc --noEmit 0 errores + cargo test --lib verde.
Archivos: $resumen"

  if git push -q origin HEAD 2>/dev/null; then
    anotar "guardado y subido: $mensaje"
  else
    anotar "guardado en local (sin subir): $mensaje"
  fi
  return 0
}

if [ "${1:-}" = "--watch" ]; then
  anotar "checkpoint en marcha: revisa cada ${MIN} min (Ctrl+C para parar)"
  while true; do
    guardar
    sleep $((MIN * 60))
  done
else
  guardar "${1:-}"
fi

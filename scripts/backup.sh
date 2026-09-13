#!/usr/bin/env bash
# NodeFlow · Respaldo del repositorio (bundle) fuera del repo, con rotación.
# Uso: scripts/backup.sh   (crea como máximo un bundle por día)

set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# git es nativo: para -C hay que pasarle la ruta en formato Windows.
REPO_WIN="$(cygpath -w "$REPO" 2>/dev/null || echo "$REPO")"
DESTINO="${NF_BACKUP_DIR:-$(dirname "$REPO")/backups}"
mkdir -p "$DESTINO"
HOY="$(date '+%F')"
ARCHIVO="$DESTINO/nodeflow-$HOY.bundle"

if [ -f "$ARCHIVO" ]; then
  echo "ya hay respaldo de hoy: $ARCHIVO"
else
  # git es un programa nativo: necesita la ruta en formato Windows, no MSYS.
  ARCHIVO_WIN="$(cygpath -w "$ARCHIVO" 2>/dev/null || echo "$ARCHIVO")"
  if git -C "$REPO_WIN" bundle create "$ARCHIVO_WIN" --all >/dev/null 2>"$DESTINO/backup.err"; then
    echo "respaldo creado: $ARCHIVO ($(du -h "$ARCHIVO" | cut -f1))"
    echo "  contiene: $(git -C "$REPO_WIN" rev-list --count --all) commits del historial completo"
    rm -f "$DESTINO/backup.err"
  else
    echo "el respaldo falló:"; tail -3 "$DESTINO/backup.err"
  fi
fi

# rotación: se conservan los últimos 10
ls -1t "$DESTINO"/nodeflow-*.bundle 2>/dev/null | tail -n +11 | while read -r viejo; do
  rm -f "$viejo" && echo "rotado (fuera): $(basename "$viejo")"
done

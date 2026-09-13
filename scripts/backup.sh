#!/usr/bin/env bash
# NodeFlow · Respaldo del repositorio (bundle) fuera del repo, con rotación.
# Uso: scripts/backup.sh   (crea como máximo un bundle por día)

set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DESTINO="${NF_BACKUP_DIR:-$(dirname "$REPO")/backups}"
mkdir -p "$DESTINO"
HOY="$(date '+%F')"
ARCHIVO="$DESTINO/nodeflow-$HOY.bundle"

if [ -f "$ARCHIVO" ]; then
  echo "ya hay respaldo de hoy: $ARCHIVO"
else
  git -C "$REPO" bundle create "$ARCHIVO" --all >/dev/null 2>&1 && \
    echo "respaldo creado: $ARCHIVO ($(du -h "$ARCHIVO" | cut -f1))"
fi

# rotación: se conservan los últimos 10
ls -1t "$DESTINO"/nodeflow-*.bundle 2>/dev/null | tail -n +11 | while read -r viejo; do
  rm -f "$viejo" && echo "rotado (fuera): $(basename "$viejo")"
done

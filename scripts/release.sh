#!/usr/bin/env bash
# NodeFlow · Nueva versión
#
# Uso: scripts/release.sh patch|minor|major   (o --dry-run para ver qué haría)
#
# Sube la versión en package.json y tauri.conf.json (SemVer), corre los chequeos, compila los
# instaladores, crea el tag anotado y lo sube. El tag es el punto al que podés volver siempre.

set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO" || exit 1

PARTE="${1:-}"
case "$PARTE" in
  patch|minor|major|--dry-run) ;;
  *) echo "uso: scripts/release.sh patch|minor|major|--dry-run"; exit 2 ;;
esac

actual=$(grep -m1 '"version"' src-tauri/tauri.conf.json | sed -E 's/.*"version": "([^"]+)".*/\1/')
IFS=. read -r MA MI PA <<< "$actual"
case "$PARTE" in
  major) MA=$((MA+1)); MI=0; PA=0 ;;
  minor) MI=$((MI+1)); PA=0 ;;
  patch) PA=$((PA+1)) ;;
  --dry-run) echo "versión actual: v$actual · el próximo patch sería v$MA.$MI.$((PA+1))"; exit 0 ;;
esac
nueva="$MA.$MI.$PA"

echo "v$actual → v$nueva"
sed -i "s/\"version\": \"$actual\"/\"version\": \"$nueva\"/" src-tauri/tauri.conf.json
sed -i "s/\"version\": \"$actual\"/\"version\": \"$nueva\"/" package.json

bash scripts/checkpoint.sh "chore(release): v$nueva" || { echo "chequeos fallaron: no se libera"; exit 1; }
npx tauri build 2>&1 | tail -3

git tag -a "v$nueva" -m "NodeFlow v$nueva"
git push -q origin HEAD && git push -q origin "v$nueva"
echo "listo: v$nueva etiquetada y subida"
ls -la src-tauri/target/release/bundle/msi/*.msi src-tauri/target/release/bundle/nsis/*.exe 2>/dev/null | tail -2

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

# Chequeos y commit explícitos. NO se delega en el checkpoint: desde que el guardado automático
# espera quietud, no commitearía el bump y el tag quedaría apuntando a una versión vieja.
echo "chequeos…"
npx --no-install tsc --noEmit >/tmp/nf-rel-tsc.log 2>&1 || { echo "✗ tsc falló"; tail -5 /tmp/nf-rel-tsc.log; exit 1; }
(cd src-tauri && cargo test --lib 2>&1 | grep -q "test result: ok") || { echo "✗ los tests de Rust fallaron"; exit 1; }
echo "  ✓ tsc y tests de Rust"
git add -A
git commit -q -m "chore(release): v$nueva" || echo "  (sin cambios que commitear)"

# ── Firma ────────────────────────────────────────────────────────────────────────────────────
# La clave privada NUNCA está en el repo: vive en el perfil del usuario. Sin ella, el build sale
# sin firma y la app instalada rechazaría la actualización (que es justo lo que queremos que pase
# si alguien publica un paquete trucho).
ENV_FIRMA="${NODEFLOW_FIRMA:-$HOME/.tauri/nodeflow-signing.env}"
if [ -f "$ENV_FIRMA" ]; then
  set -a; . "$ENV_FIRMA"; set +a
  # Tauri quiere el CONTENIDO en TAURI_SIGNING_PRIVATE_KEY (no la ruta). Se arma acá, y así la clave
  # puede ser multilínea sin romper el sourcing.
  # Compatibilidad: si el env trae una RUTA en vez del contenido, se lee el archivo. Sin
  # sustituciones: `${VAR//\\//}` se desarma por el escapeo de la shell y termina borrando las barras.
  if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ] && [ -n "${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ]; then
    TAURI_SIGNING_PRIVATE_KEY="$(cat "$TAURI_SIGNING_PRIVATE_KEY_PATH" 2>/dev/null)"
    export TAURI_SIGNING_PRIVATE_KEY
  fi
  if [ -n "${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ] && [ -f "$TAURI_SIGNING_PRIVATE_KEY_PATH" ]; then
    TAURI_SIGNING_PRIVATE_KEY="$(cat "$TAURI_SIGNING_PRIVATE_KEY_PATH")"
    export TAURI_SIGNING_PRIVATE_KEY
    echo "firma: clave cargada desde ${TAURI_SIGNING_PRIVATE_KEY_PATH##*/} (${#TAURI_SIGNING_PRIVATE_KEY} bytes)"
  fi
else
  echo "AVISO: sin $ENV_FIRMA los instaladores salen SIN firma y el updater no va a poder aplicarlos."
fi

# createUpdaterArtifacts: true hace que Tauri emita el .zip del updater y su .sig además del .exe/.msi
node_modules/.bin/tauri build 2>&1 | tail -3

# ── Publicación: release de GitHub con TODO adentro, manifiesto incluido ─────────────────────
BUNDLE="src-tauri/target/release/bundle"
# Con NSIS el artefacto del updater es el propio .exe firmado (createUpdaterArtifacts emite el .sig
# al lado); si Tauri emitiera además el .zip del updater, se prefiere ese. Lo que NO puede faltar es
# la firma: sin ella la app instalada rechazaría la actualización.
UP="$(ls -1 "$BUNDLE"/nsis/*.nsis.zip 2>/dev/null | head -1 || true)"
[ -n "$UP" ] || UP="$(ls -1 "$BUNDLE"/nsis/*.exe 2>/dev/null | head -1 || true)"
[ -n "$UP" ] || { echo "✗ no hay instalador en $BUNDLE/nsis"; exit 1; }
SIG="$UP.sig"
if [ ! -f "$SIG" ]; then
  echo "✗ falta la firma de $UP: sin clave privada el updater no puede instalar nada"
  exit 1
fi
echo "updater: $(basename "$UP") + $(basename "$SIG")"

# El manifiesto es lo que la app consulta: versión, fecha, y por plataforma la firma y de dónde bajar.
URL="https://github.com/NeonEden/NodeFlow/releases/download/v$nueva/$(basename "$UP")"
cat > latest.json <<JSON
{
  "version": "$nueva",
  "notes": "NodeFlow v$nueva",
  "pub_date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "platforms": {
    "windows-x86_64": { "signature": $(python -c "import json,sys;print(json.dumps(open(sys.argv[1]).read().strip()))" "$SIG"), "url": "$URL" }
  }
}
JSON
echo "✓ latest.json → $URL"

git tag -a "v$nueva" -m "NodeFlow v$nueva"
git push -q origin HEAD && git push -q origin "v$nueva"
NOTAS="docs/releases/v$nueva.md"
if [ -f "$NOTAS" ]; then ARGS_N=(--notes-file "$NOTAS"); else ARGS_N=(--notes "NodeFlow v$nueva"); fi
gh release create "v$nueva" --title "NodeFlow v$nueva" "${ARGS_N[@]}" \
  "$BUNDLE"/nsis/*.exe "$BUNDLE"/nsis/*.exe.sig \
  "$BUNDLE"/msi/*.msi "$BUNDLE"/msi/*.sig latest.json 2>&1 | tail -3
echo "listo: v$nueva publicada, firmada y con manifiesto (la app instalada ya puede actualizarse sola)"
rm -f latest.json
ls -1 "$BUNDLE"/nsis/ | tail -5

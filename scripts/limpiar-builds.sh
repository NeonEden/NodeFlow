#!/usr/bin/env bash
# NodeFlow · Limpieza de artefactos de compilación
#
# Los tests corren en perfil debug y esa carpeta crece sin techo: llegó a ~10 GB y fue lo que
# empezó a llenar el disco. Regla simple: si pasa el tope, se borra. Es 100% recreable — el
# próximo `cargo test` la vuelve a armar (tarda más esa vez y nada más).
#
# Lo llama scripts/auto.sh, así que corre solo, de fondo, junto al punto de guardado.
TOPE_MB="${NF_TOPE_DEBUG_MB:-3000}"
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="$REPO/src-tauri/target/debug"
LOG="$REPO/.git/checkpoint.log"

[ -d "$TARGET" ] || exit 0
MB="$(du -sm "$TARGET" 2>/dev/null | cut -f1)"
if [ "${MB:-0}" -gt "$TOPE_MB" ]; then
  rm -rf "$TARGET"
  printf '[%s] limpieza: target/debug tenía %s MB (tope %s) → borrado\n' "$(date '+%F %T')" "$MB" "$TOPE_MB" >> "$LOG"
fi

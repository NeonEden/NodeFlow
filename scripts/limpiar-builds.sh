#!/usr/bin/env bash
# NodeFlow · Limpieza de artefactos de compilación
#
# Los tests corren en perfil debug y esa carpeta crece sin techo: llegó a ~10 GB y fue lo que
# empezó a llenar el disco. Regla simple: si pasa el tope, se borra. Es 100% recreable — el
# próximo `cargo test` la vuelve a armar (tarda más esa vez y nada más).
#
# HUECO QUE ESTE SCRIPT TENÍA (medido el 19/09/2026): sólo miraba `src-tauri/target/debug` de ESTE
# repositorio. Los worktrees tienen su propio `src-tauri/target/` y nadie los miraba: tres worktrees
# de ramas ya mergeadas habían acumulado 6,7 + 5,6 + 4,8 = 17,2 GB y eran lo único que estaba
# llenando el disco. Ahora recorre TODOS los worktrees.
#
# Qué hace, en orden:
#   1. En todos los worktrees: si `target/debug` pasa el tope, se borra.
#   2. En los worktrees que NO son este repo: si `target/` entero pasa el tope, se borra entero
#      (son desechables; si la rama ya se mergeó, limpiar-worktrees.sh se lleva el worktree completo).
#   3. En este repo: `target/release` NO se toca. Ahí viven los instaladores firmados
#      (`target/release/bundle`, ~12 MB) y borrarlos obliga a rehacer la firma.
#
# Lo llama scripts/auto.sh, así que corre solo, de fondo, junto al punto de guardado.

TOPE_MB="${NF_TOPE_DEBUG_MB:-3000}"
TOPE_WT_MB="${NF_TOPE_WORKTREE_MB:-3500}"
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG="$REPO/.git/checkpoint.log"
# Ojo: los programas nativos (git) NO traducen paths estilo MSYS en este host, así que `git -C
# /c/...` falla. Se entra al repo y se llama a git sin -C. `pwd -W` da el path nativo para comparar.
cd "$REPO" || exit 1
REPO_REAL="$(pwd -W 2>/dev/null || echo "$REPO")"

anotar() { printf '[%s] limpieza: %s\n' "$(date '+%F %T')" "$1" >> "$LOG"; }
mb_de() { du -sm "$1" 2>/dev/null | cut -f1; }

borrar_si_pasa() {
  local ruta="$1" tope="$2" etiqueta="$3" mb
  [ -d "$ruta" ] || return 0
  mb="$(mb_de "$ruta")"
  [ -n "${mb:-}" ] || return 0
  if [ "$mb" -gt "$tope" ]; then
    rm -rf "$ruta" 2>/dev/null
    anotar "$etiqueta tenía ${mb} MB (tope ${tope}) → borrado"
  fi
}

while IFS=$'\t' read -r ruta rama; do
  [ -n "${ruta:-}" ] || continue
  ruta_real="$(cd "$ruta" 2>/dev/null && pwd -W 2>/dev/null || echo "$ruta")"

  # 1. target/debug en todos (es lo que crece sin techo y lo que más se recrea).
  borrar_si_pasa "$ruta/src-tauri/target/debug" "$TOPE_MB" "target/debug de ${rama:-$ruta}"

  # 2. Worktrees ajenos: si el target entero está inflado, se va completo.
  if [ "$ruta_real" != "$REPO_REAL" ]; then
    borrar_si_pasa "$ruta/src-tauri/target" "$TOPE_WT_MB" "target/ del worktree ${rama:-$ruta}"
  fi
  # 3. Este repo: target/release se deja en paz (tiene los instaladores firmados).
done < <(git worktree list --porcelain | awk '
  /^worktree /{p=$0; sub(/^worktree /,"",p)}
  /^branch /{b=$0; sub(/^branch refs\/heads\//,"",b); print p "\t" b}
  /^detached/{print p "\t(detached)"}
')

exit 0

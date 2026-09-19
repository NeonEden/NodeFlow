#!/usr/bin/env bash
# NodeFlow · Poda de worktrees terminados
#
# Por qué existe: cada worktree delegado arrastra su propio src-tauri/target/. Cuando su rama se
# mergea, ese worktree pasa a ser basura de varios GB que nadie vuelve a mirar. Medido el 19/09/2026:
# tres worktrees de ramas YA mergeadas sumaban 17,2 GB (6,7 + 5,6 + 4,8) y eran lo único que estaba
# llenando el disco. limpiar-builds.sh no los veía porque sólo mira el repo donde corre.
#
# Regla: un worktree se poda SÓLO si se cumplen las tres:
#   1. no es este repo (nunca se poda a sí mismo),
#   2. su rama YA está mergeada en la rama principal, y
#   3. no tiene cambios sin commitear ni commits sin mergear.
#
# Si tiene trabajo sucio NO se toca y se avisa: ahí puede haber algo que vale la pena mirar antes de
# perderlo (fue el caso de nf-csp-tests, que tenía 4 líneas del log de la CSP sin commitear).
#
# Uso: scripts/limpiar-worktrees.sh          # poda
#      scripts/limpiar-worktrees.sh --ver    # sólo informa, no borra nada

set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO" || exit 1
LOG="$REPO/.git/checkpoint.log"
REPO_REAL="$(cd "$REPO" && pwd -W 2>/dev/null || echo "$REPO")"

SOLO_VER=0
[ "${1:-}" = "--ver" ] && SOLO_VER=1

anotar() {
  printf '[%s] limpieza-worktrees: %s\n' "$(date '+%F %T')" "$1" >> "$LOG"
  [ "$SOLO_VER" = "1" ] && echo "  $1"
  return 0
}

# La rama principal, leída de remoto para no adivinarla. Si no hay remoto, main.
PRINCIPAL="$(git symbolic-ref --short refs/remotes/origin/HEAD 2>/dev/null | sed 's|^origin/||')"
[ -n "${PRINCIPAL:-}" ] || PRINCIPAL="main"
# Se compara contra origin/<principal> (lo mergeado de verdad, incluido lo que entró por PR), y si
# esa ref no existe se cae a la local. No se hace fetch acá a propósito: esta poda corre cada pocos
# minutos en segundo plano y un fetch cada vez es ruido de red; ser conservador (no ver un merge
# todavía) sólo significa podar en la pasada siguiente.
if git rev-parse --verify -q "refs/remotes/origin/$PRINCIPAL" >/dev/null; then
  BASE="refs/remotes/origin/$PRINCIPAL"
else
  BASE="refs/heads/$PRINCIPAL"
fi

tam_mb() { du -sm "$1" 2>/dev/null | cut -f1; }

podados=0
saltados=0

# --porcelain: un bloque por worktree. Se arma "ruta<TAB>rama" y se recorre.
while IFS=$'\t' read -r ruta rama; do
  [ -n "${ruta:-}" ] || continue

  # 1. Nunca a sí mismo.
  ruta_real="$(cd "$ruta" 2>/dev/null && pwd -W 2>/dev/null || echo "$ruta")"
  [ "$ruta_real" = "$REPO_REAL" ] && continue

  # Worktree sin rama (detached): no se toca, no hay merge que probar.
  if [ "${rama:-}" = "(DETACHED)" ] || [ -z "${rama:-}" ]; then
    anotar "SALTADO (detached, sin rama): $ruta"
    saltados=$((saltados + 1))
    continue
  fi

  # 2. ¿La rama ya está adentro de la principal?
  if ! git merge-base --is-ancestor "refs/heads/$rama" "$BASE" 2>/dev/null; then
    anotar "SALTADO (rama sin mergear): $rama"
    saltados=$((saltados + 1))
    continue
  fi

  # 3. ¿Está limpio? Trabajo sucio = no se toca.
  sucios="$(git -C "$ruta" status --porcelain 2>/dev/null | wc -l | tr -d ' ')"
  if [ "${sucios:-0}" != "0" ]; then
    anotar "SALTADO (tiene $sucios archivo(s) sin commitear): $rama — revisalo antes de podar"
    saltados=$((saltados + 1))
    continue
  fi

  mb="$(tam_mb "$ruta")"
  if [ "$SOLO_VER" = "1" ]; then
    anotar "PODABLE (${mb:-?} MB): $rama → $ruta"
    podados=$((podados + 1))
    continue
  fi

  if git worktree remove --force "$ruta" 2>/dev/null; then
    # El directorio puede quedar como stub si algo tenía un archivo tomado: se remata a mano.
    [ -d "$ruta" ] && rm -rf "$ruta" 2>/dev/null
    anotar "podado ${mb:-?} MB: $rama"
    podados=$((podados + 1))
  else
    anotar "FALLÓ la poda de $rama (¿archivo tomado por otro proceso?)"
    saltados=$((saltados + 1))
  fi
done < <(git worktree list --porcelain | awk '
  /^worktree /{p=$0; sub(/^worktree /,"",p)}
  /^branch /{b=$0; sub(/^branch refs\/heads\//,"",b); print p "\t" b}
  /^detached/{print p "\t(DETACHED)"}
')

git worktree prune 2>/dev/null

if [ "$podados" -gt 0 ] || [ "$saltados" -gt 0 ]; then
  anotar "resumen: $podados podado(s), $saltados saltado(s)"
fi
exit 0

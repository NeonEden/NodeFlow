#!/usr/bin/env bash
# NodeFlow · Bitácora de desarrollo
#
# Arma el registro de lo que venimos haciendo **desde el historial de git** (nada inventado) y lo
# escribe en dos lugares:
#   · docs/DEVLOG.md            (para el repo; es lo que se muestra)
#   · <bóveda>/NodeFlow/devlog/AAAA-MM-DD.md   (con frontmatter, para Obsidian)
#
# Uso: scripts/devlog.sh [días]      (por defecto 14 días)

set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO" || exit 1
DIAS="${1:-14}"
DOC="$REPO/docs/DEVLOG.md"
VAULT="${NODEFLOW_VAULT:-$HOME/Documents/Obsidian Vault/NodeFlow}"

total_commits=$(git rev-list --count HEAD)
ramas=$(git branch --show-current)

{
  echo "# Bitácora de desarrollo"
  echo
  echo "Generado desde el historial de git (Conventional Commits). No se edita a mano."
  echo
  echo "\`$total_commits\` commits · rama \`$ramas\` · actualizado $(date '+%F %H:%M')"
  echo
  echo "Últimos $DIAS días:"
  echo
} > "$DOC"

# días con commits, del más nuevo al más viejo
dias=$(git log --since="$DIAS days ago" --date=short --pretty=format:'%ad' | sort -u -r)

for d in $dias; do
  n=$(git log --since="$d 00:00" --until="$d 23:59" --oneline | wc -l | tr -d ' ')
  stat=$(git log --since="$d 00:00" --until="$d 23:59" --shortstat --pretty=format:'' | awk -F'[ ,]+' '
    /insertion|deletion/ { for (i=1;i<=NF;i++){ if ($i ~ /insertion/) ins+=$(i-1); if ($i ~ /deletion/) del+=$(i-1) } }
    END { printf "+%d −%d", ins, del }')
  {
    echo "## $d · $n commit(s) · $stat"
    echo
    seccion() {  # $1 = prefijo, $2 = título
      local items
      items=$(git log --since="$d 00:00" --until="$d 23:59" --pretty=format:'%s' | grep -E "^$1" | sed -E "s/^$1(\([^)]*\))?: /- /" )
      [ -z "$items" ] && return 0
      echo "**$2**"
      echo
      echo "$items"
      echo
    }
    seccion "feat" "Nuevas capacidades"
    seccion "fix" "Correcciones"
    seccion "refactor" "Reorganización"
    seccion "perf" "Rendimiento"
    seccion "docs" "Documentación"
    seccion "test" "Pruebas"
    seccion "chore" "Infraestructura"
  } >> "$DOC"
done

echo "bitácora: $DOC"

# Nota de Obsidian con frontmatter para el día de hoy (solo si hubo commits hoy)
hoy=$(date '+%F')
if git log --since="$hoy 00:00" --oneline | grep -q .; then
  mkdir -p "$VAULT/devlog"
  nota="$VAULT/devlog/$hoy.md"
  {
    echo "---"
    echo "title: Devlog $hoy"
    echo "date: $hoy"
    echo "tags: [nodeflow, devlog]"
    echo "proyecto: NodeFlow"
    echo "---"
    echo
    sed -n "/^## $hoy/,/^## [0-9]/p" "$DOC" | sed '$d'
    echo
    echo "Repositorio: https://github.com/NeonEden/NodeFlow"
  } > "$nota"
  echo "nota en la bóveda: $nota"
fi

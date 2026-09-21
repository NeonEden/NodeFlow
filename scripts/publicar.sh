#!/usr/bin/env bash
# NodeFlow · publicar un cambio sin tocar la web de GitHub.
#
#   bash scripts/publicar.sh "fix(voz): qué y por qué"              # commit + push + PR + espera CI + merge
#   bash scripts/publicar.sh --local "fix(voz): qué"                # además corre los tres árbitros acá
#   bash scripts/publicar.sh --rama mia "chore: algo"               # nombre de rama explícito
#   bash scripts/publicar.sh                                       # usa el último mensaje de commit (solo push + PR)
#
# POR QUÉ EXISTE: el camino a mano (commit → push → abrir PR → esperar → apretar Merge) son cinco clics en
# la cuenta del usuario, y es donde se pierde el tiempo. Desde el 20/09/2026:
#   · `gh auth setup-git` dejó a git usando el token de `gh` (git credential fill → username=NeonEden),
#     así que pushear no abre ningún diálogo.
#   · la cuenta fantasma `x-access-token` del Git Credential Manager (la que hacía elegir cuenta) se borró
#     con `git credential-manager github logout x-access-token`. Si vuelve a aparecer, es ESE comando.
#   · el repo tiene auto-merge habilitado, así que el merge lo autoriza este script y no el humano.
#
# LO QUE NO HACE (a propósito):
#   · Nunca commitea sobre `main`: si estás en main, crea una rama y sigue.
#   · NO corre los árbitros por defecto: compilar en release clava los 16 núcleos de esta PC y la
#     sobrecalienta. El CI corre los mismos tres en GitHub (windows-latest) y `--local` los corre acá
#     cuando de verdad haga falta.
#   · NO borra la rama al mergear cuando el PR es apilado: borrar la base de un PR hijo lo CIERRA sin
#     aviso y no se puede reabrir (le pasó a #17). Con `--podar` (y sin PRs hijos) sí borra.
set -euo pipefail

cd "$(dirname "$0")/.." || exit 1
REPO="$(pwd)"

LOCAL=0
PODAR=0
RAMA=""
MENSAJE=""
while [ $# -gt 0 ]; do
  case "$1" in
    --local) LOCAL=1; shift ;;
    --podar) PODAR=1; shift ;;
    --rama) RAMA="$2"; shift 2 ;;
    -*) echo "opción desconocida: $1"; exit 2 ;;
    *) MENSAJE="$1"; shift ;;
  esac
done

BRANCH="$(git branch --show-current)"
if [ "$BRANCH" = "main" ]; then
  if [ -z "$RAMA" ]; then
    echo "✗ estás en main y no diste --rama. Un cambio no se publica desde main:"
    echo "  usá un worktree (git worktree add ../nf-<tema> -b agente/<tema> main) o pasá --rama <nombre>."
    exit 2
  fi
  RAMA="$(echo "$RAMA" | tr ' ' '-' | tr -cd '[:alnum:]-_/')"
  echo "→ rama nueva: $RAMA"
  git switch -c "$RAMA"
  BRANCH="$RAMA"
fi

if [ -n "$(git status --porcelain)" ]; then
  if [ -z "$MENSAJE" ]; then
    echo "✗ hay cambios sin commitear y no diste mensaje."
    exit 2
  fi
  git add -A
  git -c core.safecrlf=false commit -q -F - <<EOF
$MENSAJE

Publicado con scripts/publicar.sh (los árbitros los corre el CI de GitHub: tsc --noEmit, npm run build
y cargo test --lib en windows-latest).
EOF
  echo "→ commit hecho"
else
  echo "→ sin cambios que commitear"
fi

if [ "$LOCAL" = "1" ]; then
  echo "→ árbitros locales (esto compila: no lo corras con la PC caliente, uno por vez)…"
  npx tsc --noEmit
  npm run build >/dev/null
  (cd src-tauri && cargo test --lib 2>&1 | tail -2)
fi

git push -u origin "$BRANCH"

# Un PR por rama: si ya existe, no se abre otro.
PR="$(gh pr list --head "$BRANCH" --state open --json number --jq '.[0].number // empty')"
if [ -z "$PR" ]; then
  TITULO="$(git log -1 --format=%s)"
  # `gh pr create` NO tiene --json: devuelve la URL y punto (intentarlo con --json falla en silencio y el
  # script queda sin número de PR, que es como se rompió la primera corrida).
  gh pr create --title "$TITULO" --body "$(cat <<'EOF'
Publicado con `scripts/publicar.sh`.

- **Qué cambia**: ver los commits de la rama.
- **Árbitros**: los corre el CI (`tsc --noEmit`, `npm run build`, `cargo test --lib` en windows-latest).
- **Verificación local** (opcional): `bash scripts/publicar.sh --local` antes de pushear.
EOF
)" >/dev/null
  PR="$(gh pr list --head "$BRANCH" --state open --json number --jq '.[0].number')"
  echo "→ PR #$PR abierto"
else
  echo "→ PR #$PR ya existía para esta rama"
fi
if [ -z "$PR" ]; then
  echo "✗ no pude crear ni encontrar el PR de $BRANCH"
  exit 1
fi

echo "→ esperando el check del CI (esto no compila en tu máquina)…"
if gh pr checks "$PR" --watch --interval 20 >/dev/null 2>&1; then
  echo "→ CI en verde"
else
  echo "✗ el check no pasó: mirá 'gh pr checks $PR' — no mergeo nada en rojo."
  gh pr checks "$PR" | head -5 || true
  exit 1
fi

if [ "$PODAR" = "1" ]; then
  gh pr merge "$PR" --squash --delete-branch
else
  gh pr merge "$PR" --squash
fi
echo "✓ mergeado: $(gh pr view "$PR" --json url --jq .url)"

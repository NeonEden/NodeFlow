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
MIN="${NF_CHECKPOINT_MIN:-10}"
# Minutos de quietud que tienen que pasar desde el último cambio para que el guardado automático
# tome los archivos. Es el freno que impide que commitee trabajo ajeno a medio hacer.
QUIETO="${NF_QUIETO_MIN:-10}"

anotar() { printf '[%s] %s\n' "$(date '+%F %T')" "$1" >> "$LOG"; echo "$1"; }

# Marca de cada corrida (aunque no haya nada que guardar): sirve para comprobar que la tarea
# programada está viva, sin llenar el log de líneas vacías.
marcar() { printf '%s · %s\n' "$(date '+%F %T')" "$1" > "$REPO/.git/checkpoint.ultima"; }

verificar() {
  # Corre sólo el chequeo que corresponde a lo que cambió: esto vive de fondo en tu máquina,
  # así que no gasta CPU en tests que no pueden verse afectados.
  local tocados hay_ts hay_rs
  tocados="$(git status --porcelain | awk '{print $NF}')"
  hay_ts="$(printf '%s\n' "$tocados" | grep -cE '\.(ts|tsx|js|jsx|json|css)$' || true)"
  hay_rs="$(printf '%s\n' "$tocados" | grep -cE '\.(rs|toml)$' || true)"

  if [ "${hay_ts:-0}" != "0" ]; then
    if ! npx --no-install tsc --noEmit >/tmp/nf-tsc.log 2>&1; then
      marcar "chequeos fallaron: no se guardó"
      anotar "NO guardado: tsc falló → $(tail -3 /tmp/nf-tsc.log | tr '\n' ' ')"
      return 1
    fi
    CHECKS="tsc OK"
  fi

  if [ "${hay_rs:-0}" != "0" ]; then
    if ! cargo test --manifest-path src-tauri/Cargo.toml --lib 2>&1 | grep -q "test result: ok"; then
      marcar "chequeos fallaron: no se guardó"
      anotar "NO guardado: los tests de Rust fallaron"
      return 1
    fi
    CHECKS="${CHECKS:+$CHECKS + }tests Rust OK"
  fi

  [ -n "${CHECKS:-}" ] || CHECKS="sin chequeos aplicables (sólo docs)"
  return 0
}

# ¿Hay un build en curso? Si lo hay, NO se guarda: el autoguardado se adelantaba y commiteaba
# trabajo a medio hacer con un mensaje genérico (pasó tres veces). Se probó excluyendo rust-analyzer,
# que "corre" todo el día en un editor y bloquearía los guardados sin motivo.
compilando() {
  ps -W 2>/dev/null | grep -iE "cargo\.exe|rustc\.exe|rust-lld|tauri" | grep -qiE -v "rust-analyzer|grep"
}

# ¿Alguien está trabajando AHORA? Si algún archivo sucio se tocó hace menos de QUIETO minutos, el
# guardado automático no los toma: son de alguien que todavía no terminó (un agente, un editor) y
# commitearlos con mensaje genérico borra el porqué — pasó cuatro veces, y es justo lo que rompe la
# memoria del proyecto. El guardado entra cuando el trabajo se queda quieto: ahí ya no hay autoría
# en curso que pisar, y un equipo apagado a mitad de una edición igual queda a salvo.
trabajo_reciente() {
  local ahora=0 ultimo=0 t edad f
  ahora="$(date +%s)"
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    t="$(stat -c %Y "$f" 2>/dev/null || echo 0)"
    case "${t:-0}" in ''|*[!0-9]*) t=0 ;; esac
    [ "$t" -gt "$ultimo" ] && ultimo="$t"
  done < <(git status --porcelain | awk '{print $NF}')
  # Sin mtime fiable no se arriesga: se considera que hay trabajo en curso.
  if [ "$ultimo" -eq 0 ]; then
    ULTIMO_MIN="?"
    return 0
  fi
  edad=$(( (ahora - ultimo) / 60 ))
  ULTIMO_MIN="$edad"
  [ "$edad" -lt "$QUIETO" ]
}

guardar() {
  local cambios ULTIMO_MIN="?"
  cambios="$(git status --porcelain | wc -l | tr -d ' ')"
  if [ "$cambios" = "0" ]; then
    marcar "sin cambios"
    return 0
  fi

  if compilando; then
    marcar "build en curso: se espera a que termine"
    anotar "NO guardado: hay un build compilando (se guarda en la próxima pasada)"
    return 0
  fi

  if trabajo_reciente; then
    marcar "trabajo en curso (último cambio hace ${ULTIMO_MIN} min): se espera quietud"
    anotar "NO guardado: hay trabajo reciente (menos de ${QUIETO} min). Se guarda cuando se detenga."
    return 0
  fi

  verificar || return 1

  local resumen
  resumen="$(git status --porcelain | awk '{print $2}' | head -6 | tr '\n' ' ')"
  local mensaje="${1:-chore(checkpoint): guardado automático · $cambios archivo(s) · ${ULTIMO_MIN} min sin actividad}"
  git add -A
  git commit -q -m "$mensaje

Verificado antes de guardar: $CHECKS.
Archivos: $resumen"

  if git push -q origin HEAD 2>/dev/null; then
    anotar "guardado y subido: $mensaje"
    marcar "guardado y subido ($cambios archivos)"
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

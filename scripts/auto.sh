#!/usr/bin/env bash
# NodeFlow · Lo que corre la tarea programada (cada 5 minutos, en segundo plano).
# 1) respaldo del día si falta  2) punto de guardado (verifica y recién ahí commitea/sube)

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# 0) La voz local (Kokoro TTS, puerto 8125) tiene que estar escuchando: la app avisa «voz local no
#    disponible» y sigue andando, así que sin este chequeo el TTS queda apagado hasta que alguien se
#    acuerde (medido: estuvo apagado y la respuesta hablada nunca sonó).
#
#    Desde que la voz local es un paquete descargable, la app la arranca ella misma
#    (`voz_local::arrancar`), así que acá sólo se levanta la copia de desarrollo —y sólo si la app no
#    tiene la suya—: dos servidores peleando por el mismo puerto esconden bugs.
VOZ_APP="$APPDATA/com.nodeflow.desktop/voz/python.exe"
if ! netstat -ano 2>/dev/null | grep -qE "127\.0\.0\.1:8125 .*(LISTENING|ESCUCHANDO)"; then
  if [ -f "$VOZ_APP" ]; then
    echo "[voz] la voz local la arranca la app (instalada en $VOZ_APP)" >> "$REPO/.git/checkpoint.log"
  else
    echo "[voz] Kokoro no estaba escuchando: lo arranco" >> "$REPO/.git/checkpoint.log"
    wscript.exe "$(cygpath -w "$REPO/tools/tts/arrancar-oculto.vbs")" &
  fi
fi

bash "$REPO/scripts/limpiar-builds.sh"
bash "$REPO/scripts/backup.sh"  >> "$REPO/.git/checkpoint.log" 2>&1
bash "$REPO/scripts/checkpoint.sh" >> "$REPO/.git/checkpoint.log" 2>&1

# Prueba de guardado automático: esta línea la commitea la tarea programada sola.

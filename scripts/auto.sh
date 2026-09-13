#!/usr/bin/env bash
# NodeFlow · Lo que corre la tarea programada (cada 5 minutos, en segundo plano).
# 1) respaldo del día si falta  2) punto de guardado (verifica y recién ahí commitea/sube)

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bash "$REPO/scripts/backup.sh"  >> "$REPO/.git/checkpoint.log" 2>&1
bash "$REPO/scripts/checkpoint.sh" >> "$REPO/.git/checkpoint.log" 2>&1

# Prueba de guardado automático: esta línea la commitea la tarea programada sola.

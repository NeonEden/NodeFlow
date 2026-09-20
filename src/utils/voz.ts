/**
 * ¿El plan es una CONSULTA? Sólo lectura: pregunta por lo que ya está en el lienzo y no lo toca.
 *
 * Lo usan los dos extremos del flujo: el panel, para no pedir «¿lo aplico?» por algo que no se
 * aplica (no hay impacto que aprobar), y el ejecutor, para no tomar snapshot ni escribir.
 * Función pura y con tests: es la única definición de «esto no opera».
 */
import type { VozComando } from '../services/vozService';

export function planEsConsulta(comandos?: VozComando[] | null): boolean {
  if (!comandos?.length) return false;
  return comandos.every((c) => (c?.accion || '').toLowerCase() === 'consultar');
}

/** Los temas consultados, para el resumen del panel (los que el validador dejó pasar). */
export function temasDeConsulta(comandos?: VozComando[] | null): string[] {
  if (!comandos?.length) return [];
  return comandos
    .filter((c) => (c?.accion || '').toLowerCase() === 'consultar')
    .map((c) => (c.tema || '').trim())
    .filter(Boolean);
}

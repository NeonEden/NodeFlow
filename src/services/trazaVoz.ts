/**
 * Traza del ciclo de voz, **desde el webview**.
 *
 * Por qué existe: el log de Rust sólo veía el atajo (`pressed`/`released`) y la emisión del token. Todo lo
 * que pasa del lado del panel —cuánto tarda `start()`, cuándo llega el primer parcial, con qué motivo se
 * cerró el turno, si el turno se sirvió reusando la sesión o abriendo una nueva— no quedaba en ningún lado.
 * Medido el 20/09/2026 en el log de la app: 10 pulsaciones del atajo y **13 sesiones de STT emitidas**, sin
 * una sola línea que explicara por qué. Cada diagnóstico volvía a ser una deducción; esto lo vuelve un conteo.
 *
 * Reglas:
 * - **No se espera la respuesta** y ningún fallo se propaga: una traza que frena la voz es peor que no tenerla.
 * - En el demo web (sin backend local) el POST falla en silencio: la traza no es parte de la función.
 * - Los campos van como `clave=valor` en una sola línea, para contarlos con `grep -c "voz(ui).turno.cerrado"`.
 */

import { apiUrl } from './apiBase';

/** Eventos del ciclo de voz. Son nombres estables: el log se cuenta por ellos (`grep -c`). */
export type EventoVoz =
  | 'pedido'
  | 'listo'
  | 'parcial.primero'
  | 'turno.cerrado'
  | 'turno.cancelado'
  | 'sesion.inactiva'
  | 'sesion.cerrada'
  | 'error';

/** Valor de una traza: se recorta y se le sacan los espacios de más (es una línea, no un canal de datos). */
function valor(v: string | number | boolean): string {
  if (typeof v === 'boolean') return v ? 'si' : 'no';
  return String(v).replace(/\s+/g, ' ').trim().slice(0, 80);
}

/**
 * Manda una traza al log de la app. No bloquea, no espera y no falla hacia afuera.
 *
 * @param evento Uno de `EventoVoz` (se agrega al prefijo `voz(ui)` en el backend).
 * @param campos Pares clave/valor; los vacíos (`''`, `undefined`, `null`) se omiten para no inflar el log.
 */
export function trazaVoz(
  evento: EventoVoz,
  campos: Record<string, string | number | boolean | undefined | null> = {}
): void {
  const cuerpo = Object.entries(campos)
    .filter(([, v]) => v !== undefined && v !== null && v !== '')
    .map(([k, v]) => `${k}=${valor(v as string | number | boolean)}`)
    .join(' ');
  try {
    void fetch(apiUrl('/api/voz/traza'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ evento, campos: cuerpo }),
    }).catch(() => {
      /* sin backend (demo web): la traza no es parte de la función */
    });
  } catch {
    /* ídem: ningún camino de la voz puede romperse por una traza */
  }
}

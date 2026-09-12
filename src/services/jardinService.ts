/**
 * Fase 7a — Agente jardín desde la app.
 *
 * El jardín DIAGNOSTICA (solo lectura) y convierte sus hallazgos en PROPUESTAS: nunca escribe.
 * El humano aprueba en «Cambios del agente». Es el mismo contrato que el resto del sistema.
 */
import { apiUrl } from './apiBase';

export type Gravedad = 'alta' | 'media' | 'baja';

export interface ProblemaJardin {
  tipo: string;
  detalle: string;
  gravedad: Gravedad | string;
  accion: 'podar' | 'borrar' | 'conectar' | 'nada' | string;
  ids: string[];
}

export interface Padrino {
  nodo: string;
  titulo?: string;
  padre_sugerido: string;
  padre_titulo?: string;
  similitud?: number;
  confianza?: string;
}

export interface StatsJardin {
  nodos?: number;
  aristas?: number;
  huerfanos?: number;
  sin_descripcion?: number;
  sin_madurez?: number;
}

export interface EscaneoJardin {
  sano: boolean;
  mapa?: string;
  revision?: number;
  pendientes?: number;
  bloqueantes?: number;
  problemas: ProblemaJardin[];
  padrinos: Padrino[];
  stats: StatsJardin;
  error?: string;
}

export interface RespuestaJardin {
  ok: boolean;
  propuestas?: unknown[];
  motivos?: string[];
  mensaje?: string;
  accion?: string;
  error?: string;
  pendientes?: number;
}

async function pedir<T>(ruta: string, body?: unknown, timeoutMs = 45000): Promise<T | null> {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), timeoutMs);
  try {
    const r = await fetch(apiUrl(ruta), {
      method: body === undefined ? 'GET' : 'POST',
      headers: body === undefined ? undefined : { 'Content-Type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: ctl.signal,
    });
    return (await r.json().catch(() => null)) as T | null;
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

export function escanear(): Promise<EscaneoJardin | null> {
  return pedir<EscaneoJardin>('/api/graph/garden');
}

/** Convierte los hallazgos accionables en propuestas para aprobar. */
export function proponerArreglos(): Promise<RespuestaJardin | null> {
  return pedir<RespuestaJardin>('/api/graph/garden/fix', {});
}

/** Propone el reacomodo por niveles (determinista, sin solapamientos). */
export function reacomodar(): Promise<RespuestaJardin | null> {
  return pedir<RespuestaJardin>('/api/graph/tidy', {});
}

/** Tipo de hallazgo → etiqueta legible y si el jardín puede proponer algo. */
export const TIPOS: Record<string, { etiqueta: string; arreglable: boolean }> = {
  aristas_colgadas: { etiqueta: 'Aristas colgadas', arreglable: true },
  aristas_duplicadas: { etiqueta: 'Aristas duplicadas', arreglable: true },
  ids_duplicados: { etiqueta: 'IDs duplicados', arreglable: true },
  titulos_repetidos: { etiqueta: 'Títulos repetidos', arreglable: true },
  nodos_basura: { etiqueta: 'Nodos basura (archivos generados)', arreglable: true },
  huerfanos: { etiqueta: 'Nodos sin conexiones', arreglable: true },
  islas: { etiqueta: 'Islas separadas del núcleo', arreglable: true },
  sin_madurez: { etiqueta: 'Nodos sin madurez', arreglable: false },
  hubs_inmaduros: { etiqueta: 'Hubs inmaduros', arreglable: false },
};

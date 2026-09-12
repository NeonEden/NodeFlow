/**
 * Fase 5a — Cambios del agente: propuestas que esperan aprobación humana.
 *
 * El agente (Hermes, por MCP) ya no escribe directo en el lienzo: cada escritura entra acá como
 * propuesta con su vista previa. Aprobar la aplica; rechazar la descarta. La cola vive en el vault
 * (`.nodeflow/pending.json`), así que sobrevive reinicios de la app.
 */
import { apiUrl } from './apiBase';

export type Peligro = 'bajo' | 'medio' | 'alto';

export interface VistaPropuesta {
  accion_legible: string;
  titulo: string;
  resumen: string;
  cambios?: string[];
  nodo_id?: string | null;
  padre_id?: string | null;
  padre_titulo?: string | null;
  aristas_afectadas?: number;
  aristas_quitadas?: number;
  peligro: Peligro;
  antes?: Record<string, unknown> | null;
  despues?: Record<string, unknown> | null;
}

export interface Propuesta {
  id: string;
  tipo: 'nodo' | 'conectar' | 'borrar' | 'sanear';
  creado_ms: number;
  origen: string;
  motivo: string;
  vista: VistaPropuesta;
}

export interface RespuestaPendientes {
  changed: boolean;
  revision: number;
  total: number;
  pendientes?: Propuesta[];
}

export interface RespuestaResolucion {
  ok: boolean;
  accion: string;
  cantidad: number;
  detalle?: { id: string; resultado: string; detalle?: string }[];
  errores?: { id: string; error: string }[];
  revision?: number | null;
  pendientes: number;
}

/** Lista de propuestas; con `since` el backend responde barato si nada cambió. */
export async function fetchPendientes(since?: number): Promise<RespuestaPendientes | null> {
  try {
    const url = apiUrl(typeof since === 'number' ? `/api/agent/pending?since=${since}` : '/api/agent/pending');
    const r = await fetch(url);
    if (!r.ok) return null;
    return (await r.json()) as RespuestaPendientes;
  } catch {
    return null;
  }
}

async function resolver(
  accion: 'approve' | 'reject',
  payload: { id?: string; todos?: boolean }
): Promise<RespuestaResolucion | null> {
  try {
    const r = await fetch(apiUrl(`/api/agent/${accion}`), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    const data = await r.json().catch(() => null);
    if (!r.ok) return null;
    return data as RespuestaResolucion;
  } catch {
    return null;
  }
}

export const aprobarPropuestas = (payload: { id?: string; todos?: boolean }) =>
  resolver('approve', payload);

export const rechazarPropuestas = (payload: { id?: string; todos?: boolean }) =>
  resolver('reject', payload);

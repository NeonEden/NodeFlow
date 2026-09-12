/**
 * Fase 8 — Captura de conocimiento y exportación.
 *
 * Capturar NO escribe: convierte texto crudo en propuestas que el humano aprueba en el panel
 * «Cambios del agente». Exportar sí produce archivos: el entregable legible y el estado portable.
 */
import { apiUrl } from './apiBase';

export interface Candidato {
  titulo: string;
  descripcion: string;
  categoria: string;
  madurez: number;
  caracteres: number;
  ya_en_el_lienzo?: boolean;
}

export interface RespuestaPreview {
  ok: boolean;
  caracteres: number;
  candidatos: Candidato[];
  total: number;
  error?: string;
}

export interface RespuestaCaptura {
  ok: boolean;
  propuestos: number;
  detalle: { titulo: string; resultado: string; id_pendiente: string }[];
  pendientes_totales: number;
  nota?: string;
  error?: string;
}

export interface RespuestaDocumento {
  ok: boolean;
  nombre: string;
  contenido: string;
  caracteres: number;
  nodos: number;
  aristas: number;
  error?: string;
}

export interface RespuestaJson {
  ok: boolean;
  estado?: unknown;
  error?: string;
}

async function pedir<T>(ruta: string, body?: unknown, timeoutMs = 30000): Promise<T | null> {
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

export function previsualizar(texto: string, maxNodos = 15): Promise<RespuestaPreview | null> {
  return pedir<RespuestaPreview>('/api/knowledge/preview', { texto, max_nodos: maxNodos });
}

export function capturar(payload: {
  nodos: { titulo: string; descripcion: string; categoria?: string; madurez?: number }[];
  parent?: string;
  categoria?: string;
  madurez?: number;
  motivo?: string;
}): Promise<RespuestaCaptura | null> {
  return pedir<RespuestaCaptura>('/api/knowledge/capture', payload);
}

export function generarDocumento(): Promise<RespuestaDocumento | null> {
  return pedir<RespuestaDocumento>('/api/export/document');
}

export function exportarJson(): Promise<RespuestaJson | null> {
  return pedir<RespuestaJson>('/api/export/json');
}

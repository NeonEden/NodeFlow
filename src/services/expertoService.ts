/**
 * Slice 1 — Orquestador: expertos y artefactos.
 *
 * Un Experto es una nota de la bóveda con un system prompt. Ejecutarlo sobre un nodo devuelve un
 * artefacto VALIDADO contra su destino (Flow, Copilot, TouchDesigner). El backend hace el trabajo:
 * arma el contexto (nodo + vecinos + memoria BM25), llama al modelo y valida antes de devolver.
 */
import { apiUrl } from './apiBase';

export interface Experto {
  slug: string;
  nombre: string;
  tipo_artefacto: string;
  valido: boolean;
  modelo: string;
  proveedor: string;
  descripcion: string;
  rol: string;
  system: string;
  caracteres_system: number;
}

export interface TipoArtefacto {
  tipo: string;
  destino: string;
  descripcion: string;
}

export interface RegistroExpertos {
  ok: boolean;
  expertos: Experto[];
  tipos: TipoArtefacto[];
  carpeta: string;
}

export interface TrazaProveedor {
  proveedor: string;
  modelo?: string;
  valido?: boolean;
  resultado?: string;
  problemas?: string[];
  ms: number;
}

export interface Fuente {
  ruta?: string;
  titulo?: string;
  puntaje?: number;
}

export interface RespuestaExperto {
  ok: boolean;
  tipo: string;
  experto: string;
  nodo: { id: string; titulo: string };
  artefacto: Record<string, unknown> | null;
  texto: string;
  problemas: string[];
  intentos: number;
  proveedor: string;
  traza: TrazaProveedor[];
  ms: number;
  fuentes: Fuente[];
  contexto_chars: number;
  costo: string;
  error?: string;
}

async function pedir<T>(ruta: string, body?: unknown, timeoutMs = 300000): Promise<T | null> {
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

export function listarExpertos(): Promise<RegistroExpertos | null> {
  return pedir<RegistroExpertos>('/api/expertos');
}

export function ejecutarExperto(
  nodo: string,
  experto: string,
  extra?: string
): Promise<RespuestaExperto | null> {
  return pedir<RespuestaExperto>('/api/expert/run', { nodo, experto, extra: extra || '' });
}

export interface GuardarExpertoPayload {
  /** Si se manda, se reemplaza ese archivo. Si no, el slug se deriva del nombre. */
  slug?: string;
  nombre: string;
  tipo_artefacto: string;
  descripcion?: string;
  rol?: string;
  proveedor?: string;
  modelo?: string;
  /** El system prompt completo: es el cuerpo de la nota. */
  system: string;
}

export interface GuardarExpertoResultado {
  success: boolean;
  slug?: string;
  ruta?: string;
  caracteres_system?: number;
  error?: string;
}

/**
 * Escribe el prompt de un experto en `<bóveda>/expertos/<slug>.md`.
 *
 * El prompt es identidad del usuario: la app no trae ninguno embebido, esto escribe sólo en su
 * bóveda y no toca el lienzo (no pasa por la cola de propuestas: es una nota, reversible).
 */
export function guardarExperto(
  payload: GuardarExpertoPayload
): Promise<GuardarExpertoResultado | null> {
  return pedir<GuardarExpertoResultado>('/api/expertos/guardar', payload, 20000);
}

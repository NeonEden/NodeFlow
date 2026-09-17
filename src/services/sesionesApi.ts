/**
 * Sesiones del lienzo guardadas **en la bóveda** (`.nodeflow/sesiones/`).
 *
 * Antes vivían en el `localStorage` del WebView: no viajaban en el respaldo, no las veía el otro
 * perfil y se perdían al limpiar el perfil. Ahora son archivos de la bóveda, como el resto del
 * conocimiento: el backend los escribe con la misma escritura atómica que las notas.
 *
 * El listado es liviano (sin nodos ni aristas); el contenido se pide sólo al cargar una sesión.
 */
import type { Edge } from 'reactflow';
import type { CustomNode } from '../types';
import { apiUrl } from './apiBase';

export interface SesionFicha {
  id: string;
  nombre: string;
  creado: number;
  actualizado?: number | null;
  nodos: number;
  aristas: number;
  mapa?: string;
  appearance?: { color?: string; type?: string; animated?: boolean } | null;
  rodante?: boolean;
  bytes?: number;
}

export interface SesionCompleta extends SesionFicha {
  nodes: CustomNode[];
  edges: Edge[];
  templateId?: string | null;
}

async function getJson<T>(url: string, timeoutMs = 8000): Promise<T | null> {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), timeoutMs);
  try {
    const r = await fetch(url, { signal: ctl.signal });
    if (!r.ok) return null;
    return (await r.json()) as T;
  } catch {
    // El backend puede estar recompilando: no es un error fatal.
    return null;
  } finally {
    clearTimeout(timer);
  }
}

async function postJson<T>(url: string, body: unknown, timeoutMs = 25000): Promise<T | null> {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), timeoutMs);
  try {
    const r = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
      signal: ctl.signal,
    });
    return (await r.json().catch(() => null)) as T | null;
  } catch (e) {
    console.warn('sesiones: no pude hablar con el backend', e);
    return null;
  } finally {
    clearTimeout(timer);
  }
}

export async function listarSesiones(): Promise<SesionFicha[]> {
  const d = await getJson<{ ok: boolean; sesiones: SesionFicha[]; carpeta?: string }>(
    apiUrl('/api/sesiones'),
  );
  return d?.sesiones ?? [];
}

export async function leerSesion(id: string): Promise<SesionCompleta | null> {
  const d = await getJson<{ ok: boolean; sesion: SesionCompleta }>(
    apiUrl(`/api/sesiones/leer?id=${encodeURIComponent(id)}`),
  );
  return d?.sesion ?? null;
}

export interface GuardarSesionPayload {
  id?: string;
  nombre: string;
  mapa?: string;
  nodes: CustomNode[];
  edges: Edge[];
  appearance?: unknown;
  templateId?: string | null;
}

/** Guarda (o sobrescribe, si viene `id`) una sesión. Devuelve la ficha, o null si falló. */
export async function guardarSesion(p: GuardarSesionPayload): Promise<SesionFicha | null> {
  const d = await postJson<{ ok: boolean; sesion: SesionFicha; error?: string }>(
    apiUrl('/api/sesiones/guardar'),
    p,
  );
  if (d && d.ok === false) console.warn('sesiones: el backend rechazó el guardado', d.error);
  return d?.sesion ?? null;
}

export async function borrarSesion(id: string): Promise<boolean> {
  const d = await postJson<{ ok: boolean }>(apiUrl('/api/sesiones/borrar'), { id });
  return Boolean(d?.ok);
}

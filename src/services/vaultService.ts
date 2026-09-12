/**
 * Fase 3 — Vault en disco como fuente de verdad.
 *
 * El backend en Rust escribe, dentro de la bóveda de Obsidian:
 *   `<mapa>.canvas`   → canvas nativo (se ve en Obsidian al instante)
 *   `<mapa>.md`       → índice con frontmatter + [[wikilinks]]
 *   `nodos/<slug>.md` → una nota por nodo (editable desde Obsidian)
 *
 * Este servicio es el puente: guarda el grafo (debounced desde App), lee lo que hay en disco
 * y hace polling de la revisión para detectar ediciones hechas fuera de la app.
 */
import type { Edge } from 'reactflow';
import type { CustomNode } from '../types';
import { apiUrl } from './apiBase';

export interface VaultInfo {
  vault: string;
  mapa: string;
  revision: number;
  updated_at: number;
  notas: number;
  nodos_en_disco?: number | null;
  ultimos_cambios_externos: string[];
  tiene_estado: boolean;
}

export interface VaultState {
  nodes: CustomNode[];
  edges: Edge[];
  appearance?: unknown;
  templateId?: string | null;
  name?: string;
  updated_at?: number;
}

export interface Metricas {
  objetivo_min: number;
  promedio_min: number | null;
  ultima_min: number | null;
  conversiones: number;
  sesion_activa: boolean;
  t1_ms?: number | null;
  minutos_desde_t0?: number | null;
}

export interface VaultSnapshot {
  changed: boolean;
  revision: number;
  info?: VaultInfo;
  cambios_externos?: string[];
  state?: VaultState | null;
  metricas?: Metricas | null;
}

export interface VaultSaveResult {
  ok: boolean;
  revision?: number;
  updated_at?: number;
  mapa?: string;
  vault?: string;
  nodos?: number;
  aristas?: number;
  notas_borradas?: number;
  archivos?: { ruta: string; bytes: number; tipo: string }[];
  error?: string;
}

export interface VaultSavePayload {
  name: string;
  nodes: CustomNode[];
  edges: Edge[];
  appearance?: unknown;
  templateId?: string | null;
  /** Revisión del vault que este lienzo conoce: permite al backend no perder escrituras del agente. */
  base_revision?: number;
}

async function getJson<T>(url: string, timeoutMs = 6000): Promise<T | null> {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), timeoutMs);
  try {
    const r = await fetch(url, { signal: ctl.signal });
    if (!r.ok) return null;
    return (await r.json()) as T;
  } catch {
    // El backend local puede no estar levantado (o estar recompilando): no es un error fatal.
    return null;
  } finally {
    clearTimeout(timer);
  }
}

/** Estado del vault: ruta, revisión, cantidad de notas y últimos cambios externos. */
export function fetchVaultInfo(): Promise<VaultInfo | null> {
  return getJson<VaultInfo>(apiUrl('/api/vault/info'));
}

/** Lectura completa (arranque de la app). */
export function loadVaultState(): Promise<VaultSnapshot | null> {
  return getJson<VaultSnapshot>(apiUrl('/api/graph/state'));
}

/** Polling barato: si `since` coincide con la revisión, el backend responde {changed:false}. */
export function pollVault(since: number): Promise<VaultSnapshot | null> {
  return getJson<VaultSnapshot>(apiUrl(`/api/graph/state?since=${since}`), 4000);
}

/** Guarda el grafo en el vault (canónico + .canvas + índice + una nota por nodo). */
export async function saveVault(payload: VaultSavePayload): Promise<VaultSaveResult | null> {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), 20000);
  try {
    const r = await fetch(apiUrl('/api/graph/state'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
      signal: ctl.signal,
    });
    const data = (await r.json().catch(() => null)) as VaultSaveResult | null;
    if (!r.ok) {
      return { ok: false, error: data?.error || `HTTP ${r.status}` };
    }
    return data;
  } catch (e) {
    console.warn('vault: no pude guardar en disco', e);
    return null;
  } finally {
    clearTimeout(timer);
  }
}

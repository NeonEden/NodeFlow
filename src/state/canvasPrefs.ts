import { useSyncExternalStore } from 'react';
import { CanvasTheme, PAPEL, NOCHE, temaDesdeFondo } from './canvasTheme';

/**
 * Preferencias de apariencia del lienzo: fondo y superficie de las tarjetas.
 *
 * Son preferencias de UI, no datos del grafo: viven en `localStorage` y NUNCA tocan
 * el vault ni el estado del lienzo (a diferencia del color de acento de un nodo, que
 * sí es un dato del nodo y se edita desde el nodo mismo).
 *
 * Store externo mínimo + `useSyncExternalStore` para que lo lean tres consumidores
 * sin prop-drilling: el contenedor del lienzo (App), las aristas y las tarjetas.
 */

export type ModoTarjetas = 'clara' | 'tintada' | 'cristal';

export interface CanvasPrefs {
  /** Id de preset de fondo, o 'libre' si el usuario eligió un color propio. */
  fondoId: string;
  /** Color propio de fondo (sólo si fondoId === 'libre'). */
  fondoLibre: string;
  tarjetas: ModoTarjetas;
}

export interface FondoPreset {
  id: string;
  nombre: string;
  color: string;
}

/** Presets de fondo. El primero es la paleta curada por defecto. */
export const FONDOS: FondoPreset[] = [
  { id: 'papel', nombre: 'Papel', color: PAPEL.canvasBg },
  { id: 'blanco', nombre: 'Blanco', color: '#ffffff' },
  { id: 'humo', nombre: 'Humo', color: '#eef1f6' },
  { id: 'sepia', nombre: 'Sepia', color: '#f6efe3' },
  { id: 'pizarra', nombre: 'Pizarra', color: '#0f172a' },
  { id: 'noche', nombre: 'Noche', color: NOCHE.canvasBg },
];

export const MODOS_TARJETA: { id: ModoTarjetas; nombre: string; ayuda: string }[] = [
  { id: 'clara', nombre: 'Clara', ayuda: 'Superficie sólida del tema' },
  { id: 'tintada', nombre: 'Tintada', ayuda: 'Degradé suave con el color del nodo' },
  { id: 'cristal', nombre: 'Cristal', ayuda: 'Translúcida con desenfoque' },
];

export const PREFS_POR_DEFECTO: CanvasPrefs = {
  fondoId: 'papel',
  fondoLibre: PAPEL.canvasBg,
  tarjetas: 'clara',
};

const CLAVE = 'nodeflow_canvas_prefs';

function cargar(): CanvasPrefs {
  try {
    const crudo = localStorage.getItem(CLAVE);
    if (!crudo) return PREFS_POR_DEFECTO;
    const p = JSON.parse(crudo);
    return {
      fondoId: typeof p.fondoId === 'string' ? p.fondoId : PREFS_POR_DEFECTO.fondoId,
      fondoLibre: typeof p.fondoLibre === 'string' ? p.fondoLibre : PREFS_POR_DEFECTO.fondoLibre,
      tarjetas: ['clara', 'tintada', 'cristal'].includes(p.tarjetas)
        ? p.tarjetas
        : PREFS_POR_DEFECTO.tarjetas,
    };
  } catch {
    return PREFS_POR_DEFECTO;
  }
}

let state: CanvasPrefs = cargar();
const listeners = new Set<() => void>();

function commit(next: CanvasPrefs) {
  if (
    next.fondoId === state.fondoId &&
    next.fondoLibre === state.fondoLibre &&
    next.tarjetas === state.tarjetas
  ) {
    return;
  }
  state = next;
  try {
    localStorage.setItem(CLAVE, JSON.stringify(state));
  } catch {
    /* sin persistencia: la sesión igual funciona */
  }
  listeners.forEach((l) => l());
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export const canvasPrefs = {
  subscribe,
  getState: () => state,
  setFondo: (fondoId: string) => commit({ ...state, fondoId }),
  setFondoLibre: (hex: string) => commit({ ...state, fondoId: 'libre', fondoLibre: hex }),
  setTarjetas: (tarjetas: ModoTarjetas) => commit({ ...state, tarjetas }),
  reset: () => commit(PREFS_POR_DEFECTO),
};

function calcularTema(p: CanvasPrefs): CanvasTheme {
  if (p.fondoId === 'libre') return temaDesdeFondo(p.fondoLibre);
  const preset = FONDOS.find((f) => f.id === p.fondoId);
  if (!preset) return PAPEL;
  if (preset.id === 'papel') return PAPEL;
  return temaDesdeFondo(preset.color, preset.id);
}

// Caché por identidad de preferencias: `getSnapshot` de useSyncExternalStore DEBE
// devolver siempre la misma referencia mientras nada cambie. Sin esto, cada llamada
// creaba un objeto nuevo → re-render infinito y la app se caía al cambiar el tema.
let cache: { prefs: CanvasPrefs; tema: CanvasTheme } | null = null;

/** Tema resuelto a partir de las preferencias. Referencia estable por prefs. */
export function resolverTema(p: CanvasPrefs = state): CanvasTheme {
  if (cache && cache.prefs === p) return cache.tema;
  const tema = calcularTema(p);
  cache = { prefs: p, tema };
  return tema;
}

export function usePrefs(): CanvasPrefs {
  return useSyncExternalStore(subscribe, canvasPrefs.getState);
}

export function useTema(): CanvasTheme {
  return useSyncExternalStore(subscribe, () => resolverTema());
}

export function useTarjetas(): ModoTarjetas {
  return useSyncExternalStore(subscribe, () => state.tarjetas);
}

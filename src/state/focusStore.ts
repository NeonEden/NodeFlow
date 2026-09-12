/**
 * Foco visual del lienzo: qué nodo/arista está bajo el mouse o seleccionado.
 *
 * Store externo mínimo (sin dependencias) a propósito: las 53 aristas se
 * suscriben con `useSyncExternalStore` y se re-renderizan SOLO cuando cambia el
 * foco. El componente App (3.000 líneas) no se re-renderiza al pasar el mouse
 * por un nodo, que es lo que haría un useState ahí arriba.
 */

export interface FocusState {
  hoveredNodeId: string | null;
  hoveredEdgeId: string | null;
  selectedNodeIds: string[];
}

const EMPTY: FocusState = {
  hoveredNodeId: null,
  hoveredEdgeId: null,
  selectedNodeIds: [],
};

let state: FocusState = EMPTY;
const listeners = new Set<() => void>();

/** Reemplaza el estado sólo si algo cambió: la referencia estable es el contrato
 *  de `useSyncExternalStore` (si devolvemos un objeto nuevo cada vez, re-render infinito). */
function commit(next: FocusState) {
  if (
    next.hoveredNodeId === state.hoveredNodeId &&
    next.hoveredEdgeId === state.hoveredEdgeId &&
    next.selectedNodeIds.length === state.selectedNodeIds.length &&
    next.selectedNodeIds.every((id, i) => id === state.selectedNodeIds[i])
  ) {
    return;
  }
  state = next;
  listeners.forEach((l) => l());
}

export const focusStore = {
  subscribe(listener: () => void) {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  },
  getState(): FocusState {
    return state;
  },
};

export function setHoveredNode(id: string | null) {
  if (id === state.hoveredNodeId) return;
  commit({ ...state, hoveredNodeId: id });
}

export function setHoveredEdge(id: string | null) {
  if (id === state.hoveredEdgeId) return;
  commit({ ...state, hoveredEdgeId: id });
}

export function setFocusSelection(ids: string[]) {
  commit({ ...state, selectedNodeIds: ids });
}

export function resetFocus() {
  commit(EMPTY);
}

/**
 * Nodo que manda sobre la atenuación: el hover gana sobre la selección, y la
 * selección SÓLO enfoca cuando hay un único nodo elegido (con multiselección
 * atenuar las aristas dejaría de tener un origen claro).
 */
export function focusedNodeId(s: FocusState = state): string | null {
  if (s.hoveredNodeId) return s.hoveredNodeId;
  return s.selectedNodeIds.length === 1 ? s.selectedNodeIds[0] : null;
}

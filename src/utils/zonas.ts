import { Edge } from 'reactflow';
import { CustomNode } from '../types';

/**
 * Zonas derivadas: marcos que se calculan en el render a partir de la posición y
 * de la estructura del grafo. NO son nodos del lienzo, no se guardan en el vault y
 * no tocan el modelo de datos.
 *
 * Por qué niveles de profundidad y no categorías:
 * - Categoría: medido sobre el lienzo real (42 nodos), la máxima pureza de cualquier
 *   recorte espacial por categoría es 50% y ARQUITECTURA abarca 4.370 px de alto.
 *   Un marco que dice ARQUITECTURA y contiene mitad de otra cosa miente.
 * - Ramas del grafo: los subárboles de los 9 hijos del núcleo se solapan al 100%.
 * - Proximidad pura sobre la retícula del layout: o 42 zonas de un nodo, o una sola.
 * - Profundidad: el eje X del layout ES la profundidad (120/540/960/1380/1800), así que
 *   el marco coincide exactamente con una columna real del lienzo y la etiqueta
 *   "Nivel N" es un hecho, no una interpretación.
 */

export interface Nivel {
  /** Profundidad desde el núcleo (0 = núcleo). */
  nivel: number;
  ids: string[];
  /** Caja del marco en coordenadas del lienzo. */
  x: number;
  y: number;
  width: number;
  height: number;
  /** Categoría dominante, sólo si la pureza la sostiene (≥60% y ≥3 nodos). */
  categoria: string | null;
  pureza: number;
}

const PAD_X = 34;
const PAD_TOP = 74;
const PAD_BOTTOM = 40;

/** Ancho estimado de la tarjeta según su grado (mismo criterio que IdeaNode). */
function anchoEstimado(grado: number): number {
  return grado >= 5 ? 302 : 232;
}

export function calcularNiveles(nodes: CustomNode[], edges: Edge[]): Nivel[] {
  if (nodes.length === 0) return [];

  const grado = new Map<string, number>();
  const salientes = new Map<string, string[]>();
  edges.forEach((e) => {
    grado.set(e.source, (grado.get(e.source) || 0) + 1);
    grado.set(e.target, (grado.get(e.target) || 0) + 1);
    const l = salientes.get(e.source) || [];
    l.push(e.target);
    salientes.set(e.source, l);
  });

  // Raíz: el núcleo declarado; si no hay, el nodo más conectado (determinista por id).
  const declarado = nodes.find((n) => n.data?.isRoot);
  const raiz =
    declarado?.id ||
    [...nodes].sort(
      (a, b) => (grado.get(b.id) || 0) - (grado.get(a.id) || 0) || a.id.localeCompare(b.id)
    )[0]?.id;
  if (!raiz) return [];

  const profundidad = new Map<string, number>([[raiz, 0]]);
  const cola: string[] = [raiz];
  while (cola.length) {
    const u = cola.shift() as string;
    const d = profundidad.get(u) as number;
    for (const v of salientes.get(u) || []) {
      if (!profundidad.has(v)) {
        profundidad.set(v, d + 1);
        cola.push(v);
      }
    }
  }

  // Los nodos que no cuelgan del núcleo van a un nivel propio al final, igual que el
  // layout (que los manda a una columna aparte a la derecha).
  let maxNivel = 0;
  profundidad.forEach((d) => {
    if (d > maxNivel) maxNivel = d;
  });
  nodes.forEach((n) => {
    if (!profundidad.has(n.id)) profundidad.set(n.id, maxNivel + 1);
  });

  const porNivel = new Map<number, CustomNode[]>();
  nodes.forEach((n) => {
    const d = profundidad.get(n.id) as number;
    const l = porNivel.get(d) || [];
    l.push(n);
    porNivel.set(d, l);
  });

  const niveles: Nivel[] = [];
  porNivel.forEach((miembros, nivel) => {
    const x0 = Math.min(...miembros.map((n) => n.position.x));
    const y0 = Math.min(...miembros.map((n) => n.position.y));
    const x1 = Math.max(...miembros.map((n) => n.position.x + anchoEstimado(grado.get(n.id) || 0)));
    const y1 = Math.max(...miembros.map((n) => n.position.y + 200));

    const conteo = new Map<string, number>();
    miembros.forEach((n) => {
      const c = n.data?.category || 'SIN CATEGORÍA';
      conteo.set(c, (conteo.get(c) || 0) + 1);
    });
    const [catDominante, nCat] = [...conteo.entries()].sort((a, b) => b[1] - a[1])[0] || ['', 0];
    const pureza = miembros.length ? nCat / miembros.length : 0;

    niveles.push({
      nivel,
      ids: miembros.map((n) => n.id),
      x: x0 - PAD_X,
      y: y0 - PAD_TOP,
      width: x1 - x0 + PAD_X * 2,
      height: y1 - y0 + PAD_TOP + PAD_BOTTOM,
      // La etiqueta no promete más de lo que la evidencia sostiene.
      categoria: pureza >= 0.6 && miembros.length >= 3 ? catDominante : null,
      pureza,
    });
  });

  return niveles.sort((a, b) => a.nivel - b.nivel);
}

/** Acento de la zona: el color dominante de sus miembros, para que el marco hable el
 *  idioma cromático del contenido y no imponga uno nuevo. */
export function acentoDeNivel(nodos: CustomNode[]): string {
  const conteo = new Map<string, number>();
  nodos.forEach((n) => {
    const c = n.data?.colorAccent || '#6366f1';
    conteo.set(c, (conteo.get(c) || 0) + 1);
  });
  return [...conteo.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] || '#6366f1';
}

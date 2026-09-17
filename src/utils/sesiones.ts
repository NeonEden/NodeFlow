/**
 * Sesiones del lienzo: la mitad «volver» de guardar el progreso.
 *
 * Una sesión guardada es un **conjunto de nodos**, no una foto de pantalla: mover un nodo de lugar
 * no debería marcarla como «modificada». Por eso la firma ignora las posiciones y mira el contenido
 * (ids, títulos, madurez, categoría, largo de la descripción y la forma de las aristas).
 *
 * La firma sirve para dos cosas concretas: saber qué sesión está cargada en el lienzo ahora mismo, y
 * avisar cuando el lienzo se movió respecto de ella (así «Cargar» no pisa trabajo sin avisar).
 */
import { Edge } from 'reactflow';
import { CustomNode } from '../types';

/** Hash barato (djb2) para comparar dos lienzos sin guardar el texto entero. */
function djb2(texto: string): string {
  let h = 5381;
  for (let i = 0; i < texto.length; i++) h = ((h << 5) + h + texto.charCodeAt(i)) >>> 0;
  return h.toString(36);
}

export function firmaLienzo(nodes: CustomNode[], edges: Edge[]): string {
  const nodos = nodes
    .map((n) =>
      [
        n.id,
        n.data?.title ?? '',
        n.data?.maturity ?? '',
        n.data?.category ?? '',
        (n.data?.description ?? '').length,
      ].join('~'),
    )
    .sort()
    .join('|');
  const aristas = edges
    .map((e) => `${e.source}>${e.target}`)
    .sort()
    .join('|');
  return `${nodes.length}.${edges.length}.${djb2(`${nodos}#${aristas}`)}`;
}

/** Nombre por defecto de una sesión: el título del nodo raíz + la hora, para poder distinguirlas. */
export function nombreDeSesion(nodes: CustomNode[], cuando: Date = new Date()): string {
  const hora = cuando.toLocaleTimeString('es-AR', { hour: '2-digit', minute: '2-digit' });
  const raiz = nodes.find((n) => n.data?.isRoot)?.data?.title?.trim();
  return raiz ? `${raiz} · ${hora}` : `Guardado manual ${hora}`;
}

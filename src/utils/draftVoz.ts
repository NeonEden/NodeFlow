/**
 * Borrador del turno de voz: lo que se está entendiendo **mientras** se habla.
 *
 * Por qué existe: hasta hoy el lienzo reaccionaba recién en `end_of_turn`, con el turno ya cerrado. El
 * cliente de AssemblyAI **ya emite** los `Turn` parciales (`assemblyaiRt.ts` → `ev.onParcial`), pero su
 * único consumidor los usaba para texto en pantalla y para el barge-in: la idea aparecía en el lienzo
 * sólo al terminar la frase. Esto convierte ese parcial en un NODO FANTASMA —visible, atenuado y que
 * NO se persiste— para que el lienzo crezca al ritmo de la voz.
 *
 * Reglas locales y deterministas, sin modelo (ADR 0005): el dibujo no puede depender de que un LLM
 * responda. El segmentador completo (`nada · semilla · corrección`, con estabilidad y confianza del
 * turno) vive en el backend (`/api/voz/parcial`); esto es la regla mínima del cliente, y existe para
 * que la Fase C no se caiga entera si el backend no contesta.
 *
 * Invariante que no se negocia: el fantasma NO es un nodo del grafo. No viaja al backend, no entra a la
 * cola de propuestas y se filtra en los dos guardados (localStorage y bóveda). Lo que se persiste sigue
 * pasando por el validador de `voz.rs`.
 */

/** Menos palabras que esto no es una idea: es el arranque de una palabra. */
export const MIN_PALABRAS = 3;

/** Cuántas palabras del parcial forman el título del fantasma (el título se ve; el texto va completo). */
export const PALABRAS_TITULO = 7;

/** Id fijo: hay un solo fantasma por turno, y un turno es una frase. */
export const ID_FANTASMA = 'ghost-voz-turno';

/** Clase CSS del contenedor del nodo fantasma (el estilo vive en `index.css`). */
export const CLASE_FANTASMA = 'nf-fantasma';

export function palabras(texto: string): string[] {
  return (texto || '').trim().split(/\s+/).filter(Boolean);
}

/** ¿Ya hay una idea que valga la pena dibujar? */
export function esIdeaEnVivo(texto: string): boolean {
  return palabras(texto).length >= MIN_PALABRAS;
}

/**
 * Título corto del borrador. Se recorta a `PALABRAS_TITULO` y se le saca la puntuación de los bordes:
 * el título es el encabezado de una idea a medio decir, no una cita textual.
 */
export function tituloDelBorrador(texto: string): string {
  const p = palabras(texto).slice(0, PALABRAS_TITULO);
  if (!p.length) return '';
  const t = p.join(' ').replace(/^[\s,.;:¡!¿?"'()«»“”\-]+/, '').replace(/[\s,.;:«»“”]+$/, '');
  return t || p.join(' ');
}

/**
 * ¿El parcial trae una corrección de lo que ya dijo? Se mira sólo en los primeros 5 tokens: un «no»
 * perdido en el medio de una frase es parte del contenido, no un arrepentimiento.
 *
 * `no` suelto **no** cuenta: «nodo», «norte», «nota» y «no sé» empiezan igual. Se piden marcadores
 * inequívocos — y esto es deliberadamente corto: la versión áspera (dudas, pausas, estabilidad del
 * parcial) es del segmentador del backend.
 */
const MARCADORES_CORRECCION = ['no,', 'mejor dicho', 'en realidad', 'olvidate', 'quise decir', 'corrijo', 'esperá,', 'espera,'];

export function esCorreccion(texto: string): boolean {
  const arranque = palabras(texto).slice(0, 5).join(' ').toLowerCase();
  return MARCADORES_CORRECCION.some((m) => arranque.includes(m));
}

/** Lo mínimo que necesita el fantasma para ubicarse: la posición del nodo que le sirve de ancla. */
export interface AnclaFantasma {
  position: { x: number; y: number };
}

/** El nodo fantasma listo para React Flow. Estructural: no depende de los tipos de la app. */
export interface FantasmaVoz {
  id: string;
  type: 'ideaNode';
  position: { x: number; y: number };
  draggable: false;
  selectable: false;
  className: string;
  data: {
    id: string;
    title: string;
    description: string;
    maturity: number;
    ghost: true;
  };
}

/** Desplazamiento respecto del ancla: el borrador cae al lado del árbol, no encima. */
export const OFFSET_FANTASMA = 72;

/**
 * Construye el nodo fantasma, o `null` si todavía no hay idea que dibujar. Es pura a propósito: es la
 * parte que decide **qué se ve mientras se habla** y tiene que poder probarse sin montar el lienzo.
 */
export function nodoFantasma(texto: string, ancla: AnclaFantasma | null | undefined): FantasmaVoz | null {
  if (!esIdeaEnVivo(texto)) return null;
  const position = ancla
    ? { x: ancla.position.x + OFFSET_FANTASMA, y: ancla.position.y + OFFSET_FANTASMA }
    : { x: 80, y: 80 };
  return {
    id: ID_FANTASMA,
    type: 'ideaNode',
    position,
    draggable: false,
    selectable: false,
    className: CLASE_FANTASMA,
    data: {
      id: ID_FANTASMA,
      title: tituloDelBorrador(texto),
      description: (texto || '').trim(),
      maturity: 1,
      ghost: true,
    },
  };
}

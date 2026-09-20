/**
 * ¿El plan es una CONSULTA? Sólo lectura: pregunta por lo que ya está en el lienzo y no lo toca.
 *
 * Lo usan los dos extremos del flujo: el panel, para no pedir «¿lo aplico?» por algo que no se
 * aplica (no hay impacto que aprobar), y el ejecutor, para no tomar snapshot ni escribir.
 * Función pura y con tests: es la única definición de «esto no opera».
 */
import type { VozComando } from '../services/vozService';

export function planEsConsulta(comandos?: VozComando[] | null): boolean {
  if (!comandos?.length) return false;
  return comandos.every((c) => (c?.accion || '').toLowerCase() === 'consultar');
}

/** Los temas consultados, para el resumen del panel (los que el validador dejó pasar). */
export function temasDeConsulta(comandos?: VozComando[] | null): string[] {
  if (!comandos?.length) return [];
  return comandos
    .filter((c) => (c?.accion || '').toLowerCase() === 'consultar')
    .map((c) => (c.tema || '').trim())
    .filter(Boolean);
}

/** Sin tildes y en minúsculas: el motor transcribe «sí», «si» o «SI» y la comparación no puede depender de eso. */
function plano(s: string): string {
  return (s || '')
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase();
}

/**
 * Las muletillas con las que la gente arranca una idea hablada. **Un solo prefijo**, y el más largo primero
 * («quiero explorar la idea de» antes que «quiero»), o la muletilla corta se come a la larga.
 */
const MULETILLAS = [
  'quiero explorar la idea de',
  'quiero explorar',
  'tengo una idea de',
  'tengo ganas de',
  'estaria bueno',
  'seria bueno',
  'me gustaria',
  'la idea es',
  'pense en',
  'podriamos',
  'necesito',
  'quiero',
];

/**
 * El TEMA de lo dictado, sin las muletillas del habla.
 *
 * Medido 20/09/2026: el nodo quedaba titulado con la frase entera («quiero explorar la idea de comandos
 * por voz») porque el título era el dictado recortado a 120 caracteres. El título es el tema; la frase
 * completa va en la descripción.
 *
 * Es la misma regla que `voz::normalizar_titulo` en Rust: la usan los dos caminos (el guion local acá y el
 * plan del modelo allá). Si cambia la lista en un lado, cambia en el otro — hay un test en cada extremo.
 */
export function temaDe(frase: string): string {
  let t = (frase || '').trim();
  // Bordes: comillas y puntuación vienen alternadas («…».), así que se limpia en rondas hasta que
  // no cambie nada. Una sola pasada deja el «» colgando cuando el punto va después de la comilla.
  for (let i = 0; i < 3; i++) {
    const antes = t;
    t = t
      .replace(/^["'«“”»]+/, '')
      .replace(/["'«“”»]+$/, '')
      .replace(/[.;,]+$/, '')
      .trim();
    if (t === antes) break;
  }
  if (!t) return '';
  const p = plano(t);
  for (const m of MULETILLAS) {
    if (p.startsWith(m)) {
      const resto = t.slice(m.length).replace(/^[\s,:]+/, '').trim();
      // Sólo se saca si queda algo con sentido: «quiero» solo no puede quedar en vacío.
      if (resto.length >= 2) {
        t = resto;
        break;
      }
    }
  }
  t = t.charAt(0).toUpperCase() + t.slice(1);
  if (t.length > 60) {
    const corte = t.slice(0, 60);
    const ultimo = corte.lastIndexOf(' ');
    t = (ultimo > 20 ? corte.slice(0, ultimo) : corte).trim();
  }
  return t;
}

/** ¿Dijo que no? Gana sobre cualquier afirmación que venga después («no, mejor dale vos»). */
export function esNegativo(frase: string): boolean {
  return /^\s*(no|nunca|mejor no|todavia no|aun no|para nada)\b/.test(plano(frase));
}

/**
 * ¿Dijo que sí? Antes era un regex de UNA palabra al principio de la frase: «bueno, me gustaría ver qué
 * sale» no matcheaba y la app contestaba como si hubiera dicho que no (medido 20/09/2026). Ahora se mira
 * la frase entera y se aceptan las formas con las que la gente contesta de verdad.
 */
export function esAfirmativo(frase: string): boolean {
  if (esNegativo(frase)) return false;
  const p = plano(frase);
  return (
    /\b(si|claro|dale|de una|obvio|por supuesto|vamos|hacelo|hace lo|perfecto|joya|genial|ok|okey|buenisimo|yes|sure|bueno)\b/.test(
      p
    ) ||
    /\b(me gustaria|quiero|quisiera|podriamos|seria bueno|estaria bueno|mostrame|explora|exploralo)\b/.test(p)
  );
}

/** ¿Quiere terminar la conversación? */
export function esCorte(frase: string): boolean {
  return /^\s*(corta|cortala|chau|adios|gracias|nada mas|suficiente|terminemos|paramos|listo|despues seguimos|seguimos despues)\b/.test(
    plano(frase)
  );
}

/**
 * Elige una variante sin repetir la última: la conversación no puede decir dos veces seguidas lo mismo
 * (es exactamente lo que la hacía sonar a bot con las frases fijas).
 */
export function elegirVariante(total: number, ultima: number): number {
  if (total <= 1) return 0;
  const i = Math.floor(Math.random() * (total - 1));
  return i >= ultima ? i + 1 : i;
}

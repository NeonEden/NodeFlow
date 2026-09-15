import { apiUrl } from '../services/apiBase';

/**
 * Idioma de la aplicación: estado, persistencia y voz asociada.
 *
 * Por qué sin librería: son dos idiomas y una app de escritorio que corre offline. Un catálogo
 * tipado propio da lo que i18next daría acá —cero dependencias nuevas, cero peso de bundle— y además
 * convierte una traducción faltante en un **error de compilación**, que es lo que de verdad evita que
 * la interfaz quede a medio traducir.
 */

export type Idioma = 'es' | 'en';

export const IDIOMAS: { id: Idioma; etiqueta: string; corto: string }[] = [
  { id: 'es', etiqueta: 'Español', corto: 'ES' },
  { id: 'en', etiqueta: 'English', corto: 'EN' },
];

export const POR_DEFECTO: Idioma = 'es';

/**
 * Voz de Kokoro por idioma. El switch no es sólo cosmético: la frase tiene que sonar nativa en los
 * dos casos, así que la voz viaja con el idioma hasta el servidor de voz local.
 */
export const VOZ_POR_IDIOMA: Record<Idioma, string> = {
  es: 'ef_dora',
  en: 'af_bella',
};

const CLAVE_LS = 'nodeflow.idioma';

function leerInicial(): Idioma {
  try {
    const guardado = localStorage.getItem(CLAVE_LS);
    if (guardado === 'es' || guardado === 'en') return guardado;
  } catch {
    /* sin localStorage (o bloqueado): se sigue con el idioma del sistema */
  }
  const navegador = typeof navigator !== 'undefined' ? navigator.language.slice(0, 2).toLowerCase() : '';
  return navegador === 'en' ? 'en' : POR_DEFECTO;
}

let actual: Idioma = leerInicial();
const oyentes = new Set<() => void>();

export function idiomaActual(): Idioma {
  return actual;
}

/** La voz que corresponde al idioma activo (o a uno puntual, si se pregunta por otro). */
export function vozPara(idioma: Idioma = actual): string {
  return VOZ_POR_IDIOMA[idioma];
}

export function suscribir(fn: () => void): () => void {
  oyentes.add(fn);
  return () => {
    oyentes.delete(fn);
  };
}

/** Cambia el idioma: actualiza la UI, lo recuerda y lo persiste en el backend. */
export async function setIdioma(idioma: Idioma): Promise<void> {
  if (idioma === actual) return;
  actual = idioma;
  try {
    localStorage.setItem(CLAVE_LS, idioma);
  } catch {
    /* la preferencia vive igual en el backend */
  }
  if (typeof document !== 'undefined') document.documentElement.lang = idioma;
  oyentes.forEach((fn) => fn());
  // El backend lo guarda y lo usa para la voz (transcripción y voz de salida).
  try {
    await fetch(apiUrl('/api/idioma'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ idioma }),
    });
  } catch {
    /* si el backend no responde, la preferencia del frontend ya quedó aplicada */
  }
}

/** El idioma que el backend tiene guardado (fuente de verdad al arrancar en otra máquina). */
export async function sincronizarConBackend(): Promise<Idioma | null> {
  try {
    const r = await fetch(apiUrl('/api/idioma'));
    if (!r.ok) return null;
    const d = (await r.json()) as { idioma?: string };
    if (d.idioma !== 'es' && d.idioma !== 'en') return null;
    if (d.idioma !== actual) {
      actual = d.idioma;
      try {
        localStorage.setItem(CLAVE_LS, actual);
      } catch {
        /* nada */
      }
      oyentes.forEach((fn) => fn());
    }
    return actual;
  } catch {
    return null;
  }
}

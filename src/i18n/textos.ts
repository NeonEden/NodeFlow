import type { Idioma } from './idioma';

/**
 * Catálogo de textos de la interfaz.
 *
 * Regla: la clave describe la **intención** (`voz.faltaClave`), nunca el texto en español. Así se
 * puede cambiar la redacción de cualquiera de los dos idiomas sin tocar el código, y agregar una
 * clave nueva obliga al compilador a pedir las dos versiones (`Record<Clave, string>`).
 *
 * Estado: este catálogo cubre la primera capa (barra de herramientas, switch de idioma y panel de
 * voz). El resto de la interfaz está inventariado: 407 textos en 31 archivos. Se incorporan por
 * panel, en el mismo formato, sin cambiar nada más.
 */
const ES = {
  'app.idioma': 'Idioma',
  'app.idioma.ayuda': 'Idioma de la interfaz y de la voz',
  'toolbar.sidebar.ocultar': 'Ocultar barra lateral',
  'toolbar.sidebar.mostrar': 'Mostrar barra lateral',
  'toolbar.deshacer': 'Deshacer (Ctrl+Z)',
  'toolbar.rehacer': 'Rehacer (Ctrl+Y)',
  'toolbar.organizar': 'Auto-organizar nodos en jerarquía limpia (evita solapamiento)',
  'toolbar.descubrir': 'Descubrir sinergias y fusionar ideas seleccionadas con IA',
  'toolbar.descarga': 'Descarga Mental Rápida: convierte notas o viñetas en un mapa completo (Ctrl+B)',
  'voz.faltaClave': 'Falta la clave de Speechmatics',
  'voz.nadaAplicable': 'No encontré nada aplicable en el lienzo para eso.',
  'voz.vaAPasar': 'Va a pasar esto:',
  'voz.deshacer': 'Ctrl+Z lo deshace si no te gusta.',
  'voz.investigando': 'Investigando por fases',
  'voz.motorProfundo': 'Motor profundo',
  'voz.pie': 'Speechmatics Realtime + el motor elegido en la app',
  'voz.idiomaAviso': 'La voz usa el idioma de la interfaz.',
} as const;

export type Clave = keyof typeof ES;

const EN: Record<Clave, string> = {
  'app.idioma': 'Language',
  'app.idioma.ayuda': 'Interface and voice language',
  'toolbar.sidebar.ocultar': 'Hide sidebar',
  'toolbar.sidebar.mostrar': 'Show sidebar',
  'toolbar.deshacer': 'Undo (Ctrl+Z)',
  'toolbar.rehacer': 'Redo (Ctrl+Y)',
  'toolbar.organizar': 'Auto-arrange nodes into a clean hierarchy (avoids overlap)',
  'toolbar.descubrir': 'Discover synergies and merge selected ideas with AI',
  'toolbar.descarga': 'Quick Brain Dump: turn notes or bullets into a full map (Ctrl+B)',
  'voz.faltaClave': 'Speechmatics key is missing',
  'voz.nadaAplicable': 'I found nothing on the canvas that applies to that.',
  'voz.vaAPasar': 'This is what will happen:',
  'voz.deshacer': 'Ctrl+Z undoes it if you do not like it.',
  'voz.investigando': 'Researching in phases',
  'voz.motorProfundo': 'Deep engine',
  'voz.pie': 'Speechmatics Realtime + the engine chosen in the app',
  'voz.idiomaAviso': 'Voice uses the interface language.',
};

export const CATALOGOS: Record<Idioma, Record<Clave, string>> = { es: ES, en: EN };

/** Traduce una clave, con interpolación simple de `{variable}`. */
export function traducir(idioma: Idioma, clave: Clave, vars?: Record<string, string | number>): string {
  const texto = CATALOGOS[idioma][clave] ?? CATALOGOS.es[clave] ?? clave;
  if (!vars) return texto;
  return texto.replace(/\{(\w+)\}/g, (_, nombre) =>
    nombre in vars ? String(vars[nombre]) : `{${nombre}}`,
  );
}

/** Cuántas claves están cubiertas hoy (para medir el avance de la traducción sin abrir el código). */
export const CLAVES_CUBIERTAS: number = Object.keys(ES).length;

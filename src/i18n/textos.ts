import type { Idioma } from './idioma';

/**
 * Catálogo de textos de la interfaz.
 *
 * Regla: la clave describe la **intención** (`voz.faltaClave`), nunca el texto en español. Así se
 * puede cambiar la redacción de cualquiera de los dos idiomas sin tocar el código, y agregar una
 * clave nueva obliga al compilador a pedir las dos versiones (`Record<Clave, string>`).
 *
 * Estado de cobertura (medido, no estimado): la barra de herramientas, el sidebar completo, el HUD del
 * lienzo, el panel de voz, el panel de cambios del agente y la apariencia del lienzo están traducidos.
 * El inventario total es de 339 textos; los que faltan viven en los modales secundarios (estados
 * guardados, HITL, autenticación, orquestador, síntesis) y se incorporan con `scripts/i18n-aplicar.mjs`
 * — el mismo script que aplicó esta tanda, que informa si un texto se movió de lugar.
 *
 * Por qué sin librería: son dos idiomas y una app de escritorio que corre offline. Este catálogo da lo
 * que i18next daría acá —cero dependencias nuevas, cero peso de bundle— y además convierte una
 * traducción faltante en un **error de compilación**, que es lo que de verdad evita que la interfaz
 * quede a medio traducir.
 */
const ES = {
  // --- idioma
  'app.idioma': 'Idioma',
  'app.idioma.ayuda': 'Idioma de la interfaz y de la voz',

  // --- barra lateral
  'toolbar.sidebar.ocultar': 'Ocultar barra lateral',
  'toolbar.sidebar.mostrar': 'Mostrar barra lateral',
  'toolbar.deshacer': 'Deshacer (Ctrl+Z)',
  'toolbar.rehacer': 'Rehacer (Ctrl+Y)',
  'toolbar.organizar': 'Auto-organizar nodos en jerarquía limpia (evita solapamiento)',
  'toolbar.descubrir': 'Descubrir sinergias y fusionar ideas seleccionadas con IA',
  'toolbar.descarga': 'Descarga Mental Rápida: convierte notas o viñetas en un mapa completo (Ctrl+B)',
  'toolbar.descargaCorta': 'Descarga',
  'toolbar.hibridar': 'Hibridador IA',
  'toolbar.buscar': 'Buscar nodos... (Ctrl+F)',
  'toolbar.exportar': 'Exportar',
  'toolbar.exportar.ayuda': 'Exportar el mapa: Obsidian, JSON o estados guardados',
  'toolbar.mas': 'Más',
  'toolbar.mas.ayuda': 'Más herramientas: inteligencia, núcleos de ideas y sistema',
  'toolbar.inteligencia': 'Inteligencia',
  'toolbar.nucleos': 'Núcleos de ideas',
  'toolbar.sistema': 'Sistema',
  'toolbar.perfil.ayuda': 'Autenticación y perfil de usuario',

  // --- lienzo: controles y HUD
  'lienzo.root': 'Root',
  'lienzo.root.ayuda': 'Crear nodo raíz',
  'lienzo.logic': 'Logic',
  'lienzo.logic.ayuda': 'Crear nodo de lógica / hipótesis',
  'lienzo.borrar': 'Borrar / Limpiar Lienzo',
  'lienzo.persistencia': 'Persistencia',
  'hud.agente': 'Cambios del agente',
  'hud.agente.ayuda': 'Propuestas del agente esperando aprobación',
  'hud.almacenamiento': 'Almacenamiento',
  'hud.persistente': 'Persistente',
  'hud.ultimoGuardado': 'Último guardado',
  'hud.nodosConexiones': 'Nodos / conexiones',
  'hud.vaultEnDisco': 'Vault en disco',
  'hud.paneles': 'Paneles',
  'hud.guardarAhora': 'Guardar progreso ahora',
  'busqueda.sinResultados': 'No se encontraron nodos coincidentes.',

  // --- paneles del sidebar
  'panel.memoria': 'Memoria del vault',
  'panel.memoria.ayuda': 'Buscar en todas tus notas y traer una al lienzo',
  'panel.conocimiento': 'Conocimiento',
  'panel.conocimiento.ayuda': 'Convertir texto en nodos propuestos y exportar el mapa',
  'panel.voz': 'Voz',
  'panel.investigacion': 'Investigación',
  'panel.pensar': 'Pensar',
  'panel.siguiente': 'Lo que sigue',
  'panel.evaluacion': 'Evaluación',
  'panel.jardin': 'Jardín del lienzo',
  'panel.orquestador': 'Orquestador',

  // --- aprendizaje continuo
  'hitl.titulo': 'Aprendizaje Continuo',
  'hitl.decisiones': 'Decisiones HITL',
  'hitl.aceptacion': 'Aceptación',
  'hitl.auto': 'Aprendizaje automático',
  'hitl.configurar': 'Configurar Aprendizaje',

  // --- acciones sobre el nodo
  'accion.unir': 'Unir Conexión',
  'accion.unir.ayuda': 'Crear conexión directa entre ambos nodos (Atajo: U)',
  'accion.hibridar': 'Hibridar IA',
  'accion.puentes': 'Puentes Ocultos',
  'accion.puentes.ayuda': 'Escanear puentes semánticos y relaciones ocultas',
  'accion.ramificar': 'Ramificar',
  'accion.deseleccionar': 'Deseleccionar (Esc)',
  'cocreacion.titulo': 'Co-creación IA',
  'cocreacion.hibridador': 'Hibridador IA',
  'cocreacion.sintesis': 'SÍNTESIS ESTRATÉGICA',
  'norte.titulo': 'Norte estratégico',

  // --- vista y modales
  'zoom.alejar': 'Alejar',
  'zoom.acercar': 'Acercar',
  'zoom.ajustar': 'Ajustar y centrar vista',
  'atajos.titulo': 'Atajos',
  'atajos.ayuda': 'Atajos de teclado y ayuda',
  'modal.cerrar': 'Cerrar (Enter)',
  'modal.cerrarCorto': 'Cerrar',

  // --- apariencia
  'apariencia.titulo': 'Apariencia del lienzo',
  'apariencia.colores': 'Colores',
  'apariencia.colores.ayuda': 'Colores del fondo y de las tarjetas',
  'apariencia.restaurar': 'Volver a los colores por defecto',
  'apariencia.colorPropio': 'Color propio:',
  'apariencia.colorPropio.ayuda': 'Elegir cualquier color de fondo',

  // --- panel de cambios del agente
  'agente.sinPendientes': 'No hay cambios pendientes',

  // --- voz
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
  'toolbar.descargaCorta': 'Brain dump',
  'toolbar.hibridar': 'AI Hybridizer',
  'toolbar.buscar': 'Search nodes... (Ctrl+F)',
  'toolbar.exportar': 'Export',
  'toolbar.exportar.ayuda': 'Export the map: Obsidian, JSON or saved states',
  'toolbar.mas': 'More',
  'toolbar.mas.ayuda': 'More tools: intelligence, idea cores and system',
  'toolbar.inteligencia': 'Intelligence',
  'toolbar.nucleos': 'Idea cores',
  'toolbar.sistema': 'System',
  'toolbar.perfil.ayuda': 'Authentication and user profile',

  'lienzo.root': 'Root',
  'lienzo.root.ayuda': 'Create a root node',
  'lienzo.logic': 'Logic',
  'lienzo.logic.ayuda': 'Create a logic / hypothesis node',
  'lienzo.borrar': 'Clear / Wipe Canvas',
  'lienzo.persistencia': 'Persistence',
  'hud.agente': 'Agent changes',
  'hud.agente.ayuda': 'Agent proposals waiting for approval',
  'hud.almacenamiento': 'Storage',
  'hud.persistente': 'Persistent',
  'hud.ultimoGuardado': 'Last saved',
  'hud.nodosConexiones': 'Nodes / connections',
  'hud.vaultEnDisco': 'Vault on disk',
  'hud.paneles': 'Panels',
  'hud.guardarAhora': 'Save progress now',
  'busqueda.sinResultados': 'No matching nodes found.',

  'panel.memoria': 'Vault memory',
  'panel.memoria.ayuda': 'Search all your notes and bring one onto the canvas',
  'panel.conocimiento': 'Knowledge',
  'panel.conocimiento.ayuda': 'Turn text into proposed nodes and export the map',
  'panel.voz': 'Voice',
  'panel.investigacion': 'Research',
  'panel.pensar': 'Think',
  'panel.siguiente': 'What is next',
  'panel.evaluacion': 'Evaluation',
  'panel.jardin': 'Canvas garden',
  'panel.orquestador': 'Orchestrator',

  'hitl.titulo': 'Continuous Learning',
  'hitl.decisiones': 'HITL decisions',
  'hitl.aceptacion': 'Acceptance',
  'hitl.auto': 'Automatic learning',
  'hitl.configurar': 'Configure learning',

  'accion.unir': 'Link nodes',
  'accion.unir.ayuda': 'Create a direct connection between both nodes (Shortcut: U)',
  'accion.hibridar': 'Hybridize with AI',
  'accion.puentes': 'Hidden bridges',
  'accion.puentes.ayuda': 'Scan for semantic bridges and hidden relations',
  'accion.ramificar': 'Branch',
  'accion.deseleccionar': 'Deselect (Esc)',
  'cocreacion.titulo': 'AI co-creation',
  'cocreacion.hibridador': 'AI Hybridizer',
  'cocreacion.sintesis': 'STRATEGIC SYNTHESIS',
  'norte.titulo': 'Strategic north star',

  'zoom.alejar': 'Zoom out',
  'zoom.acercar': 'Zoom in',
  'zoom.ajustar': 'Fit and center view',
  'atajos.titulo': 'Shortcuts',
  'atajos.ayuda': 'Keyboard shortcuts and help',
  'modal.cerrar': 'Close (Enter)',
  'modal.cerrarCorto': 'Close',

  'apariencia.titulo': 'Canvas appearance',
  'apariencia.colores': 'Colors',
  'apariencia.colores.ayuda': 'Background and card colors',
  'apariencia.restaurar': 'Back to default colors',
  'apariencia.colorPropio': 'Custom color:',
  'apariencia.colorPropio.ayuda': 'Pick any background color',

  'agente.sinPendientes': 'No pending changes',

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

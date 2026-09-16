/**
 * Aplica el catálogo i18n al camino crítico de la interfaz.
 *
 * Por qué un script y no ediciones a mano: son ~55 reemplazos en 5 archivos, y cada reemplazo tiene que
 * ser **exacto** (literal JSX o atributo). El script informa cuáles no encontró, así que un texto que se
 * movió no pasa desapercibido: se ve en la lista de "sin aplicar" en vez de quedar en español sin aviso.
 *
 * Uso:  node scripts/i18n-aplicar.mjs        (idempotente: si ya está aplicado, lo dice)
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..');

/** [archivo, texto en español (literal o como atributo), clave, tipo] */
const CAMBIOS = [
  // --- App.tsx: sidebar, HUD y controles del lienzo
  ['src/App.tsx', 'No se encontraron nodos coincidentes.', 'busqueda.sinResultados', 'jsx'],
  ['src/App.tsx', 'Crear nodo raíz', 'lienzo.root.ayuda', 'title'],
  ['src/App.tsx', 'Root', 'lienzo.root', 'jsx'],
  ['src/App.tsx', 'Crear nodo de lógica / hipótesis', 'lienzo.logic.ayuda', 'title'],
  ['src/App.tsx', 'Logic', 'lienzo.logic', 'jsx'],
  ['src/App.tsx', 'Borrar / Limpiar Lienzo', 'lienzo.borrar', 'jsx'],
  ['src/App.tsx', 'Persistencia', 'lienzo.persistencia', 'jsx'],
  ['src/App.tsx', 'Propuestas del agente esperando aprobación', 'hud.agente.ayuda', 'title'],
  ['src/App.tsx', 'Cambios del agente', 'hud.agente', 'jsx'],
  ['src/App.tsx', 'Almacenamiento', 'hud.almacenamiento', 'jsx'],
  ['src/App.tsx', 'Persistente', 'hud.persistente', 'jsx'],
  ['src/App.tsx', 'Último guardado', 'hud.ultimoGuardado', 'jsx'],
  ['src/App.tsx', 'Nodos / conexiones', 'hud.nodosConexiones', 'jsx'],
  ['src/App.tsx', 'Vault en disco', 'hud.vaultEnDisco', 'jsx'],
  ['src/App.tsx', 'Paneles', 'hud.paneles', 'jsx'],
  ['src/App.tsx', 'Buscar en todas tus notas y traer una al lienzo', 'panel.memoria.ayuda', 'title'],
  ['src/App.tsx', 'Memoria del vault', 'panel.memoria', 'jsx'],
  ['src/App.tsx', 'Convertir texto en nodos propuestos y exportar el mapa', 'panel.conocimiento.ayuda', 'title'],
  ['src/App.tsx', 'Conocimiento', 'panel.conocimiento', 'jsx'],
  ['src/App.tsx', 'Voz', 'panel.voz', 'jsx'],
  ['src/App.tsx', 'Investigación', 'panel.investigacion', 'jsx'],
  ['src/App.tsx', 'Pensar', 'panel.pensar', 'jsx'],
  ['src/App.tsx', 'Lo que sigue', 'panel.siguiente', 'jsx'],
  ['src/App.tsx', 'Evaluación', 'panel.evaluacion', 'jsx'],
  ['src/App.tsx', 'Jardín del lienzo', 'panel.jardin', 'jsx'],
  ['src/App.tsx', 'Orquestador', 'panel.orquestador', 'jsx'],
  ['src/App.tsx', 'Guardar progreso ahora', 'hud.guardarAhora', 'jsx'],
  ['src/App.tsx', 'Aprendizaje Continuo', 'hitl.titulo', 'jsx'],
  ['src/App.tsx', 'Decisiones HITL', 'hitl.decisiones', 'jsx'],
  ['src/App.tsx', 'Aceptación', 'hitl.aceptacion', 'jsx'],
  ['src/App.tsx', 'Aprendizaje automático', 'hitl.auto', 'jsx'],
  ['src/App.tsx', 'Configurar Aprendizaje', 'hitl.configurar', 'jsx'],
  ['src/App.tsx', 'SÍNTESIS ESTRATÉGICA', 'cocreacion.sintesis', 'jsx'],
  ['src/App.tsx', 'Alejar', 'zoom.alejar', 'title'],
  ['src/App.tsx', 'Acercar', 'zoom.acercar', 'title'],
  ['src/App.tsx', 'Ajustar y centrar vista', 'zoom.ajustar', 'title'],
  ['src/App.tsx', 'Atajos de teclado y ayuda', 'atajos.ayuda', 'title'],
  ['src/App.tsx', 'Atajos', 'atajos.titulo', 'jsx'],
  ['src/App.tsx', 'Cerrar (Enter)', 'modal.cerrar', 'title'],
  ['src/App.tsx', 'Norte estratégico', 'norte.titulo', 'jsx'],
  ['src/App.tsx', 'Crear conexión directa entre ambos nodos (Atajo: U)', 'accion.unir.ayuda', 'title'],
  ['src/App.tsx', 'Unir Conexión', 'accion.unir', 'jsx'],
  ['src/App.tsx', 'Hibridar IA', 'accion.hibridar', 'jsx'],
  ['src/App.tsx', 'Escanear puentes semánticos y relaciones ocultas', 'accion.puentes.ayuda', 'title'],
  ['src/App.tsx', 'Puentes Ocultos', 'accion.puentes', 'jsx'],
  ['src/App.tsx', 'Deseleccionar (Esc)', 'accion.deseleccionar', 'title'],
  ['src/App.tsx', 'Co-creación IA', 'cocreacion.titulo', 'jsx'],
  ['src/App.tsx', 'Hibridador IA', 'cocreacion.hibridador', 'jsx'],
  ['src/App.tsx', 'Ramificar', 'accion.ramificar', 'jsx'],

  // --- Toolbar.tsx (claves que ya existían en el catálogo y no se usaban)
  ['src/components/Toolbar.tsx', 'Buscar nodos... (Ctrl+F)', 'toolbar.buscar', 'placeholder'],
  ['src/components/Toolbar.tsx', 'Deshacer (Ctrl+Z)', 'toolbar.deshacer', 'title'],
  ['src/components/Toolbar.tsx', 'Rehacer (Ctrl+Y)', 'toolbar.rehacer', 'title'],
  ['src/components/Toolbar.tsx', 'Organizar', 'toolbar.organizar', 'jsx'],
  ['src/components/Toolbar.tsx', 'Descubrir sinergias y fusionar ideas seleccionadas con IA', 'toolbar.descubrir', 'title'],
  ['src/components/Toolbar.tsx', 'Hibridador IA', 'toolbar.hibridar', 'jsx'],
  ['src/components/Toolbar.tsx', 'Descarga', 'toolbar.descargaCorta', 'jsx'],
  ['src/components/Toolbar.tsx', 'Exportar el mapa: Obsidian, JSON o estados guardados', 'toolbar.exportar.ayuda', 'title'],
  ['src/components/Toolbar.tsx', 'Exportar', 'toolbar.exportar', 'jsx'],
  ['src/components/Toolbar.tsx', 'Más herramientas: inteligencia, núcleos de ideas y sistema', 'toolbar.mas.ayuda', 'title'],
  ['src/components/Toolbar.tsx', 'Más', 'toolbar.mas', 'jsx'],
  ['src/components/Toolbar.tsx', 'Inteligencia', 'toolbar.inteligencia', 'jsx'],
  ['src/components/Toolbar.tsx', 'Núcleos de ideas', 'toolbar.nucleos', 'jsx'],
  ['src/components/Toolbar.tsx', 'Sistema', 'toolbar.sistema', 'jsx'],
  ['src/components/Toolbar.tsx', 'Autenticación y perfil de usuario', 'toolbar.perfil.ayuda', 'title'],

  // --- AparienciaHud.tsx
  ['src/components/AparienciaHud.tsx', 'Colores del fondo y de las tarjetas', 'apariencia.colores.ayuda', 'title'],
  ['src/components/AparienciaHud.tsx', 'Colores', 'apariencia.colores', 'jsx'],
  ['src/components/AparienciaHud.tsx', 'Apariencia del lienzo', 'apariencia.titulo', 'jsx'],
  ['src/components/AparienciaHud.tsx', 'Volver a los colores por defecto', 'apariencia.restaurar', 'title'],
  ['src/components/AparienciaHud.tsx', 'Color propio:', 'apariencia.colorPropio', 'jsx'],
  ['src/components/AparienciaHud.tsx', 'Elegir cualquier color de fondo', 'apariencia.colorPropio.ayuda', 'title'],

  // --- AgentChangesPanel.tsx
  ['src/components/AgentChangesPanel.tsx', 'No hay cambios pendientes', 'agente.sinPendientes', 'jsx'],
  ['src/components/AgentChangesPanel.tsx', 'Cerrar', 'modal.cerrarCorto', 'title'],
];

/** Texto a reemplazar -> patrón. En JSX el literal puede venir con saltos de línea alrededor. */
function patron(texto, tipo) {
  const esc = texto.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return tipo === 'jsx'
    ? new RegExp(`>\\s*${esc}\\s*<`, 'g')
    : new RegExp(`${tipo}="${esc}"`, 'g');
}

function reemplazo(clave, tipo) {
  return tipo === 'jsx' ? `>{t('${clave}')}<` : `${tipo}={t('${clave}')}`;
}

const porArchivo = new Map();
for (const [archivo, texto, clave, tipo] of CAMBIOS) {
  if (!porArchivo.has(archivo)) porArchivo.set(archivo, []);
  porArchivo.get(archivo).push([texto, clave, tipo]);
}

let aplicados = 0;
const sinAplicar = [];
const clavesPorArchivo = new Map();
for (const [archivo, lista] of porArchivo) {
  const p = join(RAIZ, archivo);
  let src = readFileSync(p, 'utf-8');
  const claves = [];
  for (const [texto, clave, tipo] of lista) {
    const re = patron(texto, tipo);
    if (!re.test(src)) {
      // Puede estar ya aplicado (idempotencia) o haberse movido.
      if (src.includes(`t('${clave}')`)) continue;
      sinAplicar.push(`${archivo} :: ${texto}`);
      continue;
    }
    re.lastIndex = 0;
    src = src.replace(re, reemplazo(clave, tipo));
    claves.push(clave);
    aplicados++;
  }
  writeFileSync(p, src, 'utf-8');
  if (claves.length) clavesPorArchivo.set(archivo, claves);
}

console.log(`reemplazos aplicados: ${aplicados}`);
if (sinAplicar.length) {
  console.log(`\nSIN APLICAR (${sinAplicar.length}) — revisar si se movieron:`);
  for (const s of sinAplicar) console.log('  ·', s);
} else {
  console.log('todos los textos del mapa se encontraron en su archivo');
}
console.log('\nclaves usadas por archivo:');
for (const [a, c] of clavesPorArchivo) console.log(`  ${a}: ${c.length}`);

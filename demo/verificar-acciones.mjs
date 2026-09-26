/**
 * Árbitro del demo: comprueba que `/api/ai/action` devuelve, para CADA acción que ofrece la interfaz,
 * exactamente la clave que el front lee (el bug era justamente ése: el mock respondía siempre el plan de
 * voz y las acciones quedaban mudas, sin error y sin nodo).
 *
 * Corre sin red y sin claves: `node demo/verificar-acciones.mjs`
 */
import { handle } from './lib/mockApi.mjs';

const NODO = {
  id: 'n-1',
  title: 'Copiloto espacial operativo',
  description: 'Asistente que vive en el lienzo y propone el próximo movimiento.',
  category: 'NÚCLEO',
  tags: ['Copiloto', 'Voz'],
};
const SELECCION = [
  NODO,
  { id: 'n-2', title: 'Modelo local cuantizado', description: 'Cerebro residente sin nube en la placa.', tags: ['Local'] },
];

const NODOS_LIENZO = [
  { id: 'a', data: { title: 'Motor RAG de grafos', category: 'ARQUITECTURA', description: 'Recupera contexto del grafo y de la bóveda vectorial.' } },
  { id: 'b', data: { title: 'Voz de ida y vuelta', category: 'ARQUITECTURA', description: 'Dictado en vivo y respuesta hablada con el motor local.' } },
  { id: 'c', data: { title: 'Búsqueda web con grounding', category: 'INVESTIGACIÓN', description: 'Investiga por fases y cita las fuentes.' } },
];
const ARISTAS = [{ source: 'a', target: 'b' }];

const casos = [
  ['branch (Ramificar)', { type: 'branch', nodeData: NODO }, (r) => Array.isArray(r.variations) && r.variations.length === 3 && r.variations.every((v) => v.title && v.description)],
  ['explore (Explorar)', { type: 'explore', nodeData: NODO }, (r) => Array.isArray(r.variations) && r.variations.length === 3],
  ['critique (Abogado del diablo)', { type: 'critique', nodeData: NODO }, (r) => Array.isArray(r.variations) && r.variations.length === 3],
  ['socratic (Preguntas)', { type: 'socratic', nodeData: NODO }, (r) => Array.isArray(r.variations) && r.variations.length === 3],
  ['hybrid (Hibridar IA)', { type: 'hybrid', selectedNodes: SELECCION }, (r) => !!(r.hybrid && r.hybrid.title && r.hybrid.description && Array.isArray(r.hybrid.tags))],
  ['condensar (Macro)', { type: 'condensar', selectedNodes: SELECCION, objetivo: 'cerebro local' }, (r) => !!(r.condensar && r.condensar.title && r.condensar.description)],
  ['synthesize (Síntesis del mapa)', { type: 'synthesize', nodes: NODOS_LIENZO }, (r) => !!(r.synthesis && typeof r.synthesis.summary === 'string' && Array.isArray(r.synthesis.pillars) && Array.isArray(r.synthesis.actionItems) && Array.isArray(r.synthesis.keyOpportunities))],
  ['braindump (Descarga mental)', { type: 'braindump', rawText: 'Cerebro local\nmodelo cuantizado\nbóveda de notas' }, (r) => !!(r.structure && r.structure.root && r.structure.root.title && Array.isArray(r.structure.nodes) && r.structure.nodes.length === 2)],
  ['find_bridges (Puentes)', { type: 'find_bridges', nodes: NODOS_LIENZO, edges: ARISTAS }, (r) => Array.isArray(r.bridges) && r.bridges.every((b) => b.sourceId && b.targetId && b.label && b.rationale)],
  ['refresh_templates (Núcleos frescos)', { type: 'refresh_templates' }, (r) => Array.isArray(r.templates)],
  ['voz (plan de dictado)', { type: 'voz', texto: 'limpiá el lienzo y dejá sólo lo que se conecta con el copiloto espacial' }, (r) => !!(r.voz && Array.isArray(r.voz.comandos))],
];

let fallos = 0;
for (const [nombre, body, ok] of casos) {
  // eslint-disable-next-line no-await-in-loop
  const salida = await handle({ method: 'POST', ruta: '/api/ai/action', query: {}, body, ip: 'test' });
  const r = salida.json || {};
  const bien = salida.status === 200 && r.success === true && ok(r);
  if (!bien) fallos += 1;
  console.log(`${bien ? 'ok  ' : 'FALLA'}  ${nombre.padEnd(34)} status=${salida.status} claves=${Object.keys(r).join(',')}`);
}

// El TTS del demo tiene que devolver un WAV, no JSON: si no, el panel grita en cada turno de voz.
const tts = await handle({ method: 'POST', ruta: '/api/voz/decir', query: {}, body: { texto: 'hola' }, ip: 'test' });
const wavOk = tts.status === 200 && Buffer.isBuffer(tts.body) && tts.body.slice(0, 4).toString() === 'RIFF' && tts.body.length > 1000;
if (!wavOk) fallos += 1;
console.log(`${wavOk ? 'ok  ' : 'FALLA'}  /api/voz/decir (WAV de silencio)     bytes=${tts.body?.length ?? 0}`);

// El uso tiene que venir con forma completa: sin `proveedor`/`tokens` el demo no puede decir qué motor
// contestó ni cuánto costó (y el costo real sólo se informa si hay tarifa declarada por entorno).
const conUso = await handle({ method: 'POST', ruta: '/api/ai/action', query: {}, body: { type: 'hybrid', selectedNodes: SELECCION }, ip: 'test' });
const u = conUso.json?.uso || {};
const usoOk = typeof u.proveedor === 'string' && typeof u.modelo === 'string' && !!u.tokens
  && typeof u.tokens.prompt === 'number' && typeof u.tokens.completion === 'number' && typeof u.ms === 'number';
if (!usoOk) fallos += 1;
console.log(`${usoOk ? 'ok  ' : 'FALLA'}  uso (proveedor/modelo/tokens/ms)     ${JSON.stringify(u)}`);

// El catálogo de motores del demo no puede listar engines que no existen (los Ollama de la máquina del
// autor): el visitante cree que corre local y eso desconfía de todo el demo.
const cat = (await handle({ method: 'GET', ruta: '/api/ai/motores', query: {}, body: {}, ip: 'test' })).json || {};
const catOk = Array.isArray(cat.motores) && cat.motores.length === 1
  && cat.motores.every((m) => m.id === cat.efectivo && !/ollama/i.test(String(m.proveedor)))
  && cat.motores[0].disponible === true;
if (!catOk) fallos += 1;
console.log(`${catOk ? 'ok  ' : 'FALLA'}  /api/ai/motores (un solo motor real)  ${JSON.stringify(cat.motores?.[0]?.etiqueta)}`);

console.log(fallos === 0 ? '\nTODO EN VERDE' : `\n${fallos} FALLA(S)`);
process.exit(fallos === 0 ? 0 : 1);

/**
 * Núcleo del API del DEMO WEB de NodeFlow.
 *
 * Por qué existe: el jurado del hackathon necesita una URL pública interactiva y la app real es un
 * binario Tauri. Esto sirve la MISMA interfaz (el bundle de `src/`) contra un backend simulado que
 * reusa respuestas REALES cosechadas del backend Rust (`demo/fixtures/*.json`), y mantiene en memoria
 * lo que el demo necesita que cambie: el lienzo, la cola de propuestas y el plan de voz.
 *
 * Lo que sí es en vivo acá:
 *   · `/api/voz/jwt`  → token temporal de AssemblyAI (la clave vive en el servidor, nunca en el bundle).
 *   · `/api/ai/action` → el plan. Con `DEMO_MOTOR_*` apunta a un motor real; sin eso, replay de planes
 *     reales grabados (determinista: un demo que no puede fallar en vivo).
 *
 * Lo que NO es el producto: la persistencia es en memoria (cada instancia arranca del lienzo semilla).
 */
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const AQUI = dirname(fileURLToPath(import.meta.url));
const FIXTURES = join(AQUI, '..', 'fixtures');

// ---------------------------------------------------------------- fixtures

function cargarFixtures() {
  const out = new Map();
  for (const f of readdirSync(FIXTURES)) {
    if (!f.endsWith('.json')) continue;
    const ruta = '/' + f.replace(/\.json$/, '').replace(/__/g, '/');
    try {
      out.set(ruta, JSON.parse(readFileSync(join(FIXTURES, f), 'utf-8')));
    } catch {
      /* una fixture rota no puede tumbar el demo */
    }
  }
  return out;
}

const FIXTURAS = cargarFixtures();
const PLANES = (() => {
  const p = join(FIXTURES, 'planes-demo.json');
  return existsSync(p) ? JSON.parse(readFileSync(p, 'utf-8')) : { planes: [] };
})();

// ---------------------------------------------------------------- estado en memoria

/** Estado del lienzo del demo: arranca del lienzo real y vive mientras la instancia esté caliente. */
function estadoInicial() {
  // La respuesta real del backend es un sobre: { state: { nodes, edges, … }, revision, info, metricas… }
  const s = FIXTURAS.get('/api/graph/state') || {};
  const st = s.state || s;
  return { nodes: JSON.parse(JSON.stringify(st.nodes || [])), edges: JSON.parse(JSON.stringify(st.edges || [])) };
}
let LIENZO = estadoInicial();
let PROPUESTAS = [];
let REV = 1;
let ID = 0;
const nuevoId = (p = 'demo') => `${p}-${Date.now().toString(36)}-${(ID++).toString(36)}`;

/** El sobre completo que espera el front, con el lienzo vivo adentro. */
function sobreEstado() {
  const base = FIXTURAS.get('/api/graph/state') || {};
  const st = JSON.parse(JSON.stringify(base.state || {}));
  st.nodes = LIENZO.nodes;
  st.edges = LIENZO.edges;
  return {
    ...base,
    state: st,
    revision: REV,
    changed: false,
    pendientes: PROPUESTAS.length,
    metricas: { ...(base.metricas || {}), nodos: LIENZO.nodes.length, aristas: LIENZO.edges.length, huerfanos: 0, aristas_colgadas: 0 },
  };
}

function nodoNuevo({ title, description = '', category = 'IDEA', maturity = 1, parent = null, link_label = null, x, y }) {
  return {
    id: nuevoId('n'),
    type: 'ideaNode',
    position: { x: x ?? 120 + (LIENZO.nodes.length % 7) * 210, y: y ?? 140 + Math.floor(LIENZO.nodes.length / 7) * 150 },
    data: { title, description, category, maturity, tags: ['demo'], aiOrigin: true, ...(parent ? { parent, linkLabel: link_label } : {}) },
  };
}

// ---------------------------------------------------------------- planes

/** Normaliza el pedido del usuario para buscarlo en los planes grabados. */
const norm = (s) => String(s || '').toLowerCase().normalize('NFD').replace(/[\u0300-\u036f]/g, '').replace(/[^a-z0-9 ]/g, ' ').replace(/\s+/g, ' ').trim();

function planGrabado(texto) {
  const t = norm(texto);
  let mejor = null;
  for (const p of PLANES.planes || []) {
    for (const frase of p.frases || []) {
      const f = norm(frase);
      if (t === f) return { plan: p, exacto: true };
      if (!mejor && f && (t.includes(f) || f.includes(t))) mejor = p;
    }
  }
  return mejor ? { plan: mejor, exacto: false } : null;
}

async function motorReal(texto) {
  const url = process.env.DEMO_MOTOR_URL;
  const clave = process.env.DEMO_MOTOR_KEY;
  const modelo = process.env.DEMO_MOTOR_MODELO;
  if (!url || !clave || !modelo) return null;
  const spec = existsSync(join(AQUI, 'spec-voz.txt')) ? readFileSync(join(AQUI, 'spec-voz.txt'), 'utf-8') : '';
  const t0 = Date.now();
  const r = await fetch(url, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${clave}` },
    body: JSON.stringify({
      model: modelo,
      messages: [
        { role: 'system', content: spec },
        { role: 'user', content: `Pedido del usuario: ${texto}\nRespondé sólo con JSON.` },
      ],
      response_format: { type: 'json_object' },
      max_tokens: 900,
    }),
  });
  if (!r.ok) throw new Error(`motor ${r.status}`);
  const d = await r.json();
  const txt = d.choices?.[0]?.message?.content || '';
  return { plan: JSON.parse(txt), ms: Date.now() - t0, modelo };
}

async function planDeVoz(texto) {
  try {
    const real = await motorReal(texto);
    if (real?.plan) return { plan: real.plan, motor: `demo@${real.modelo}`, ms: real.ms, vivo: true };
  } catch (e) {
    /* si el motor real falla, se cae al plan grabado: el demo nunca se queda sin respuesta */
  }
  const g = planGrabado(texto);
  if (g) return { plan: g.plan.plan, motor: 'demo@grabado', ms: 40, vivo: false, exacto: g.exacto };
  return {
    plan: { comandos: [], intencion: 'demo', respuesta: 'En el demo online el planificador corre con motores grabados: probá una de las frases sugeridas.' },
    motor: 'demo@sugerencias',
    ms: 10,
    vivo: false,
  };
}

// ---------------------------------------------------------------- token de AssemblyAI

async function tokenAssemblyAI() {
  const clave = process.env.ASSEMBLYAI_API_KEY || (existsSync(join(AQUI, '..', 'clave-assemblyai.txt'))
    ? readFileSync(join(AQUI, '..', 'clave-assemblyai.txt'), 'utf-8').trim()
    : '');
  if (!clave) return { status: 503, json: { success: false, error: 'El demo no tiene clave de AssemblyAI configurada.' } };
  const r = await fetch('https://streaming.assemblyai.com/v3/token?expires_in_seconds=600', {
    headers: { authorization: clave },
  });
  if (!r.ok) return { status: 502, json: { success: false, error: `AssemblyAI respondió ${r.status}` } };
  const d = await r.json();
  // La MISMA forma que devuelve el backend real (`/api/voz/jwt`): si falta `protocolo` el panel elige
  // el cliente equivocado y `jwt` es el campo que termina en el WebSocket.
  return {
    status: 200,
    json: {
      success: true,
      token: d.token,
      jwt: d.token,
      expira_en_s: d.expires_in_seconds ?? 600,
      url: 'wss://streaming.assemblyai.com/v3/ws',
      proveedor: 'assemblyai',
      etiqueta: 'AssemblyAI Universal-Streaming',
      protocolo: 'assemblyai-v3',
      modelo: 'universal-3-5-pro',
      idioma: 'es',
      codec: 'pcm_s16le 16000 Hz',
    },
  };
}

// ---------------------------------------------------------------- router

const json = (status, body) => ({ status, json: body });

export async function handle({ method, ruta, query, body }) {
  const m = (method || 'GET').toUpperCase();
  const t0 = Date.now();

  // --- salud y estado: en vivo, no fixtures
  if (ruta === '/api/health') return json(200, { ok: true, status: 'ok', demo: true, version: PLANES.version || '0.3.5' });
  if (ruta === '/api/graph/state') {
    if (m === 'GET') return json(200, sobreEstado());
    const st = (typeof body === 'object' && body) || {};
    const entrante = st.state && typeof st.state === 'object' ? st.state : st;
    if (Array.isArray(entrante.nodes)) {
      LIENZO = { nodes: entrante.nodes, edges: Array.isArray(entrante.edges) ? entrante.edges : LIENZO.edges };
    }
    REV += 1;
    // El front decide con `res.ok` (VaultSaveResult), no con `success`: sin `ok` el demo mostraba
    // «Vault: error al escribir» aunque el lienzo estuviera intacto.
    return json(200, { ok: true, success: true, revision: REV, nodos: LIENZO.nodes.length, aristas: LIENZO.edges.length, rev: REV });
  }

  // --- voz
  if (ruta === '/api/voz/jwt') return tokenAssemblyAI();
  if (ruta === '/api/voz/estado') return json(200, { ...(FIXTURAS.get('/api/voz/estado') || {}), configurada: true, proveedor: 'assemblyai', idioma: 'es', aviso: null });

  // --- el plan
  if (ruta === '/api/ai/action') {
    const texto = body?.texto || body?.prompt || body?.rawText || '';
    const { plan, motor, ms, vivo, exacto } = await planDeVoz(texto);
    return json(200, {
      success: true,
      voz: plan,
      source: motor,
      cadena: ['demo'],
      modo: 'demo',
      ms,
      demo: { vivo: !!vivo, exacto: !!exacto, aviso: vivo ? 'Plan generado en vivo por un motor real.' : 'Plan grabado de una corrida real del motor en la app.' },
      uso: { proveedor: motor, total_tokens: 0, cache_hit: 0 },
    });
  }
  if (ruta === '/api/ai/motores') return json(200, FIXTURAS.get('/api/ai/motores') || { success: true, motores: [], seleccionado: null });
  if (ruta === '/api/ai/motor') return json(200, { ok: true, seleccionado: body?.id || null, demo: true });
  if (ruta === '/api/ai/cache') return json(200, FIXTURAS.get('/api/ai/cache') || { ok: true });
  if (ruta === '/api/ai/evaluar' || ruta === '/api/ai/delegar' || ruta === '/api/ai/investigar') {
    return json(200, { ...(FIXTURAS.get(ruta) || {}), corriendo: false, demo: true });
  }

  // --- propuestas del agente: la cola funciona de verdad dentro del demo
  if (ruta === '/api/agent/pending') return json(200, { success: true, pendientes: PROPUESTAS });
  if (ruta === '/api/graph/node') {
    const b = body || {};
    const n = nodoNuevo(b);
    LIENZO.nodes.push(n);
    if (b.parent) LIENZO.edges.push({ id: nuevoId('e'), source: b.parent, target: n.id, label: b.link_label || '' });
    REV += 1;
    return json(200, { success: true, nodo: n, nodos: LIENZO.nodes.length, aristas: LIENZO.edges.length, rev: REV });
  }
  if (ruta === '/api/agent/approve') {
    const aplicadas = PROPUESTAS.length;
    PROPUESTAS = [];
    REV += 1;
    return json(200, { success: true, aplicadas, nodos: LIENZO.nodes.length, aristas: LIENZO.edges.length });
  }
  if (ruta === '/api/agent/reject') {
    PROPUESTAS = [];
    return json(200, { success: true, rechazadas: true });
  }

  // --- todo lo demás: la última respuesta real conocida, o un OK vacío.
  if (FIXTURAS.has(ruta)) return json(200, FIXTURAS.get(ruta));

  // Rutas con parámetros (`/api/vault/note?ruta=…`): prefijo.
  for (const [r, valor] of FIXTURAS) {
    if (ruta.startsWith(r + '/')) return json(200, valor);
  }
  return json(200, { success: true, demo: true, sin_datos: true, ruta, metodo: m, ms: Date.now() - t0 });
}

export const meta = { fixtures: FIXTURAS.size, planes: (PLANES.planes || []).length, version: PLANES.version || '0.3.5' };

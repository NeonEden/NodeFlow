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
/** Idioma de la interfaz, tal como lo pide el switch (el backend real también es fuente de verdad). */
let IDIOMA = (FIXTURAS.get('/api/idioma') || {}).idioma === 'en' ? 'en' : 'es';
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

// ---------------------------------------------------------------- motor en vivo (con guardrails)

/**
 * Límite por IP: el motor en vivo cuesta dinero real del autor, así que el demo acepta un tope de
 * pedidos por minuto por IP y, pasado el tope, responde con los planes grabados en vez de fallar.
 */
const LIMITE = { ventanaMs: 60_000, tope: Number(process.env.DEMO_TOPE_POR_IP || 8) };
const USO_POR_IP = new Map();

function permitido(ip) {
  const ahora = Date.now();
  const marcas = (USO_POR_IP.get(ip) || []).filter((t) => ahora - t < LIMITE.ventanaMs);
  if (marcas.length >= LIMITE.tope) {
    USO_POR_IP.set(ip, marcas);
    return false;
  }
  marcas.push(ahora);
  USO_POR_IP.set(ip, marcas);
  return true;
}

/** Digest del lienzo para el prompt: el motor en vivo tiene que poder citar ids REALES. */
function contextoLienzo(tope = 60) {
  const ns = LIENZO.nodes.slice(0, tope).map((n) => ({
    id: n.id,
    titulo: n.data?.title ?? '',
    categoria: n.data?.category ?? '',
  }));
  return ns.map((n) => `- ${n.id} · ${n.titulo} [${n.categoria}]`).join('\n');
}

/** ¿Hay motor en vivo configurado? La clave puede venir del entorno (Vercel) o de un archivo local. */
function claveMotor() {
  if (process.env.DEMO_MOTOR_KEY) return process.env.DEMO_MOTOR_KEY;
  const f = join(AQUI, '..', 'clave-motor.txt');
  return existsSync(f) ? readFileSync(f, 'utf-8').trim() : '';
}
const hayMotorEnVivo = () => !!(process.env.DEMO_MOTOR_URL && process.env.DEMO_MOTOR_MODELO && claveMotor());

async function motorReal(texto) {
  const url = process.env.DEMO_MOTOR_URL;
  const modelo = process.env.DEMO_MOTOR_MODELO;
  // La clave NUNCA va al repo: en Vercel es una variable de entorno; en local, un archivo gitignoreado.
  const clave = claveMotor();
  if (!url || !clave || !modelo) return null;
  const spec = existsSync(join(AQUI, 'spec-voz.txt')) ? readFileSync(join(AQUI, 'spec-voz.txt'), 'utf-8') : '';
  // Tope de salida propio: una consulta maliciosa no puede drenar créditos.
  const maxTokens = Number(process.env.DEMO_MOTOR_MAX_TOKENS || 700);
  const esperaMs = Number(process.env.DEMO_MOTOR_TIMEOUT_MS || 20000);
  const t0 = Date.now();

  const pedir = async (mensajes, max) => {
    const ctl = new AbortController();
    const reloj = setTimeout(() => ctl.abort(), esperaMs);
    try {
      const r = await fetch(url, {
        method: 'POST',
        signal: ctl.signal,
        headers: { 'content-type': 'application/json', authorization: `Bearer ${clave}` },
        body: JSON.stringify({ model: modelo, messages: mensajes, response_format: { type: 'json_object' }, max_tokens: max }),
      });
      if (!r.ok) throw new Error(`motor ${r.status}: ${(await r.text()).slice(0, 120)}`);
      const d = await r.json();
      return { texto: d.choices?.[0]?.message?.content || '', requestId: d.request_id || d.id || null };
    } finally {
      clearTimeout(reloj);
    }
  };

  const base = [
    { role: 'system', content: spec },
    {
      role: 'user',
      content:
        `Nodos del lienzo (usá SOLO estos ids):\n${contextoLienzo()}\n\n` +
        `Pedido del usuario: ${texto}\nRespondé sólo con JSON.`,
    },
  ];

  let { texto: txt, requestId } = await pedir(base, maxTokens);
  let plan = null;
  try {
    plan = JSON.parse(txt);
  } catch {
    plan = null;
  }
  // Reintento correctivo: si devolvió el ESQUEMA ({"type":"OBJECT","properties":…}) o cualquier cosa sin
  // `comandos`, se lo dice el código una vez — el modelo propone, el código verifica. Medido: con el
  // esquema crudo en el prompt pasaba seguido; una segunda pasada lo resuelve.
  if (!plan || !Array.isArray(plan.comandos)) {
    const correccion = await pedir(
      [
        ...base,
        { role: 'assistant', content: txt.slice(0, 400) },
        {
          role: 'user',
          content:
            'Eso no es lo que pedí: devolviste el esquema o un objeto sin `comandos`. Devolvé AHORA los datos ' +
            'del plan (intencion, respuesta, motivo y comandos con los ids reales del lienzo), sólo JSON.',
        },
      ],
      maxTokens
    );
    try {
      const p2 = JSON.parse(correccion.texto);
      if (Array.isArray(p2.comandos)) {
        plan = p2;
        requestId = correccion.requestId || requestId;
      }
    } catch {
      /* sigue sin servir: lo decide el que llama */
    }
  }
  if (!plan || !Array.isArray(plan.comandos)) throw new Error('el motor no devolvió comandos');
  return { plan, ms: Date.now() - t0, modelo, requestId };
}

async function planDeVoz(texto, ip = 'anon') {
  const motorConfigurado = hayMotorEnVivo();
  let limitado = false;
  if (motorConfigurado && permitido(ip)) {
    try {
      const real = await motorReal(texto);
      if (real?.plan) {
        return { plan: real.plan, motor: `demo@${real.modelo}`, ms: real.ms, vivo: true };
      }
    } catch (e) {
      // Fallback silencioso: si el motor vivo falla o tarda, el demo sigue respondiendo con lo grabado.
      console.warn('demo: motor en vivo falló, cae a planes grabados —', String(e?.message || e));
    }
  } else if (motorConfigurado) {
    limitado = true;
  }
  const g = planGrabado(texto);
  if (g) return { plan: g.plan.plan, motor: 'demo@grabado', ms: 40, vivo: false, exacto: g.exacto, limitado };
  return {
    plan: { comandos: [], intencion: 'demo', respuesta: 'En el demo online el planificador corre con motores grabados: probá una de las frases sugeridas.' },
    motor: 'demo@sugerencias',
    ms: 10,
    vivo: false,
    limitado,
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
      modelo: 'u3-rt-pro',
      idioma: 'es',
      codec: 'pcm_s16le 16000 Hz',
    },
  };
}

// ---------------------------------------------------------------- router

const json = (status, body) => ({ status, json: body });

export async function handle({ method, ruta, query, body, ip = 'anon' }) {
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

  // --- idioma: el switch de la interfaz tiene que pegarse también en el demo
  if (ruta === '/api/idioma') {
    if (m === 'POST' && typeof body?.idioma === 'string') IDIOMA = body.idioma === 'en' ? 'en' : 'es';
    return json(200, { success: true, idioma: IDIOMA });
  }

  // --- voz
  if (ruta === '/api/voz/jwt') return tokenAssemblyAI();
  if (ruta === '/api/voz/estado') return json(200, { ...(FIXTURAS.get('/api/voz/estado') || {}), configurada: true, proveedor: 'assemblyai', idioma: 'es', aviso: null });

  // --- el plan
  if (ruta === '/api/ai/action') {
    const texto = body?.texto || body?.prompt || body?.rawText || '';
    const { plan, motor, ms, vivo, exacto, limitado } = await planDeVoz(texto, ip);
    return json(200, {
      success: true,
      voz: plan,
      source: motor,
      cadena: ['demo'],
      modo: 'demo',
      ms,
      demo: {
        vivo: !!vivo,
        exacto: !!exacto,
        limitado: !!limitado,
        aviso: vivo
          ? 'Plan generado en vivo por un motor real.'
          : limitado
            ? 'Límite por IP alcanzado: se responde con planes grabados de corridas reales.'
            : 'Plan grabado de una corrida real del motor en la app.',
      },
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

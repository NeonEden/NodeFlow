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
      return {
        texto: d.choices?.[0]?.message?.content || '',
        requestId: d.request_id || d.id || null,
        // Los tokens que informa el proveedor: sin esto el demo no puede decir cuánto costó la llamada.
        tokens: d.usage ? { prompt: d.usage.prompt_tokens ?? 0, completion: d.usage.completion_tokens ?? 0 } : null,
      };
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

  let { texto: txt, requestId, tokens } = await pedir(base, maxTokens);
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
        // El reintento correctivo también se paga: se suman los tokens de las dos pasadas.
        if (correccion.tokens) {
          tokens = { prompt: (tokens?.prompt ?? 0) + correccion.tokens.prompt, completion: (tokens?.completion ?? 0) + correccion.tokens.completion };
        }
      }
    } catch {
      /* sigue sin servir: lo decide el que llama */
    }
  }
  if (!plan || !Array.isArray(plan.comandos)) throw new Error('el motor no devolvió comandos');
  return { plan, ms: Date.now() - t0, modelo, requestId, tokens };
}

async function planDeVoz(texto, ip = 'anon') {
  const motorConfigurado = hayMotorEnVivo();
  let limitado = false;
  if (motorConfigurado && permitido(ip)) {
    try {
      const real = await motorReal(texto);
      if (real?.plan) {
        return { plan: real.plan, motor: `demo@${real.modelo}`, ms: real.ms, vivo: true, tokens: real.tokens };
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

// ---------------------------------------------------------------- acciones del lienzo (deterministas)
//
// Por qué existen: el demo tiene que responder SIEMPRE — sin claves, sin nube y sin que el visitante
// configure nada. Cada acción deriva su respuesta del contenido real que manda el front (títulos,
// descripciones, categorías, aristas), así el resultado se siente hecho a medida del lienzo aunque
// no haya ningún modelo detrás. Con `DEMO_MOTOR_*` configurado el motor en vivo tiene prioridad y
// esto queda como red: el modelo propone, el código valida (ADR 0003).

/** Hash estable: la misma entrada da la misma salida. Un demo no puede ser aleatorio. */
function semilla(texto) {
  let h = 2166136261;
  const t = String(texto || '');
  for (let i = 0; i < t.length; i += 1) {
    h ^= t.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return Math.abs(h);
}

const limpio = (s, n = 140) => {
  const t = String(s ?? '').replace(/\s+/g, ' ').trim();
  return t.length > n ? `${t.slice(0, n - 1)}…` : t;
};

/** Toma n moldes distintos, rotando por semilla: el mismo nodo siempre recibe el mismo trío. */
function elegir(lista, s, n) {
  const pool = lista.slice();
  const out = [];
  while (out.length < n && pool.length) out.push(pool.splice(s % pool.length, 1)[0]);
  return out;
}

/** Moldes por acción. Son la materia prima: el título del nodo real entra en cada molde. */
const MOLDES = {
  branch: [
    (t) => ({
      title: `Alcance mínimo de ${t}`,
      description: `Reducir «${t}» a la versión que se puede probar en una tarde: un caso de uso, un usuario, una métrica. Si no sobrevive a eso, el resto del diseño sobra.`,
      category: 'VARIACIÓN',
      tags: ['MVP', 'Alcance'],
    }),
    (t) => ({
      title: `${t} como plataforma`,
      description: `Dar vuelta el eje: en vez de que «${t}» sea la función, que sea el sustrato donde otras funciones se enchufan. Cambia el modelo de negocio y el orden de construcción.`,
      category: 'VARIACIÓN',
      tags: ['Plataforma', 'Escala'],
    }),
    (t) => ({
      title: `Contra-hipótesis de ${t}`,
      description: `Escribir el caso donde «${t}» es innecesario: qué tendría que ser verdad del usuario o del entorno para que esta línea no valga la pena. Es la rama que ahorra meses.`,
      category: 'VARIACIÓN',
      tags: ['Contra-hipótesis', 'Riesgo'],
    }),
  ],
  explore: [
    (t) => ({
      title: `Dimensión técnica de ${t}`,
      description: `Qué necesita «${t}» para funcionar de verdad: dependencias, latencia, costos por llamada, qué parte corre local y qué parte no puede.`,
      category: 'ÁMBITO',
      tags: ['Técnica', 'Infraestructura'],
    }),
    (t) => ({
      title: `Dimensión de negocio de ${t}`,
      description: `Quién paga por «${t}», cuánto y contra qué alternativa. Si no hay una respuesta en una frase, la idea todavía no está lista para construir.`,
      category: 'ÁMBITO',
      tags: ['Negocio', 'Modelo'],
    }),
    (t) => ({
      title: `Dimensión de usuario de ${t}`,
      description: `Cómo se entera el usuario de «${t}», qué tiene que aprender y en qué momento lo abandona. El punto de abandono es el verdadero requisito.`,
      category: 'ÁMBITO',
      tags: ['UX', 'Adopción'],
    }),
    (t) => ({
      title: `Dimensión regulatoria y de datos de ${t}`,
      description: `Qué datos toca «${t}», dónde viven y qué se puede publicar. Definir esto después de construir es la forma más caras de descubrir un bloqueo.`,
      category: 'ÁMBITO',
      tags: ['Datos', 'Cumplimiento'],
    }),
  ],
  critique: [
    (t) => ({
      title: `Riesgo de adopción de ${t}`,
      description: `El supuesto más frágil de «${t}» no es técnico: es que alguien cambie un hábito por esto. ¿Qué se rompe en su rutina actual y quién se lo pide?`,
      category: 'CRÍTICA Y RIESGO',
      tags: ['Riesgo', 'Adopción'],
    }),
    (t) => ({
      title: `Deuda técnica de ${t}`,
      description: `Qué parte de «${t}» se va a tener que reescribir si funciona: formato de datos, contratos entre módulos y todo lo que hoy está resuelto a mano.`,
      category: 'CRÍTICA Y RIESGO',
      tags: ['Deuda', 'Arquitectura'],
    }),
    (t) => ({
      title: `Supuesto sin validar en ${t}`,
      description: `Listar en una hoja lo que «${t}» da por cierto y marcar qué está medido y qué está imaginado. Lo imaginado es donde se pierde el tiempo.`,
      category: 'CRÍTICA Y RIESGO',
      tags: ['Supuestos', 'Evidencia'],
    }),
  ],
  socratic: [
    (t) => ({
      title: `¿Qué tendría que ser cierto para que ${t} funcione?`,
      description: `Enumerar las condiciones de éxito de «${t}» y ver cuáles dependen de vos y cuáles del mundo. Las segundas no se construyen, se monitorean.`,
      category: 'PREGUNTA',
      tags: ['Pregunta', 'Supuestos'],
    }),
    (t) => ({
      title: `¿Qué evidencia falta para decidir sobre ${t}?`,
      description: `Diseñar el experimento más barato que cambie la decisión sobre «${t}»: si el resultado no cambia qué hacés, no vale el experimento.`,
      category: 'PREGUNTA',
      tags: ['Pregunta', 'Experimento'],
    }),
    (t) => ({
      title: `¿Qué pasa si no hacés nada con ${t}?`,
      description: `El costo de la inacción: qué se pierde, quién se adelanta y si la ventana de «${t}» se cierra o simplemente espera.`,
      category: 'PREGUNTA',
      tags: ['Pregunta', 'Prioridad'],
    }),
  ],
};

function variaciones(tipo, nodo) {
  const t = limpio(nodo?.title || 'esta idea', 56);
  const moldes = MOLDES[tipo] || MOLDES.branch;
  return elegir(moldes, semilla(`${tipo}:${t}`), 3).map((f) => f(t));
}

function hibrido(seleccionados) {
  const a = limpio(seleccionados[0]?.title || 'Idea A', 44);
  const b = limpio(seleccionados[1]?.title || 'Idea B', 44);
  const tags = new Set(['Híbrido', 'Fusión']);
  for (const n of seleccionados) {
    for (const tg of Array.isArray(n?.tags) ? n.tags : []) if (tags.size < 6) tags.add(String(tg));
  }
  return {
    title: `Síntesis · ${a} × ${b}`,
    description:
      `Fusión de «${a}» y «${b}»: el mecanismo de una sostiene el alcance de la otra y ninguna queda ` +
      `subordinada. Primer experimento honesto: un flujo corto donde «${a}» abre el caso y «${b}» lo ` +
      `cierra, medido en una sola sesión de uso.`,
    tags: [...tags],
  };
}

function condensa(seleccionados, objetivo) {
  const n = seleccionados.length;
  const titulos = seleccionados.map((s) => limpio(s?.title || '', 40)).filter(Boolean);
  const cabeza = objetivo ? limpio(objetivo, 48) : limpio(titulos[0] || 'Selección', 48);
  return {
    title: `Macro · ${cabeza}`,
    description:
      `Colapsa ${n} nodos en un solo concepto operativo. El hilo que los une: ${titulos.slice(0, 4).join(' · ')}. ` +
      `Los detalles no se pierden — quedan como linaje del macro y se pueden volver a abrir.`,
    tags: ['Condensado', 'Macro', ...(objetivo ? [limpio(objetivo, 24)] : [])],
    resumen: `Un solo nodo que reemplaza ${n}. Si al leerlo no sabés qué hacer después, el corte fue prematuro.`,
    principio: 'Un concepto por nodo: cuando hacen falta dos para explicarlo, todavía no está condensado.',
    match: Math.min(0.95, 0.6 + n * 0.05),
  };
}

function sintesis(nodos) {
  const cats = new Map();
  for (const n of nodos) {
    const c = limpio(n?.data?.category || n?.category || 'SIN CATEGORÍA', 28).toUpperCase();
    cats.set(c, (cats.get(c) || 0) + 1);
  }
  const orden = [...cats.entries()].sort((x, y) => y[1] - x[1]);
  const titulos = nodos.map((n) => limpio(n?.data?.title || n?.title || '', 60)).filter(Boolean);
  return {
    summary:
      `La red tiene ${nodos.length} nodos y su peso está en ${orden.slice(0, 3).map(([c, q]) => `${c} (${q})`).join(', ') || 'una sola línea'}. ` +
      `Lo que se ve al mirarla junta: las ideas no están compitiendo, están en fases distintas de la misma apuesta.`,
    pillars: orden.slice(0, 4).map(([c, q]) => `${c} · ${q} nodo${q === 1 ? '' : 's'}`),
    actionItems: [
      `Validar el nodo más cargado (${titulos[0] || 'el núcleo'}) con una prueba de una sesión.`,
      `Marcar qué nodos son supuesto y cuáles son decisión: los supuestos se miden, las decisiones no.`,
      'Elegir el camino crítico y congelar el resto por dos semanas.',
    ],
    keyOpportunities: [
      `Cruzar «${titulos[1] || titulos[0] || 'el núcleo'}» con «${titulos[2] || 'otra línea'}»: es la intersección que nadie está mirando.`,
      'Convertir el hilo más maduro en herramienta usable y dejar el resto como notas.',
    ],
  };
}

const PARADAS = new Set(['de', 'del', 'la', 'el', 'los', 'las', 'una', 'uno', 'para', 'con', 'sin', 'por', 'que', 'como', 'the', 'and', 'for', 'with', 'desde', 'hacia', 'sobre']);
const palabrasClave = (s) =>
  String(s || '').toLowerCase().normalize('NFD').replace(/[\u0300-\u036f]/g, '')
    .split(/[^a-z0-9]+/).filter((w) => w.length > 4 && !PARADAS.has(w));

function puentes(nodos, aristas = []) {
  const yaConectados = new Set(aristas.map((e) => [e.source, e.target].sort().join('::')));
  const fuera = [];
  const lista = nodos.slice(0, 40);
  for (let i = 0; i < lista.length; i += 1) {
    for (let j = i + 1; j < lista.length; j += 1) {
      const a = lista[i];
      const b = lista[j];
      if (yaConectados.has([a.id, b.id].sort().join('::'))) continue;
      const pa = new Set(palabrasClave(`${a.data?.title || a.title} ${a.data?.description || ''}`));
      const comunes = [...new Set(palabrasClave(`${b.data?.title || b.title} ${b.data?.description || ''}`))].filter((w) => pa.has(w));
      if (comunes.length === 0) continue;
      fuera.push({
        id: `puente-${a.id}-${b.id}`,
        sourceId: a.id,
        targetId: b.id,
        sourceTitle: a.data?.title || a.title || 'Idea A',
        targetTitle: b.data?.title || b.title || 'Idea B',
        label: `Sinergia · ${comunes[0]}`,
        rationale: `Comparten «${comunes.slice(0, 3).join('», «')}» y no hay arista entre ellos: la relación existe en el texto, no en el grafo.`,
        puntaje: comunes.length,
      });
    }
  }
  return fuera.sort((x, y) => y.puntaje - x.puntaje).slice(0, 3);
}

const CATS_BD = ['ARQUITECTURA', 'ESTRATEGIA', 'EJECUCIÓN', 'MÉTRICAS', 'VALIDACIÓN'];
function braindump(rawText) {
  const lineas = String(rawText || '').split(/\r?\n+/).map((l) => l.replace(/^[-*•\d.)\s]+/, '').trim()).filter(Boolean);
  const cuerpo = lineas.length ? lineas : [String(rawText || '').slice(0, 60)];
  const raiz = cuerpo[0].slice(0, 45) || 'Idea Central';
  return {
    root: {
      title: raiz,
      description: String(rawText || '').length > 60 ? limpio(rawText, 160) : 'Núcleo conceptual principal',
      category: 'NÚCLEO',
      tags: ['BrainDump', 'Núcleo'],
    },
    nodes: cuerpo.slice(1, 11).map((linea, i) => ({
      tempId: `node-${i + 1}`,
      connectsTo: 'root',
      title: linea.slice(0, 42),
      description: linea.length > 42 ? linea : 'Concepto derivado de la descarga mental',
      category: CATS_BD[i % CATS_BD.length],
      tags: ['BrainDump'],
    })),
  };
}

/** Qué espera el front de cada acción: si el motor en vivo no cumple, se descarta y gana el determinista. */
const VALIDADORES = {
  branch: (r) => Array.isArray(r?.variations) && r.variations.length > 0 && typeof r.variations[0].title === 'string',
  explore: (r) => Array.isArray(r?.variations) && r.variations.length > 0 && typeof r.variations[0].title === 'string',
  critique: (r) => Array.isArray(r?.variations) && r.variations.length > 0 && typeof r.variations[0].title === 'string',
  socratic: (r) => Array.isArray(r?.variations) && r.variations.length > 0 && typeof r.variations[0].title === 'string',
  hybrid: (r) => typeof r?.hybrid?.title === 'string' && typeof r.hybrid.description === 'string',
  condensar: (r) => typeof r?.condensar?.title === 'string' && typeof r.condensar.description === 'string',
  synthesize: (r) => typeof r?.synthesis?.summary === 'string' && Array.isArray(r.synthesis.pillars),
  braindump: (r) => typeof r?.structure?.root?.title === 'string' && Array.isArray(r.structure.nodes),
  find_bridges: (r) => Array.isArray(r?.bridges),
  refresh_templates: (r) => Array.isArray(r?.templates),
};

const PEDIDOS = {
  branch: 'Generá 3 ramificaciones distintas del nodo dado. Devolvé {"variations":[{"title","description","category","tags"}]}.',
  explore: 'Explorá 3 ámbitos (técnico, negocio, usuario) del nodo dado. Devolvé {"variations":[{"title","description","category","tags"}]}.',
  critique: 'Hacé de abogado del diablo: 3 riesgos o puntos ciegos del nodo dado. Devolvé {"variations":[{"title","description","category","tags"}]}.',
  socratic: 'Formulá 3 preguntas catalizadoras sobre el nodo dado. Devolvé {"variations":[{"title","description","category","tags"}]}.',
  hybrid: 'Fusioná los nodos elegidos en UNA idea superior. Devolvé {"hybrid":{"title","description","tags"}}.',
  condensar: 'Condensá los nodos elegidos en un macro-concepto. Devolvé {"condensar":{"title","description","tags","resumen","principio"}}.',
  synthesize: 'Sintetizá el mapa. Devolvé {"synthesis":{"summary","pillars":[],"actionItems":[],"keyOpportunities":[]}}.',
  braindump: 'Estructurá la descarga mental. Devolvé {"structure":{"root":{"title","description","category","tags"},"nodes":[{"tempId","connectsTo","title","description","category","tags"}]}}.',
  find_bridges: 'Encontrá relaciones no evidentes entre nodos sin arista. Devolví {"bridges":[{"sourceId","targetId","label","rationale"}]} usando ids reales.',
  refresh_templates: 'Generá 3 núcleos de ideas frescos. Devolvé {"templates":[{"title","description","category","root":{"title","description","tags"},"nodes":[]}]}.',
};

/**
 * Motor en vivo para las acciones del lienzo. Devuelve null si no hay motor, si se pasó el tope por IP
 * o si la respuesta no respeta la forma que el front espera — en ese caso decide el generador local.
 */
async function accionConMotor(tipo, body) {
  const url = process.env.DEMO_MOTOR_URL;
  const modelo = process.env.DEMO_MOTOR_MODELO;
  const clave = claveMotor();
  if (!url || !modelo || !clave || !PEDIDOS[tipo]) return null;
  const t0 = Date.now();
  const entrada = JSON.stringify({ nodeData: body?.nodeData, selectedNodes: body?.selectedNodes, nodes: body?.nodes?.slice(0, 40), edges: body?.edges, rawText: body?.rawText, objetivo: body?.objetivo }).slice(0, 6000);
  const ctl = new AbortController();
  const reloj = setTimeout(() => ctl.abort(), Number(process.env.DEMO_MOTOR_TIMEOUT_MS || 20000));
  try {
    const r = await fetch(url, {
      method: 'POST',
      signal: ctl.signal,
      headers: { 'content-type': 'application/json', authorization: `Bearer ${clave}` },
      body: JSON.stringify({
        model: modelo,
        messages: [
          { role: 'system', content: `${PEDIDOS[tipo]}\nEscribí en castellano rioplatense, concreto, sin relleno. Sólo JSON.` },
          { role: 'user', content: entrada },
        ],
        response_format: { type: 'json_object' },
        max_tokens: Number(process.env.DEMO_MOTOR_MAX_TOKENS || 700),
      }),
    });
    if (!r.ok) return null;
    const d = await r.json();
    const txt = d?.choices?.[0]?.message?.content || '';
    const parsed = JSON.parse(txt);
    if (!VALIDADORES[tipo](parsed)) return null;
    return {
      datos: parsed,
      ms: Date.now() - t0,
      tokens: d.usage ? { prompt: d.usage.prompt_tokens ?? 0, completion: d.usage.completion_tokens ?? 0 } : null,
    };
  } catch (e) {
    console.warn(`demo: motor en vivo falló en ${tipo}, cae al generador local —`, String(e?.message || e));
    return null;
  } finally {
    clearTimeout(reloj);
  }
}

/** Respuesta determinista por acción, con la MISMA forma que devuelve el backend Rust. */
function accionLocal(tipo, body) {
  switch (tipo) {
    case 'branch':
    case 'explore':
    case 'critique':
    case 'socratic':
      return { variations: variaciones(tipo, body?.nodeData) };
    case 'hybrid':
      return { hybrid: hibrido(Array.isArray(body?.selectedNodes) ? body.selectedNodes : []) };
    case 'condensar':
      return { condensar: condensa(Array.isArray(body?.selectedNodes) ? body.selectedNodes : [], body?.objetivo) };
    case 'synthesize':
      return { synthesis: sintesis(Array.isArray(body?.nodes) ? body.nodes : []) };
    case 'braindump':
      return { structure: braindump(body?.rawText) };
    case 'find_bridges':
      return { bridges: puentes(Array.isArray(body?.nodes) ? body.nodes : [], Array.isArray(body?.edges) ? body.edges : []) };
    case 'refresh_templates':
      // Vacío a propósito: el front rota su propio paquete de núcleos cuando la lista llega vacía.
      return { templates: [] };
    default:
      return { sin_datos: true, tipo };
  }
}

/**
 * Bloque de uso que el front pinta en la traza (`proveedor · ms · tok · $`). La tarifa es opcional: si no
 * está declarada, se informan los tokens y NO se inventa un costo (regla de la casa, `costo.rs`).
 */
function usoDe(proveedor, tokens, ms) {
  const entrada = Number(process.env.DEMO_MOTOR_PRECIO_IN || 0);
  const salida = Number(process.env.DEMO_MOTOR_PRECIO_OUT || 0);
  const uso = {
    proveedor,
    modelo: String(proveedor).replace(/^demo@/, ''),
    ms: ms ?? 0,
    tokens: tokens || { prompt: 0, completion: 0 },
    cache: 'miss',
  };
  if (entrada > 0 || salida > 0) {
    uso.costo_usd = Number((((tokens?.prompt ?? 0) / 1e6) * entrada + ((tokens?.completion ?? 0) / 1e6) * salida).toFixed(6));
  }
  return uso;
}

// ---------------------------------------------------------------- router

const json = (status, body) => ({ status, json: body });

/** 0,25 s de silencio a 16 kHz mono 16-bit: la respuesta de `/api/voz/decir` sin motor TTS local. */
const WAV_SILENCIO = (() => {
  const datos = Buffer.alloc(4000 * 2);
  const cab = Buffer.alloc(44);
  cab.write('RIFF', 0);
  cab.writeUInt32LE(36 + datos.length, 4);
  cab.write('WAVE', 8);
  cab.write('fmt ', 12);
  cab.writeUInt32LE(16, 16);
  cab.writeUInt16LE(1, 20);
  cab.writeUInt16LE(1, 22);
  cab.writeUInt32LE(16000, 24);
  cab.writeUInt32LE(32000, 28);
  cab.writeUInt16LE(2, 32);
  cab.writeUInt16LE(16, 34);
  cab.write('data', 36);
  cab.writeUInt32LE(datos.length, 40);
  return Buffer.concat([cab, datos]);
})();

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
  // TTS: el backend real sintetiza con Kokoro (127.0.0.1:8125) y devuelve un WAV. En el demo web no hay
  // motor local, así que devolvemos 0,25 s de silencio con el mismo content-type: el panel reproduce algo
  // válido en vez de tirar «La voz local no respondió» en cada turno.
  if (ruta === '/api/voz/decir') return { status: 200, body: WAV_SILENCIO, contentType: 'audio/wav' };

  // --- el plan de voz y las acciones del lienzo (ramificar, hibridar, puentes, condensar…)
  if (ruta === '/api/ai/action') {
    const tipo = String(body?.type || 'voz');

    if (tipo !== 'voz') {
      // El front lee una clave distinta por acción (`variations`, `hybrid`, `condensar`, `bridges`,
      // `structure`, `synthesis`, `templates`): sin esa clave no pasa nada y no hay error — por eso el
      // demo «no hacía nada». Motor en vivo si está configurado; si no, generador local determinista.
      const t0 = Date.now();
      let datos = null;
      let fuente = 'demo@determinista';
      let tokens = null;
      let msMotor = null;
      if (permitido(ip)) {
        const vivo = await accionConMotor(tipo, body).catch(() => null);
        if (vivo) {
          datos = vivo.datos;
          tokens = vivo.tokens;
          msMotor = vivo.ms;
          fuente = `demo@${process.env.DEMO_MOTOR_MODELO}`;
        }
      }
      if (!datos) datos = accionLocal(tipo, body);
      const ms = msMotor ?? Date.now() - t0;
      return json(200, {
        success: true,
        ...datos,
        source: fuente,
        cadena: ['demo'],
        modo: 'demo',
        ms,
        demo: { vivo: msMotor !== null, aviso: 'Acción resuelta en el demo, sin salir del navegador.' },
        uso: usoDe(fuente, tokens, ms),
      });
    }

    const texto = body?.texto || body?.prompt || body?.rawText || '';
    const { plan, motor, ms, vivo, exacto, limitado, tokens } = await planDeVoz(texto, ip);
    return json(200, {
      success: true,
      voz: plan,
      modelUsed: motor,
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
      uso: usoDe(motor, tokens, ms),
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

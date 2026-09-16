/**
 * Sanitiza las fixtures del demo web.
 *
 * Por qué existe: las fixtures se cosechan del backend REAL, así que traen lo que hay en la máquina
 * del autor — rutas con su usuario, su bitácora interna, su perfil HITL aprendido (que es un retrato
 * de cómo piensa) y el estado de sus proyectos. Nada de eso tiene por qué ser público: el demo necesita
 * la FORMA de las respuestas, no su contenido privado.
 *
 * Contrato:
 *   demo/fixtures-crudas/*.json   → lo cosechado (gitignoreado, nunca al repo)
 *   demo/fixtures/*.json          → lo que se publica (esto lo que genera este script)
 *
 * Uso:  node demo/sanitizar.mjs        (idempotente: se puede correr las veces que haga falta)
 */
import { readFileSync, writeFileSync, readdirSync, existsSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..');
const CRUDAS = join(RAIZ, 'demo', 'fixtures-crudas');
const PUBLICAS = join(RAIZ, 'demo', 'fixtures');

// ---------------------------------------------------------------- reglas de texto

/** Reemplazos literales: identidad y rutas de la máquina del autor. */
const LITERALES = [
  [/C:\\+Users\\+tomas\\+Documents\\+Obsidian Vault[^"',]*/gi, '<bóveda>'],
  [/C:\\+Users\\+tomas[^"',]*/gi, '<ruta-local>'],
  [/C:\/Users\/tomas[^"',]*/gi, '<ruta-local>'],
  [/tomaspieruz@gmail\.com/gi, '<usuario>@example.com'],
  [/TOMAS\.WAV/gi, '<autor>'],
  [/Tomas Pieruz/gi, '<autor>'],
  [/\btomas\b/gi, '<usuario>'],
];

/** El perfil aprendido no se publica: es un retrato del autor, no un dato del producto. */
const PERFIL_GENERICO = {
  profile: {
    acceptanceRate: 78,
    autoAprendizaje: { activo: false, cada: 10, decisionesEnLaUltima: 0, ultimaMs: 0 },
    categoriesAccepted: ['ARQUITECTURA', 'SISTEMAS', 'DATOS'],
    categoriesRejected: ['GENERAL'],
    decisions: 0,
    learnedProfile:
      'Perfil de demostración: el sistema aprende de las decisiones del usuario y ajusta el ruteo. ' +
      'En el demo web este perfil es de ejemplo (no hay aprendizaje real acumulado).',
    prefers: ['conceptos técnicos', 'estructuras verificables'],
    rejects: [],
  },
  fuente: 'demo',
};

/** Bitácora y planes: contenido de ejemplo, sin decisiones internas reales. */
const ESPACIO_GENERICO = (base) => ({
  ...base,
  bitacora: {
    existe: true,
    chars: 420,
    texto:
      '---\ntitulo: "Bitácora (demo)"\ntipo: nota\ngenerado_por: demo\n---\n\n' +
      '# Bitácora (demo)\n\nEn la app real esta nota la escribe el agente residente: cada turno deja una\n' +
      'entrada con fecha y autor, y el humano puede dejarle la suya. En el demo web el contenido es de\n' +
      'ejemplo para no publicar las decisiones internas del proyecto.\n',
    actualizado: '2026-09-01T00:00',
  },
  planes: Array.isArray(base?.planes)
    ? base.planes.slice(0, 2).map((p, i) => ({
        ...p,
        nombre: i === 0 ? 'plan-de-ejemplo' : 'plan-de-ejemplo-2',
        titulo: i === 0 ? 'Plan de ejemplo' : 'Segundo plan de ejemplo',
        estado: 'vigente',
        texto:
          '---\ntitulo: "Plan de ejemplo"\ntipo: nota\n---\n\n# Plan de ejemplo\n\n' +
          'El cerebro mantiene sus planes como notas de la bóveda. Acá se muestra la forma: estado,\n' +
          'fecha y cuerpo en markdown. El texto real es privado del autor.\n',
      }))
    : [],
});

/** Expertos: se muestran dos genéricos y sin la carpeta del sistema. */
const EXPERTOS_GENERICOS = {
  carpeta: '<bóveda>/expertos',
  expertos: [
    {
      slug: 'brief-documento',
      nombre: 'Brief Documento',
      rol: 'consultor de producto',
      descripcion: 'Convierte un concepto en un brief ejecutable de una página.',
      caracteres_system: 420,
      system:
        'Sos consultor de producto. Escribís en español, en prosa clara y sin relleno. Devolvés un brief ' +
        'de una página: problema, propuesta, cómo se verifica y qué falta. (Ejemplo de demo.)',
      proveedor: 'demo',
      modelo: '',
    },
    {
      slug: 'revisor-tecnico',
      nombre: 'Revisor Técnico',
      rol: 'arquitecto de software',
      descripcion: 'Critica un nodo del lienzo buscando huecos y supuestos no verificados.',
      caracteres_system: 380,
      system:
        'Sos arquitecto de software. Señalás supuestos no verificados, deuda y riesgos concretos, citando ' +
        'lo que dice el nodo. (Ejemplo de demo.)',
      proveedor: 'demo',
      modelo: '',
    },
  ],
  success: true,
};

/** Paneles cuyo contenido interno no aporta al demo: se publican vacíos pero con la forma correcta. */
const VACIOS = {
  'api__ai__delegar.json': { corriendo: false, cerebro: { sesion: 'demo', notas: 0 }, resultado: null },
  'api__ai__investigar.json': (d) => ({
    corriendo: false,
    ...(d && d.fases ? { fases: d.fases } : {}),
    investigacion: { fase: null, ok: false, pasos: [] },
  }),
  'api__ai__evaluar.json': { corriendo: false, success: true, tabla: { motores: [], ganador_por_prueba: {}, cuando: '0' } },
  'api__metrics.json': { conversiones: 0, detalle: [], objetivo_min: 30, promedio_min: null, ultima_min: null, sesion_activa: false },
  'api__vault__memory.json': (d) => ({
    ...d,
    raiz: '<bóveda>',
    notas_del_vault_del_usuario: 0,
    docs_indexados: d?.nodos_del_lienzo ?? 51,
  }),
  'api__cerebro__herramientas.json': (d) => ({
    ...d,
    carpeta: '<bóveda>/cerebro/herramientas',
  }),
};

// ---------------------------------------------------------------- motor

const reemplazarTexto = (s) => LITERALES.reduce((acc, [re, rep]) => acc.replace(re, rep), s);

function limpiar(v, clave = '') {
  if (typeof v === 'string') return reemplazarTexto(v);
  if (Array.isArray(v)) return v.map((x) => limpiar(x, clave));
  if (v && typeof v === 'object') {
    const out = {};
    for (const [k, val] of Object.entries(v)) out[k] = limpiar(val, k);
    return out;
  }
  return v;
}

function sanitizar(nombre, json) {
  // 1) reemplazos específicos por archivo
  if (nombre === 'api__hitl__preferences.json') return PERFIL_GENERICO;
  if (nombre === 'api__expertos.json') return EXPERTOS_GENERICOS;
  if (nombre === 'api__cerebro__espacio.json') return ESPACIO_GENERICO(limpiar(json, nombre));
  const vacio = VACIOS[nombre];
  if (typeof vacio === 'function') return limpiar(vacio(json), nombre);
  if (vacio) return vacio;
  // 2) el resto: scrub de identidad/rutas
  return limpiar(json, nombre);
}

// ---------------------------------------------------------------- ejecución

if (!existsSync(CRUDAS)) {
  console.error(`No existe ${CRUDAS}: copiá ahí las respuestas crudas del backend real antes de sanitizar.`);
  process.exit(1);
}
mkdirSync(PUBLICAS, { recursive: true });

let n = 0;
for (const f of readdirSync(CRUDAS)) {
  if (!f.endsWith('.json')) continue;
  const json = JSON.parse(readFileSync(join(CRUDAS, f), 'utf-8'));
  writeFileSync(join(PUBLICAS, f), JSON.stringify(sanitizar(f, json), null, 1) + '\n', 'utf-8');
  n++;
}
console.log(`sanitizadas ${n} fixtures → demo/fixtures/ (crudas en demo/fixtures-crudas/, gitignoreadas)`);

// El seed del lienzo y los planes viven en las fixtures sanitizadas: si quedó alguna ruta o identidad, es un bug.
const sucio = [];
for (const f of readdirSync(PUBLICAS)) {
  const t = readFileSync(join(PUBLICAS, f), 'utf-8');
  for (const pat of [/C:\\+Users/i, /tomaspieruz@gmail/i, /\btomas\b/i, /TOMAS\.WAV/i]) {
    if (pat.test(t)) sucio.push(`${f} (${pat})`);
  }
}
console.log(sucio.length ? `OJO — quedó sin limpiar: ${sucio.join(', ')}` : 'chequeo final: ninguna fixture lleva rutas ni identidad del autor');

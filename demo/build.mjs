/**
 * Build del DEMO WEB: compila el mismo front de la app apuntando al API simulado y deja todo en
 * `demo/public` (lo que sirve el server local y lo que se publica en Vercel).
 *
 *   node demo/build.mjs
 */
import { execFileSync } from 'node:child_process';
import { cpSync, mkdirSync, readFileSync, writeFileSync, existsSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..');
const DESTINO = join(RAIZ, 'demo', 'public');

console.log('· limpiando demo/public');
rmSync(DESTINO, { recursive: true, force: true });
mkdirSync(DESTINO, { recursive: true });

console.log('· vite build → demo/public (VITE_API_BASE=same-origin)');
// `npx` no es resoluble de forma confiable en Windows/MSYS: se llama al binario de vite con node.
const viteBin = join(RAIZ, 'node_modules', 'vite', 'bin', 'vite.js');
execFileSync(process.execPath, [viteBin, 'build', '--outDir', 'demo/public', '--emptyOutDir'], {
  cwd: RAIZ,
  stdio: 'inherit',
  env: { ...process.env, VITE_API_BASE: 'same-origin' },
});

// El spec de voz del backend entra al demo: es el mismo prompt que usa la app para planificar.
// OJO: NO se adjunta el esquema JSON crudo. Medido: el modelo lo devuelve a él ({"type":"OBJECT",
// "properties":…}) en vez de los datos — es un imán de eco. En prosa, acierta.
const spec = join(RAIZ, 'src-tauri', 'specs', 'actions.json');
if (existsSync(spec)) {
  const d = JSON.parse(readFileSync(spec, 'utf-8'));
  const voz = d?.voz || {};
  const forma = [
    'Devolvé SOLO un objeto JSON (sin markdown, sin el esquema) con estos campos:',
    '- intencion: "capturar" o "comando"',
    '- respuesta: una frase en primera persona, como se lo dirías hablando',
    '- motivo: por qué elegiste esos comandos (una frase)',
    '- comandos: lista de objetos; cada uno lleva accion ("crear" | "enlazar" | "enfocar" | "condensar" | "criticar" | "actualizar" | "delegar") y los campos que esa acción necesita',
    'Si no hay nada que hacer, devolvé comandos: [].',
  ].join('\n');
  writeFileSync(join(RAIZ, 'demo', 'lib', 'spec-voz.txt'), `${voz.prompt || ''}\n\n${forma}\n`, 'utf-8');
  console.log('· spec de voz exportado a demo/lib/spec-voz.txt (en prosa, sin el esquema crudo)');
}

console.log('listo. Serví con: node demo/server.mjs');

/**
 * Server local del DEMO WEB: sirve el bundle del front (`demo/public`) y el API simulado bajo `/api`.
 * Es el mismo núcleo que corre en la función serverless, así que lo que se ve acá es lo que se publica.
 *
 *   node demo/server.mjs [puerto]
 */
import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { apiHandler } from './lib/http.mjs';

const PUERTO = Number(process.argv[2] || process.env.PORT || 4173);
const RAIZ = join(import.meta.dirname, 'public');
const TIPOS = {
  '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8', '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml', '.png': 'image/png', '.jpg': 'image/jpeg', '.woff2': 'font/woff2',
  '.ico': 'image/x-icon', '.map': 'application/json',
};

const server = createServer(async (req, res) => {
  const u = new URL(req.url, 'http://localhost');
  if (u.pathname.startsWith('/api/')) return apiHandler(req, res);
  let p = normalize(decodeURIComponent(u.pathname)).replace(/^([.][.][/\\])+/, '');
  if (p === '/' || p === '\\') p = '/index.html';
  let archivo = join(RAIZ, p);
  try {
    const s = await stat(archivo);
    if (s.isDirectory()) archivo = join(archivo, 'index.html');
  } catch {
    archivo = join(RAIZ, 'index.html'); // SPA: lo que no es archivo, es la app
  }
  try {
    const datos = await readFile(archivo);
    res.statusCode = 200;
    res.setHeader('content-type', TIPOS[extname(archivo)] || 'application/octet-stream');
    res.end(datos);
  } catch (e) {
    res.statusCode = 404;
    res.setHeader('content-type', 'text/plain; charset=utf-8');
    res.end(`404 — ${e.message}. ¿Corriste \`npm run demo:build\`?`);
  }
});

server.listen(PUERTO, () => {
  console.log(`demo NodeFlow · http://localhost:${PUERTO}  (API simulada en /api)`);
});

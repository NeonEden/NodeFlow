/**
 * Adaptador HTTP del núcleo del demo: sirve para el server local (`server.mjs`) y para la función
 * serverless de Vercel (`api/[...ruta].mjs`), que reciben (req, res) igual.
 */
import { handle } from './mockApi.mjs';

async function leerCuerpo(req) {
  if (req.body && typeof req.body === 'object') return req.body; // Vercel ya lo parseó
  const partes = [];
  for await (const c of req) partes.push(c);
  const raw = Buffer.concat(partes).toString('utf-8');
  if (!raw) return {};
  try {
    return JSON.parse(raw);
  } catch {
    return { _raw: raw };
  }
}

export async function apiHandler(req, res) {
  const u = new URL(req.url, 'http://localhost');
  // El camino puede llegar por query (rewrite de Vercel: `/api/:camino*` → `/api/index?camino=…`) o
  // directamente en la URL cuando lo sirve el server local. Así el ruteo no depende del hosting.
  const camino = u.searchParams.get('camino');
  const ruta = camino ? `/api/${camino.replace(/^\/+/, '')}` : u.pathname.replace(/\/+$/, '') || '/';
  // La IP real (detrás de Vercel viene en x-forwarded-for) es lo que limita el motor en vivo.
  const ip = (req.headers['x-forwarded-for'] || '').split(',')[0].trim()
    || req.headers['x-real-ip']
    || req.socket?.remoteAddress
    || 'anon';
  let salida;
  try {
    const body = (req.method || 'GET').toUpperCase() === 'GET' ? Object.fromEntries(u.searchParams) : await leerCuerpo(req);
    salida = await handle({ method: req.method, ruta, query: Object.fromEntries(u.searchParams), body, ip });
  } catch (e) {
    salida = { status: 500, json: { success: false, error: String(e?.message || e) } };
  }
  res.statusCode = salida.status || 200;
  res.setHeader('access-control-allow-origin', '*');
  res.setHeader('access-control-allow-headers', 'content-type');
  // Respuestas binarias (el WAV de `/api/voz/decir`): sólo el JSON se serializa.
  if (salida.body) {
    res.setHeader('content-type', salida.contentType || 'application/octet-stream');
    res.end(salida.body);
    return;
  }
  res.setHeader('content-type', 'application/json; charset=utf-8');
  res.end(JSON.stringify(salida.json ?? {}));
}

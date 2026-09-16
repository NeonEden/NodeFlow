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
  const ruta = u.pathname.replace(/\/+$/, '') || '/';
  let salida;
  try {
    const body = (req.method || 'GET').toUpperCase() === 'GET' ? Object.fromEntries(u.searchParams) : await leerCuerpo(req);
    salida = await handle({ method: req.method, ruta, query: Object.fromEntries(u.searchParams), body });
  } catch (e) {
    salida = { status: 500, json: { success: false, error: String(e?.message || e) } };
  }
  res.statusCode = salida.status || 200;
  res.setHeader('content-type', 'application/json; charset=utf-8');
  res.setHeader('access-control-allow-origin', '*');
  res.setHeader('access-control-allow-headers', 'content-type');
  res.end(JSON.stringify(salida.json ?? {}));
}

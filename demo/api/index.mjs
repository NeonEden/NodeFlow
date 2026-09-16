/**
 * Única función serverless del demo. Todas las rutas entran acá.
 *
 * Por qué una sola: el plan Hobby de Vercel limita a 12 funciones por deployment (medido: con 61 el
 * build falla con `● Error`), y el catch-all `api/[...ruta].mjs` sólo resolvió paths de un segmento.
 * El camino real llega por `?camino=` desde el rewrite, así que el routing no depende de la sintaxis
 * de brackets ni del filesystem de Vercel.
 */
export { apiHandler as default } from '../lib/http.mjs';

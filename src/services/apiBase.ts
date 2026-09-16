/**
 * Base URL de la API local de NodeFlow.
 *
 * En la app de escritorio (Tauri) la API la sirve el backend en Rust en 127.0.0.1:37371.
 * Se puede sobreescribir con VITE_API_BASE (útil si alguna vez el puerto cambia o si
 * querés apuntar al server.ts de Express para comparar comportamientos).
 */
const DEFAULT_API_BASE = 'http://127.0.0.1:37371';

/**
 * `VITE_API_BASE=same-origin` (o `/`) sirve la API en el MISMO origen que la interfaz, que es como
 * corre el demo web publicado: ahí `/api/...` ya es la ruta completa y no hay host que anteponer.
 * La app de escritorio no setea esta variable, así que sigue pegándole al backend local.
 */
const RAW = import.meta.env?.VITE_API_BASE as string | undefined;

export const API_BASE: string =
  RAW === 'same-origin' || RAW === '/' ? '' : RAW?.replace(/\/$/, '') || DEFAULT_API_BASE;

/** Une la base con una ruta relativa ('/api/...'). */
export function apiUrl(path: string): string {
  return `${API_BASE}${path.startsWith('/') ? path : `/${path}`}`;
}

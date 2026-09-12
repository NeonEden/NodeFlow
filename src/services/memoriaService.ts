/**
 * Fase 5b — Memoria semántica de la bóveda de Obsidian.
 *
 * Busca en TODAS las notas del usuario (BM25 en el backend, acentos plegados) y permite traer una
 * nota al lienzo. Traerla NO la escribe: crea una propuesta que el usuario aprueba en
 * «Cambios del agente» (Fase 5a).
 */
import { apiUrl } from './apiBase';

export interface ResultadoMemoria {
  ruta: string;
  titulo: string;
  puntaje: number;
  fragmento: string;
  ya_en_el_lienzo: boolean;
  modificado_ms: number;
}

export interface RespuestaMemoria {
  ok: boolean;
  consulta?: string;
  terminos?: string[];
  raiz?: string;
  docs_indexados?: number;
  coincidencias?: number;
  resultados?: ResultadoMemoria[];
  error?: string;
}

export interface RespuestaNota {
  ok: boolean;
  ruta?: string;
  titulo?: string;
  caracteres?: number;
  texto?: string;
  error?: string;
}

export interface RespuestaPropuestaNota {
  ok: boolean;
  accion?: string;
  id_pendiente?: string;
  pendientes?: number;
  vista?: { resumen?: string; peligro?: string };
  error?: string;
}

async function getJson<T>(url: string, timeoutMs = 15000): Promise<T | null> {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), timeoutMs);
  try {
    const r = await fetch(url, { signal: ctl.signal });
    const data = (await r.json().catch(() => null)) as T | null;
    if (!r.ok) return data;
    return data;
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

export function buscarEnVault(consulta: string, limite = 8): Promise<RespuestaMemoria | null> {
  return getJson<RespuestaMemoria>(
    apiUrl(`/api/vault/search?q=${encodeURIComponent(consulta)}&limit=${limite}`)
  );
}

export function leerNotaDeVault(ruta: string, maxChars = 4000): Promise<RespuestaNota | null> {
  return getJson<RespuestaNota>(apiUrl(`/api/vault/note?ruta=${encodeURIComponent(ruta)}`))
    .then((d) => {
      if (d && d.texto && d.texto.length > maxChars) {
        return { ...d, texto: d.texto.slice(0, maxChars) + '\n\n[…recortado]' };
      }
      return d;
    });
}

/** Cuerpo útil de una nota: sin frontmatter, con espacios colapsados y acotado. */
function cuerpoUtil(texto: string, maxChars = 900): string {
  let t = texto;
  if (t.startsWith('---')) {
    const fin = t.indexOf('\n---', 3);
    if (fin > 0) t = t.slice(fin + 4);
  }
  return t.replace(/\s+/g, ' ').trim().slice(0, maxChars);
}

/** Propone un nodo a partir de una nota: entra a la cola de aprobación, no al lienzo. */
export async function traerNotaAlLienzo(
  resultado: ResultadoMemoria,
  textoCompleto: string,
  parent?: string | null
): Promise<RespuestaPropuestaNota | null> {
  const body: Record<string, unknown> = {
    title: resultado.titulo,
    description: `Desde el vault: ${resultado.ruta}\n\n${cuerpoUtil(textoCompleto)}`,
    category: 'MEMORIA',
    maturity: 1,
    tags: ['vault', 'memoria'],
    motivo: `Traído desde ${resultado.ruta}`,
    mode: 'propose',
  };
  if (parent) {
    body.parent = parent;
    body.link_label = 'desde el vault';
  }
  try {
    const r = await fetch(apiUrl('/api/graph/node'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    const data = (await r.json().catch(() => null)) as RespuestaPropuestaNota | null;
    if (!r.ok) return { ok: false, error: data?.error || `HTTP ${r.status}` };
    return data;
  } catch (e) {
    return { ok: false, error: String(e) };
  }
}

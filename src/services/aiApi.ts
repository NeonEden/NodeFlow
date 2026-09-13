import { apiUrl } from './apiBase';

/**
 * Centralized client-side AI proxy caller.
 * Invokes the local NodeFlow API (Rust backend inside Tauri; antes era Express).
 * If the user configured a custom Gemini API Key in their browser (BYOK),
 * it is forwarded securely via the x-gemini-api-key request header to the backend.
 * The server-side GEMINI_API_KEY environment variable is never exposed to the client.
 */
export async function postAiAction(payload: Record<string, any>): Promise<Response> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  };

  try {
    const customKey = localStorage.getItem('user_gemini_api_key');
    if (customKey && customKey.trim()) {
      headers['x-gemini-api-key'] = customKey.trim();
    }
  } catch (err) {
    // Gracefully handle any localStorage access restriction in sandboxed iframes
  }

  // Fase 11: el modo de inferencia elegido en el interruptor viaja con cada acción.
  let modo = 'auto';
  try {
    const guardado = localStorage.getItem('nodeflow_modo_inferencia');
    if (guardado === 'local' || guardado === 'nube') modo = guardado;
  } catch {
    /* almacenamiento restringido: se usa la cadena configurada */
  }

  const inicio = performance.now();
  const respuesta = await fetch(apiUrl('/api/ai/action'), {
    method: 'POST',
    headers,
    body: JSON.stringify({ ...payload, modo }),
  });

  // Traza real de la corrida para el interruptor (proveedor, tokens, costo, caché y latencia).
  try {
    respuesta
      .clone()
      .json()
      .then((d: any) => {
        const uso = d?.uso ?? {};
        window.dispatchEvent(
          new CustomEvent('nodeflow:traza', {
            detail: {
              modo: d?.modo ?? modo,
              proveedor: uso.proveedor ?? d?.source ?? 'sin respuesta',
              modelo: uso.modelo ?? d?.modelUsed ?? '',
              ms: Math.round(uso.ms ?? performance.now() - inicio),
              tokens: (uso.tokens?.prompt ?? 0) + (uso.tokens?.completion ?? 0),
              tokens_evitados: uso.tokens_evitados ?? 0,
              costo_usd: uso.costo_usd ?? 0,
              cache: uso.cache ?? 'miss',
            },
          }),
        );
      })
      .catch(() => {
        /* respuesta sin JSON: no hay traza que mostrar */
      });
  } catch {
    /* clone() no disponible: se omite la traza */
  }

  return respuesta;
}

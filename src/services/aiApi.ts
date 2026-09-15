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

  // Fase 12: el motor lo decide el selector global (se guarda en el backend), así que la petición
  // no lleva preferencia: todas las funciones de la app usan el mismo motor elegido.
  const inicio = performance.now();
  const respuesta = await fetch(apiUrl('/api/ai/action'), {
    method: 'POST',
    headers,
    body: JSON.stringify(payload),
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
              modo: d?.modo ?? 'global',
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

  // B — «ocupado»: la app corre una generación por vez. Un 409 se avisa en la UI en vez de quedar mudo.
  if (respuesta.status === 409) {
    const aviso = (texto: string) =>
      window.dispatchEvent(new CustomEvent('nodeflow:aviso', { detail: texto }));
    respuesta
      .clone()
      .json()
      .then((d: any) => aviso(d?.error || 'Ya hay una generación en curso: esperá a que termine.'))
      .catch(() => aviso('Ya hay una generación en curso: esperá a que termine.'));
  }

  return respuesta;
}

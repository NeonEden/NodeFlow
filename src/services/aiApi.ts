/**
 * Centralized client-side AI proxy caller.
 * Strictly invokes the application server endpoint (/api/ai/action).
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

  return fetch('/api/ai/action', {
    method: 'POST',
    headers,
    body: JSON.stringify(payload),
  });
}

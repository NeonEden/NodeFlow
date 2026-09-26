# Pedido de habilitación — LLM Gateway (AssemblyAI)

Copiar y pegar tal cual en `support@assemblyai.com` y/o en el Discord de lablab.ai (canal del
hackathon, taggeando al sponsor).

---

**Asunto:** LLM Gateway access request — account has no access (hackathon participant)

Hi AssemblyAI team,

My account (`tomaspieruz@gmail.com`) has working Speech-to-Text — I verified a live streaming session
and the temporary-token endpoint today — but **LLM Gateway returns no access for the account**.

What I get:

- `GET https://llm-gateway.assemblyai.com/v1/models` → **200** (I can list 37 models)
- Any `POST https://llm-gateway.assemblyai.com/v1/chat/completions` → **400**:

```json
{"error":"Your account does not have access to LLM Gateway. Please upgrade or contact us at support@assemblyai.com for more information.","status":"error","request_id":"f44b9ba6-9735-4d6b-b8b4-2f9014d16a0d"}
```

Per-model attempts all return `Your account does not have access to this LLM Gateway model` with
`request_id` `b1308b8e-f72d-44e6-96e3-fa671904a59d` (models tried: `gpt-5-mini`, `gemini-3.5-flash-lite`,
`gemini-3.6-flash`, `gpt-oss-20b`, `minimax-m3`; `claude-haiku-4-5` reports "model is not supported").

Context: I'm building for the **AssemblyAI Voice Agent Hackathon** (lablab.ai, Sep 1–30, 2026) — a
voice-first canvas agent that turns spoken Spanish into structured graph operations. I'm using
Universal-Streaming v3 for the live transcript and I want the LLM Gateway as the reasoning layer so
both STT and the LLM ride on the same key and balance, as the hackathon asks participants to build on
AssemblyAI.

Could you enable LLM Gateway for this account, or tell me the step I'm missing (billing/upgrade)? Both
`request_id`s above should let you find the exact calls.

Thanks,
Tomas Pieruz

---

## Reintento del 26/09/2026 — sigue bloqueado, con evidencia fresca

Volví a sondear con la misma clave. **Lo que cambió respecto del 16/09 es el mensaje, y eso orienta el
pedido a soporte: el bloqueo pasó de ser de la cuenta entera a ser por modelo.** Los `request_id` de hoy
son los que hay que citar (`GET /v1/models` sigue dando 200, así que el problema no es la autenticación):

| Llamada | Resultado | `request_id` |
|---|---|---|
| `POST /v1/chat/completions` · `gemini-2.5-flash-lite` · **con tools** | 400 | `d3bd2f93-fab7-4ded-9729-899f96ff6e36` |
| `POST /v1/chat/completions` · `gemini-3.5-flash-lite` · con tools | 400 | `17e67e93-7b3e-4900-9fe4-4851e10bcd20` |
| `POST /v1/chat/completions` · `gemini-3.6-flash` · con tools | 400 | `8ee51340-0524-4409-95d4-abf8b83f4d32` |
| `POST /v1/chat/completions` · `gpt-5-mini` · con tools | 400 | `3d2f6e4a-9af3-4bcd-a3bd-f501ac283754` |
| `POST /v1/chat/completions` · `gemini-2.5-flash-lite` · **cuerpo mínimo, sin tools** | 400 | `c372636a-accd-49d7-901a-e355ae517e01` |
| `POST /v1/chat/completions` · `claude-sonnet-4-5-20250929` · cuerpo mínimo | 400 | `221b7278-b553-4fae-b9f5-f1f3f53d6f04` |

Cuerpo del error (idéntico en los seis): `{"metadata":{"errors":["Your account does not have access to
this LLM Gateway model"]},"message":"invalid request body","code":400}`.

Dos comprobaciones que descartan causas nuestras: el **cuerpo mínimo sin `tools` da lo mismo** (no es el
schema) y **dos proveedores distintos dan lo mismo** (no es un modelo sin soporte). `GET /v1/models`
devuelve **47 modelos** con `supported_parameters` que incluyen `tools` y `tool_choice`, así que el
tool calling existe del lado del Gateway: lo que falta es la habilitación de la cuenta.

**Qué pedir, entonces:** que habiliten el acceso al LLM Gateway (o digan qué desbloquea, si es
verificación de facturación) y que confirmen con estos `request_id`, que son de hoy.

## Notas para Tomás (no van en el mail)

- Los dos `request_id` están comprobados: los generé yo con tu clave. Son la evidencia que soporte pide.
- Alternativa de respaldo mientras tanto: el demo online corre planes reales con tu motor de Azure
  Foundry (server-side, centavos) o con planes grabados (determinista, cero claves). Ninguna de las dos
  depende del Gateway.
- Si te habilitan el Gateway, el cambio es **una línea** en el demo: `DEMO_MOTOR_URL` →
  `https://llm-gateway.assemblyai.com/v1/chat/completions`, `DEMO_MOTOR_KEY` → tu clave de AssemblyAI,
  `DEMO_MOTOR_MODELO` → el modelo que elijas del catálogo vivo (`gemini-2.5-flash-lite` para latencia,
  `gpt-oss-20b` si no necesitás JSON estricto — ojo: `gpt-oss` no soporta `response_format`).
- Y el extra que habilita: el WS de streaming acepta un parámetro `llm_gateway` que corre el LLM
  **sobre cada turno** y devuelve mensajes `LLMGatewayResponse` por el mismo socket. Eso convertiría el
  plan en algo incremental (mientras hablás se va armando), sin llamada HTTP aparte.

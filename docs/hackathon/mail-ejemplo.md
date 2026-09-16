# Mail de ejemplo (listo para enviar)

Corresponde al pedido de habilitación del **LLM Gateway** de AssemblyAI. Los `request_id`
completos de los intentos están en [`pedido-llm-gateway.md`](pedido-llm-gateway.md).

---

**Para:** support@assemblyai.com
**Asunto:** LLM Gateway access — account with working Streaming STT (request ids inside)

---

Hi team,

I'm building **NodeFlow** (https://github.com/NeonEden/NodeFlow), a desktop knowledge-graph tool
where you speak and the canvas executes a validated plan. The voice path is **already running on
Universal-Streaming v3** in production — short-lived tokens minted by our own backend, PCM 16 kHz
over the v3 WebSocket, and Spanish verified end to end against the live socket.

I would like to move the reasoning step onto your **LLM Gateway** so that speech and reasoning share
one key, one bill and one data-residency story. Right now every model I try through the gateway
answers:

```
HTTP 400 · "Your account does not have access to this LLM Gateway model"
```

`GET /v1/models` returns 200 and lists the catalogue, so the key itself works — the models are what
appear to be gated. I tried: `gemini-3.5-flash-lite`, `gemini-3.6-flash`, `gpt-oss-20b`,
`claude-haiku-4-5`, `minimax-m3`.

Could you enable LLM Gateway access for the account, or tell me what I need to complete first?
I have the two support request ids from the in-app attempts: `f44b9ba6…` and `b1308b8e…`
(full ids in the repository, under `docs/hackathon/pedido-llm-gateway.md`).

Happy to share a minimal reproduction — it is a five-line `curl` against `/v1/chat/completions`
that returns the 400 above.

Thanks,
**Tomas Pieruz** · TOMAS.WAV
NodeFlow — the canvas you can talk to

---

## Por qué está escrito así

| Decisión | Motivo |
|---|---|
| Empieza por lo que **ya funciona** | Streaming STT en producción es la prueba de que el problema no es de integración ni de credenciales. |
| Cita el **error exacto** y los **modelos probados** | Soporte no tiene que preguntar nada ni reproducir a ciegas. |
| Incluye los **request_id** | Permite encontrar los intentos en su sistema sin pedir más datos. |
| Ofrece una **reproducción de 5 líneas** | Convierte un reclamo en un reporte. |
| No menciona el hackathon como presión | El pedido se sostiene solo: es un caso de uso real, con producto real detrás. |

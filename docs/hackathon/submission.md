# Submission — AssemblyAI Voice Agent Hackathon (lablab.ai)

Todo lo que pide el formulario, listo para pegar. Los textos respetan los límites del reglamento
(título ≤50 caracteres, short ≤255, long ≥100 palabras).

## Título (máx. 50)

```
NodeFlow — Voice-First Canvas Agent
```
*(34 caracteres)*

## Short description (máx. 255)

```
You speak, the canvas thinks. NodeFlow turns spoken Spanish into validated operations on a knowledge
graph: create, link, focus, condense or delegate — every command checked against the real nodes before
it runs. Built on AssemblyAI Universal-Streaming v3.
```
*(238 caracteres)*

## Long description (mín. 100 palabras)

```
Ideas don't die from lack of thinking — they die between the note app, the chat window and the
whiteboard. NodeFlow removes the typing step: you talk to your canvas and it reorganizes itself.

Speak a command in Spanish ("clean the canvas and keep only what connects to the spatial copilot") and
NodeFlow does three things in order. First, AssemblyAI Universal-Streaming v3 transcribes live over a
WebSocket. Second, the transcript becomes a structured plan: the model proposes operations — create,
link, focus, condense, update, delegate — as strict JSON. Third, and this is the part that matters, the
plan is validated in code against the graph: node ids that don't exist are discarded, and nothing
touches the canvas until the operations pass. The model proposes; the code verifies.

Every agent write lands in an approval queue, so the human keeps control of what changes. The graph
lives in plain markdown in the user's own Obsidian vault — local-first by design, with local models
running the fast loop on the user's GPU and cloud models only for the expensive reasoning.

It runs as a signed, self-updating desktop app (Tauri v2 + Rust + React Flow).
```

## Technology tags

`AssemblyAI Universal-Streaming v3` · `AssemblyAI LLM Gateway` · `Tauri v2` · `Rust (axum)` ·
`React 19` · `React Flow` · `TypeScript` · `Ollama (local LLMs)` · `Obsidian / markdown vault` ·
`WebSocket realtime` · `Structured outputs` · `DeepSeek / Azure AI Foundry`

## Categorías / tracks sugeridos

Voice Agents · Productivity · Developer Tools · Local-first AI

## Video (≤5 min, MP4)

| Tiempo | Qué mostrar |
|---|---|
| 0:00–0:30 | Problema: la idea se pierde entre notas y chat. Hablar es más rápido que tipear |
| 0:30–2:30 | **Demo en vivo**: dictado en español → transcripción AssemblyAI en pantalla → el plan → los comandos se validan contra el lienzo → se ejecutan y el grafo crece |
| 2:30–4:00 | Valor: 3 casos (sumar ideas, condensar un racimo, delegar una investigación) + números medidos (plan en 0,5–5 s, caché semántica con 6.5k tokens evitados, 216/216 tests) |
| 4:00–5:00 | Roadmap: cerebro residente local, más idiomas, y el salto a agentes de voz con ida y vuelta |

Grabar en inglés o con subtítulos (el jurado es internacional). Mostrar el selector de motor de voz con
**AssemblyAI** elegido.

## Cover image (16:9, PNG/JPG)

Sugerencia: captura del lienzo real con el panel de voz abierto y la transcripción visible. El lienzo
ya tiene la paleta propia; no hace falta diseñar nada.

## Slides (PDF, obligatorio)

1. Problema · 2. Solución en una frase · 3. Cómo funciona (STT → plan → validación → lienzo) ·
4. Demo (2 capturas) · 5. Por qué AssemblyAI (Universal-Streaming v3 + por qué la validación en código) ·
6. Medido (latencia, tokens, tests) · 7. Mercado: conocimiento personal y equipos chicos ·
8. Roadmap · 9. Stack + repo.

## Links

- Repo: https://github.com/NeonEden/NodeFlow (público, MIT, instalador firmado con auto-update)
- **Demo online**: https://nodeflow-demo.vercel.app *(lienzo real con 51 nodos + voz AssemblyAI en vivo
  desde el navegador; el planificador corre en vivo con un motor real y cae a planes grabados si falla
  o si se supera el tope por IP)*

## Checklist del reglamento

- [x] Repo público con commits dentro de la ventana del evento
- [x] Video MP4 ≤5 min
- [x] **Demo URL interactiva** → https://nodeflow-demo.vercel.app
- [ ] Slides PDF
- [ ] Cover 16:9
- [ ] Registro en lablab + crear/unirse a un equipo (aplica también si vas solo)

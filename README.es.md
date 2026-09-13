# NodeFlow

**Orquestador de inferencia híbrida Edge/Cloud para grafos de conocimiento.**
App nativa de escritorio (Tauri v2 + Rust) que convierte ideas dispersas en un grafo de conocimiento estructurado y versionado — decidiendo *por tarea* si la inferencia corre **local** (privada, sin cuota, instantánea) o en la **nube** (razonamiento profundo), y contabilizando el costo de cada llamada.

> Versión en inglés (principal, para la hackathon): [`README.md`](README.md)

## El problema

1. **Todo va a la nube.** Resumir una nota o extraer entidades no necesita un modelo frontera remoto: necesita uno que ya está instalado. Enviarlo afuera cuesta, agrega latencia y expone notas privadas.
2. **Nada se contabiliza.** El costo por artefacto es invisible: nadie sabe qué operaciones vale la pena pagar.
3. **Nada se reutiliza.** El mismo prompt sobre el mismo nodo se recalcula siempre.

## Por qué es infraestructura, no una app de notas

| Eje | Qué hace NodeFlow |
|---|---|
| **Cómputo edge** | Modelo local (`Ollama`, `granite3.3:2b`) redacta títulos, categorías y tags: sin cuota, sin internet, las notas nunca salen de la máquina. |
| **Cómputo nube** | El razonamiento complejo sobre varias ramas del grafo se delega a un endpoint en la nube (Gemini hoy; cualquier endpoint compatible con OpenAI encaja). |
| **Costo y reuso** | Cada llamada se tarifa por proveedor (`costo.rs`) y se cachea por hash de *nodo + prompt + proveedor*, con contrato versionado. |
| **Eficiencia de recursos** | Shell nativo en Rust: **28,6 MB de RAM** medidos sobre el binario de release (un equivalente en Electron ronda 10-20× eso), dejando la GPU libre para la inferencia local. |
| **Determinismo** | El modelo propone, el código valida: cada salida estructurada se verifica campo por campo antes de tocar el grafo. |

## Arquitectura

```mermaid
flowchart LR
  UI["React 18 + React Flow + Tailwind"] <--> IPC["Tauri v2 · IPC"]
  IPC <--> AX["axum :37371 · 33 endpoints"]
  AX --> COST["costo.rs · tarifas + cache por hash"]
  COST --> OL["Ollama :11434"]
  COST --> GE["Gemini · razonamiento"]
  AX --> BOR["borrador.rs · gramatica JSON + validador"]
  BOR --> GR["granite3.3:2b"]
  AX --> VAU["vault.rs · notas .md + frontmatter YAML"]
  VAU --> VAULT[("Vault Obsidian · .nodeflow/ai-cache.json")]
```

## Números medidos

Medidos en esta máquina, no estimados.

| Métrica | Valor |
|---|---|
| Memoria residente del binario nativo | **28,6 MB** (`tasklist` sobre el release en ejecución) |
| Inferencia repetida (mismo nodo + prompt) | **27,9 s / 9.061 tokens → 0,4 s / 0 tokens** |
| Tokens evitados por la caché | **23.893** en 17 entradas |
| Borrador local | 5,1 s en frío · **0,7 s en caliente** |
| Tests Rust | **60 passed / 0 failed** (`cargo test --lib`) |
| Grafo en uso diario | 54 nodos · 68 aristas · 0 colgadas |
| Líneas propias | 23.860 (TSX 9.931 · Rust 8.583 · TS 4.110 · Python 950 · CSS 286) |

## Qué funciona hoy

Canvas de nodos y aristas (React Flow) con auto-organización, lentes por categoría y zonas · borradores locales validados por código · contabilidad de costo y caché por artefacto · corridas de experto sobre nodo o selección · diagnóstico y reparación del grafo (jardín) · integración con Obsidian (Markdown + frontmatter) · human-in-the-loop: el agente **propone** y el cambio se aplica con aprobación explícita.

## Qué falta (honesto)

Streaming (SSE) hacia el nodo activo · interruptor visible Local/Cloud por tarea · builds para macOS/Linux · instaladores firmados y updater · entrada de voz.

## Inicio rápido

Requisitos: Windows 10/11 x64, WebView2, Node.js 22+, Rust 1.77+, `Ollama` para inferencia local.

```bash
npm install
ollama pull granite3.3:2b     # modelo de borradores local
npm run dev:web               # Vite (:5173)
npx tauri dev                 # app completa: backend Rust + ventana nativa
npx tauri build               # binario de release + instaladores MSI y NSIS
cargo test --manifest-path src-tauri/Cargo.toml --lib   # 60 tests
```

## Principios de diseño

1. **El modelo propone, el código valida** (una gramática garantiza la forma, no la verdad).
2. **La clave de caché no puede llevar el reloj** (con `now_iso()` se medían 6 fallos / 0 aciertos en corridas idénticas).
3. **Medir, no suponer**: contrastes, memoria, aciertos de caché y rechazos del validador se midieron; varias decisiones cambiaron cuando la medición contradijo el supuesto.

## Licencia

MIT. Hecho por **TOMAS.WAV** (Tomas Pieruz).

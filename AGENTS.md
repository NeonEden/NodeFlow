# AGENTS.md · NodeFlow

Contrato para cualquier agente (opencode, Cline, Codex, Gemini CLI, Claude Code, un subagente) que entre a este
repositorio. Está escrito para que **no tengas que preguntar nada** y para que un error tuyo no cueste una sesión
humana. Si algo de acá contradice lo que creés saber del proyecto, manda este archivo.

## Qué es esto

**NodeFlow** — app de escritorio *local-first* para convertir ideas sueltas en artefactos: un lienzo de nodos
(Tauri v2 + React) con backend HTTP propio en Rust (axum, `127.0.0.1:37371`), grafo embebido (Kùzu), bóveda
Obsidian, voz (STT AssemblyAI/Speechmatics + TTS local Kokoro en `127.0.0.1:8125`) y un servidor MCP propio
(`mcp-server/nodeflow_mcp.py`). El estado del usuario **no vive en el repo**.

## Comandos canónicos (usá estos; no inventes otros)

| Para qué | Comando | Tiempo |
|---|---|---|
| Tipos del frontend | `npx tsc --noEmit` | ~15 s |
| Tests de Rust (281) | `cd src-tauri && cargo test --lib` | ~1 min |
| Compilar el frontend | `npm run build` | ~10 s |
| Compilar Rust en release | `cd src-tauri && cargo build --release --lib` | 1-2 min |
| Instalar la app en la máquina (dev) | `bash scripts/instalar.sh` | 2-3 min |
| Armar el instalador de Windows | `bash scripts/instalador.sh` | 3-4 min |
| **Diagnóstico: ¿por qué la ventana está vacía?** | `bash scripts/verificar-app.sh` | 10 s |
| Nueva versión publicada | `bash scripts/release.sh patch` (bump + tests + tag + GitHub) | 5 min |

**Los tres árbitros de cualquier cambio: `npx tsc --noEmit`, `npm run build` y `cargo test --lib`.** El CI
(`.github/workflows/ci.yml`) corre exactamente esos tres en `windows-latest`. Un cambio sin los tres en verde no
está terminado, y no hay excepción por «es un cambio chico».

> Si te delegaron como **worker con presupuesto de pasos acotado**, esos tres comandos los corre quien te delegó:
> no gastes tus pasos esperando un build de minutos (en un worktree nuevo compila todo desde cero). Editá bien y
> reportá; el veredicto no es tuyo.

## Reglas de la casa (no negociables)

1. **El modelo propone, el código valida** (ADR `0003`). Ningún dato generado por un modelo entra al estado del
   usuario sin pasar por un validador en código: los planes se validan contra el lienzo real, los borradores se
   validan con el schema, los ids se verifican contra el grafo.
2. **Sin LLM en el camino caliente** (ADR `0005`). Ninguna ruta que el usuario use a diario puede depender de una
   llamada a un modelo. Si el LLM no responde, la función tiene que seguir funcionando por reglas locales.
3. **Ruteo híbrido: primero local** (ADR `0004`). Antes de mandar algo a la nube, preguntate si un modelo local
   (Ollama) o una regla lo resuelven.
4. **Todo lo que se mide queda escrito.** Comentario en español que explica **por qué** y cita la medición
   (`// medido 19/09: 4,16 s de audio en 1.706 ms`). Un comentario que repite lo que hace el código no sirve:
   se borra. Un cambio sin el porqué se revierte en el siguiente refactor.
5. **Los textos de cara al usuario van en los dos idiomas** (`src/i18n/textos.ts`, español e inglés). Si agregás
   UI sin i18n, la app queda a medio traducir.
6. **Nada de secretos**: ni claves en el repo, ni en `nodeflow.config.json`, ni en logs, ni en un mensaje de
   commit. Las claves viven en el llavero de Windows (`claves.rs`) y se resuelven por `claves::obtener`.
7. **No agregues dependencias** sin justificarlo en el commit (qué te ahorra y qué te cuesta). Este repo tiene
   un árbol deliberadamente chico.

## Qué NO tocar (y por qué)

- **`%APPDATA%\com.nodeflow.desktop\`** — datos del usuario: bóveda, config, claves, ~405 MB de la voz local.
  Leer está bien (para diagnosticar); escribir **no**, salvo que el pedido sea explícitamente sobre eso.
- **`src-tauri/icons/` y `src-tauri/installer/`** — los iconos y las imágenes del instalador se **generan** con
  `python scripts/instalador-imagenes.py` a partir de `icons/icon.png`. Editar los BMP a mano se pierde en la
  próxima corrida.
- **`src-tauri/target/`** — artefactos; no se commitean ni se editan.
- **El nombre del ejecutable instalado** (`NodeFlow.exe`) y el `[[bin]]` de `Cargo.toml`: el instalador NSIS y
  los accesos directos dependen de ese nombre.
- **`docs/`** — la bitácora del proyecto. Se agrega información, no se reescribe lo ya fechado.

## Protocolo de trabajo (esto es lo que se espera de vos)

1. **Nunca trabajes sobre `main`.** Pedí un worktree: `git worktree add ../nf-<tema> -b agente/<tema> main`.
2. **Un pedido, un alcance.** Si el pedido es «arreglá los warnings», no reformatees otra cosa. Si encontrás un bug
   distinto, **anotalo en el reporte**, no lo arregles de prepo.
3. **Corré los tres árbitros** antes de decir que terminaste. Si no pasan, arreglá o revertí; nunca los dejes en
   rojo «para que los vea el humano».
4. **Commit con el estilo del repo** (`feat|fix|chore|docs(alcance): qué y por qué`, en español, describiendo el
   efecto, no el archivo): `fix(voz): la voz no queda muda si Kokoro no está`.
5. **PR, nunca merge.** Se entrega con `gh pr create` y el humano aprueba. El merge a `main` no es tuyo.
6. **Reportá con evidencia**: qué cambiaste, la salida de los árbitros, y qué quedó afuera. «Listo» sin salida de
   tests no es un reporte.

## Trampas ya conocidas (nos costaron horas — no las repitas)

- **Ventana en blanco**: pasa cuando el binario no embebe la interfaz. Antes de decir «la app no anda», corré
  `bash scripts/verificar-app.sh`: te dice si el binario instalado trae la UI adentro (`embebe la interfaz: sí/no`).
- **`tauri build` embebe el frontend**: si tocaste `src/`, corré `npm run build` antes, o vas a embeber UI vieja.
- **El instalador NSIS sólo crea el acceso directo del escritorio en instalaciones nuevas o silenciosas**: cuando
  el instalador corre con `/UPDATE` (así lo lanza el updater) sale sin tocarlo, a propósito. El hook
  `src-tauri/windows/instalador-hooks.nsh` lo repone.
- **Vite escucha sólo en IPv6** (`[::1]:5173`): sondear `127.0.0.1:5173` da «conexión rechazada» aunque esté vivo.
- **`curl` está bloqueado** por el firewall de egreso de esta máquina: para pedir HTTP usá Python (`urllib`) o
  Docker.
- **La cuota de búsqueda de Gemini (grounding) se agota antes que la del modelo**: con `tools: [{google_search}]`
  responde `429 RESOURCE_EXHAUSTED` mientras el mismo modelo sin esa herramienta responde `200`.
- **GitHub Models está retirado** (30/07/2026): `models.github.ai` contesta `410 Gone` con un cuerpo que dice
  «temporarily unavailable». No construyas nada sobre ese endpoint.
- **Un solo modelo grande por vez en la GPU** (`OLLAMA_MAX_LOADED_MODELS=1`) y 16 GB de RAM en total: no lances
  cinco procesos con modelos locales en paralelo.

## Mapa del proyecto

```
src/                     Frontend React (App.tsx es grande: paneles en src/components/)
src-tauri/src/           Backend Rust
  server.rs              Todas las rutas HTTP (~5.800 líneas) y los adaptadores de proveedor
  cerebro*.rs            Cerebro residente, gateway, arquitectura, herramientas
  grafo.rs, curador.rs   Grafo del lienzo y su mantenimiento
  investigacion.rs       Investigación por fases: investigador (Gemini con grounding) + contraste
  voz.rs, voz_local.rs, stt.rs, dialogo.rs   Voz: validación de planes, TTS local, STT, hilo de conversación
  claves.rs, idioma.rs, costo.rs, motores.rs   Claves, idioma, contabilidad y catálogo de motores
  vault.rs               Bóveda Obsidian (lectura/escritura de notas)
mcp-server/              Servidor MCP propio (Python, stdlib) que expone el lienzo a Hermes
scripts/                 Todo lo operativo (instalar, instalador, verificar, release, checkpoint, i18n)
docs/                    ADRs, planes, notas de versión, informes, DEVLOG
demo/                    Demo web de la hackathon (servidor + mock)
```

## Cómo saber si tu cambio sirve

- ¿Pasa los tres árbitros? ¿El cambio resiste una lectura por alguien que no conoce el código?
- ¿Podés decir **por qué** y **cómo se mide**, en una línea?
- ¿El usuario nota la diferencia sin que se la expliques?
- Si tocaste algo de cara al usuario: ¿está en los dos idiomas? ¿Se ve bien con el lienzo vacío y con 200 nodos?

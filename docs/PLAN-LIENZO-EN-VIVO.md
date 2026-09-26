---
title: El lienzo en vivo — voz conversacional sobre AssemblyAI + IBM Bob 2.0
date: 2026-09-26
status: propuesto
ventana: 26–30 de septiembre de 2026
deadlines:
  ibm-bob-2-hackathon: 2026-09-27   # 48 h · 25–27 sept
  assemblyai-voice-agent-hackathon: 2026-09-30   # mes largo · 1–30 sept
supera_a: docs/hackathon/submission.md
---

# PLAN · El lienzo en vivo

## 0 · La idea en una frase

**Hoy** la voz cierra un turno y recién ahí actúa: hablás → `end_of_turn` → el modelo propone un plan →
se valida → se aplica. **Lo que sigue** es que el lienzo se dibuje **mientras** pensás, que las dudas no
interrumpan, que la corrección («no, mejor conectalo a X») sea un gesto y no un turno nuevo, y que cada
idea entre ya clasificada (Semilla → Concepto → Código).

No es un agente de voz que responde: es un **resonador** que materializa lo que decís sobre tu propio
grafo. La voz es el canal; el artefacto es el lienzo.

## 1 · Qué ya existe (medido, no supuesto)

Se verificó en el repo el 26/09/2026. Esta tabla es la línea de base: lo que sigue se construye **encima**,
nada de esto se rehace.

| Pieza | Dónde | Estado real |
|---|---|---|
| Streaming STT v3 | `src/services/assemblyaiRt.ts` (297 líneas) | Maneja `Begin` · `Turn` · `Termination` · `Error`, `format_turns`, latido para que la sesión no se cierre entre turnos, **una sesión reusada por conversación** (medido 20/09: abrir una por turno daba 11 sesiones en 90 s) |
| Cierre de turno | `assemblyaiRt.ts` → `cerrarTurno()` | Resuelve cuando llega `Turn` con `end_of_turn` y limpia el acumulador (bug medido y corregido 20/09: el segundo turno devolvía el texto del primero) |
| Token temporal | `src-tauri/src/stt.rs` (20.663 B) | `GET https://streaming.assemblyai.com/v3/token?expires_in_seconds=600` con la clave cruda; modelo declarado `u3-rt-pro`; catálogo de motores con etiqueta propia |
| Validación del plan | `src-tauri/src/voz.rs` (35 KB) | `ACCIONES` de **11** acciones, ids verificados contra el grafo real, topes de comandos/nodos; el modelo propone, el código valida |
| Contrato de la acción `voz` | `src-tauri/specs/actions.json` | Campos `intencion · respuesta · motivo · comandos` (entra al binario por `include_str!` ⇒ requiere recompilar) |
| Cola de propuestas | `/api/agent/pending · approve · reject` | Toda escritura del agente nace como propuesta; el panel «Cambios del agente» la aprueba |
| TTS local | Kokoro, `127.0.0.1:8125` | Medido 2,3–2,7× tiempo real; viaja como paquete descargable (405 MB, 75 s) |
| Hilo de diálogo | `src-tauri/src/dialogo.rs` | Últimos 12 turnos + foco, expira a los 30 min |
| Modo conversación | `App.tsx` (`turnoConversacion`) | Guion local de 0 tokens; un turno por gesto, sin autoescucha por construcción |

## 2 · El hueco (lo que no existe y no es un detalle de implementación)

1. **El parcial no se usa.** Hoy el frontend ignora los `Turn` intermedios: actúa sólo en `end_of_turn`.
   La doc de v3 (verificada hoy en `assemblyai.com/docs/speech-to-text/universal-streaming`) dice que el
   servidor emite **varios parciales antes de cada `end_of_turn`** y que el turno trae
   `end_of_turn_confidence`. Eso es exactamente la señal que falta para que el lienzo reaccione al ritmo.
2. **No hay tool calling.** El plan viaja como JSON dentro de un prompt de texto y se valida a mano. El
   LLM Gateway de AssemblyAI (doc verificada hoy en `assemblyai.com/docs/llm-gateway`) es
   **OpenAI-compatible** (`https://llm-gateway.assemblyai.com/v1/chat/completions`) y soporta
   **tool/function calling**, *model fallbacks*, **json-repair** para tool calls rotas, y —clave— permite
   **aplicar una request del Gateway en cada turno del STT en vivo** para bajar la latencia E2E.
3. **No hay corrección ni arrepentimiento.** Las 11 acciones crean y conectan; ninguna deshace lo último
   dicho. «No, eso no» hoy no tiene representación.
4. **No hay clasificación en vivo.** La madurez se escribe al final, por acción. La forma del nodo ya
   cambia sola (`MATURITY_CONFIGS` + `clip-path`), pero nadie la mueve mientras la idea crece.
5. **El Gateway estaba bloqueado para esta cuenta** (16/09: `POST /v1/chat/completions` → 400 *«Your
   account does not have access to LLM Gateway»*; hay mail redactado en
   `docs/hackathon/pedido-llm-gateway.md`). **Estado hoy: sin verificar** — el intento de sondearlo desde
   un proceso suelto no llegó a la clave (vive en el llavero `com.nodeflow.desktop` y no es legible desde
   fuera de la app). Es el primer paso de la Fase A y **no es opcional**: de esto depende si el
   «traductor» de ideas→operaciones corre en el Gateway o en nuestro propio camino.

## 3 · Arquitectura: el Resonador Conceptual, aterrizado a los módulos reales

```
mic ──PCM 16k──▶ [ assemblyaiRt.ts · WS v3 ] ──Turn parcial + end_of_turn_confidence──▶
                                                                    │
                                        [ SEGMENTADOR · Rust, SIN LLM ]  ◀── ADR 0005
                                        ¿sigue pensando? · ¿semilla nueva? · ¿corrige lo último?
                                                                    │
                                        [ TRADUCTOR · tools JSON-Schema ]
                                        Gateway (si está habilitado) o motor del ruteo actual
                                                                    │
                                        [ VALIDADOR · voz.rs ]  ◀── el contrato NO se relaja
                                        ACCIONES + ids reales + topes
                                                                    │
                                        [ APLICADOR ]  borrador del turno ──confirmación──▶ lienzo
                                                                    │
                                        [ Kokoro · acuse de una palabra ] ──▶ oído
```

Cuatro decisiones de diseño, todas ya escritas en los ADR de la casa:

- **El segmentador es local y sin modelo** (ADR 0005). Decidir «esto ya es una idea» con reglas: longitud
  del parcial, estabilidad entre ticks, conectores de duda («o sea», «no, mejor», «esperá», «y también»),
  y `end_of_turn_confidence`. Si el LLM no está, el lienzo sigue reaccionando.
- **El contrato se deriva del código, no se escribe de nuevo.** Las herramientas del tool calling se
  **generan** de `ACCIONES` (`voz.rs`) + `specs/actions.json`. Una sola fuente de verdad: hoy el mismo
  contrato vive en dos lugares que se actualizan juntos a mano, y ya costó un bug (una acción agregada al
  validador y no al spec ⇒ el modelo elegía la acción parecida).
- **El validador no se toca.** El modo vivo cambia *cuándo* se propone, nunca *qué* se acepta.
- **Local primero** (ADR 0004): el Gateway entra como un proveedor más del catálogo (`base_url` + clave),
  no como una dependencia nueva. Si no responde, el ruteo actual sigue funcionando.

## 4 · Fases y pruebas de fuego

Cada fase se cierra con algo **medido**, no con «anda». El criterio de cada una está escrito para poder
decir que no.

### Fase A · El proveedor que falta (≈30 min) — **bloqueante de B**

1. Verificar el acceso al LLM Gateway. Dos caminos, el primero más barato:
   - con la app corriendo: `POST /api/voz/traza` / el panel de voz ya tiene la clave en el llavero; o
   - con la clave en una variable: `ASSEMBLYAI_API_KEY=… python %LOCALAPPDATA%\Temp\nf-docs\probar_gateway2.py`
     (el script ya está escrito: sondea `/v3/token`, `/v1/models` y un `chat/completions` **con tools**,
     y sólo imprime status, conteos y nombres de modelo).
2. Si está habilitado: declararlo en `nodeflow.config.json → proveedores[]`
   (`base_url` + `clave_config: assemblyai_api_key`) y comprobar que aparece en el catálogo y responde.
   **Prueba de fuego**: un `chat/completions` con la herramienta `crear_nodo` devuelve `tool_calls` no
   vacío para la frase del sintetizador visual.
3. Si sigue bloqueado: **Plan B declarado** — el traductor corre con el motor del ruteo actual (local o
   Azure Foundry) pidiendo tool calls por prompt, y el Gateway queda como camino opcional. En ese caso el
   `submission.md` **no puede seguir listando «AssemblyAI LLM Gateway» como tecnología usada**: se cambia
   por lo que de verdad corre.

### Fase B · Tool calling con JSON-Schema (el contrato ya existe)

- Generar el array `tools` desde el código (una función pura en Rust, con test de round-trip: el schema
  derivado declara exactamente las acciones de `ACCIONES`).
- El camino nuevo: parcial/turno → `tools` → `tool_calls` → validación → aplicador. El camino viejo
  (`{comandos:[…]}` en texto) queda como respaldo y **se elige por configuración**, para poder comparar.
- **Prueba de fuego**: una frase con dos operaciones produce **2 tool_calls** con ids reales del lienzo;
  un id inventado se descarta y se cuenta; se publica la tabla de latencia y tokens contra el camino JSON
  actual (misma frase, misma máquina, `sin_cache: true`).

### Fase C · Lienzo en vivo (intra-turno) — el corazón

- `assemblyaiRt.ts`: emitir cada `Turn` parcial al backend (hoy se descartan), con el `end_of_turn` como
  cierre.
- Segmentador en Rust: `Nada · Semilla · Corrección`, con las reglas locales de §3 y **vencimiento**: la
  clave de idempotencia es `(corrida, hash del parcial normalizado)`, no un contador — lección ya pagada
  dos veces en este proyecto (banderas «en curso» sin expiración e índices de paso que se repiten).
- Aplicación: el turno abierto se materializa en un **borrador del turno** (nodos y aristas visibles pero
  marcados), y el `end_of_turn` lo confirma. Una confirmación **por turno**, no por nodo: el humano ve el
  dibujo crecer y decide una vez. Nada entra al grafo sin pasar por el validador.
- **Prueba de fuego (la que va al video)**: una frase de ~20 s con tres ideas → los nodos aparecen
  **antes** de que termine la frase, y se mide **ms desde el fin de la palabra hasta que el nodo existe**.
  Cero duplicados con la misma frase repetida 3 veces; `canvas_stats` → `aristas colgadas: 0`.

### Fase D · Pensar en voz alta (no interrumpir, corregir)

- Acción nueva `corregir` en **los tres lugares** (validador, spec, ejecutor): deshace la última operación
  del turno (con linaje, como el colapso) y aplica la nueva.
- Acuse hablado de una palabra con Kokoro («anotado», «conectado», «corregido») — **nunca** una respuesta
  larga: el sistema no conversa, escucha.
- **Prueba de fuego**: un monólogo de 60 s con tres autocorrecciones («no, eso no», «mejor como
  concepto», «borrá eso»… ) sale con un árbol coherente, sin nodos basura, y el log declara cuántos
  parciales se descartaron por ruido.

### Fase E · Semilla → Concepto → Código en vivo

- Clasificación al crear: madurez 1..5 (la que ya define las formas) + categoría, decidida por el
  traductor y **validada en código**; el nodo cambia de forma sin recargar.
- **Prueba de fuego**: la misma idea sube de Semilla 🌱 a Artefacto 🚀 durante una sola toma, y el cambio
  de forma se ve en pantalla.

### Fase F · Los dos entregables

- **Video AssemblyAI** (cierra 30/09): sobre el guion que ya existe (`guion-video-nodeflow.md`, 7 beats
  medidos) + tres beats nuevos: lienzo dibujándose en vivo, corrección hablada, clasificación en vivo.
- **Bob 2.0** (cierra **27/09 — mañana**): el envío pide explicar **en ≤500 palabras cómo y dónde se usó
  IBM Bob**, así que el uso tiene que ser real y registrado (§5).

## 5 · Bob: cómo entra y cómo se prueba

Bob 2.0 está instalado (`%LOCALAPPDATA%\Programs\IBM Bob\IBM Bob.exe`, `bobide` 1.126.0+bob2.2.0, con
ventana de agentes, merge de 3 vías y perfiles) y su base vive en `~/.bob/db/bob.db`.

**Lo que hace valioso el registro** (leído hoy, solo lectura): la tabla `attribution_logs` tiene
`file_uri · repo_name · branch_name · tool_name · contribution_text · start_line · end_line`. Es decir:
Bob deja **atribución por archivo y por línea**, que es exactamente lo que hay que mostrar para responder
«dónde usaste Bob» con datos en vez de con un relato.

**Estado hoy**: `tasks` = **2**, `messages` = **0**, `attribution_logs` = **0**. El video no se puede
sostener con eso: hay que usarlo de verdad y con tareas verificables.

Tareas asignadas a Bob (una por fase, con el archivo que toca):

| Tarea para Bob | Archivo / alcance | Cómo se prueba |
|---|---|---|
| Segmentador con tests unitarios | `src-tauri/src/voz.rs` (o `segmentador.rs`) | `cargo test --manifest-path src-tauri/Cargo.toml --lib` en verde |
| Emitir parciales al backend | `src/services/assemblyaiRt.ts` + tests del front (vitest) | `npx vitest run` |
| Derivar el schema de herramientas del código | generador puro + test de round-trip | test en verde |
| Refactor chico y acotado en la ruta de voz | un solo archivo por tarea | los tres árbitros |

Reglas del repo que Bob hereda (están en `AGENTS.md`): PR y no merge, un pedido un alcance, comentario
con el porqué y la medición, i18n en los dos idiomas, sin secretos, sin dependencias nuevas.

**Evidencia para el video**: exportar `tasks` (título, directorio, estado, fechas) y `attribution_logs`
(archivo, líneas, herramienta) a `docs/hackathon/bob-evidencia.md` **desde la base real**, no escrito a
mano. Eso es lo que se cuenta en las 500 palabras.

## 6 · Reparto

| Quién | Qué hace | Qué no hace |
|---|---|---|
| **Tomas** | Dirección, el fraseo de la demo, las decisiones de §7, la grabación | no pelea con el build |
| **Hermes** (coordina) | Este plan, los worktrees y PRs, las mediciones, la verificación de cada prueba de fuego, la evidencia de Bob | no mergea a `main` |
| **Bob 2.0** | Implementación dentro del IDE, sobre tareas acotadas y con atribución por línea | no decide alcance ni toca `main` |

## 7 · Decisiones abiertas (las tiene que tomar Tomas)

1. **¿Cómo se aplica el borrador del turno?** (a) se dibuja en vivo y se confirma al cerrar el turno
   *(recomendado: el humano ve crecer el árbol y decide una vez)*, (b) se aplica directo con `deshacer`, o
   (c) sigue todo por la cola de propuestas, sin dibujo previo.
2. **Si el Gateway sigue bloqueado**: ¿vamos con el traductor en el ruteo actual y el Gateway como
   opcional, o insistimos con soporte antes de construir?
3. **El video nuevo**: ¿reemplaza el que ya está subido, o se sube como actualización del proyecto?
4. **Bob**: ¿reemplaza a los agentes que ya venías usando para código, o se suma para lo del hackathon?

## 8 · Deadlines y lo que falta del submission

| Cuándo | Qué cierra | Qué falta de verdad |
|---|---|---|
| **27/09 (mañana)** | IBM Bob 2.0 (48 h) | usar Bob con tareas registradas + video con las 500 palabras + evidencia exportada |
| **30/09** | AssemblyAI Voice Agent | el sistema de las fases A–E, video nuevo, y del `submission.md`: **slides PDF**, **cover 16:9**, **registro en lablab / equipo**, y corregir el claim del LLM Gateway si no se habilita |

## 9 · Riesgos (declarados, con su mitigación)

| Riesgo | Por qué importa | Mitigación |
|---|---|---|
| **Latencia** | El Gateway «aplica una request por turno» baja la E2E pero mete la nube en el bucle de voz (ADR 0004/0005) | segmentador y aplicador locales; el traductor se mide antes de adoptarlo y hay camino B |
| **Duplicados** | Es el riesgo #1 histórico del proyecto (upsert, cola que coalesce, parciales que se repiten) | idempotencia por `(corrida, hash del parcial)`; `canvas_stats` después de cada prueba |
| **Coste por minuto** | Voz en vivo = muchas llamadas; el Gateway tiene rate limit por modelo en ventana de 60 s | caché semántica ya existente + tope de llamadas por turno + medir el gasto en el log |
| **El lienzo se ensucia con parciales** | Un grafo con ruido es peor que no tener la función | el «nada» del segmentador es la respuesta por defecto; lo descartado se cuenta y se declara |
| **La regla de la casa contra el «en vivo»** | «Nada toca el lienzo sin validar» es lo que hace confiable la demo | el validador no se relaja: cambia el momento, con borrador y deshacer del turno |
| **Prometer lo que no corre** | El submission ya lista el LLM Gateway, que estaba bloqueado | la Fase A lo verifica **antes**; si no está, se corrige el texto y el video |

## 10 · Definición de terminado

Una toma de 3 minutos, sin cortes, en la que: hablás en español y el lienzo **se dibuja mientras hablás**;
te equivocás, te corregís y el árbol se acomoda sin basura; la idea que nació Semilla se ve convertida en
Artefacto; y al final el panel dice el motor de voz que corrió (**AssemblyAI Universal-Streaming v3**), el
traductor que decidió, cuántos parciales se descartaron y cuántos nodos se crearon. Con los tres árbitros
en verde (`npx tsc --noEmit`, `npm run build`, `cargo test --lib`) y la evidencia de Bob exportada.

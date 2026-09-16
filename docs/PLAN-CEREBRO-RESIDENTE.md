# Plan · El cerebro residente: que el modelo habite el campo de los nodos

**Objetivo (en palabras del usuario):** que Hermes *habite* el lienzo. Que la memoria, el contexto y la
visión del proyecto **crezcan desde adentro de la app** —no desde el chat— y que, cuando haya que modificar
algo, se haga **desde adentro de la app**, con las herramientas de Hermes ejecutando ahí.

Este documento es la evaluación de viabilidad + el plan. Todo lo marcado *verificado* se probó el 15/09
sobre la máquina, no se supuso.

---

## 0. Estado del plan (actualizado 15/09, noche)

| Fase | Qué | Estado |
|---|---|---|
| 0 | Habilitar las 22 tools MCP del lienzo para Hermes | ✅ **hecho y permanente** (`exclude: []`; verificado en un proceso nuevo: contó los nodos del lienzo con sus tools) |
| 1 | Corte vertical: `delegar` con sesión nombrada + nota episódica | ✅ **hecho y verificado en vivo**: dos turnos en la misma sesión (prueba dura en `state.db`), notas en `<bóveda>/cerebro/` con hora local |
| 2 | El contexto lo arma la app (recuerdo dirigido), no el lienzo masticado | ✅ **hecho**: `cerebro::prompt_turno` (visión + top-6 BM25 + foco + contadores + puntero a las herramientas); la investigación comparte la misma sesión; el contexto usado se guarda en `delegacion.json` |
| 3 | Panel «Pensar desde el lienzo» (pedido, progreso, aterrizaje, contexto usado) | ⏳ pendiente |
| 4 | De `hermes -z` a `hermes serve`: delegar → habitar | ⏳ pendiente |

Extra hecho esa noche, por directiva del usuario (*«las investigaciones tienen que aportar información que
sirva para la construcción del cerebro/lienzo»*): la investigación aterriza en **un** nodo con la evidencia
adentro de su nota y colgado del Norte (antes dejaba 5-6 nodos de bibliografía suelta), y la síntesis tiene
**fallback declarado** (DeepSeek → Hermes) con el error visible en vez de morir en silencio.

---

## 1. Dónde estamos hoy (estado real, verificado)

### El puente existe, pero en un solo sentido y sin memoria
- **NodeFlow → Hermes:** `POST /api/ai/delegar` corre `hermes -z "<prompt>"` como subproceso sin consola
  (`voz::hermes_exe()`, tope 240 s, `NODEFLOW_HERMES` para mover el binario). **Es one-shot y sin
  continuidad**: cada delegación arranca sin recordar la anterior. Medido: una delegación real tardó 215 s.
- **Hermes → NodeFlow:** el servidor MCP propio (`mcp-server/nodeflow_mcp.py`, 22 herramientas) expone el
  lienzo, la bóveda, el jardín y los expertos. **Hoy está apagado** para Hermes por
  `mcp_servers.nodeflow.tools.exclude: ['*']` en `config.yaml` — por eso esta sesión tuvo que hablarle por
  stdio (y funcionó: así se creó y aprobó el nodo de rendimiento).

### Lo que la app ya tiene y sirve de base (no hay que inventarlo)
| Pieza | Estado |
|---|---|
| Cola de propuestas + HITL con auditoría | En producción: nada toca el lienzo sin aprobación; `origen: hermes` visible |
| Hilo de diálogo con foco | 12 turnos (se muestran 6), expira a los 30 min, vive en `<bóveda>/.nodeflow/dialogo.json` |
| Memoria de la bóveda | BM25 sobre todas las notas, con `limite` acotado a 1..40 |
| Memoria continua del proyecto | `GET /api/vault/memory` |
| Investigación por fases · expertos · borradores locales | En producción |
| Guardas de escritura | `base_revision` + rescate de escrituras del agente + reconciliación |
| Auto-actualización firmada | v0.3.5 publicada el 15/09 |

### Las puertas de Hermes (probadas hoy)
| Puerta | Qué es | Veredicto |
|---|---|---|
| `hermes -z "<prompt>"` | one-shot | **Sin estado.** Es lo que usa la app hoy |
| `hermes --resume <id> -z "<prompt>"` | reanuda una sesión **por id** | **Funciona**: reanudó una sesión real y respondió con su contenido |
| `hermes -c <nombre> -z "<prompt>"` | reanuda **por título** | Funciona si la sesión ya existe (`hermes sessions rename <id> <nombre>`). `--create-if-missing` **no existe** (el propio mensaje de error lo sugiere y es falso) |
| `hermes serve` | backend JSON-RPC/WebSocket (default `127.0.0.1:9119`), headless: **es el que usa la app de escritorio** | La puerta para ser una *superficie* de verdad: streaming, eventos de herramienta, aprobaciones |
| `hermes acp` | Agent Client Protocol (editores: VS Code / Zed / JetBrains) | Alternativa estándar, pero orientada a IDE: la app necesitaría un cliente ACP |
| `hermes mcp serve` | Hermes **como servidor MCP** | **Ojo, corrección:** expone 10 herramientas de **conversaciones** (`conversations_list`, `messages_read`, `messages_send`, `events_poll/wait`, `permissions_*`) — es un puente de mensajería, **no** ejecuta turnos del agente. No sirve para "que el modelo trabaje desde la app" |

---

## 2. La arquitectura propuesta: tres capas, una sola verdad

```
                 ┌─────────────────────────── NodeFlow (la verdad) ───────────────────────────┐
   contexto      │  bóveda .md (frontmatter)  ·  lienzo (nodos/aristas)  ·  hilo de diálogo    │
   lo arma la app│  pendientes (propuestas)   ·  memoria episódica (notas fechadas)           │
                 └───────────────┬──────────────────────────────────────────▲───────────────┘
                                 │ turno: pedido + top-k BM25 + foco          │ residuo: nodos,
                                 ▼                                            │ notas, decisiones,
                 ┌──────────────────────────── Hermes (el ejecutor) ──────────┴───────────────┐
   ejecución     │  sesión NOMBRADA y persistente  ·  herramientas (terminal, archivos, web)   │
   la hace Hermes│  skills (procedimiento)  ·  aprobaciones  ·  modelo con fallback local       │
                 └─────────────────────────────────────────────────────────────────────────────┘
```

**El cambio de fondo:** hoy el conocimiento vive en *esta* conversación y Hermes es el dueño del contexto.
En la arquitectura propuesta, **la app es dueña del contexto y del recuerdo**; Hermes es un ejecutor
stateless-ish con herramientas, y todo lo que produce vuelve al lienzo. Hermes conserva sus *skills*
(cómo se hace) pero **no** la memoria del proyecto (qué sabemos): eso es de la bóveda.

### Reglas de diseño (no negociables, son las que evitan el desastre)
1. **Una sola verdad:** la bóveda. Ni Hermes ni el estado en memoria de la app mandan por encima del `.md`.
2. **Toda escritura vuelve por la cola de propuestas** (ya construido). El agente propone, la persona aprueba.
3. **El turno no manda el lienzo:** manda el pedido + el top-k recuperado (BM25 ≤40 + foco + hilo). Medido:
   un pedido de lienzo son ~1.039 tokens hoy; mandar el grafo entero escala con los nodos y no aporta.
4. **El residuo se escribe siempre:** cada turno deja (a) un nodo o actualización si hubo decisión, y (b) una
   nota fechada en la bóveda con lo que pasó. Si no deja nada, el cerebro no creció: el turno se consideró vano.
5. **Sesión nombrada, no "la última":** la app guarda el nombre/id de la sesión del proyecto. `--resume latest`
   es una trampa: probado hoy, resumió *otra* conversación.

---

## 3. Fases (cada una verificable y reversible)

**Fase 0 · Habilitar las manos (30 min).** Sacar `exclude: ['*']` del MCP `nodeflow` en `config.yaml` (con
`hermes mcp configure`, no a mano) y reiniciar la app de Hermes. Criterio: las 22 herramientas aparecen en el
catálogo y `canvas_summary` responde sin harness.
*Decisión del usuario: sí/no.*

**Fase 1 · El corte vertical: memoria que sobrevive (medio día).** En el backend, `delegar` deja de ser
one-shot: guarda un nombre de sesión por proyecto (`nf-cerebro`) en `.nodeflow/` y llama
`hermes -c nf-cerebro -z ...`; al terminar, escribe el intercambio como **nota episódica fechada** en la bóveda
(con frontmatter: `tipo: turno`, `pedido`, `resultado`, `sesion`). Criterio: cerrar la app, reabrir, pedir
"seguí con lo de ayer" y que el pedido resuelva con el historial real. Sin la app abierta, `hermes -c nf-cerebro`
también debe poder continuar el hilo (probado que la continuidad por id/título funciona).

**Fase 2 · El contexto lo arma la app (1-2 días).** El prompt delegado se compone desde la bóveda:
top-k BM25 + foco del hilo + nodos vecinos + pendientes. Se expone en el panel para poder **ver qué se le mandó**
(transparencia: hoy el prompt es invisible). Criterio: pedir algo que requiera un nodo viejo y ver que lo trae
sin nombrarlo.

**Fase 3 · La boca en la app (2-3 días).** Panel «Pensar desde el lienzo»: se escribe el pedido ahí, se ve el
progreso (el mismo polling que ya usa `delegar`/investigación), y el resultado aterriza como nodos propuestos +
nota. Nada sale del lienzo. Criterio: un pedido real resuelto de punta a punta sin abrir esta chat.

**Fase 4 · De delegación a superficie (1-2 semanas, el salto grande).** Pasar de `hermes -z` a `hermes serve`
(JSON-RPC/WebSocket, `127.0.0.1:9119`): la app se conecta como cliente, como lo hace la app de escritorio de
Hermes. Se gana: streaming token a token, eventos de cada herramienta en vivo, aprobaciones nativas, turnos
largos sin el tope de 240 s, y multi-turno sin re-lanzar procesos. Criterio: un turno con 5 herramientas
mostrando cada paso en el panel, y el registro quedando en la bóveda.
*Esta es la fase que convierte "delegar" en "habitar". Las anteriores son útiles sin ella.*

**Fase 5 · La memoria del proyecto, derivada (continuo).** Cron de la app que, cada N horas o al cerrar una
sesión, sintetiza: «visión del proyecto» (nodo del Norte), decisiones tomadas, preguntas abiertas y deuda
técnica — regenerado **desde el grafo**, nunca escrito a mano. Las `references/` de los skills de Hermes pasan
a ser un resumen derivado de la bóveda (Hermes recuerda *cómo*, la bóveda *qué*).

---

## 4. Viabilidad: veredicto honesto

**Sí, es viable**, y no requiere investigación nueva: las tres cuartas partes están construidas (cola HITL,
bóveda como verdad, BM25, hilo, guardas, MCP en los dos sentidos). Lo que falta es **cableado y política**,
no tecnología. Esfuerzo realista: **Fases 0-3 en 3-5 días de trabajo**; la Fase 4 es la inversión grande
(1-2 semanas) y es la única que cambia la naturaleza del sistema.

### Riesgos y límites (los que pueden hacer fracasar esto)
1. **El tope de 240 s y el one-shot** son la jaula actual: mientras `delegar` sea un subproceso, el "habitante"
   es un visitante. Fase 1-3 lo hacen *útil*; Fase 4 lo hace *residente*.
2. **Costo y ruido por turno:** un turno con herramientas es 10-50× un pedido de lienzo. Sin la regla 3
   (contexto recuperado, no completo) el proyecto se vuelve caro justo cuando crece.
3. **La placa:** el "cerebro local permanente" (meta del usuario) no convive con ventanas de 16k: medido, 7B@16k
   = 426 s de TTFT. El diseño honesto es local para lo chico del bucle y nube para lo profundo, con el router
   que ya existe (`auto:tarea`).
4. **Proveedor inestable:** hoy DeepSeek no estuvo fino (errores de conexión y respuestas pobres). Cualquier
   diseño residente necesita **fallback declarado** (Hermes ya tiene `hermes fallback`): si el residente depende
   de un proveedor que falla, la app parece rota.
5. **Fricción de aprobación:** si todo es propuesta, el agente parece inútil; si nada lo es, el usuario pierde
   el control. Ya existe el perfil de autonomía aprendido (`/api/hitl/*`): la política de qué se auto-aprueba
   es una **decisión de producto**, no un detalle técnico.
6. **Doble escritor:** ya resuelto por `base_revision` + rescate + propuestas, pero hay que **no romperlo**: todo
   camino nuevo de escritura pasa por el mismo contrato.

---

## 5. Decisiones abiertas (para el usuario)
1. **¿Habilitamos las 22 herramientas MCP** del lienzo para Hermes (Fase 0)? Es la que más valor da por minuto
   invertido y es reversible en un comando.
2. **¿Qué se auto-aprueba?** Propuesta conservadora: el agente puede **crear notas y nodos nuevos** sin pedir,
   pero **modificar o borrar** lo existente siempre pide aprobación.
3. **¿Fase 4 sí o sí, o alcanza con Fases 1-3?** Las 1-3 dan un cerebro con memoria que vive en el lienzo. La 4
   agrega ver al agente trabajar en vivo y conversaciones largas de verdad.

---
*Borrador del 15/09/2026. Basado en lo verificado en esta sesión: continuidad por id/título (probada),
`--create-if-missing` inexistente (probado), `hermes mcp serve` = conversaciones y no ejecución (probado),
`hermes serve` = JSON-RPC/WebSocket :9119 (documentado en su `--help`), y los números de escala en
`references/rendimiento-y-escala.md` del skill `nodeflow-mcp`.*

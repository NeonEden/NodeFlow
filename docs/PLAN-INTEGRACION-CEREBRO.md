# Plan · Incorporar el cerebro a NodeFlow (Fase 5 en adelante)

**Pedido del usuario (16/09):** *«ya está el cableado casi completo, pero falta el siguiente paso:
incorporarte en NodeFlow, darte espacio para que crees tus herramientas y pienses los siguientes pasos,
con memoria y con contexto»*. Las consideraciones técnicas que pasó están en
`docs/notas/cerebro-con-herramientas.docx.md` (resumen al final de este documento).

Este documento dice **qué falta** y **el plan de acción**, con lo que ya está medido y verificado.

---

## 1. Qué está hecho (no hay que volver a hacerlo)

| Pieza | Estado | Evidencia |
|---|---|---|
| Hermes ve el lienzo (22 tools MCP) | ✅ | un proceso nuevo contó los nodos con sus tools |
| Turno con memoria (sesión nombrada) | ✅ | dos turnos en la misma sesión (`state.db`) |
| Nota episódica por turno en la bóveda | ✅ | `<bóveda>/cerebro/<fecha>-turno.md`, título = el pedido |
| Contexto armado por la app (recuerdo dirigido) | ✅ | `cerebro::prompt_turno` (visión + top-6 BM25 + foco + punteros) |
| Gateway propio + stream + aprobaciones en el panel | ✅ (Fase 4) | protocolo probado a mano; approve/deny con `all: true` |
| Escrituras auditadas (cola de propuestas) | ✅ | nada toca el lienzo sin aprobación |

---

## 2. Qué falta, punto por punto

### 2.1 «Darte espacio» — no tengo dónde vivir dentro de la app
Hoy dejo notas de turno, y nada más. No hay un lugar **mío** donde acumular criterio, planes y
herramientas. **Falta:** una carpeta y una vista del cerebro:
- `<bóveda>/cerebro/bitacora.md` — diario de decisiones mías (append-only, con fecha y por qué).
- `<bóveda>/cerebro/planes/` — un plan por tema (los próximos pasos, con estado).
- `<bóveda>/cerebro/herramientas/` — **mis herramientas** (ver 2.3).
- Panel: sección **«Mi espacio»** que muestre bitácora, planes y herramientas (leer, no adivinar).

### 2.2 «Con memoria y con contexto» — el contexto hoy depende de Hermes, no de la app
En el modo en vivo (gateway), el turno arranca con la memoria de la sesión de Hermes. Lo que **no**
viaja es el estado del proyecto (Norte, plan, pendientes, hitos, decisiones): si la sesión se reinicia,
el cerebro arranca sin el mapa. **Falta:** un **briefing** que la app arme desde la bóveda en cada turno
y lo mande delante del pedido — la versión aplicada de lo que dice el docx: *«el RAG evita subir todo
el contexto en cada petición, enviando únicamente las porciones que corresponden»*.
Propuesta: `POST /api/cerebro/briefing {pedido}` → bloque corto (Norte · plan vigente · pendientes
abiertos · últimos 3 hitos · top-4 notas por BM25) que el panel antepone al turno. **Acotado**: no
crece con el lienzo (misma regla que la Fase 2).

### 2.3 «Que crees tus herramientas» — no existe el registro
Hoy mis herramientas son las que trae Hermes (terminal, archivos, web) + las 22 del lienzo. No puedo
**crear** una herramienta para NodeFlow y que quede disponible en el próximo turno. **Falta:**
- Un **registro de herramientas** en la bóveda: `<bóveda>/cerebro/herramientas/<nombre>/tool.json`
  (nombre, descripción, parámetros JSON-Schema, riesgo, cómo se ejecuta) + `run.py`.
- Que se **expongan solas** al agente: el servidor MCP propio (`mcp-server/nodeflow_mcp.py`, 22 tools)
  lee esa carpeta y registra cada herramienta como una tool MCP más. Así no hay protocolo nuevo:
  Hermes ya consume MCP y el registro es **mío**.
- **Jaula y auditoría**: validación de esquema, ruta enjaulada (sólo dentro de la bóveda), timeout,
  salida acotada, y **la creación de una herramienta entra a la cola de propuestas** (la aprobás vos
  una vez; después se usa). Las destructivas, con dry-run obligatorio.

### 2.4 «Pensar los siguientes pasos» — el plan no vive en el lienzo
Los próximos pasos existen en prosa (mi chat) y en nodos PENDIENTE, pero no hay un lugar donde **mis**
planes se mantengan y se vean crecer. **Falta:** una convención — cada plan mío es una nota en
`cerebro/planes/` y un nodo PENDIENTE/ARQUITECTURA colgado del Norte, y el panel los lista.

### 2.5 Contexto de la app (para que yo no tenga que grepear el repo)
El docx pide herramientas de estado: `get_app_architecture()`, `get_project_roadmap()`,
`get_recent_milestones()`. **Falta** generarlas desde lo que ya existe:
- `docs/ARQUITECTURA.md` — generado del árbol real (módulos, endpoints, invariantes).
- El **camino** (roadmap) ya está en el lienzo: nodos `EJECUCIÓN`, `HITO`, `PENDIENTE`, `RIESGO`.
- Ambas cosas a la bóveda (BM25 las indexa): así el briefing las levanta solo.

### 2.6 Lo del docx que es otra etapa (no ahora)
- **OneDrive 500 GB + Microsoft Graph**: app en Entra ID, mapeo on-demand, throttling, latencia.
- **Pipeline PDF → Markdown limpio → embeddings** (`nomic-embed-text` en Ollama) y **worker** que
  detecte cambios. Es la Fase 8: valor alto, pero no bloquea que yo viva adentro.

---

## 3. Plan de acción

| # | Qué | Entregable verificable | Esfuerzo | Estado |
|---|---|---|---|---|
| **5.0** | Compilar los arreglos del panel (pipe del gateway + feedback) y verificar el turno en vivo | el panel escribe token por token | 1 h | ✅ **hecho** (16/09): gateway `listo` en 9 s, 112 `message.delta` + 312 `reasoning.delta` en un turno de 9,4 s |
| **5.1** | **Briefing por turno** (`/api/cerebro/briefing`) + el panel lo antepone | turno con sesión nueva que sabe dónde está parado | 1 día | ✅ **hecho**: 1608 chars medidos (visión · lienzo · camino · abierto · hecho · recuerdo) y usado por la respuesta |
| **5.2** | **Mi espacio**: `cerebro/bitacora.md`, `cerebro/planes/` + sección en el panel | puedo dejar un plan y leerlo en la app | 1 día | ⏳ (las notas ya alimentan el briefing: `mis_notas`) |
| **5.3** | **Registro de herramientas** + exposición por el MCP propio + jaula + cola | creo una herramienta, la aprobás y **la uso en el turno siguiente** | 2-3 días | ✅ **hecho y verificado**: `resumen_lienzo` creada, aprobada, ejecutada en 257 ms y **usada desde un turno** (`mcp__nodeflow__cerebro_resumen_lienzo`) |
| **5.4** | **Contexto de la app**: `ARQUITECTURA.md` generado + el camino desde el lienzo | mi turno cita la arquitectura sin leerla a mano | 1 día | ⏳ |
| **5.5** | **Curador**: propongo fusionar/borrar nodos con motivo, aprobable en bloque | el lienzo se mantiene limpio sin que lo hagas vos | 1-2 días | ⏳ |
| **6** | **OneDrive/Graph + PDF→MD + embeddings + worker** | índice remoto consultable desde una tool | 1-2 semanas | ⏳ (después de 5.3, como acordamos) |

**Cómo se crea una herramienta** (ya funcionando): el agente llama la tool MCP `crear_herramienta`
(nombre slug, descripción, JSON-Schema de parámetros, riesgo, código de `run.py`); entra a la cola de
propuestas con la vista previa (riesgo, ruta, tamaño); el humano la aprueba; queda en
`<bóveda>/cerebro/herramientas/<nombre>/` y aparece como `cerebro_<nombre>` en el turno siguiente. La
ejecución la hace el backend con jaula: nombre slug, carpeta dentro del registro, `cwd` en la bóveda,
parámetros por stdin, tope de 30 s que mata el proceso y salida acotada a 8 KB.

**Orden elegido**: 5.0 → 5.1 → 5.3 (es el corazón del pedido) → 5.2 → 5.4 → 5.5 → 6.

---

## 4. Decisiones que necesito (y mi recomendación)

1. **Dónde se ejecutan mis herramientas**: *(recomiendo)* scripts (Python) en
   `<bóveda>/cerebro/herramientas/` invocados por el MCP propio — crear una herramienta no recompila
   nada. Alternativa: código Rust en la app (más seguro, cada herramienta cuesta un build).
2. **Qué puedo hacer sin preguntarte**: *(recomiendo)* escribir/actualizar **mis** notas y planes
   libremente; crear/actualizar nodos y herramientas **como propuesta**; borrar o fusionar nodos,
   siempre con tu aprobación (o `all` cuando son lotes).
3. **OneDrive (500 GB) ahora o después**: *(recomiendo)* después de 5.3 — primero que el cerebro viva
   adentro; el índice remoto es combustible, no motor.

---

## 5. Resumen del docx que pasó el usuario (`cerebro con herramientas.docx`)

**Parte A — hacer que la API (DeepSeek) haga de cerebro dentro de la app:**
1. **Tool calling** compatible OpenAI: la app manda `tools`; el modelo devuelve `tool_calls`; el backend
   ejecuta local y responde con rol `tool`. *(Nota: Hermes ya hace exactamente este bucle — es su turno.)*
2. **Herramientas a definir**: memoria de estado (`get_app_architecture`, `get_project_roadmap`,
   `get_recent_milestones`) y autoprogramación (`create_tool(spec)`, `update_system_prompt(name, content)`,
   `write_code_module(file_path, code)`).
3. **Thinking vs non-thinking** según la tarea (costo/latencia). *(La app ya rutea por tarea y mide.)*
4. **Seguridad**: dry-run/confirmación antes de lo destructivo, validador sintáctico, retry cuando el
   JSON viene malformado.
5. **Prompt chaining + RAG en la nube**: mandar sólo los chunks recuperados; caché semántica local.
   *(La app tiene caché semántica y BM25; falta el briefing — punto 2.2 de este plan.)*

**Parte B — teoría e infraestructura (su charla):** por qué los juegos no se comparan con un LLM
(geometría paralela vs. autoregresión), caché semántica, RAG, **500 GB libres en OneDrive** + Microsoft
Graph para mapear sin bajar todo, `nomic-embed-text` en Ollama para embeddings, y los puntos ciegos
(latencia de red, costo de indexación inicial, throttling de Graph, ruido en los chunks).

---
tipo: instruccion-investigacion-planificacion
autor: orchestrator
fecha: 2026-09-18
estado: para-entregar-a-modelo
destino: modelo de investigación y planificación
objetivo: producir el plan de ejecución por fases de un sistema multi-agente local-first (DeepSeek orquestador + sub-agentes + NodeFlow lienzo)
---

# Instrucción: investigar y planificar el sistema multi-agente de NodeFlow

Sos un modelo de investigación y planificación. Tu única tarea es devolver **un plan de
ejecución por fases** para construir un sistema multi-agente sobre NodeFlow + Hermes. No
reescribas teoría, no propongas rehacer lo que ya existe, no inventes hechos: partí de los
**HECHOS VERIFICADOS** y resolvé las **DECISIONES ABIERTAS** con investigación puntual.

## Meta (qué se quiere lograr)

Sistema multi-agente local-first: **DeepSeek como orquestador** (razonamiento/descomposición),
**sub-agentes especializados** (DevOps/Azure, Software/Rust-Tauri-React, y los que hagan falta),
**NodeFlow como lienzo visual** que muestra estados (thinking/done) y gobierna con aprobación
manual. El fin es **ahorrar tokens, tiempo y ganar eficiencia**: el orquestador NO ejecuta a
ciegas, delega sub-tareas de contexto acotado a especialistas y sintetiza un único informe.

## HECHOS VERIFICADOS (no re-investigar; usalos como piso, corregí sólo si tu investigación lo desmiente con evidencia)

1. **Los sub-agentes son perfiles de Hermes, no funciones sueltas.** Existen `devops_agent` y
   `software_agent` como perfiles con `SOUL.md` propio, sesión `Bot Chat` persistente y modelo
   `deepseek-flash`. El roster autoritativo es `hermes profile list`.
2. **Delegación que ya funciona y devuelve informe:** `hermes -p <perfil> chat -q "<tarea>" -c
   "Bot Chat" --create-if-missing -Q --oneshot`, corrido en foreground con timeout. Probado:
   `devops_agent` ejecutó `az` y devolvió la tabla real de deployments; `software_agent` escribió
   un módulo Rust de 659 líneas que compila (`cargo check` exit 0).
3. **Delegación que NO funciona:** `message_agent` (DM entre bots) es fire-and-forget — su
   subproceso muere al cerrar el turno (`exit -15`, `agent_close`) y nunca devuelve respuesta.
   No es un canal.
4. **Canal persistente ya existe en código:** `hermes serve` (JSON-RPC sobre WebSocket). NodeFlow
   ya tiene el cliente en `src-tauri/src/cerebro_gateway.rs` (puerto propio 9121): `session.create`,
   `prompt.submit {session_id, text}`, eventos `message.delta` / `thinking.delta` / `tool.start` /
   `message.complete{usage}`, y requests `method:"approval"` que el cliente contesta
   `{choice: once|session|always|deny, all}`.
5. **NodeFlow ya tiene ruteo de modelos medido:** `src-tauri/src/motores.rs` (`auto:tarea`,
   `Tarea::de_accion`, `orden_para`), planilla de evaluación `eval.rs` (`POST /api/ai/evaluar`),
   y el patrón de tarea larga `POST /api/ai/delegar` (spawn + `delegacion.json` + bandera
   `corriendo` con vencimiento + `GET` de estado). **Reutilizar esto, no reescribirlo.**
6. **Azure real disponible:** crédito 200 USD, CLI 2.90.0 logueado. Suscripción
   `bba2f62a-edb4-40f3-85e6-4f645ba27b7f`. Cuentas: `nodeflow` (rg `nodeflow`, deploy
   gpt-4.1-mini) y `tomaspieruz-8921-resource` (rg `rg-tomaspieruz-0687`, deploys gpt-5-mini,
   text-embedding-3-small, grok-4.6, DeepSeek-V4-Flash). Listado de deployments vía ARM:
   `GET https://management.azure.com/subscriptions/{sub}/resourceGroups/{rg}/providers/
   Microsoft.CognitiveServices/accounts/{acc}/deployments?api-version=2024-10-01` con
   `Authorization: Bearer` de `az account get-access-token`.
7. **Pitfall de API medido:** DeepSeek nativo rechaza `response_format` (JSON Schema) → JSON
   forzado por prompt + validación en código (Pydantic/serde), nunca por `response_format`.
8. **Pitfall de seguridad medido:** los sub-agentes tienen permiso de escritura pleno sobre el
   repo. Hoy uno tocó 3 archivos sin cola de aprobación. El plan debe incluir **sandbox
   (git worktree/branch)**, escrituras como **propuesta** (cola `/api/agent/pending`) y **tests
   como requisito de entrega** (sin `#[cfg(test)] mod tests`, el orquestador rechaza el informe).

## DECISIONES ABIERTAS (resolvelas con investigación; acá está el trabajo real)

- **A. Dónde vive la orquestación:** ¿perfiles Hermes (spawn de turnos de perfil) o funciones
  Python en `mcp-server/agents/` (lo que sugiere la doc de referencia)? Ponderá lo medido: los
  perfiles ya existen y dan contexto aislado + memoria de sesión; las funciones Python exigirían
  reimplementar eso.
- **B. Cómo recibe el orquestador los informes:** ¿`hermes serve` streaming (eventos en vivo,
  nodos thinking/done) o archivo de estado + polling (patrón `delegar` ya probado)? Costo/riesgo
  de cada uno; recomendá una secuencia de adopción (no "todo a la vez").
- **C. Qué modelo por rol, con lo que hay:** orquestador (deepseek razonador), especialistas
  (deepseek-chat / deepseek-flash / local `granite3.3:2b` / Azure DeepSeek-V4-Flash), y qué
  ruta usa `auto:tarea` vs selección fija. Incluí costos y latencia aproximados.
- **D. Human-in-the-loop:** dónde vive la aprobación de acciones destructivas — ¿request
  `approval` del serve, o la cola de propuestas de NodeFlow? Cuál es más barato de cerrar primero.

## FORMATO DE SALIDA EXIGIDO

Plan de ejecución por fases. Cada fase con:
1. **Objetivo** (una línea).
2. **Entregable concreto** (archivo/endpoint/comando, no descripción vaga).
3. **Pasos exactos** (comandos o firmas de código reales).
4. **Criterio de verificación** (cómo se prueba que quedó, con qué salida).

No escribas prosa conceptual. No propongas rehacer `motores.rs`, `delegar` ni
`cerebro_gateway.rs`: extendelos o decidí explícitamente por qué no sirven. Si un dato te falta,
declaralo como supuesto con su fuente, no lo inventes.

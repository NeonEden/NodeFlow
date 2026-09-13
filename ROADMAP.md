# Roadmap

**v0.1.0 — MVP (actual, septiembre 2026).** Orquestador híbrido funcionando en uso diario: canvas de grafo, borradores con modelo local validados por código, contabilidad de costo y caché por artefacto, corridas de experto, diagnóstico del grafo, integración con Obsidian, propuestas con aprobación explícita.

## v0.2 — Infraestructura de inferencia explícita
- **Motor de Condensación (Fase A ✓)** — Norte Estratégico persistente + condensación de una selección en un macro-nodo con linaje guardado (poda sin pérdida) y restauración del sub-grafo. Pendiente Fase B (condensación total con los 3 pases) y Fase C (descarte socrático de lo ya resuelto).
- Runtime local **independiente**: el modelo local corre dentro de la app, sin depender de un agente externo.
- **Interruptor Local / Cloud visible** por tarea, con costo y latencia esperados a la vista antes de ejecutar.
- **Streaming (SSE)** de tokens hacia el nodo activo.
- Endpoints de nube intercambiables (cualquier API compatible con OpenAI), con tarifas declaradas en `costo.rs`.

## v0.3 — Co-agente de voz
- Entrada de voz (STT) para dictar y ver el grafo crecer en vivo.
- Respuesta por voz (TTS) opcional para el diálogo de propósito ("¿cuál es la hipótesis de esta red de ideas?").
- El co-agente pide el objetivo de una investigación y reorganiza las aristas del grafo según la jerarquía de pensamiento.

## v0.4 — Control móvil human-in-the-loop
- Servidor WebSocket en el backend nativo, expuesto por túnel privado.
- Panel móvil: asignar tareas a la PC, recibir notificaciones y **autorizar** escrituras o ejecuciones.
- Cola de tareas con estados y auditoría.

## v0.5 — Pipeline de ingesta y síntesis
- Recolector local: búsqueda web con modelo liviano, filtrado y recolección cruda.
- Sintetizador: conceptos clave → estructura JSON → nodos y aristas automáticos en el canvas.
- Inyección en la bóveda con frontmatter, manteniendo la privacidad del contenido.

## v1.0 — Distribución
- Instaladores firmados y *updater*.
- Builds para macOS y Linux.
- Onboarding y configuración de proveedores desde la UI (sin tocar archivos).

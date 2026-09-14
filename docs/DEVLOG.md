# Bitácora de desarrollo

Generado desde el historial de git (Conventional Commits). No se edita a mano.

`96` commits · rama `main` · actualizado 2026-09-14 14:09

Últimos 14 días:

## 2026-09-14 · 16 commit(s) · +1503 −121

**Nuevas capacidades**

- taxonomia geometrica de 5 estados + busqueda Tavily + tags del nucleo limpios

**Correcciones**

- proponer una sola vez por corrida (y limpiar el ruido de «Actualizar»)
- los hallazgos entran como propuestas del agente, no directo al lienzo
- la investigación se ve y se aplica sin depender del panel de voz
- rechazar ids de motor que no existen
- el saneo tambien limpia aristas repetidas, no solo colgadas
- luz de borde en las formas recortadas

**Rendimiento**

- el prompt se ordena para el cache de prefijo y la app mide el ahorro

**Documentación**

- las cuatro capas del mapa del proyecto y la regla de sincronizacion con la evidencia
- Tavily verificado (la clave responde con el texto completo de las paginas) y tutorial corregido
- como funciona NodeFlow de punta a punta, con la tabla honesta de limites

**Infraestructura**

- v0.3.0

**Guardado automático**

- 4 punto(s) de seguridad sin mensaje propio (omitidos de la bitácora)

## 2026-09-13 · 62 commit(s) · +11025 −6343

**Nuevas capacidades**

- el nodo que crece por fases (semilla, friccion, capsula, hexagono)
- accion actualizar (mutar un nodo que ya existe) y tolerancia a la clave mal pegada
- una conversacion en curso manda el pedido a un modelo a la altura
- hilo de conversacion — de comandos sueltos a conversacion
- motor por tipo de tarea, con lo local como camino principal
- NodeFlow puede delegar en Hermes y usar sus herramientas
- bucle hablado — Kokoro local y voz selectiva decidida en código
- Fase A — Norte Estratégico + macro-nodos con linaje (poda sin pérdida)
- automatic profile learning, honest wording and readable panel
- global engine selector - one place decides where the AI runs
- per-task local/cloud mode with measured cost and cache trace

**Correcciones**

- el ganador por prueba comparaba ms contra tokens
- medir al modelo real y no a la cache
- tags duplicados y contraste de texto, con auditor
- el comando 'enfocar' enfoca de verdad (y no colapsa el lienzo por sorpresa)
- the automatic pass uses the selected engine, not a cloud key
- the panel footer no longer claims an Express backend
- light form fields in the add-API panel and a creator credit
- make local engines usable, messages honest and engines honest too
- retry the API bind instead of opening a window without API

**Reorganización**

- one place per action - toolbar reorganized into clusters and menus

**Documentación**

- planilla medida y sus conclusiones
- notas de v0.1.1 con el Motor de Condensación y los SHA-256
- bitácora de la Fase A
- notas de v0.1.0 listas para publicar
- add README (EN/ES), roadmap, ADRs, MIT license and real env example

**Infraestructura**

- v0.2.0
- limpieza automática de target/debug (los tests lo inundaban)
- fuera el backend Express del prototipo (server.ts)
- limpieza de Dependabot
- v0.1.1
- unignore src/data, sync lock and add CI
- merge the v0.1 native rewrite onto the NeuralMind prototype history
- fix identity - nodeflow-desktop 0.1.0, MIT

**Guardado automático**

- 24 punto(s) de seguridad sin mensaje propio (omitidos de la bitácora)

## 2026-09-12 · 15 commit(s) · +37588 −500

## 2026-09-11 · 1 commit(s) · +1781 −216

**Nuevas capacidades**

- add rotatable idea template packs

## 2026-09-09 · 2 commit(s) · +10558 −11

**Nuevas capacidades**

- initialize NeuralMind interactive node map


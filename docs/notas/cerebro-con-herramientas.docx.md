# Notas de Tomas · «cerebro con herramientas» (16/09/2026)

Material que pasó el usuario para la integración del cerebro. Se guarda acá para que el plan
(`docs/PLAN-INTEGRACION-CEREBRO.md`) sea autocontenido y para que la bóveda pueda recordarlo.

## A. Integrar la API (DeepSeek) como orquestador dentro de la app

1. **Protocolo de Tool Calling (ejecución agéntica).** Compatible OpenAI: la app envía la consulta con un
   arreglo `tools`; si el modelo deduce que hay que actuar, no devuelve texto sino `tool_calls` con
   argumentos en JSON; el backend intercepta, ejecuta local y devuelve el resultado con rol `tool` para
   que el modelo siga razonando (*agent loop*).
2. **Definición y alcance de las herramientas.** Memoria de estado: `get_app_architecture()`,
   `get_project_roadmap()`, `get_recent_milestones()`. Autoprogramación con E/S controlada:
   `create_tool(spec)`, `update_system_prompt(name, content)`, `write_code_module(file_path, code)`.
3. **Modos de razonamiento.** *Thinking* para tareas complejas (reestructurar la app, generar
   herramientas); sin razonamiento extenso para memoria/consulta rápida (menos costo y latencia).
4. **Bucle de seguridad (sandbox).** Dry-run / confirmación en la UI para lo destructivo, validador
   sintáctico antes de aplicar, y *retry loop* devolviéndole el error a la API cuando el JSON viene
   malformado o faltan argumentos.
5. **Encadenamiento de prompts y RAG en la nube.** No subir todo el código/contexto en cada petición:
   mandar sólo los chunks recuperados que corresponden a la función a ejecutar. La caché semántica se
   mantiene local para no llamar a la API cuando la instrucción es idéntica a una ya ejecutada.

## B. Teorías e información sin procesar (de su charla)

- **Falla de lógica percibida:** los juegos mueven gigas de texturas/shaders y no exigen la placa;
  la inferencia de un LLM hace girar los ventiladores con una consulta simple.
- **Premisa técnica:** por qué varios modelos locales en paralelo no son eficientes (conmutación de
  contexto, VRAM/RAM). ¿Se pueden usar archivos estáticos o precalculados, como en los juegos, para que
  el modelo no calcule todo desde cero?
- **Recursos:** ~500 GB libres en OneDrive sin usar; aprovechar la nube sin ocupar disco local.
- **Hipótesis:** usar ese almacenamiento para alimentar un modelo local eficiente en RAM/VRAM, con caché
  semántica y documentación técnica verificada (PDFs → Markdown).

## C. Resumen procesado

- **Juegos vs. LLM:** los juegos reocupan geometría y texturas ya procesadas en paralelo; un LLM ejecuta
  iterativamente billones de operaciones por **cada** palabra (autoregresión).
- **Caché semántica:** reutilizar respuestas previas para consultas conceptualmente idénticas.
- **RAG:** separa el «conocimiento» de la «capacidad de razonamiento»; el modelo busca en un índice
  externo en vez de recordarlo todo.

## D. Conclusiones y métodos de acción

- **Indexación remota (RAG + OneDrive):** registrar una app en Microsoft Entra ID (gratis para devs) para
  usar la API de Microsoft Graph y mapear los archivos **on demand**, sin clonar 500 GB.
- **Conocimiento verificado:** PDFs → Markdown (Pandoc o plugins de Obsidian); embeddings con un modelo
  ligero (`nomic-embed-text` por Ollama).
- **Optimización local:** delegar el almacenamiento pesado a la nube e indexar sólo vectores chicos.

## E. Puntos ciegos que él mismo marcó

- **Latencia de red** (Graph API): si el RAG depende de la red para cada chunk, la respuesta es lenta.
- **Costo de indexación inicial:** convertir mucho texto a vectores exige tiempo intensivo de CPU/GPU.
- **Rate limit / throttling** de Graph al leer miles de archivos de golpe.
- **Fragmentación y ruido en los chunks:** cabeceras y números de página bajan la precisión del RAG.

## F. Hoja de ruta que propuso

- [ ] **Prompt chaining:** toda consulta pasa primero por RAG/caché, se valida que la información sea
      sólida y recién después se estructura la respuesta (evita alucinaciones).
- [ ] **Tooling / function calling:** los scripts de búsqueda de Graph, como herramientas que el modelo
      llame solo cuando el contexto lo pida.
- [ ] **Autonomía:** un worker en segundo plano que detecte cambios en OneDrive y actualice el índice.
- [ ] **Pipeline de ingesta:** PDF → Markdown limpio → embeddings, optimizado para la bóveda de Obsidian.

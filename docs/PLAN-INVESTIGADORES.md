# Investigadores: repartir el trabajo pesado entre motores

Anotado el 19/09/2026, después de revisar qué hay construido. **No es un plan de multiagentes**: es
usar la costura que la app ya tiene, con los motores que ya tienen clave en el llavero.

## Lo que ya existe (medido, no supuesto)

`src-tauri/src/investigacion.rs` corre una investigación en cuatro fases y **ya reparte el trabajo
entre dos motores**:

| Fase | Qué pasa | Quién lo hace hoy |
|---|---|---|
| 🌱 Semilla | nace el nodo de la investigación | La app |
| ⚔️ Fricción | fuentes reales, una por nodo | **Hermes** sale al mundo + `buscar_con_tavily` |
| 🧪 Cápsula | síntesis y poda | **DeepSeek** (`sintetizar`) |
| 🚀 Hexágono | cristaliza en la bóveda | La app |

El resultado vuelve como **comandos en el mismo formato que el plan de voz** (`comandos_de_fuentes`,
`comandos_de_sintesis`) y los aplica la app con deshacer disponible. La costura para meter un motor
nuevo ya está: es `correr_hermes` / `correr_deepseek` (`investigacion.rs`) y el catálogo de
`motores.rs`.

Estado de claves en el llavero (`GET /api/claves/estado`, sólo huellas): DeepSeek ✓, **Gemini ✓**,
Speechmatics ✓, AssemblyAI ✓, **Tavily ✗ ausente**.

> **El agujero concreto:** la fase de Fricción —la que consigue fuentes reales— depende de Tavily, y
> no hay clave. Sin ella, esa fase no puede cumplir lo que promete.

## El reparto que tiene sentido

| Rol | Motor | Por qué | Estado |
|---|---|---|---|
| **Investigador** (fuentes con URL) | **Gemini** con *Google Search grounding* | la clave ya está en el llavero; devuelve resultados con URL y cita, y no cuesta una suscripción | a conectar |
| **Recaudador / segunda opinión** | **Copilot** (GitHub Models, token de `gh`) | ya se usa Copilot como destino de artefactos (`artefactos.rs` genera `prompt_para_copilot`); acá se le puede pedir el material crudo o el contraste | a decidir el canal |
| **Sintetizador** | **DeepSeek** | ya funciona: 5/5 en la planilla, centavos | hecho |
| **Salida al mundo** | **Hermes** CLI | ya funciona (`correr_hermes`) | hecho |

Regla que no se toca: **el modelo propone, el código valida** (ADR 0005). Un investigador devuelve
`{titulo, url, cita, quien}`; la app valida que la URL sea real y que la cita exista antes de crear el
nodo. Nada de «fuentes» inventadas: una fuente sin URL no entra al lienzo.

## Primer paso concreto

1. `investigacion.rs`: agregar `correr_gemini_busqueda` (rol *investigador*) usando
   `generativelanguage` con `google_search` como herramienta, y usarlo en la fase Fricción **antes**
   de Tavily y de Hermes: si Gemini trae fuentes con URL, esas son las que se cuelgan del nodo.
2. `claves.rs`: sumar `github_token` (o leer el token de `gh auth token`) para el rol Copilot.
3. Panel: mostrar **quién trajo cada fuente** (chip con el motor), como ya se muestra el motor de voz.

## Decidido (19/09/2026)

- **Copilot → GitHub Models** con el token de `gh` (`gh auth token`): API HTTP, se llama desde Rust,
  no hace falta instalar ni suscribir nada. El rol es *recaudador / segunda opinión*: contraste del
  material y armado del artefacto, que es lo que hoy se pide a mano (`prompt_para_copilot`).
- **Gemini → por investigación completa**: una tanda de consultas al empezar la investigación y las
  fuentes se reparten entre los nodos de la fase Fricción. No una búsqueda por idea (gastaría una
  llamada por nodo). El reparto lo hace `comandos_de_fuentes`, que ya existe.

## Preguntas abiertas

- ¿El tope de consultas por investigación? Propuesta: 3 (una por tema del pedido), configurable.

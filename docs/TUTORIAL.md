# NodeFlow — cómo funciona, de punta a punta

*Guía operativa y de evaluación. Todo lo que dice acá está medido en esta máquina o verificado contra el código.*

## 1. Qué es

Un **lienzo de co-creación**: pensás en voz alta o escribís, y un agente interpreta (no transcribe) para
**crear, conectar, enfocar, criticar y condensar nodos**. El modelo **propone**; el código **valida**; vos
**aprobás**. Esa es la regla que ordena todo lo demás.

## 2. Arrancarlo

| Pieza | Cómo | Verificación |
|---|---|---|
| La app | `NodeFlow.exe` (instalada en `%LOCALAPPDATA%\Programs\NodeFlow`) | si abre con el lienzo vacío, mirá el log: `%LOCALAPPDATA%\com.nodeflow.desktop\logs\NodeFlow.log` |
| Backend | se levanta con la app, en `127.0.0.1:37371` | `GET /api/health` → `status: ok` |
| Modelos locales | Ollama en `127.0.0.1:11434` | `granite3.3:2b` (rápido) · `deepseek-r1:7b` (profundo) · `qwen2.5vl:7b` (ve imágenes) |
| Voz (salida) | Kokoro local, `tools/tts/servidor.py` | `GET /api/voz/estado` → `disponible: true` |

**Regla de la placa**: un modelo cargado por vez (`OLLAMA_MAX_LOADED_MODELS=1`). Correr tres en paralelo pide
12,8 GB en una placa de 12 GB: medido, no es opinión.

## 3. El lienzo

- **Nodos** = conceptos. **Aristas** = vínculos con una etiqueta.
- **La forma dice el estado** (taxonomía geométrica) y **muta sola** al subir de fase:

| Estado | Forma | Qué significa |
|---|---|---|
| 🌱 Semilla | círculo de borde discontinuo | una intuición, todavía sin trabajar |
| ⚔️ Fricción | octágono | la sometiste a contraste |
| 🧪 Probada | cápsula | resistió y quedó sintetizada |
| 💎 Axioma | bloque de bordes nítidos | la das por cierta |
| 🚀 Artefacto | hexágono | ya es entregable (nota en la bóveda) |

- **Todo nodo guarda**: título, descripción, categoría, etiquetas y fase. El **núcleo** no se puede borrar.

## 4. El circuito básico

1. **Decís o escribís** (panel de voz o el campo de texto).
2. El agente devuelve un **plan de operaciones** y una **tarjeta de impacto** ("crearía 2 nodos, conectaría 1").
3. **Aprobás** → se aplica al lienzo. Nada toca tu trabajo sin ese paso. `Ctrl+Z` deshace.

Las operaciones son **datos** (`src-tauri/specs/actions.json`), no código: agregar una acción no toca el motor.

## 5. Qué hace cada acción

| Acción | Qué hace |
|---|---|
| `crear` | nodos nuevos colgando de un padre |
| `enlazar` | conecta dos nodos con etiqueta |
| `enfocar` | deja en foco **exactamente** los nodos elegidos (no expande vecinos) |
| `condensar` | funde varios nodos en un macro-concepto, con **linaje restaurable** (badge ◈ N) |
| `criticar` | abre el flujo socrático sobre los nodos elegidos |
| `actualizar` | muta un nodo existente (fase, etiquetas, descripción) |
| `delegar` | le pasa el pedido a **Hermes**, que sale a la web con sus herramientas |

En la **barra del nodo** (al seleccionarlo): **Ramificar** (3 ramas) · **Explorar** (viabilidad y riesgos) ·
**Crítica** (abogado del diablo) · **Socrático** (preguntas que destraban).

**El jardín** (`Panel Jardín`) diagnostica el grafo sin tocarlo: huérfanos, islas, aristas repetidas, nodos
sin madurez. Sus hallazgos se convierten en **propuestas** para aprobar. **Reacomodar** ordena el árbol por
niveles sin solapamientos.

## 6. Investigación por fases

Dictás *"investigá X"* y el nodo crece solo: **🌱 Semilla** (nace al instante) → **⚔️ Fricción** (fuentes
reales, un nodo por fuente) → **🧪 Cápsula** (síntesis y poda) → **🚀 Hexágono** (cristaliza con su nota).
Corre de fondo: la ventana nunca se congela. Medido: **5 fuentes reales en 87 s**, una del EPA.

## 7. Los motores

- **`auto:tarea`** (recomendado) elige según lo que pedís, y **el ganador medido va primero**.
- Medición real de esta máquina:

| Motor | Aciertos (5 tareas) | Tiempo | Costo |
|---|---|---|---|
| `deepseek-r1` (nube, razona) | **5/5** | 6,4 s | centavos |
| `deepseek-chat` (nube) | 4/5 | **2,7 s** | centavos |
| `granite3.3:2b` (local) | 3/5 | 3,7 s | US$0 |
| `deepseek-r1:7b` (local) | 4/5 | 14,9 s | US$0 |

- **Los locales son la demostración** de que el diseño no depende de la nube; en una PC con más recursos
  rinden mejor. La nube es la red de seguridad, no el camino principal.
- **Ahorro medido**: los prompts están ordenados para el caché de prefijo del proveedor. Resultado real:
  de **256 a 1.152** tokens de entrada servidos desde caché (**57-58%** a precio de caché).

## 8. La voz

- **Entrada**: Speechmatics (token temporal firmado por el backend; la clave nunca sale de ahí).
- **Salida**: **Kokoro local** — verificado: 104 KB de WAV en 1,9 s.
- **Habla selectiva**: sólo avisa hallazgos y contradicciones; cuando actúa, actúa en silencio. El **mute**
  se respeta y persiste.

## 9. El motor profundo

`delegar` corre una pasada completa de Hermes con sus herramientas (búsqueda web incluida) **sin bloquear**
(0,0 s contra los 215 s de antes) y la respuesta llega sola al panel. Medido: una investigación real de 28 s
que devolvió un sensor vigente, su cumplimiento REACH/RoHS y dos alternativas.

## 10. Seguridad y gobernanza

- Toda escritura del agente es **propuesta** hasta que la aprobás (`mode: apply` es explícito).
- El **núcleo** no se borra. Las aristas rotas o repetidas se rechazan al escribir.
- Las **claves** viven en `%APPDATA%\com.nodeflow.desktop\nodeflow.config.json` (o en variables de entorno)
  y nunca vuelven al frontend ni al repositorio.
- El autoguardado **no guarda mientras compila**.

## 11. Lo que funciona, lo que es demostración y lo que falta

La parte más útil para nosotros: acá están los puntos ciegos, no las virtudes.

| Función | Estado real |
|---|---|
| Crear/conectar/enfocar/condensar/criticar/actualizar | ✅ funciona, verificado por API y en la app |
| Investigación por fases | ✅ funciona (87 s, 5 fuentes reales) |
| Formas por estado y mutación | ✅ funciona (mide el DOM: sin recortes) |
| Jardín: diagnóstico y propuestas | ✅ funciona (el saneo limpia colgadas **y** repetidas) |
| Voz entrada/salida | ✅ funciona (WAV real; el token de Speechmatics se firma) |
| Delegar a Hermes | ✅ funciona sin bloquear |
| Tavily con texto real de las páginas | 🟡 implementado; **necesita tu clave** para citar en vez de recordar |
| Escalada a la nube | 🟡 corre, pero los modelos `-cloud` del daemon devuelven HTTP 402 (sin créditos) |
| Caché semántico | ❌ falta (hoy hay caché **exacta**: 20.519 tokens evitados medidos) |
| Escritura de streaming (partial JSON) | ❌ falta: el primer nodo brota al terminar la fase, no antes |
| Semiótica de aristas (ámbar = contradicción, *marching ants* al dictar) | ❌ falta |
| Contraste fino (chips de 9-10 px del sidebar) | 🟡 24 violaciones restantes (peor 2,17) |
| Autoguardado | 🟡 se adelanta y commitea cambios que no escribió (mensaje genérico) |
| Selección múltiple real | 🟡 el usuario debe probarla una vez antes de grabar (no se puede simular) |

## 12. Cómo mostrarlo (guion de 3 tomas que se ven solas)

1. **El nodo que investiga**: dictás *"investigá sensores de humedad de suelo"* y el nodo nace, se llena de
   fuentes y **cambia de forma** al subir de fase — sin tocar nada.
2. **El jardín**: un clic en el diagnóstico y las propuestas aparecen (23 aristas repetidas, nodos huérfanos)
   → aprobás → la salud del grafo sube.
3. **La voz**: hablás, la tarjeta de impacto aparece, aprobás, y Kokoro dice el hallazgo.

---
*NodeFlow · MIT · por TOMAS.WAV*

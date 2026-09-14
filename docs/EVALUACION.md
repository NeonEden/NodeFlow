# Planilla de evaluación — medir en vez de suponer

Cada motor corre **las tareas reales del lienzo** por el mismo camino que usa la app (su spec, su
validación, su caché —salteada a propósito— y su contabilidad de costo). El veredicto lo da **código**,
no la opinión de un modelo sobre otro: ¿el plan sirvió, cuánto tardó, cuánto costó?

- Lienzo de la corrida: **15 nodos**.
- Cinco pruebas: `voz-enfocar`, `voz-crear`, `voz-delegar`, `condensar`, `braindump`.
- Todo **local y US$0**: no gasta plan de tokens. Un motor por vez, y se descarga de la VRAM al terminar.

| Motor | Aciertos | Tiempo medio | Tokens | Costo |
|---|---|---|---|---|
| `deepseek-r1:7b` | **4/5** | 15.2 s | 9436 | US$0 |
| `granite3.3:2b` | **3/5** | 4.3 s | 8308 | US$0 |

### Prueba por prueba

| Prueba | deepseek-r1:7b | granite3.3:2b | Qué mide |
|---|---|---|---|
| `voz-enfocar` | ✓ 22.4 s | ✓ 9.4 s | entiende una orden de limpieza (un solo `enfocar`) |
| `voz-crear` | ✓ 16.2 s | ✗ 2.6 s | suma ideas nuevas sin destruir el lienzo |
| `voz-delegar` | ✗ 13.1 s | ✗ 2.2 s | reconoce cuándo hay que ir al motor profundo |
| `condensar` | ✓ 13.0 s | ✓ 2.3 s | condensa respetando el contrato (título, descripción, match 0..1) |
| `braindump` | ✓ 11.2 s | ✓ 5.1 s | descompone una idea en un mapa con raíz y ramas |

### Qué dice esto (y qué no)

- **El modelo chico gana el bucle rápido**: `granite3.3:2b` promedia **4,3 s** contra **15,2 s** del R1, y
  en tres de las cinco pruebas es el único que llega en tiempo razonable. Para hablar con el lienzo,
  el local chico alcanza.
- **Donde el chico falla, el grande sirve**: `voz-crear` (sumar ideas nuevas) lo resolvió sólo
  `deepseek-r1:7b` — 2 nodos con título. Es el candidato natural para "pensar despacio".
- **Ningún modelo local reconoce cuándo delegar** (0 comandos en los dos, tres corridas seguidas).
  Esa decisión necesita el motor de nube: es evidencia, no corazonada.
- **Costo real de operar el lienzo en tu máquina: US$0** en las diez corridas.

Límites honestos: cinco pruebas dan una **señal**, no un veredicto; el veredicto es binario (¿cumple el
contrato?) y no mide calidad subjetiva. Y las pruebas que fallan en los dos motores dicen tanto del
límite del modelo como del de la tarea.

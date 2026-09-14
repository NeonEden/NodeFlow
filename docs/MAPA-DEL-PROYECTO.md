# Mapa del proyecto NodeFlow

> **Qué es esto**: la estructura con la que el lienzo de NodeFlow refleja el estado real del proyecto
> y permite dirigir el progreso. No es documentación *sobre* la app: es el esquema *del* lienzo.

## Las cuatro capas

| Capa | Qué guarda | Cómo se reconoce |
|---|---|---|
| **N0 · Norte** (1 nodo) | La tesis y el objetivo | Único nodo raíz real de todo el grafo |
| **N1 · Pilares** (8) | Las capacidades que perduran | Categorías estables: NÚCLEO · DATOS · UX · ARQUITECTURA · GOBERNANZA · SEGURIDAD · SISTEMAS · VISIÓN |
| **N2 · Capacidades** | Fases y features concretas | La madurez dice cuánta evidencia tienen |
| **N3 · Evidencia** | Investigaciones y fuentes | Categorías INVESTIGACIÓN y FUENTE, colgando de la capacidad que alimentan |
| **N4 · Movimientos** | **Lo que dirige** | PENDIENTE · DECISIÓN · RIESGO · HITO |

## La madurez es evidencia, no entusiasmo

| Nivel | Forma | Significa |
|---|---|---|
| 1 | 🌱 Semilla | Dicho, sin contrastar |
| 2 | ⚔️ Fricción | Contrastado: hay una fuente, un dato o una prueba externa |
| 3 | 🧪 Cápsula | Probado: funciona en una corrida real |
| 4 | 💎 Axioma | Sostenido: verificado y en uso |
| 5 | 🚀 Hexágono | Artefacto: entregado o publicado |

## Las aristas tienen semántica

`contiene` (Norte → pilar) · **`requiere`** (capacidad → capacidad: la dependencia) ·
**`bloquea`** (riesgo → capacidad) · `entrega` (capacidad → hito) ·
`alimenta` (fuente → investigación → capacidad) · `decide` (decisión → capacidad)

**Con eso, «¿qué sigue?» deja de ser intuición**: es el nodo con más `requiere` sin cumplir y más
`bloquea` encima. Eso es el camino crítico.

## La regla que lo mantiene vivo

**El mapa se genera de la evidencia; no se mantiene a mano.** Igual que la bitácora sale de git
(`scripts/devlog.sh`), un `scripts/mapa.py` lee:

- `git log` (qué se cerró, con qué mensaje) · las releases publicadas
- `docs/*.md` (el estado declarado) · el skill del proyecto (los pendientes elegidos)
- la cola de propuestas y los artefactos (`evaluacion.json`, `investigacion.json`)

…y emite **propuestas** (`POST /api/graph/node` / `/api/graph/edge`) que el humano aprueba. Corre en
el mismo checkpoint de 5 minutos que ya existe: así el lienzo nunca queda viejo y el estado actual es
una **consulta**, no un recuerdo.

## Estado al 14/09/2026 (lo que el mapa dice hoy)

- **Columna vertebral**: 15 nodos con categoría y madurez reales.
- **Escombros**: 11 nodos de un lote socrático/telemetría, sin madurez y sin dueño — se reenganchan a
  la capacidad que preguntan (telemetría, provenance) y se recategorizan como ANÁLISIS.
- **Movimientos sembrados**: 6 pendientes, 2 riesgos, 2 decisiones, 2 hitos.
- **Dependencias**: 5 aristas `requiere` reales.

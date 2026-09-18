---
titulo: Plan de Ejecución del Sistema Multi-Agente (v2 — corregido)
fecha: 2026-09-18
version: 2
tags:
  - nodeflow
  - hermes
  - multi-agente
  - azure
  - arquitectura
  - obsidian
estado: listo-para-ejecutar
correcciones_vs_v1: duplicación de endpoints, bug Windows de Command::new, flags de hermes serve, comando de test
---

# Plan de Ejecución: Sistema Multi-Agente (NodeFlow + Hermes) — v2

> **v2 corrige 4 errores de la v1** que habrían costado trabajo y frustración. Cada corrección
> cita el código real del repo. Nada acá es teórico: todo lo marcado como «ya existe» se verificó
> el 2026-09-18.

## Correcciones aplicadas sobre la v1

| # | Error de la v1 | Corrección (evidencia) |
|---|---|---|
| 1 | Proponía crear `src-tauri/src/agent_pending.rs` + endpoint `/api/agent/pending` nuevos | **Ya existe**: `GET /api/agent/pending` (`server.rs:303`), `POST /api/agent/approve` (`:330`), `POST /api/agent/reject` (`:331`) + el módulo `agente.rs`. **Extender la cola, no crearla.** |
| 2 | `Command::new("hermes")` en `hermes_cli_bridge.rs` | Falla en Windows con **WinError 2** (Rust no resuelve PATHEXT; mismo caso que `az`). Usar `crate::voz::hermes_exe()`, que ya resuelve el binario (`voz.rs:30`). |
| 3 | `hermes serve --port 9121` | Falta `--skip-build`: sin él el serve intenta compilar la web UI con npm y **cuelga en headless**. El repo ya usa `serve --port <n> --skip-build` (`cerebro_gateway.rs::argv`). |
| 4 | «`cargo check` y `cargo test` devuelven exit 0» | `cargo test` a secas falla («could not find Cargo.toml»). Es **`cargo test --manifest-path src-tauri/Cargo.toml --lib`**. |

## Estado de partida verificado (18/09/2026)

- Rama `main`; único worktree: `Desktop/NodeFlow/nodeflow-desktop`.
- Perfiles Hermes vivos: `devops_agent`, `software_agent` (SOUL propio, sesión `Bot Chat`, `deepseek-flash`). Roster autoritativo: `hermes profile list`.
- Delegación que **funciona**: `hermes -p <perfil> chat -q "<tarea>" -c "Bot Chat" --create-if-missing -Q --oneshot` (foreground, con timeout).
- Delegación que **NO funciona**: `message_agent` (muere con el turno, `exit -15`).
- **Pendiente de limpiar**: `src-tauri/src/azure.rs` (+ `lib.rs`/`server.rs` modificados) está **sin commitear** en el working tree desde la delegación de hoy. El checkpoint de 5 min lo va a tragar en un `chore(checkpoint)`. **Commitear con mensaje propio o mover al worktree ANTES de la Fase 1.**

---

## Fase 1: Sandbox de seguridad y aislamiento de ejecución

### 1. Objetivo
Que ningún sub-agente altere el repositorio principal sin aprobación explícita y tests en verde.

### 2. Entregable concreto
- Worktree aislado (comando, sin script nuevo).
- **Extensión** de la cola de propuestas existente: tipo `archivo` en el módulo `agente.rs` / los handlers `agent_pending` / `agent_approve` de `server.rs`. **No** crear `agent_pending.rs`.

### 3. Pasos exactos

**3.1 — Aislar el entorno (worktree):**
```bash
cd "C:/Users/tomas/Desktop/NodeFlow/nodeflow-desktop"
git worktree add -b agent/execution-sandbox ../nodeflow-sandbox main
cd ../nodeflow-sandbox && git status          # debe estar limpio
```
Los sub-agentes trabajan en `../nodeflow-sandbox`. El checkpoint del repo principal no lo toca.

**3.2 — Extender el tipo de propuesta (NO un endpoint nuevo).**
La cola ya tiene tipos (nodo, fusionar, herramienta). Agregar `archivo` a la variante
existente en `agente.rs`, con estos campos:
```rust
// Variante NUEVA dentro del enum/struct de propuestas que ya existe en agente.rs
// (no un módulo nuevo):
PropuestaArchivo {
    id: String,
    agente: String,             // "software_agent" | "devops_agent"
    archivo_destino: String,    // ruta RELATIVA al worktree; validar con camino-jaula
    contenido_propuesto: String,
    diff: String,               // para revisión humana
    test_status: Option<bool>,  // None = no corrió; true = check+test en verde
}
```
- Reusar la validación de camino que ya existe (la misma familia que `ruta_nota_valida()` en `vault.rs`):
  nada absoluto, nada con `..`, nada fuera del worktree.
- El endpoint `GET /api/agent/pending` ya lo expone automáticamente al serializar la cola.

**3.3 — Regla de entrega (el freno automático):**
El informe de `software_agent` **solo** se procesa si, dentro de `../nodeflow-sandbox`:
```bash
cargo check --manifest-path src-tauri/Cargo.toml --message-format short
cargo test  --manifest-path src-tauri/Cargo.toml --lib
```
…y **ambos devuelven exit 0**. Y **sin `#[cfg(test)] mod tests`** en el módulo nuevo, la
propuesta se marca `test_status: None` y el orquestador la rechaza (es la regla que el crate ya
sigue: 34 módulos con tests).

### 4. Criterio de verificación
Pedirle a `software_agent` una modificación simulada sobre `../nodeflow-sandbox`. Verificar:
1. `git status` en `nodeflow-desktop` **sin cambios** en los archivos objetivo.
2. `GET /api/agent/pending` devuelve el objeto con `tipo: "archivo"`, su `diff` y `test_status: true`.
3. Aprobar con `POST /api/agent/approve {id}` y confirmar que recién ahí el archivo se escribe.

---

## Fase 2: Delegación por perfiles + ruteo de modelos (polling)

### 1. Objetivo
Integrar la invocación de perfiles Hermes con el ruteo `auto:tarea` y el patrón `/api/ai/delegar`.

### 2. Entregable concreto
- `src-tauri/src/hermes_perfil.rs`: **una** función de invocación, construida sobre lo que ya existe.
- Ruteo de perfiles en `src-tauri/src/motores.rs`.

### 3. Pasos exactos

**3.1 — Invocación correcta (los 4 puntos que la v1 omitía):**
```rust
use std::os::windows::process::CommandExt;   // #[cfg(windows)] — el repo ya lo hace en agente.rs

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Corre un turno de un perfil Hermes y devuelve su informe.
/// Bloqueante a propósito: se llama desde spawn_blocking.
fn turno_perfil(perfil: &str, prompt: &str, tope_s: u64) -> Result<String, String> {
    let exe = crate::voz::hermes_exe();      // (1) NO "hermes" pelado → WinError 2
    let mut cmd = std::process::Command::new(exe);
    cmd.args([
            "-p", perfil, "chat",
            "-q", prompt,
            "-c", "Bot Chat",
            "--create-if-missing",
            "-Q", "--oneshot",
        ])
        .creation_flags(CREATE_NO_WINDOW)     // (2) sin consola parpadeando
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let salida = cmd.output().map_err(|e| e.to_string())?;
    if !salida.status.success() {
        return Err(String::from_utf8_lossy(&salida.stderr).to_string());
    }
    // (3) la salida trae una línea `session_id:` — hay que sacarla
    let texto = String::from_utf8_lossy(&salida.stdout).to_string();
    Ok(crate::cerebro::limpiar_salida(&texto))
}
```
Y llamarlo **siempre** desde `tokio::task::spawn_blocking` (como `investigacion.rs:461`), nunca
`.output()` directo sobre el runtime async. Aplicar `--run-budget` o timeout por reloj.

**3.2 — Ruteo en `motores.rs` (extender, no reescribir):**
`motores::Tarea` + `orden_para()` ya resuelven qué modelo usar. Agregar el mapeo de **perfil**:

| Rol | Perfil | Modelo | Ruta |
|---|---|---|---|
| Orquestador | (no es perfil) | `deepseek-chat` (ver Fase 4) | API DeepSeek directa |
| DevOps / Azure | `devops_agent` | `deepseek-flash` | `hermes_exe` + CLI |
| Software / Rust | `software_agent` | `deepseek-flash` | `hermes_exe` + CLI |
| Sintaxis trivial | (ninguno) | `granite3.3:2b` | Ollama local `:11434` |

**3.3 — Larga duración:** usar el patrón que ya existe (`POST` arranca + bandera `corriendo`
con **vencimiento** + `GET` de estado + archivo JSON). **Nunca** sostener la petición HTTP durante
un turno de minutos.

### 4. Criterio de verificación
`POST /api/ai/delegar` con una tarea de lectura de infraestructura → el estado pasa a `corriendo`,
se crea el JSON, y al terminar el resultado vino de `devops_agent` (no del orquestador). Y: con
`tope` de 1 vuelta, el corte se declara explícitamente en vez de fallar en silencio.

---

## Fase 3: Streaming y WebSocket en tiempo real

### 1. Objetivo
Que el lienzo pinte `thinking` / `done` y la aprobación en vivo.

### 2. Entregable concreto
Extensión del cliente ya existente `src-tauri/src/cerebro_gateway.rs` (no reescribirlo).

### 3. Pasos exactos

**3.1 — Levantar el serve con los flags REALES:**
```bash
hermes serve --port 9121 --skip-build        # sin --skip-build cuelga compilando la web UI
```
En el código, el comando ya está armado en `cerebro_gateway.rs::argv()` — usarlo, no duplicarlo.

**3.2 — Canalizar los eventos que el protocolo ya emite:**
- `thinking.delta` → el nodo pasa a **Thinking…**
- `tool.start` → se pinta la arista activa hacia el nodo del sub-agente (`devops_agent` / `software_agent`)
- `tool.complete` / `message.complete{usage}` → el nodo pasa a **Done**
- request `method:"approval"` → modal del lienzo; contestar `{choice: once|session|always|deny, all}`

**3.3 — Trampa conocida:** un `hermes serve` huérfano de una corrida anterior **sobrevive a un cierre
forzado**, sigue escuchando y contesta health → el panel recibe 403 con el token nuevo. Por eso
`cerebro_gateway.rs` ya elige un puerto realmente libre (`puerto_libre()`) y compara tokens con un
handshake real. **Respetar ese camino**, no saltearlo.

### 4. Criterio de verificación
Orden compleja desde NodeFlow → el log del gateway muestra `message.delta` y el lienzo se actualiza
**sin bloquear** la UI. Y un `serve` ya corriendo en 9121 no rompe el arranque (se elige otro puerto).

---

## Fase 4: Bucle de control y consolidación del orquestador

### 1. Objetivo
Pipeline final: límite de iteraciones, JSON validado en código, y síntesis en un único informe.

### 2. Entregable concreto
- `prompts/orchestrator_system.md`
- Validador de salida en el servidor (serde en Rust).

### 3. Pasos exactos

**3.1 — El prompt del orquestador (sin `response_format`: DeepSeek nativo lo rechaza):**
```markdown
Devolvés SIEMPRE JSON puro, sin texto alrededor, con esta estructura exacta:
{
  "plan": ["paso 1", "paso 2"],
  "delegaciones": [
    {"agente": "devops_agent",  "instruccion": "..."},
    {"agente": "software_agent","instruccion": "..."}
  ],
  "informe_final": "..."
}
Reglas:
- Solo podés delegar a perfiles que existan en `hermes profile list`.
- Cada `instruccion` es autocontenida y acotada: contexto verificado + formato de entrega.
- No ejecutes trabajo de un especialista vos mismo.
- Máximo 3 iteraciones. Si no cerrás, devolvé el estado y `informe_final` explicando el bloqueo.
```

**3.2 — Validación en código, no en el prompt:**
El parser valida estructura y tipos; si falla, **reintenta hasta 3 veces** y después **declara el
fallo** (no lo maquilla). Regla del repo: *el modelo propone, el código valida el contrato*.

**3.3 — Elección de modelo del orquestador (decidir con medición, no por fama):**
`deepseek-reasoner` (R1) **no soporta function-calling** y emite `reasoning_content` + CoT largo →
más latencia y costo. Como el paso 3.1 usa JSON forzado por prompt, **R1 no aporta** acá. Empezá con
`deepseek-chat` y medí; pasá a un razonador solo si la descomposición falla seguido.

**3.4 — Cierre:** el informe sintetizado se materializa como nota en la bóveda (vía
`POST /api/cerebro/espacio/nota`, **no** escribiendo el `.md` a mano) y como nodo completado en el
lienzo.

### 4. Criterio de verificación
Solicitud completa que combine Azure + módulo Rust → todo corre en el worktree, los tests pasan,
las escrituras piden aprobación humana, y el informe ejecutivo estructurado aterriza en NodeFlow
con fuentes y sin nodos sueltos (`canvas_stats` → `aristas_colgadas: 0`).

---

## Reglas que no se negocian (aprendidas hoy, con evidencia)

1. **Verificá el roster antes de delegar**: `hermes profile list`. Un handle que no está ahí, no tiene destino.
2. **`message_agent` no es un canal**: dispara y muere con el turno. Para recibir informes, `hermes serve` o subproceso foreground acotado.
3. **Tests como requisito de entrega**, no como cortesía: sin `mod tests`, el informe se rechaza.
4. **Nada de escrituras directas al repo principal**: worktree + cola de propuestas.
5. **`Command::new("azar"|"hermes")` en Windows = WinError 2.** Siempre `.exe`/`.cmd` con ruta real + `CREATE_NO_WINDOW`.
6. **Ninguna credencial a la respuesta ni al log.** En el config o el llavero.

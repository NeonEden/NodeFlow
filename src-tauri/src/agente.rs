//! Fase «sin Hermes» — **el bucle de agente propio de NodeFlow** (etapas 1 y 3 del plan
//! `cerebro/planes/nodeflow-sin-hermes.md`).
//!
//! Hasta acá el razonamiento con herramientas vivía en un subproceso a Hermes (`cerebro::argv` →
//! `hermes chat -q … --oneshot`) y en el servidor MCP. Este módulo corta esa dependencia por dos
//! piezas que van juntas:
//!
//! 1. **El bucle multi-turno** (`correr`): manda las herramientas al proveedor compatible con OpenAI,
//!    ejecuta los `tool_calls` que devuelve, realimenta los resultados y repite hasta que el modelo
//!    responde sin pedir nada más — con tope de vueltas, de tiempo y de tamaño de cada salida.
//! 2. **Las herramientas de repo con jaula** (`ejecutar`): leer con rango, listar, buscar, firmas y
//!    correr comandos de una **lista blanca**. Son de **sólo lectura**: en esta etapa el agente mira y
//!    verifica, no escribe (la escritura con aprobación es la etapa 4).
//!
//! Dos decisiones que hacen que esto sea verificable y no una promesa:
//!
//! - El bucle **no sabe de HTTP**: recibe `llamar` (messages, tools) → `Result<Value, String>`. Por eso
//!   sus tests corren con un modelo de mentira que devuelve `tool_calls` a medida, sin red ni claves.
//! - La jaula es **pura y testeada** (`ruta_enjaulada`): nada de `..`, nada de absolutas, nada fuera de
//!   la raíz del repo, y una lista de nombres prohibidos (`clave-*.txt`, `.env`, el config del usuario).
//!   El agente puede leer el código; no puede leer las llaves del usuario.
//!
//! Medido el 17/09 contra la API real: DeepSeek acepta `tools` y devuelve `finish_reason: tool_calls`
//! (sin eso habría chat, no agente). El `temperature` se omite a propósito: los razonadores rechazan
//! cualquier valor que no sea el default con HTTP 400.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// Vueltas máximas del bucle (una vuelta = una llamada al modelo). Es el freno del turno.
pub const TOPE_PASOS: usize = 12;
/// Tope de tiempo del turno completo, en segundos.
pub const TOPE_S: u64 = 300;
/// Tope de tiempo de un comando suelto (`correr`), en segundos.
pub const TOPE_CMD_S: u64 = 180;
/// Tope de lo que se le devuelve al modelo por herramienta: entra en el contexto que se paga.
pub const TOPE_SALIDA: usize = 8_000;
/// Presupuesto de **entrada** por vuelta, en tokens estimados (≈4 caracteres por token). Es el freno del
/// contexto, que es lo que de verdad se paga: medido el 17/09, un turno de 19 herramientas mandó 90.594
/// tokens de entrada e **igual cerró**, así que el volumen no compraba nada — sólo ruido y factura.
pub const PRESUPUESTO_ENTRADA: usize = 48_000;
/// Cuántos resultados de herramienta quedan **enteros** al podar. El último par que el modelo acaba de
/// pedir nunca se recorta: sin eso el modelo no puede razonar sobre lo que pidió hace un segundo.
pub const CONSERVAR_ULTIMAS: usize = 3;
/// Cuántos caracteres se guardan de una salida de **una sola línea** demasiado larga (un `listar`, por ej.).
const TOPE_LINEA: usize = 400;
/// Marca de una salida ya recortada. Es lo que hace la poda **idempotente y monótona**: una salida
/// podada no se vuelve a tocar ni se re-expande, así el prefijo sigue siendo idéntico entre vueltas y la
/// caché del proveedor (medida: 80,8 % de la entrada) sigue pegando. Podar distinto en cada vuelta sería
/// bajar tokens para pagarlos a precio pleno.
const MARCA_PODA: &str = "… (recortado por presupuesto: ";

/// Tope de vueltas del recorrido del repo (un árbol con `node_modules` no se recorre entero nunca).
const TOPE_ARCHIVOS: usize = 4_000;

/// Carpetas que no se recorren: son ruido y pesan (el repo tiene `target` de GB).
const SALTAR: [&str; 8] = [
    "node_modules",
    "target",
    ".git",
    ".vercel",
    "dist",
    "public",
    "__pycache__",
    ".venv",
];

/// Nombres que **nunca** se leen, ni listan, ni buscan: son las llaves de la máquina, no el código.
const PROHIBIDOS: [&str; 9] = [
    ".env",
    "clave-",
    ".key",
    ".pem",
    "id_rsa",
    "id_ed25519",
    "credentials",
    "nodeflow.config.json",
    ".ssh",
];

/// Comandos que el agente puede correr. **Lista blanca por prefijo**: no hay shell libre. Todo lo que
/// no empiece con uno de estos rebota con la lista a la vista (y eso también es información útil).
pub const COMANDOS: [&str; 12] = [
    "cargo test --manifest-path src-tauri/Cargo.toml --lib",
    "cargo test --lib",
    "cargo check",
    "cargo build",
    "npx tsc --noEmit",
    "npm run lint",
    "git status",
    "git diff",
    "git log",
    // La forma que el agente intenta de verdad cuando quiere evitar el paginador (medido: la pidió
    // sola en su primer turno). Sin ella, el rechazo le cuesta una vuelta.
    "git --no-pager log",
    "git --no-pager diff",
    "git --no-pager status",
];

/// Ajustes del turno. Prioridad: el cuerpo del pedido → `nodeflow.config.json → agente{...}` → default.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub tope_pasos: usize,
    pub tope_s: u64,
    pub tope_cmd_s: u64,
    /// ¿Puede correr comandos de la lista blanca? Apagarlo deja al agente en sólo-lectura pura.
    pub comandos: bool,
    /// Presupuesto de entrada por vuelta, en tokens estimados. Al pasarlo, el historial se poda.
    pub presupuesto_entrada: usize,
    /// Resultados de herramienta que quedan enteros al podar (el par reciente nunca se toca).
    pub conservar_ultimas: usize,
    /// Raíz del repo. Sin ella no hay herramientas de código (el turno queda sin manos).
    pub repo: PathBuf,
    /// Carpeta de datos de la app: ahí vive la cola de parches que el agente propone.
    pub estado: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            tope_pasos: TOPE_PASOS,
            tope_s: TOPE_S,
            tope_cmd_s: TOPE_CMD_S,
            comandos: true,
            presupuesto_entrada: PRESUPUESTO_ENTRADA,
            conservar_ultimas: CONSERVAR_ULTIMAS,
            repo: PathBuf::new(),
            estado: PathBuf::new(),
        }
    }
}

impl Config {
    pub fn desde(cfg: Option<&Value>, repo: PathBuf, estado: PathBuf) -> Config {
        let mut c = Config {
            repo,
            estado,
            ..Config::default()
        };
        if let Some(s) = cfg {
            if let Some(v) = s.get("tope_pasos").and_then(|v| v.as_u64()) {
                c.tope_pasos = (v as usize).clamp(1, 40);
            }
            if let Some(v) = s.get("tope_s").and_then(|v| v.as_u64()) {
                c.tope_s = v.clamp(30, 1800);
            }
            if let Some(v) = s.get("tope_cmd_s").and_then(|v| v.as_u64()) {
                c.tope_cmd_s = v.clamp(10, 600);
            }
            if let Some(v) = s.get("comandos").and_then(|v| v.as_bool()) {
                c.comandos = v;
            }
            if let Some(v) = s.get("presupuesto_entrada").and_then(|v| v.as_u64()) {
                c.presupuesto_entrada = (v as usize).clamp(4_000, 200_000);
            }
            if let Some(v) = s.get("conservar_ultimas").and_then(|v| v.as_u64()) {
                c.conservar_ultimas = (v as usize).clamp(1, 12);
            }
        }
        c
    }
}

/// Una herramienta ejecutada, con lo que devolvió. Es lo que el panel muestra paso a paso: la
/// diferencia entre «el agente dijo algo» y «el agente hizo algo verificable».
#[derive(Clone, Debug, PartialEq)]
pub struct Paso {
    pub herramienta: String,
    pub argumentos: Value,
    pub salida: String,
    pub ok: bool,
    pub ms: u64,
}

impl Paso {
    /// Una línea para el log y para el panel.
    pub fn resumen(&self) -> String {
        format!(
            "{} {} · {} ms · {}",
            if self.ok { "ok" } else { "error" },
            self.herramienta,
            self.ms,
            self.salida
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(80)
                .collect::<String>()
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// La jaula
// ─────────────────────────────────────────────────────────────────────────────

/// Resuelve una ruta **relativa** contra la raíz del repo y verifica que quede adentro.
///
/// Rechaza: absolutas, cualquier `..`, nombres prohibidos (llaves del usuario) y —cuando el archivo
/// existe— cualquier resultado que al canonicalizar se escape de la raíz. La comprobación de nombres se
/// hace sobre **cada** segmento, así `src/../clave-x.txt` no pasa por la puerta de atrás.
pub fn ruta_enjaulada(raiz: &Path, rel: &str) -> Result<PathBuf, String> {
    let limpio = rel.trim().replace('\\', "/");
    if limpio.is_empty() {
        return Err("falta la ruta".into());
    }
    if limpio.starts_with('/') || limpio.contains(':') {
        return Err(format!("«{rel}» no es una ruta relativa al repo"));
    }
    let mut destino = raiz.to_path_buf();
    for parte in limpio.split('/').filter(|p| !p.is_empty() && *p != ".") {
        if parte == ".." {
            return Err(format!("«{rel}» intenta salir del repo"));
        }
        let minuscula = parte.to_ascii_lowercase();
        if PROHIBIDOS
            .iter()
            .any(|p| minuscula.starts_with(p) || minuscula == *p)
        {
            return Err(format!(
                "«{parte}» está en la lista de archivos que el agente no lee (llaves y configuración)"
            ));
        }
        destino.push(parte);
    }
    if let Ok(real) = destino.canonicalize() {
        let raiz_real = raiz
            .canonicalize()
            .map_err(|e| format!("raíz ilegible: {e}"))?;
        if !real.starts_with(&raiz_real) {
            return Err(format!("«{rel}» apunta fuera del repo"));
        }
    }
    Ok(destino)
}

/// ¿Un comando está en la lista blanca? Se compara por prefijo para permitir filtros de test
/// (`cargo test … --lib mi_test`) sin abrir la puerta a nada más.
pub fn comando_autorizado(comando: &str) -> bool {
    let c = comando.trim();
    COMANDOS
        .iter()
        .any(|p| c == *p || c.starts_with(&format!("{p} ")))
}

fn se_salta(nombre: &str) -> bool {
    let n = nombre.to_ascii_lowercase();
    SALTAR.contains(&n.as_str()) || n.starts_with('.') && n != "."
}

/// Recorre el repo devolviendo rutas relativas, saltando lo pesado y lo que no es código legible.
fn recorrer(raiz: &Path, desde: &Path, filtro_ext: Option<&str>, tope: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pila = vec![desde.to_path_buf()];
    let mut vistos = 0usize;
    while let Some(dir) = pila.pop() {
        let Ok(entradas) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entradas.filter_map(|e| e.ok()) {
            vistos += 1;
            if vistos > TOPE_ARCHIVOS || out.len() >= tope {
                return out;
            }
            let p = e.path();
            let nombre = e.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                if !se_salta(&nombre) {
                    pila.push(p);
                }
                continue;
            }
            if se_salta(&nombre) || nombre.to_ascii_lowercase().starts_with("clave-") {
                continue;
            }
            if let Some(ext) = filtro_ext {
                let ext = ext.trim_start_matches('.');
                let ok = p
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.eq_ignore_ascii_case(ext))
                    .unwrap_or(false);
                if !ok {
                    continue;
                }
            }
            if let Ok(rel) = p.strip_prefix(raiz) {
                out.push(rel.to_path_buf());
            }
        }
    }
    out.sort();
    out
}

fn ext_de(nombre: &str) -> String {
    Path::new(nombre)
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

// ─────────────────────────────────────────────────────────────────────────────
// Las herramientas
// ─────────────────────────────────────────────────────────────────────────────

/// Las definiciones que ve el modelo (formato `tools` de OpenAI).
pub fn definiciones(comandos: bool) -> Vec<Value> {
    let fn_def = |nombre: &str, desc: &str, props: Value, req: Vec<&str>| {
        json!({
            "type": "function",
            "function": {
                "name": nombre,
                "description": desc,
                "parameters": { "type": "object", "properties": props, "required": req }
            }
        })
    };
    let mut t = vec![
        fn_def(
            "listar",
            "Lista un directorio del repo (una ruta relativa; vacío = raíz). Devuelve nombres, con / al final para carpetas.",
            json!({ "ruta": { "type": "string", "description": "Ruta relativa, ej. src-tauri/src" } }),
            vec![],
        ),
        fn_def(
            "leer_archivo",
            "Lee un archivo de código del repo, con números de línea. Usá desde/lineas para traer sólo el tramo que necesitás (no pidas el archivo entero si no hace falta).",
            json!({
                "ruta": { "type": "string", "description": "Ruta relativa al repo" },
                "desde": { "type": "integer", "description": "Primera línea (1 por defecto)" },
                "lineas": { "type": "integer", "description": "Cuántas líneas (por defecto 120, máximo 400)" }
            }),
            vec!["ruta"],
        ),
        fn_def(
            "firmas",
            "Devuelve las firmas (fn/struct/impl/trait en Rust; export/function/interface en TS) de un archivo o de una carpeta, con su línea. Es el corte de contexto barato: mirá las firmas antes de leer el cuerpo.",
            json!({ "ruta": { "type": "string", "description": "Archivo o carpeta relativa al repo" } }),
            vec!["ruta"],
        ),
        fn_def(
            "buscar",
            "Busca un texto en el código del repo (case-insensitive) y devuelve archivo:línea: contenido. Filtro opcional por extensión (rs, ts, tsx, mjs).",
            json!({
                "texto": { "type": "string", "description": "Texto a buscar" },
                "extension": { "type": "string", "description": "Ej. rs · ts · tsx" },
                "tope": { "type": "integer", "description": "Máximo de resultados (por defecto 30, máximo 80)" }
            }),
            vec!["texto"],
        ),
    ];
    t.push(fn_def(
        "proponer_parche",
        "Propone un cambio en el código del repo: una lista de ediciones ancladas. NO escribe nada: la propuesta queda esperando la aprobación del humano, que después de aplicarla corre los tests. Copiá el texto de `buscar` EXACTAMENTE como lo leíste (con su indentación); si el ancla no está o aparece más de una vez, la propuesta rebota. Máximo 4 archivos.",
        json!({
            "motivo": { "type": "string", "description": "Por qué este cambio, en una frase" },
            "ediciones": {
                "type": "array",
                "description": "Una por cambio; `buscar` es el texto exacto a reemplazar",
                "items": {
                    "type": "object",
                    "properties": {
                        "ruta": { "type": "string", "description": "Ruta relativa al repo" },
                        "buscar": { "type": "string", "description": "Texto exacto que hay hoy" },
                        "reemplazar": { "type": "string", "description": "Texto que queda" },
                        "todos": { "type": "boolean", "description": "true = reemplazar todas las apariciones" }
                    },
                    "required": ["ruta", "buscar", "reemplazar"]
                }
            }
        }),
        vec!["motivo", "ediciones"],
    ));
    if comandos {
        t.push(fn_def(
            "correr",
            "Corre un comando de la lista blanca en la raíz del repo y devuelve su salida (tests, chequeo de tipos, git en modo lectura). Lista permitida: cargo test --manifest-path src-tauri/Cargo.toml --lib · cargo check · cargo build · npx tsc --noEmit · npm run lint · git status · git diff · git log.",
            json!({ "comando": { "type": "string", "description": "El comando completo, tal cual" } }),
            vec!["comando"],
        ));
    }
    t
}

/// Ejecuta una herramienta. Los errores **no** cortan el turno: vuelven como texto para que el modelo
/// pueda corregir y seguir (un agente que muere al primer error no sirve para trabajar).
pub fn ejecutar(raiz: &Path, nombre: &str, args: &Value, cfg: &Config) -> Result<String, String> {
    match nombre {
        "listar" => {
            let rel = args.get("ruta").and_then(|v| v.as_str()).unwrap_or("");
            let dir = if rel.trim().is_empty() {
                raiz.to_path_buf()
            } else {
                ruta_enjaulada(raiz, rel)?
            };
            let entradas = std::fs::read_dir(&dir)
                .map_err(|e| format!("no pude listar «{}»: {e}", rel.trim()))?;
            let mut lineas: Vec<String> = entradas
                .filter_map(|e| e.ok())
                .filter(|e| !se_salta(&e.file_name().to_string_lossy()))
                .map(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    if e.path().is_dir() {
                        format!("{n}/")
                    } else {
                        n
                    }
                })
                .collect();
            lineas.sort();
            if lineas.is_empty() {
                return Ok(format!(
                    "(vacío): {}",
                    if rel.trim().is_empty() { "." } else { rel }
                ));
            }
            Ok(recorta(&lineas.join("  ")))
        }
        "leer_archivo" => {
            let rel = args
                .get("ruta")
                .and_then(|v| v.as_str())
                .ok_or("falta `ruta`")?;
            let ruta = ruta_enjaulada(raiz, rel)?;
            let texto =
                std::fs::read_to_string(&ruta).map_err(|e| format!("no pude leer «{rel}»: {e}"))?;
            let desde = args
                .get("desde")
                .and_then(|v| v.as_u64())
                .unwrap_or(1)
                .max(1) as usize;
            let cuantas = args
                .get("lineas")
                .and_then(|v| v.as_u64())
                .unwrap_or(120)
                .clamp(1, 400) as usize;
            let total = texto.lines().count();
            let cuerpo: String = texto
                .lines()
                .enumerate()
                .filter(|(i, _)| {
                    let n = i + 1;
                    n >= desde && n < desde + cuantas
                })
                .map(|(i, l)| format!("{:>5}| {l}", i + 1))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(recorta(&format!(
                "{rel} (líneas {desde}-{} de {total})\n{cuerpo}",
                (desde + cuantas - 1).min(total)
            )))
        }
        "firmas" => {
            let rel = args
                .get("ruta")
                .and_then(|v| v.as_str())
                .ok_or("falta `ruta`")?;
            let base = ruta_enjaulada(raiz, rel)?;
            let archivos: Vec<PathBuf> = if base.is_dir() {
                recorrer(raiz, &base, None, 120)
            } else {
                vec![PathBuf::from(rel.trim().replace('\\', "/"))]
            };
            let mut out = Vec::new();
            for f in archivos {
                let ext = ext_de(&f.to_string_lossy());
                if !matches!(ext.as_str(), "rs" | "ts" | "tsx" | "mjs" | "js") {
                    continue;
                }
                let Ok(texto) = std::fs::read_to_string(raiz.join(&f)) else {
                    continue;
                };
                let mut en_archivo = Vec::new();
                for (i, l) in texto.lines().enumerate() {
                    let t = l.trim_start();
                    let es = match ext.as_str() {
                        "rs" => [
                            "pub fn ",
                            "pub async fn ",
                            "fn ",
                            "async fn ",
                            "pub struct ",
                            "struct ",
                            "impl ",
                            "pub enum ",
                            "enum ",
                            "pub trait ",
                            "trait ",
                            "pub const ",
                            "pub type ",
                        ]
                        .iter()
                        .any(|p| t.starts_with(p)),
                        _ => [
                            "export const ",
                            "export function ",
                            "export async function ",
                            "export interface ",
                            "export type ",
                            "export default ",
                            "export class ",
                        ]
                        .iter()
                        .any(|p| t.starts_with(p)),
                    };
                    if es {
                        let firma: String = t.chars().take(120).collect();
                        en_archivo.push(format!("{:>5}| {firma}", i + 1));
                    }
                }
                if !en_archivo.is_empty() {
                    out.push(format!(
                        "· {} ({} firmas)\n{}",
                        ruta_rel(&f),
                        en_archivo.len(),
                        en_archivo.join("\n")
                    ));
                }
            }
            if out.is_empty() {
                return Ok(format!("sin firmas legibles en «{rel}»"));
            }
            Ok(recorta(&out.join("\n\n")))
        }
        "buscar" => {
            let texto = args
                .get("texto")
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
                .ok_or("falta `texto`")?;
            let ext = args.get("extension").and_then(|v| v.as_str());
            let tope = args
                .get("tope")
                .and_then(|v| v.as_u64())
                .unwrap_or(30)
                .clamp(1, 80) as usize;
            let aguja = texto.to_lowercase();
            let archivos = recorrer(raiz, raiz, ext, 900);
            let mut out = Vec::new();
            for f in archivos {
                let Ok(contenido) = std::fs::read_to_string(raiz.join(&f)) else {
                    continue;
                };
                for (i, l) in contenido.lines().enumerate() {
                    if l.to_lowercase().contains(&aguja) {
                        let linea: String = l.trim().chars().take(160).collect();
                        out.push(format!("{}:{}: {linea}", ruta_rel(&f), i + 1));
                        if out.len() >= tope {
                            break;
                        }
                    }
                }
                if out.len() >= tope {
                    break;
                }
            }
            if out.is_empty() {
                return Ok(format!("sin resultados para «{texto}»"));
            }
            Ok(recorta(&out.join("\n")))
        }
        "proponer_parche" => {
            let motivo = args
                .get("motivo")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let eds = crate::parche::parsear(args.get("ediciones").unwrap_or(&Value::Null))?;
            // Se valida AHORA (sin escribir): una propuesta que no se puede aplicar no llega al humano.
            let archivos = crate::parche::validar(raiz, &eds)?;
            if cfg.estado.as_os_str().is_empty() {
                return Err("no sé dónde guardar la propuesta (falta la carpeta de datos)".into());
            }
            crate::parche::guardar_propuesta(&cfg.estado, &motivo, &eds, &archivos)?;
            Ok(format!(
                "PROPUESTA registrada (no se escribió nada todavía): {} edición(es) en {} archivo(s): {}. El humano la aprueba o la rechaza, y al aprobarla corren los tests.",
                eds.len(),
                archivos.len(),
                archivos.iter().map(|(r, n)| format!("{r} ({n})")).collect::<Vec<_>>().join(", ")
            ))
        }
        "correr" => {
            if !cfg.comandos {
                return Err("los comandos están apagados en este turno".into());
            }
            let comando = args
                .get("comando")
                .and_then(|v| v.as_str())
                .ok_or("falta `comando`")?
                .trim();
            if !comando_autorizado(comando) {
                return Err(format!(
                    "comando no autorizado. Permitidos: {}",
                    COMANDOS.join(" · ")
                ));
            }
            correr_comando(&cfg.repo, comando, cfg.tope_cmd_s)
        }
        otro => Err(format!("herramienta desconocida: «{otro}»")),
    }
}

/// El ejecutor enjaulado, expuesto para que la etapa 4 (parche + verificación) corra sus *gates* con
/// exactamente las mismas reglas: lista blanca, tope de tiempo y salida acotada.
pub fn correr_comando_publico(repo: &Path, comando: &str, tope_s: u64) -> Result<String, String> {
    if !comando_autorizado(comando) {
        return Err(format!(
            "comando no autorizado. Permitidos: {}",
            COMANDOS.join(" · ")
        ));
    }
    correr_comando(repo, comando, tope_s)
}

/// Corre un comando autorizado en la raíz del repo, sin consola, con tope de tiempo y salida acotada.
fn correr_comando(repo: &Path, comando: &str, tope_s: u64) -> Result<String, String> {
    use std::process::{Command, Stdio};
    // Se parte el comando por espacios: viene de una lista blanca, no de un shell.
    let partes: Vec<&str> = comando.split_whitespace().collect();
    let (exe, args) = partes.split_first().ok_or("comando vacío")?;
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .current_dir(repo)
        // Sin esto `git log` abre un paginador y **se cuelga hasta el tope** (medido: 180 s de los 197
        // del turno, tirados). Un proceso sin consola no puede paginar ni preguntar nada: que imprima
        // todo y siga.
        .env("GIT_PAGER", "cat")
        .env("PAGER", "cat")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_EDITOR", "true")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let inicio = Instant::now();
    let mut hijo = cmd
        .spawn()
        .map_err(|e| format!("no pude correr «{comando}»: {e}"))?;
    let tope = Duration::from_secs(tope_s);
    // Tope **de verdad**: un `wait_with_output()` pelado espera para siempre y colgaría el turno.
    let salida = loop {
        match hijo.try_wait() {
            Ok(Some(_)) => break hijo.wait_with_output().map_err(|e| e.to_string())?,
            Ok(None) => {
                if inicio.elapsed() > tope {
                    let _ = hijo.kill();
                    let _ = hijo.wait();
                    return Err(format!("«{comando}» tardó más de {tope_s} s y lo corté"));
                }
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(e) => return Err(format!("no pude seguir «{comando}»: {e}")),
        }
    };
    let out = String::from_utf8_lossy(&salida.stdout).to_string();
    let err = String::from_utf8_lossy(&salida.stderr).to_string();
    let mezcla = if err.trim().is_empty() {
        out
    } else {
        format!("{out}\n[stderr]\n{err}")
    };
    let codigo = salida.status.code().unwrap_or(-1);
    // Para los comandos largos (tests) lo que importa está al final: se guarda cabeza y cola.
    Ok(format!(
        "«{comando}» → exit {codigo} · {} ms\n{}",
        inicio.elapsed().as_millis(),
        recorta_cabeza_cola(&mezcla)
    ))
}

/// Rutas relativas **siempre con `/`**: el modelo ve un formato estable y los tests no dependen del
/// separador del sistema.
fn ruta_rel(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn recorta(s: &str) -> String {
    if s.chars().count() <= TOPE_SALIDA {
        return s.to_string();
    }
    let corte: String = s.chars().take(TOPE_SALIDA).collect();
    format!("{corte}\n…(salida cortada a {TOPE_SALIDA} chars)")
}

fn recorta_cabeza_cola(s: &str) -> String {
    if s.chars().count() <= TOPE_SALIDA {
        return s.to_string();
    }
    let cabeza: String = s.chars().take(4_000).collect();
    let cola: String = s
        .chars()
        .rev()
        .take(3_500)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{cabeza}\n…(recortado)…\n{cola}")
}

// ─────────────────────────────────────────────────────────────────────────────
// El bucle
// ─────────────────────────────────────────────────────────────────────────────

/// El desenlace de un turno.
///
/// Los pasos viajan **siempre**: que el turno se corte por tope de vueltas, por presupuesto de tiempo o
/// porque el motor se cayó **no borra lo que el agente ya hizo**. Antes el tope devolvía un `Err` y se
/// perdían (el panel mostraba «0 herramientas» en un turno que había usado 32 y hasta propuesto un
/// parche): un desenlace parcial es información, no basura.
pub struct Turno {
    /// El texto con el que cerró, o el motivo por el que no cerró.
    pub texto: String,
    pub pasos: Vec<Paso>,
    /// `true` sólo si el modelo cerró el turno por su cuenta (sin tope, sin corte, sin falla de motor).
    pub ok: bool,
    /// La falla de infraestructura (motor, proveedor, respuesta ilegible), si la hubo.
    pub error: Option<String>,
    /// Cuántos resultados de herramienta se recortaron por presupuesto de entrada (0 = entró entero).
    pub podas: usize,
    /// Estimación de la entrada de la última vuelta (≈4 caracteres por token).
    pub entrada_estimada: usize,
}

impl Turno {
    fn cerrado(texto: String, pasos: Vec<Paso>) -> Turno {
        Turno {
            texto,
            pasos,
            ok: true,
            error: None,
            podas: 0,
            entrada_estimada: 0,
        }
    }

    /// Le pega la cuenta del presupuesto al desenlace: cuántas salidas se podaron y cuánta entrada
    /// estimada tenía la última vuelta. Va en todos los finales, incluso los cortados.
    fn con_cuenta(mut self, podas: usize, entrada_estimada: usize) -> Turno {
        self.podas = podas;
        self.entrada_estimada = entrada_estimada;
        self
    }
    /// Se cortó por una regla nuestra (tope de vueltas, presupuesto): no es un error del motor.
    fn cortado(texto: String, pasos: Vec<Paso>) -> Turno {
        Turno {
            texto,
            pasos,
            ok: false,
            error: None,
            podas: 0,
            entrada_estimada: 0,
        }
    }
    /// Falla del motor o del proveedor: también conserva los pasos.
    fn fallado(e: String, pasos: Vec<Paso>) -> Turno {
        Turno {
            texto: format!("el motor falló antes de cerrar el turno: {e}"),
            pasos,
            ok: false,
            error: Some(e),
            podas: 0,
            entrada_estimada: 0,
        }
    }
}

/// Estimación de los tokens de **entrada** que va a costar el hilo (≈4 caracteres por token, la misma
/// regla que `costo::aprox_tokens`). Se serializa el mensaje entero a propósito: las claves, las comillas
/// y los `tool_call_id` también viajan y también se pagan.
pub fn estimar_entrada(messages: &[Value]) -> usize {
    messages
        .iter()
        .map(|m| {
            serde_json::to_string(m)
                .map(|s| s.chars().count() / 4 + 6)
                .unwrap_or(6)
        })
        .sum()
}

/// Poda el historial para volver a entrar en el presupuesto: de los resultados de herramienta **viejos**
/// (nunca los últimos `conservar_ultimas`) deja la primera línea y esconde el resto detrás de la marca.
///
/// Es **idempotente y monótona a propósito**: lo ya podado no se vuelve a tocar ni se re-expande, así el
/// prefijo del hilo queda idéntico entre vueltas y la caché de prefijo del proveedor (medida el 17/09:
/// 80,8 % de la entrada, a ~1/50 del precio) sigue pegando. Podar distinto en cada vuelta sería bajar
/// tokens para pagarlos a precio pleno. Devuelve cuántas salidas recortó.
pub fn podar(messages: &mut [Value], cfg: &Config) -> usize {
    let idxs: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.get("role").and_then(|r| r.as_str()) == Some("tool"))
        .map(|(i, _)| i)
        .collect();
    if idxs.len() <= cfg.conservar_ultimas {
        return 0;
    }
    let mut recortadas = 0;
    for &i in &idxs[..idxs.len() - cfg.conservar_ultimas] {
        let Some(c) = messages[i].get("content").and_then(|c| c.as_str()) else {
            continue;
        };
        if c.contains(MARCA_PODA) {
            continue; // ya podada: la marca sobrevive en el texto, así que no se re-toca (monótona)
        }
        let lineas: Vec<&str> = c.lines().collect();
        let nuevo = if lineas.len() > 1 {
            format!("{}\n{MARCA_PODA}{} líneas)", lineas[0], lineas.len() - 1)
        } else if c.chars().count() > TOPE_LINEA {
            format!(
                "{}\n{MARCA_PODA}1 línea de {} caracteres)",
                c.chars().take(TOPE_LINEA).collect::<String>(),
                c.chars().count()
            )
        } else {
            continue; // una línea corta ya es mínima: no hay nada que recortar
        };
        messages[i]["content"] = json!(nuevo);
        recortadas += 1;
    }
    recortadas
}

/// Corre el turno completo: pide al modelo, ejecuta lo que pida, realimenta y repite.
///
/// `llamar(messages, tools)` es el único acceso al mundo: lo inyecta el backend (proveedor real) y lo
/// inyecta el test (modelo de mentira). Devuelve el texto final y **todos** los pasos ejecutados, que
/// es lo que el panel muestra y lo que hace auditable la corrida.
pub async fn correr<F, Fut>(pedido: &str, sistema: &str, cfg: &Config, llamar: F) -> Turno
where
    F: Fn(Vec<Value>, Vec<Value>) -> Fut,
    Fut: std::future::Future<Output = Result<Value, String>>,
{
    let herramientas = definiciones(cfg.comandos);
    let mut messages = vec![
        json!({ "role": "system", "content": sistema }),
        json!({ "role": "user", "content": pedido }),
    ];
    let mut pasos: Vec<Paso> = Vec::new();
    let mut podas: usize = 0;
    let mut entrada_estimada: usize = estimar_entrada(&messages);
    let t0 = Instant::now();

    for vuelta in 1..=cfg.tope_pasos {
        if t0.elapsed() > Duration::from_secs(cfg.tope_s) {
            return Turno::cortado(
                format!(
                    "el turno pasó el presupuesto de {} s (vueltas: {vuelta}, herramientas: {})",
                    cfg.tope_s,
                    pasos.len()
                ),
                pasos,
            )
            .con_cuenta(podas, entrada_estimada);
        }
        // Presupuesto de entrada: el hilo se reenvía **entero** en cada vuelta, así que es el único lugar
        // donde el turno se puede engordar sin que nadie mire. Si no entra, se poda lo viejo; si aun así no
        // entra, se corta como el tope — conservando los pasos, que es la lección que ya nos costó un bug.
        entrada_estimada = estimar_entrada(&messages);
        if entrada_estimada > cfg.presupuesto_entrada {
            podas += podar(&mut messages, cfg);
            entrada_estimada = estimar_entrada(&messages);
            if entrada_estimada > cfg.presupuesto_entrada && !pasos.is_empty() {
                return Turno::cortado(
                    format!(
                        "el turno pasó el presupuesto de {} tokens de entrada (vueltas: {vuelta}, \
                         herramientas: {}, recortes: {podas})",
                        cfg.presupuesto_entrada,
                        pasos.len()
                    ),
                    pasos,
                )
                .con_cuenta(podas, entrada_estimada);
            }
        }
        let r = match llamar(messages.clone(), herramientas.clone()).await {
            Ok(r) => r,
            Err(e) => return Turno::fallado(e, pasos).con_cuenta(podas, entrada_estimada),
        };
        let msg = r["choices"][0]["message"].clone();
        if msg.is_null() {
            return Turno::fallado(
                format!("respuesta sin `message`: {}", recorta(&r.to_string())),
                pasos,
            )
            .con_cuenta(podas, entrada_estimada);
        }
        let llamadas = msg
            .get("tool_calls")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if llamadas.is_empty() {
            let texto = msg
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if texto.is_empty() {
                return Turno::cortado("el modelo no devolvió texto ni herramientas".into(), pasos)
                    .con_cuenta(podas, entrada_estimada);
            }
            return Turno::cerrado(texto, pasos).con_cuenta(podas, entrada_estimada);
        }

        // El mensaje del asistente con sus tool_calls tiene que ir en el hilo: sin eso el proveedor
        // rechaza el `role: tool` que viene después.
        messages.push(msg.clone());
        for tc in llamadas {
            let nombre = tc["function"]["name"].as_str().unwrap_or("").to_string();
            let crudos = tc["function"]["arguments"].as_str().unwrap_or("{}");
            let args: Value = serde_json::from_str(crudos)
                .unwrap_or_else(|_| json!({ "_argumentos_ilegibles": crudos }));
            let inicio = Instant::now();
            let (salida, ok) = match ejecutar(&cfg.repo, &nombre, &args, cfg) {
                Ok(s) => (s, true),
                Err(e) => (format!("ERROR: {e}"), false),
            };
            let paso = Paso {
                herramienta: nombre.clone(),
                argumentos: args,
                salida: salida.clone(),
                ok,
                ms: inicio.elapsed().as_millis() as u64,
            };
            log::info!("agente: {}", paso.resumen());
            messages.push(json!({
                "role": "tool",
                "tool_call_id": tc["id"].as_str().unwrap_or(""),
                "content": salida
            }));
            pasos.push(paso);
        }
    }

    Turno::cortado(
        format!(
            "el agente agotó las {} vueltas sin cerrar el turno (herramientas usadas: {})",
            cfg.tope_pasos,
            pasos.len()
        ),
        pasos,
    )
    .con_cuenta(podas, entrada_estimada)
}

/// El sistema del turno: el agente sabe **qué** es, **dónde** está y **qué reglas** tiene. Que sepa que
/// no escribe es parte del contrato: en esta etapa mira y verifica.
pub fn sistema(repo: &Path) -> String {
    format!(
        "Sos el agente de NodeFlow, la app de lienzo de conocimiento del usuario, y estás corriendo \
         DENTRO de NodeFlow (no en otro asistente). Tu repo es {} y tenés herramientas para mirarlo: \
         `firmas` y `buscar` primero (contexto barato), `leer_archivo` con rango sólo para el tramo que \
         importa, y `correr` para verificar (tests y chequeo de tipos) en vez de suponer.\n\n\
         Reglas: (1) no inventes rutas, líneas ni resultados — si no lo miraste, decilo; (2) no escribís \
         archivos en esta etapa: proponés el cambio y el humano lo aprueba; (3) cerrá con un plan \
         concreto en 3 a 6 renglones, en español, con los archivos y los comandos que hay que correr; \
         (4) si una herramienta falla, leé el error y corregí el pedido en vez de repetirlo igual.",
        repo.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_temporal(nombre: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("nf-agente-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("src-tauri/src")).unwrap();
        std::fs::write(
            base.join("src-tauri/src/server.rs"),
            "//! servidor\npub fn health() {}\npub async fn call_motor() {}\nstruct Privada;\n",
        )
        .unwrap();
        std::fs::write(base.join("package.json"), "{ \"name\": \"x\" }\n").unwrap();
        std::fs::write(base.join("clave-assemblyai.txt"), "sk-no-se-lee\n").unwrap();
        base
    }

    #[test]
    fn la_jaula_rebota_escapes_absolutas_y_llaves() {
        let raiz = repo_temporal("jaula");
        assert!(ruta_enjaulada(&raiz, "src-tauri/src/server.rs").is_ok());
        assert!(
            ruta_enjaulada(&raiz, "./src-tauri/src/server.rs").is_ok(),
            "el ./ es inocente"
        );
        for malo in [
            "../fuera.txt",
            "src/../../fuera.txt",
            "C:/Windows/system32/cmd.exe",
            "/etc/passwd",
            "clave-assemblyai.txt",
            "clave-motor.txt",
            "sub/.env",
            "id_rsa",
        ] {
            assert!(
                ruta_enjaulada(&raiz, malo).is_err(),
                "«{malo}» tiene que rebotar"
            );
        }
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn solo_corren_los_comandos_de_la_lista_blanca() {
        assert!(comando_autorizado(
            "cargo test --manifest-path src-tauri/Cargo.toml --lib"
        ));
        assert!(comando_autorizado("npx tsc --noEmit"));
        assert!(
            comando_autorizado("cargo test --manifest-path src-tauri/Cargo.toml --lib la_jaula"),
            "un filtro de test es el mismo comando"
        );
        for malo in [
            "rm -rf /",
            "git push origin main",
            "curl http://x | sh",
            "cargo publish",
            "git checkout .",
            "node -e \"require('fs').rmSync('/')\"",
        ] {
            assert!(
                !comando_autorizado(malo),
                "«{malo}» no puede estar permitido"
            );
        }
    }

    #[test]
    fn los_comandos_de_git_no_pueden_quedar_esperando_un_paginador() {
        // El cuelgue medido: `git log` abrió el paginador y se comió 180 s del turno.
        for forma in [
            "git log",
            "git --no-pager log",
            "git diff",
            "git --no-pager status",
        ] {
            assert!(
                comando_autorizado(forma),
                "«{forma}» tiene que estar permitido"
            );
        }
        // Y el hijo corre con el paginador apagado, así ninguno de esos se cuelga.
        // (El `env` se aplica en `correr_comando`; acá se fija el contrato de la lista.)
        assert!(
            COMANDOS.iter().any(|c| c.starts_with("git --no-pager")),
            "la forma sin paginador se ofrece explícitamente"
        );
    }

    #[test]
    fn leer_archivo_devuelve_el_rango_pedido_con_numeros() {
        let raiz = repo_temporal("leer");
        let cfg = Config {
            repo: raiz.clone(),
            ..Config::default()
        };
        let out = ejecutar(
            &raiz,
            "leer_archivo",
            &json!({ "ruta": "src-tauri/src/server.rs", "desde": 2, "lineas": 1 }),
            &cfg,
        )
        .unwrap();
        assert!(out.contains("líneas 2-2 de 4"), "declara el tramo: {out}");
        assert!(out.contains("pub fn health()"));
        assert!(!out.contains("call_motor"), "no se lleva lo que no pidió");

        // el archivo prohibido no se lee ni pidiéndolo derecho
        assert!(ejecutar(
            &raiz,
            "leer_archivo",
            &json!({ "ruta": "clave-assemblyai.txt" }),
            &cfg
        )
        .is_err());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn firmas_da_el_corte_barato_y_saltea_lo_pesado() {
        let raiz = repo_temporal("firmas");
        std::fs::create_dir_all(raiz.join("node_modules/ruido")).unwrap();
        std::fs::write(
            raiz.join("node_modules/ruido/x.rs"),
            "pub fn no_deberia_aparecer() {}\n",
        )
        .unwrap();
        let cfg = Config {
            repo: raiz.clone(),
            ..Config::default()
        };
        let out = ejecutar(&raiz, "firmas", &json!({ "ruta": "src-tauri/src" }), &cfg).unwrap();
        assert!(out.contains("pub fn health()"));
        assert!(out.contains("pub async fn call_motor()"));
        assert!(
            !out.contains("no_deberia_aparecer"),
            "`node_modules` no se recorre: {out}"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn buscar_encuentra_y_acota() {
        let raiz = repo_temporal("buscar");
        let cfg = Config {
            repo: raiz.clone(),
            ..Config::default()
        };
        let out = ejecutar(
            &raiz,
            "buscar",
            &json!({ "texto": "call_motor", "extension": "rs" }),
            &cfg,
        )
        .unwrap();
        assert!(
            out.contains("src-tauri/src/server.rs:3"),
            "archivo:línea → {out}"
        );
        let vacio = ejecutar(&raiz, "buscar", &json!({ "texto": "no-existe-xyz" }), &cfg).unwrap();
        assert!(vacio.starts_with("sin resultados"));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn correr_rechaza_lo_que_no_esta_en_la_lista_y_respeta_el_apagado() {
        let raiz = repo_temporal("correr");
        let cfg = Config {
            repo: raiz.clone(),
            ..Config::default()
        };
        let e = ejecutar(&raiz, "correr", &json!({ "comando": "rm -rf /" }), &cfg).unwrap_err();
        assert!(e.contains("no autorizado"), "{e}");
        let apagado = Config {
            comandos: false,
            ..cfg.clone()
        };
        assert!(ejecutar(
            &raiz,
            "correr",
            &json!({ "comando": "git status" }),
            &apagado
        )
        .is_err());
        assert!(
            !definiciones(false)
                .iter()
                .any(|d| d["function"]["name"] == "correr"),
            "si está apagado, la herramienta ni se ofrece"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// Modelo de mentira: devuelve una secuencia de respuestas. El bucle no sabe de HTTP.
    fn modelo(
        respuestas: Vec<Value>,
    ) -> impl Fn(
        Vec<Value>,
        Vec<Value>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, String>>>> {
        let celda = std::sync::Arc::new(std::sync::Mutex::new(respuestas));
        move |_m: Vec<Value>, _t: Vec<Value>| {
            let c = celda.clone();
            Box::pin(async move {
                let mut g = c.lock().unwrap();
                if g.is_empty() {
                    return Err("el modelo se quedó sin respuestas".into());
                }
                Ok(g.remove(0))
            })
                as std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, String>>>>
        }
    }

    #[tokio::test]
    async fn el_bucle_ejecuta_la_herramienta_y_cierra_con_el_texto() {
        let raiz = repo_temporal("bucle");
        let cfg = Config {
            repo: raiz.clone(),
            ..Config::default()
        };
        let secuencia = vec![
            json!({ "choices": [ { "message": { "role": "assistant", "content": null, "tool_calls": [
                { "id": "c1", "type": "function", "function": { "name": "firmas", "arguments": "{\"ruta\":\"src-tauri/src\"}" } } ] } } ] }),
            json!({ "choices": [ { "message": { "role": "assistant", "content": "Listo: el módulo tiene 2 funciones públicas." } } ] }),
        ];
        let t = correr("¿qué hay en el módulo?", "sys", &cfg, modelo(secuencia)).await;
        assert!(t.ok, "cerró el turno solo: {}", t.texto);
        assert!(t.error.is_none());
        let (texto, pasos) = (t.texto, t.pasos);
        assert!(texto.contains("2 funciones públicas"));
        assert_eq!(pasos.len(), 1, "una herramienta ejecutada");
        assert_eq!(pasos[0].herramienta, "firmas");
        assert!(pasos[0].ok);
        assert!(pasos[0].salida.contains("pub fn health()"));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[tokio::test]
    async fn una_herramienta_que_falla_no_mata_el_turno() {
        let raiz = repo_temporal("falla");
        let cfg = Config {
            repo: raiz.clone(),
            ..Config::default()
        };
        let secuencia = vec![
            json!({ "choices": [ { "message": { "tool_calls": [
                { "id": "c1", "type": "function", "function": { "name": "leer_archivo", "arguments": "{\"ruta\":\"no/existe.rs\"}" } } ] } } ] }),
            json!({ "choices": [ { "message": { "content": "No existe ese archivo; probemos otro camino." } } ] }),
        ];
        let t = correr("leé lo que no existe", "sys", &cfg, modelo(secuencia)).await;
        assert!(t.ok, "un paso que falla no corta el turno");
        let (texto, pasos) = (t.texto, t.pasos);
        assert!(pasos[0].ok == false, "el paso se marca como error");
        assert!(
            pasos[0].salida.starts_with("ERROR:"),
            "el error viaja como texto"
        );
        assert!(texto.contains("probemos otro camino"), "el turno siguió");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn la_poda_deja_la_primera_linea_y_no_toca_las_ultimas() {
        let cfg = Config {
            conservar_ultimas: 2,
            ..Config::default()
        };
        let mut msgs: Vec<Value> = vec![
            json!({ "role": "system", "content": "sys" }),
            json!({ "role": "user", "content": "pedido" }),
        ];
        for i in 0..4 {
            msgs.push(json!({ "role": "tool", "tool_call_id": format!("c{i}"),
                              "content": format!("salida {i} de la herramienta\n{}
fin", "x".repeat(600)) }));
        }
        let n = podar(&mut msgs, &cfg);
        assert_eq!(n, 2, "con conservar_ultimas=2 se recortan las 2 más viejas");
        let vieja = msgs[2]["content"].as_str().unwrap();
        assert!(vieja.starts_with("salida 0 de la herramienta"), "{vieja}");
        assert!(
            vieja.contains(MARCA_PODA) && vieja.contains("2 líneas"),
            "{vieja}"
        );
        assert!(vieja.chars().count() < 200, "lo recortado no viaja");
        let ultima = msgs[5]["content"].as_str().unwrap();
        assert_eq!(
            ultima.lines().count(),
            3,
            "la última queda entera: el modelo la acaba de pedir"
        );
    }

    #[test]
    fn la_poda_es_monotona_y_no_se_repite() {
        let cfg = Config {
            conservar_ultimas: 1,
            ..Config::default()
        };
        let mut msgs: Vec<Value> = vec![json!({ "role": "system", "content": "sys" })];
        for i in 0..4 {
            msgs.push(json!({ "role": "tool", "tool_call_id": format!("c{i}"),
                              "content": format!("primera {i}\n{}
fin", "y".repeat(600)) }));
        }
        assert_eq!(podar(&mut msgs, &cfg), 3);
        let despues: Vec<String> = msgs.iter().map(|m| m.to_string()).collect();
        // Segunda pasada: no recorta nada nuevo y **no re-expande** lo ya recortado.
        assert_eq!(podar(&mut msgs, &cfg), 0, "idempotente");
        let ahora: Vec<String> = msgs.iter().map(|m| m.to_string()).collect();
        assert_eq!(
            despues, ahora,
            "monótona: el prefijo no cambia entre vueltas"
        );
    }

    #[test]
    fn la_estimacion_de_entrada_sigue_la_regla_de_cuatro_caracteres() {
        let msgs = vec![json!({ "role": "system", "content": "a".repeat(4_000) })];
        let est = estimar_entrada(&msgs);
        assert!(
            (1_000..1_100).contains(&est),
            "≈1000 tokens para 4000 caracteres, dio {est}"
        );
        assert!(estimar_entrada(&[]) == 0);
    }

    #[tokio::test]
    async fn con_presupuesto_chico_el_turno_corta_y_conserva_los_pasos() {
        let raiz = repo_temporal("presupuesto");
        // Un archivo grande: cada lectura devuelve el tope de salida (8.000 caracteres ≈ 2.000 tokens).
        std::fs::write(
            raiz.join("src-tauri/src/grande.rs"),
            (0..1_000)
                .map(|i| format!("// línea {i} con relleno suficiente para pesar\n"))
                .collect::<String>(),
        )
        .unwrap();
        let cfg = Config {
            repo: raiz.clone(),
            tope_pasos: 6,
            presupuesto_entrada: 4_000,
            conservar_ultimas: 3,
            ..Config::default()
        };
        let siempre = (0..6)
            .map(|i| {
                json!({ "choices": [ { "message": { "tool_calls": [
                    { "id": format!("c{i}"), "type": "function", "function": { "name": "leer_archivo",
                      "arguments": "{\"ruta\":\"src-tauri/src/grande.rs\",\"desde\":1,\"lineas\":400}" } } ] } } ] })
            })
            .collect();
        let t = correr("dale", "sys", &cfg, modelo(siempre)).await;
        assert!(!t.ok, "no cerró: {}", t.texto);
        assert!(
            t.texto.contains("presupuesto de 4000 tokens"),
            "{}",
            t.texto
        );
        assert!(
            t.error.is_none(),
            "un presupuesto no es una falla de motor: es una regla nuestra"
        );
        assert!(
            t.pasos.len() >= 2,
            "los pasos NO se pierden: {}",
            t.pasos.len()
        );
        assert!(
            t.entrada_estimada > 4_000,
            "informa cuánta entrada tenía: {}",
            t.entrada_estimada
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[tokio::test]
    async fn la_poda_deja_seguir_el_turno_cuando_lo_viejo_es_lo_que_sobra() {
        let raiz = repo_temporal("poda-turno");
        std::fs::write(
            raiz.join("src-tauri/src/grande.rs"),
            (0..1_000)
                .map(|i| format!("// línea {i} con relleno suficiente para pesar\n"))
                .collect::<String>(),
        )
        .unwrap();
        let cfg = Config {
            repo: raiz.clone(),
            tope_pasos: 5,
            presupuesto_entrada: 4_000,
            conservar_ultimas: 1,
            ..Config::default()
        };
        let siempre = (0..5)
            .map(|i| {
                json!({ "choices": [ { "message": { "tool_calls": [
                    { "id": format!("c{i}"), "type": "function", "function": { "name": "leer_archivo",
                      "arguments": "{\"ruta\":\"src-tauri/src/grande.rs\",\"desde\":1,\"lineas\":400}" } } ] } } ] })
            })
            .collect();
        let t = correr("dale", "sys", &cfg, modelo(siempre)).await;
        assert!(
            t.texto.contains("agotó las 5 vueltas"),
            "el turno siguió hasta el tope: {}",
            t.texto
        );
        assert!(
            t.podas >= 2,
            "podó lo viejo en vez de morir por presupuesto: {}",
            t.podas
        );
        assert_eq!(t.pasos.len(), 5);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[tokio::test]
    async fn el_tope_de_vueltas_corta_el_bucle_infinito() {
        let raiz = repo_temporal("tope");
        let cfg = Config {
            repo: raiz.clone(),
            tope_pasos: 3,
            ..Config::default()
        };
        // El modelo siempre pide la misma herramienta: sin tope, esto no termina nunca.
        let siempre = (0..10).map(|i| json!({ "choices": [ { "message": { "tool_calls": [
            { "id": format!("c{i}"), "type": "function", "function": { "name": "listar", "arguments": "{}" } } ] } } ] })).collect();
        let t = correr("dale", "sys", &cfg, modelo(siempre)).await;
        assert!(!t.ok, "no cerró el turno");
        assert!(t.texto.contains("agotó las 3 vueltas"), "{}", t.texto);
        assert!(
            t.error.is_none(),
            "un tope no es una falla de motor: es una regla nuestra"
        );
        // El bug que destapó el turno real: el tope tiraba los pasos a la basura.
        assert_eq!(
            t.pasos.len(),
            3,
            "los pasos NO se pierden cuando se agota el tope"
        );
        assert!(t.pasos.iter().all(|p| p.ok), "y conservan su marca");
        let _ = std::fs::remove_dir_all(&raiz);
    }
}

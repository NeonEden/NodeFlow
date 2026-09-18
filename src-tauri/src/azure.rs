//! Azure, **plano de gestión** (ARM): qué modelos hay desplegados en una cuenta de Azure AI.
//!
//! Por qué el plano de gestión y no el de datos: el plano de datos contesta «¿esta clave puede usar
//! `gpt-4o`?», que depende del despliegue exacto; ARM contesta «¿qué desplegaste, con qué versión y
//! capacidad?», que es lo que el panel necesita para poblar el catálogo de motores sin adivinar.
//!
//! Reglas que no se rompen acá:
//!   · el token **nunca** sale del módulo (ni al log ni al JSON de respuesta);
//!   · `value[]` se parsea **entrada por entrada**: un despliegue con forma rara se cuenta como
//!     ignorado en vez de tirar abajo la lista completa;
//!   · si falta configuración o token se dice **qué** falta y **dónde** ponerlo, en vez de devolver
//!     una lista vacía que parezca «no hay modelos».

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Value};

/// Raíz del plano de gestión.
pub const ARM: &str = "https://management.azure.com";
/// Versión de API del listado de despliegues (Cognitive Services / Foundry).
pub const API_VERSION: &str = "2024-10-01";
/// Recurso para el que se pide el token. **Con barra final**: así lo emite Entra ID.
const RECURSO: &str = "https://management.azure.com/";
/// `az` emite tokens de ~60 min; se renueva a los 45 para no cortar un turno en el peor momento.
const VIDA_TOKEN_S: u64 = 45 * 60;
/// Tope del CLI: una sesión vencida se queda esperando y colgaría el handler.
const TOPE_AZ_S: u64 = 20;

// ─────────────────────────────────────────────────────────────────────────────
// Errores
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum Fallo {
    /// Falta configuración del usuario (suscripción, grupo, cuenta).
    Config(String),
    /// No hay token utilizable (CLI sin sesión, salida inesperada).
    Token(String),
    /// ARM contestó distinto de 2xx, o no se llegó. `estado == 0` = error de red.
    Arm { estado: u16, mensaje: String },
}

impl std::fmt::Display for Fallo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fallo::Config(m) | Fallo::Token(m) => write!(f, "{m}"),
            Fallo::Arm { estado: 0, mensaje } => write!(f, "{mensaje}"),
            Fallo::Arm { estado, mensaje } => write!(f, "ARM {estado}: {mensaje}"),
        }
    }
}

impl Fallo {
    /// Código para la respuesta local: lo que le falta al usuario es 400; lo que falló del otro lado
    /// (o en la red) es 502.
    pub fn codigo(&self) -> u16 {
        match self {
            Fallo::Config(_) | Fallo::Token(_) => 400,
            Fallo::Arm { .. } => 502,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Destino ARM
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Destino {
    pub suscripcion: String,
    pub grupo: String,
    pub cuenta: String,
}

impl Destino {
    pub fn nuevo(
        suscripcion: impl Into<String>,
        grupo: impl Into<String>,
        cuenta: impl Into<String>,
    ) -> Result<Self, Fallo> {
        let limpiar = |s: String| s.trim().trim_matches('"').to_string();
        let d = Destino {
            suscripcion: limpiar(suscripcion.into()),
            grupo: limpiar(grupo.into()),
            cuenta: limpiar(cuenta.into()),
        };
        let mut faltan = Vec::new();
        if d.suscripcion.is_empty() {
            faltan.push("suscripcion");
        }
        if d.grupo.is_empty() {
            faltan.push("grupo");
        }
        if d.cuenta.is_empty() {
            faltan.push("cuenta");
        }
        if !faltan.is_empty() {
            return Err(Fallo::Config(format!(
                "faltan datos de Azure: {}. Pasalos por query (?suscripcion=&grupo=&cuenta=), por entorno \
                 (AZURE_SUBSCRIPTION_ID / AZURE_RESOURCE_GROUP / AZURE_COGNITIVE_ACCOUNT) o en \
                 nodeflow.config.json → {{\"azure\": {{\"suscripcion\": \"…\", \"grupo\": \"…\", \"cuenta\": \"…\"}}}}",
                faltan.join(", ")
            )));
        }
        Ok(d)
    }

    /// URL del listado de despliegues. Cada segmento va percent-encoded: un nombre con espacios o
    /// acentos no rompe la ruta.
    pub fn url(&self, version: &str) -> String {
        format!(
            "{ARM}/subscriptions/{}/resourceGroups/{}/providers/Microsoft.CognitiveServices/accounts/{}/deployments?api-version={version}",
            codificar(&self.suscripcion),
            codificar(&self.grupo),
            codificar(&self.cuenta)
        )
    }
}

fn codificar(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Coordenadas ARM: query > entorno > `nodeflow.config.json` (`azure.*`, o plano `azure_*`).
pub fn destino_desde(data_dir: &Path, q: &HashMap<String, String>) -> Result<Destino, Fallo> {
    let cfg: Option<Value> = std::fs::read_to_string(data_dir.join("nodeflow.config.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());

    let buscar = |claves: &[&str], envs: &[&str]| -> String {
        for k in claves {
            if let Some(v) = q.get(*k) {
                if !v.trim().is_empty() {
                    return v.trim().to_string();
                }
            }
        }
        for e in envs {
            if let Ok(v) = std::env::var(e) {
                if !v.trim().is_empty() {
                    return v.trim().to_string();
                }
            }
        }
        if let Some(c) = cfg.as_ref() {
            let seccion = c.get("azure");
            for k in claves {
                if let Some(v) = seccion.and_then(|s| s.get(*k)).and_then(|v| v.as_str()) {
                    if !v.trim().is_empty() {
                        return v.trim().to_string();
                    }
                }
                if let Some(v) = c.get(&format!("azure_{k}")).and_then(|v| v.as_str()) {
                    if !v.trim().is_empty() {
                        return v.trim().to_string();
                    }
                }
            }
        }
        String::new()
    };

    Destino::nuevo(
        buscar(
            &["suscripcion", "sub", "subscription_id"],
            &["AZURE_SUBSCRIPTION_ID", "NODEFLOW_AZURE_SUBSCRIPTION"],
        ),
        buscar(
            &["grupo", "rg", "resource_group"],
            &["AZURE_RESOURCE_GROUP", "NODEFLOW_AZURE_RG"],
        ),
        buscar(
            &["cuenta", "account"],
            &["AZURE_COGNITIVE_ACCOUNT", "NODEFLOW_AZURE_ACCOUNT"],
        ),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Forma de la respuesta de ARM (tolerante: todo opcional y con `default`)
// ─────────────────────────────────────────────────────────────────────────────

/// `type` y `location` se ignoran a propósito: el panel no los usa y declararlos obligaría a
/// mantenerlos sincronizados con ARM sin ganar nada.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Despliegue {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub sku: Option<Sku>,
    #[serde(default)]
    pub properties: Option<Propiedades>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Sku {
    #[serde(default)]
    pub name: Option<String>,
    /// ARM la manda como número casi siempre; algún backend como texto (`"10"`).
    #[serde(default, deserialize_with = "entero_opcional")]
    pub capacity: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Propiedades {
    #[serde(default, rename = "provisioningState")]
    pub estado: Option<String>,
    #[serde(default)]
    pub model: Option<ModeloArm>,
    #[serde(default, rename = "raiPolicyName")]
    pub politica: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModeloArm {
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

/// El sobre del listado. `value` queda **crudo** (`Value`): cada entrada se parsea por separado, así
/// una sola deforme no invalida la página entera.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Pagina {
    #[serde(default)]
    pub value: Vec<Value>,
    #[serde(default, rename = "nextLink")]
    pub siguiente: Option<String>,
}

/// ARM manda `capacity` como número casi siempre, pero algún backend como texto. Aceptar los dos es
/// la diferencia entre listar y fallar por un detalle de formato.
fn entero_opcional<'de, D>(d: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Option::<Value>::deserialize(d)?;
    Ok(match v {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => n.as_u64(),
        Some(Value::String(s)) => s.trim().parse::<u64>().ok(),
        _ => None,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Vista para el panel
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct Modelo {
    /// Id de ARM: sirve para linkear al portal o para borrar/actualizar el despliegue.
    pub recurso: Option<String>,
    pub despliegue: String,
    pub modelo: String,
    pub version: Option<String>,
    pub formato: Option<String>,
    pub estado: Option<String>,
    pub sku: Option<String>,
    pub capacidad: Option<u64>,
    pub politica: Option<String>,
}

impl Modelo {
    /// Entra si tiene nombre de despliegue; lo que falte se completa con `None` en vez de romper.
    fn desde(d: &Despliegue) -> Option<Self> {
        let nombre = d.name.as_deref()?.trim();
        if nombre.is_empty() {
            return None;
        }
        let props = d.properties.clone().unwrap_or_default();
        let modelo = props.model.clone().unwrap_or_default();
        let sku = d.sku.clone().unwrap_or_default();
        Some(Modelo {
            recurso: d.id.clone(),
            despliegue: nombre.to_string(),
            modelo: modelo.name.unwrap_or_default(),
            version: modelo.version,
            formato: modelo.format,
            estado: props.estado,
            sku: sku.name,
            capacidad: sku.capacity,
            politica: props.politica,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Informe {
    pub ok: bool,
    pub suscripcion: String,
    pub grupo: String,
    pub cuenta: String,
    pub url: String,
    pub total: usize,
    /// Entradas de `value[]` que no se pudieron leer. Se informa: un 0 silencioso esconde drift.
    pub ignorados: usize,
    /// ARM pagina con `nextLink`. Se reporta pero no se sigue: encadenar llamadas sin tope es lo que
    /// hace que un handler tarde minutos.
    pub hay_mas: bool,
    pub siguiente: Option<String>,
    pub modelos: Vec<Modelo>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Llamada a ARM
// ─────────────────────────────────────────────────────────────────────────────

/// `GET .../deployments`. Devuelve los modelos, cuántas entradas se ignoraron y el `nextLink`.
pub async fn listar(
    http: &reqwest::Client,
    destino: &Destino,
    token: &str,
) -> Result<(Vec<Modelo>, usize, Option<String>), Fallo> {
    let url = destino.url(API_VERSION);
    let resp = http
        .get(&url)
        .bearer_auth(token)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| Fallo::Arm {
            estado: 0,
            mensaje: format!("no pude llegar a ARM: {e}"),
        })?;

    let estado = resp.status().as_u16();
    let cuerpo = resp.text().await.unwrap_or_default();
    if !(200..300).contains(&estado) {
        return Err(Fallo::Arm {
            estado,
            mensaje: mensaje_de_arm(estado, &cuerpo),
        });
    }

    let pagina: Pagina = serde_json::from_str(&cuerpo).map_err(|e| Fallo::Arm {
        estado,
        mensaje: format!("la respuesta de ARM no tiene la forma esperada: {e}"),
    })?;

    Ok(modelos_de_pagina(&pagina))
}

/// Parte **pura** del listado: `value[]` se parsea entrada por entrada, así un despliegue con forma
/// rara se cuenta como ignorado en vez de tirar abajo la lista completa. Vive aparte para poder
/// probarla sin red y sin token.
fn modelos_de_pagina(pagina: &Pagina) -> (Vec<Modelo>, usize, Option<String>) {
    let mut modelos = Vec::with_capacity(pagina.value.len());
    let mut ignorados = 0usize;
    for entrada in &pagina.value {
        match serde_json::from_value::<Despliegue>(entrada.clone()) {
            Ok(d) => match Modelo::desde(&d) {
                Some(m) => modelos.push(m),
                None => ignorados += 1,
            },
            Err(_) => ignorados += 1,
        }
    }
    let siguiente = pagina
        .siguiente
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string());
    (modelos, ignorados, siguiente)
}

/// Traduce el fallo de ARM a algo accionable (y sin filtrar el token, que no viaja en el cuerpo).
fn mensaje_de_arm(estado: u16, cuerpo: &str) -> String {
    let detalle = detalle_arm(cuerpo);
    match estado {
        401 => format!(
            "el token no sirve o venció: corré `az login` (o poné uno nuevo en azure_token). ARM dijo: {detalle}"
        ),
        403 => format!(
            "la identidad no puede leer despliegues de esta cuenta: hace falta, como mínimo, el rol Lector sobre la cuenta (o el grupo de recursos). ARM dijo: {detalle}"
        ),
        404 => format!(
            "no existe la suscripción, el grupo de recursos o la cuenta indicada (revisá suscripcion/grupo/cuenta). ARM dijo: {detalle}"
        ),
        429 => format!("ARM está limitando pedidos (429): esperá unos segundos. Detalle: {detalle}"),
        _ => format!("ARM contestó {estado}: {detalle}"),
    }
}

fn detalle_arm(cuerpo: &str) -> String {
    let v: Value = serde_json::from_str(cuerpo).unwrap_or(Value::Null);
    let codigo = v["error"]["code"].as_str().unwrap_or("").trim();
    let mensaje = v["error"]["message"].as_str().unwrap_or("").trim();
    let texto = match (codigo.is_empty(), mensaje.is_empty()) {
        (false, false) => format!("{codigo}: {mensaje}"),
        (false, true) => codigo.to_string(),
        (true, false) => mensaje.to_string(),
        (true, true) => cuerpo.to_string(),
    };
    recorte(&texto)
}

/// ARM puede contestar HTML (proxy corporativo, 502 de una puerta). Recortar es la diferencia entre
/// un error legible y 4 KB de HTML en el panel.
fn recorte(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() <= 300 {
        return s.to_string();
    }
    let cabeza: String = s.chars().take(300).collect();
    format!("{cabeza}…")
}

// ─────────────────────────────────────────────────────────────────────────────
// Token Bearer
// ─────────────────────────────────────────────────────────────────────────────

/// Caché de proceso del token. Vive en el proceso del backend, nunca en disco ni en el frontend.
static CACHE: OnceLock<Mutex<Option<(String, Instant)>>> = OnceLock::new();

fn cache() -> &'static Mutex<Option<(String, Instant)>> {
    CACHE.get_or_init(|| Mutex::new(None))
}

fn cache_leer() -> Option<String> {
    let g = cache().lock().ok()?;
    let (t, cuando) = g.as_ref()?;
    (cuando.elapsed() < Duration::from_secs(VIDA_TOKEN_S)).then(|| t.clone())
}

fn cache_guardar(t: String) {
    if let Ok(mut g) = cache().lock() {
        *g = Some((t, Instant::now()));
    }
}

/// Tira el token cacheado: para cuando ARM contesta 401 (relogin) o el panel pide `?refrescar`.
pub fn olvidar_token() {
    if let Ok(mut g) = cache().lock() {
        *g = None;
    }
}

/// Token OAuth para ARM. Orden: caché → token pegado (entorno / llavero / config) → CLI de Azure.
pub async fn token(data_dir: &Path) -> Result<String, Fallo> {
    if let Some(t) = cache_leer() {
        return Ok(t);
    }
    if let Some(t) = token_pegado(data_dir) {
        cache_guardar(t.clone());
        return Ok(t);
    }
    let t = az_token().await?;
    cache_guardar(t.clone());
    Ok(t)
}

/// Un token de vida larga (o de un service principal) evita depender del CLI instalado y logueado.
fn token_pegado(data_dir: &Path) -> Option<String> {
    for nombre in ["NODEFLOW_AZURE_TOKEN", "AZURE_ACCESS_TOKEN"] {
        if let Ok(v) = std::env::var(nombre) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    crate::claves::obtener("azure_token", data_dir)
}

async fn az_token() -> Result<String, Fallo> {
    let cli = ejecutable_az();
    // El CLI tarda cientos de ms y es bloqueante: fuera del reactor.
    tokio::task::spawn_blocking(move || correr_az(&cli))
        .await
        .map_err(|e| Fallo::Token(format!("no pude esperar al CLI de Azure: {e}")))?
}

const ARGS_TOKEN: [&str; 8] = [
    "account",
    "get-access-token",
    "--resource",
    RECURSO,
    "--query",
    "accessToken",
    "-o",
    "tsv",
];

fn correr_az(cli: &str) -> Result<String, Fallo> {
    let salida = lanzar(cli, &ARGS_TOKEN).map_err(Fallo::Token)?;
    // `az` puede adelantar avisos de versión: el token es la última línea con forma de JWT (dos puntos).
    let token = salida
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && l.matches('.').count() >= 2)
        .last()
        .unwrap_or("")
        .to_string();
    if token.is_empty() {
        return Err(Fallo::Token(format!(
            "`az` no devolvió un token (¿sesión vencida? corré `az login`). Salida: {}",
            recorte(&salida)
        )));
    }
    Ok(token)
}

/// Lanza el CLI con el camino que exista en este Windows.
///
/// `Command::new("az")` **no** funciona: CreateProcess no prueba las extensiones de PATHEXT y muere
/// con WinError 2 (os error 2). Se usa el `az.cmd` real, y si igual falla (`.cmd` lanzado directo da
/// «no es una aplicación Win32 válida» en algunos equipos) se reintenta por `cmd.exe /C`, siempre con
/// CREATE_NO_WINDOW para que no parpadee una consola.
fn lanzar(programa: &str, args: &[&str]) -> Result<String, String> {
    let primer_error = match intentar(programa, args) {
        Ok(s) => return Ok(s),
        Err(e) => e,
    };
    #[cfg(windows)]
    {
        let mut con_cmd = vec!["/C", programa];
        con_cmd.extend_from_slice(args);
        if let Ok(s) = intentar("cmd.exe", &con_cmd) {
            return Ok(s);
        }
    }
    Err(primer_error)
}

/// Ruta del CLI de Azure. `NODEFLOW_AZURE_AZ` la fija a mano; si no, se buscan las instalaciones
/// típicas y se cae en `az.cmd` (que es lo que resuelve el PATH de Windows).
pub fn ejecutable_az() -> String {
    if let Ok(p) = std::env::var("NODEFLOW_AZURE_AZ") {
        let p = p.trim().to_string();
        if !p.is_empty() {
            return p;
        }
    }
    #[cfg(windows)]
    {
        let bases = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"];
        let rels = [
            "Microsoft SDKs/Azure/CLI2/wbin/az.cmd",
            "Microsoft/Azure CLI/wbin/az.cmd",
        ];
        for base in bases {
            let Ok(raiz) = std::env::var(base) else {
                continue;
            };
            for rel in rels {
                let ruta = Path::new(&raiz).join(rel);
                if ruta.is_file() {
                    return ruta.to_string_lossy().to_string();
                }
            }
        }
        "az.cmd".to_string()
    }
    #[cfg(not(windows))]
    {
        "az".to_string()
    }
}

fn intentar(programa: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = std::process::Command::new(programa);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        // Un `az` sin consola no puede preguntar nada: que falle rápido en vez de colgarse esperando.
        .env("AZURE_CORE_ONLY_SHOW_ERRORS", "true")
        .env("AZURE_CORE_NO_COLOR", "true");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }

    let mut hijo = cmd
        .spawn()
        .map_err(|e| format!("no pude lanzar `{programa}`: {e}"))?;

    // Tope **de verdad**: un `wait_with_output()` pelado espera para siempre y colgaría el handler
    // (mismo patrón que el runner de comandos de `agente.rs`). La salida de `az` son unos pocos KB,
    // así que el pipe no se llena mientras se espera.
    let inicio = Instant::now();
    let salida = loop {
        match hijo.try_wait() {
            Ok(Some(_)) => break hijo.wait_with_output().map_err(|e| e.to_string())?,
            Ok(None) => {
                if inicio.elapsed() > Duration::from_secs(TOPE_AZ_S) {
                    let _ = hijo.kill();
                    let _ = hijo.wait();
                    return Err(format!(
                        "`{programa}` tardó más de {TOPE_AZ_S} s y lo corté (¿sesión vencida? corré `az login`)"
                    ));
                }
                std::thread::sleep(Duration::from_millis(120));
            }
            Err(e) => return Err(format!("no pude seguir `{programa}`: {e}")),
        }
    };

    let out = String::from_utf8_lossy(&salida.stdout).to_string();
    let err = String::from_utf8_lossy(&salida.stderr).to_string();
    if !salida.status.success() {
        let texto = if err.trim().is_empty() { out } else { err };
        return Err(format!(
            "`{programa}` salió con {} · {}",
            salida.status.code().unwrap_or(-1),
            recorte(&texto)
        ));
    }
    Ok(if out.trim().is_empty() { err } else { out })
}

// ─────────────────────────────────────────────────────────────────────────────
// Entrada del handler
// ─────────────────────────────────────────────────────────────────────────────

/// Listado listo para el panel. `refrescar` descarta el token cacheado antes de pedir (útil después
/// de un `az login` sin reiniciar la app).
pub async fn consultar(
    http: &reqwest::Client,
    data_dir: &Path,
    q: &HashMap<String, String>,
    refrescar: bool,
) -> Result<Value, Fallo> {
    let destino = destino_desde(data_dir, q)?;
    if refrescar {
        olvidar_token();
    }
    let token = match token(data_dir).await {
        Ok(t) => t,
        Err(e) => return Err(e),
    };
    let (modelos, ignorados, siguiente) = listar(http, &destino, &token).await?;
    // La URL se arma **antes** del `json!`: los campos de `destino` se mueven al objeto.
    let url = destino.url(API_VERSION);
    Ok(json!(Informe {
        ok: true,
        suscripcion: destino.suscripcion,
        grupo: destino.grupo,
        cuenta: destino.cuenta,
        url,
        total: modelos.len(),
        ignorados,
        hay_mas: siguiente.is_some(),
        siguiente,
        modelos,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests — todo lo puro: armado de URL, cascada de configuración, parseo tolerante
// de ARM y traducción de errores. Sin red, sin token y sin CLI de Azure.
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn destino() -> Destino {
        Destino::nuevo("sub-1", "rg-x", "cuenta-y").expect("destino válido")
    }

    /// ¿Esta máquina tiene las variables de entorno que pisan al config? Si las tiene, el entorno
    /// gana **por diseño** y los tests de cascada no aplican: no es un fallo del código.
    fn el_entorno_pisa() -> bool {
        [
            "AZURE_SUBSCRIPTION_ID",
            "NODEFLOW_AZURE_SUBSCRIPTION",
            "AZURE_RESOURCE_GROUP",
            "NODEFLOW_AZURE_RG",
            "AZURE_COGNITIVE_ACCOUNT",
            "NODEFLOW_AZURE_ACCOUNT",
        ]
        .iter()
        .any(|k| std::env::var(k).map(|v| !v.trim().is_empty()).unwrap_or(false))
    }

    // ── Destino ──────────────────────────────────────────────────────────────

    #[test]
    fn un_destino_vacio_dice_que_falta_y_donde_ponerlo() {
        let e = Destino::nuevo("  ", "\"\"", "").unwrap_err();
        let texto = e.to_string();
        for campo in ["suscripcion", "grupo", "cuenta"] {
            assert!(texto.contains(campo), "el error tiene que nombrar «{campo}»: {texto}");
        }
        assert!(
            texto.contains("nodeflow.config.json"),
            "tiene que decir dónde configurarlo: {texto}"
        );
        assert_eq!(e.codigo(), 400, "lo que le falta al usuario es 400, no 502");
    }

    #[test]
    fn el_destino_saca_espacios_y_comillas_de_lo_pegado() {
        // Pegar desde el portal arrastra comillas y espacios: no puede romper la ruta.
        let d = Destino::nuevo("  \"sub-1\" ", "\trg-x\n", " cuenta-y ").unwrap();
        assert_eq!(d.suscripcion, "sub-1");
        assert_eq!(d.grupo, "rg-x");
        assert_eq!(d.cuenta, "cuenta-y");
    }

    #[test]
    fn la_url_arma_la_ruta_de_arm_con_la_version_pedida() {
        assert_eq!(
            destino().url("2024-10-01"),
            "https://management.azure.com/subscriptions/sub-1/resourceGroups/rg-x/providers/\
             Microsoft.CognitiveServices/accounts/cuenta-y/deployments?api-version=2024-10-01"
        );
    }

    #[test]
    fn un_nombre_con_espacios_o_acentos_no_rompe_la_url() {
        let d = Destino::nuevo("s", "mi grupo", "cuenta ñ").unwrap();
        let url = d.url(API_VERSION);
        assert!(
            url.contains("resourceGroups/mi%20grupo"),
            "el espacio va percent-encoded: {url}"
        );
        assert!(
            url.contains("/accounts/cuenta%20%C3%B1"),
            "el acento va percent-encoded: {url}"
        );
        assert!(!url.contains(' '), "no puede quedar un espacio crudo en la URL");
    }

    #[test]
    fn codificar_deja_los_caracteres_seguros_y_escapa_el_resto() {
        assert_eq!(codificar("aZ0-._~"), "aZ0-._~", "los seguros no se tocan");
        assert_eq!(codificar("a/b"), "a%2Fb", "la barra se escapa: no puede cambiar la ruta");
        assert_eq!(codificar("a:b"), "a%3Ab");
        assert_eq!(codificar("a b"), "a%20b");
    }

    // ── Cascada de configuración ─────────────────────────────────────────────

    #[test]
    fn la_query_gana_sobre_todo_lo_demas() {
        // Determinista: no depende de lo que haya configurado en esta máquina.
        let dir = std::env::temp_dir().join("nf-azure-test-query");
        let mut q = HashMap::new();
        q.insert("suscripcion".to_string(), "  sub-q  ".to_string());
        q.insert("grupo".to_string(), "rg-q".to_string());
        q.insert("cuenta".to_string(), "cuenta-q".to_string());
        let d = destino_desde(&dir, &q).unwrap();
        assert_eq!(
            (d.suscripcion.as_str(), d.grupo.as_str(), d.cuenta.as_str()),
            ("sub-q", "rg-q", "cuenta-q")
        );
    }

    #[test]
    fn sin_query_cae_al_config_del_usuario() {
        if el_entorno_pisa() {
            return;
        }
        let dir = std::env::temp_dir().join("nf-azure-test-config");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("nodeflow.config.json"),
            r#"{"azure":{"suscripcion":"sub-cfg","grupo":"rg-cfg","cuenta":"cuenta-cfg"}}"#,
        )
        .unwrap();
        let d = destino_desde(&dir, &HashMap::new()).unwrap();
        assert_eq!(
            (d.suscripcion.as_str(), d.grupo.as_str(), d.cuenta.as_str()),
            ("sub-cfg", "rg-cfg", "cuenta-cfg")
        );
    }

    #[test]
    fn sin_nada_configurado_el_error_lo_explica() {
        if el_entorno_pisa() {
            return;
        }
        let dir = std::env::temp_dir().join("nf-azure-test-vacio");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::remove_file(dir.join("nodeflow.config.json"));
        let e = destino_desde(&dir, &HashMap::new()).unwrap_err();
        assert!(e.to_string().contains("faltan datos de Azure"), "{e}");
    }

    // ── Parseo tolerante de la respuesta de ARM ──────────────────────────────

    /// Forma real del listado de ARM (recortada), más tres entradas que NO tienen que entrar.
    const PAGINA_REAL: &str = r#"{
      "value": [
        {
          "id": "/subscriptions/s/resourceGroups/rg/providers/Microsoft.CognitiveServices/accounts/a/deployments/gpt-5-mini",
          "name": "gpt-5-mini",
          "sku": { "name": "GlobalStandard", "capacity": 50 },
          "properties": {
            "provisioningState": "Succeeded",
            "model": { "format": "OpenAI", "name": "gpt-5-mini", "version": "2025-08-07" },
            "raiPolicyName": "Microsoft.Default"
          }
        },
        {
          "name": "text-embedding-3-small",
          "sku": { "name": "Standard", "capacity": "120" }
        },
        { "name": "   " },
        { "sin": "nombre" }
      ],
      "nextLink": "https://management.azure.com/.../deployments?$skipToken=abc"
    }"#;

    fn leer(cuerpo: &str) -> (Vec<Modelo>, usize, Option<String>) {
        let pagina: Pagina = serde_json::from_str(cuerpo).expect("la página tiene que parsear");
        modelos_de_pagina(&pagina)
    }

    #[test]
    fn la_pagina_real_de_arm_se_lee_entera() {
        let (modelos, ignorados, siguiente) = leer(PAGINA_REAL);
        assert_eq!(modelos.len(), 2, "los dos despliegues con nombre entran");
        assert_eq!(modelos[0].despliegue, "gpt-5-mini");
        assert_eq!(modelos[0].modelo, "gpt-5-mini");
        assert_eq!(modelos[0].version.as_deref(), Some("2025-08-07"));
        assert_eq!(modelos[0].formato.as_deref(), Some("OpenAI"));
        assert_eq!(modelos[0].estado.as_deref(), Some("Succeeded"));
        assert_eq!(modelos[0].sku.as_deref(), Some("GlobalStandard"));
        assert_eq!(modelos[0].capacidad, Some(50));
        assert_eq!(modelos[0].politica.as_deref(), Some("Microsoft.Default"));
        assert!(modelos[0]
            .recurso
            .as_deref()
            .unwrap()
            .ends_with("/gpt-5-mini"));
        assert_eq!(
            ignorados, 2,
            "sin nombre (o en blanco) se cuenta como ignorado, no se inventa"
        );
        assert!(siguiente.is_some(), "ARM pagina: hay que reportarlo");
    }

    #[test]
    fn una_entrada_deforme_no_tira_abajo_la_lista() {
        // El invariante del módulo: `value[]` se parsea entrada por entrada.
        let (modelos, ignorados, _) = leer(r#"{"value":[{"name":"bueno"}, 42, {"name":"otro"}]}"#);
        assert_eq!(modelos.len(), 2, "las buenas sobreviven");
        assert_eq!(ignorados, 1, "la deforme se cuenta");
    }

    #[test]
    fn capacity_como_texto_o_numero_da_lo_mismo() {
        // Medido: algún backend manda la capacidad como string.
        let (modelos, _, _) = leer(PAGINA_REAL);
        assert_eq!(modelos[1].capacidad, Some(120), "capacity \"120\" tiene que leerse como 120");
    }

    #[test]
    fn una_capacidad_ilegible_no_convierte_el_despliegue_en_ignorado() {
        let (modelos, ignorados, _) = leer(r#"{"value":[{"name":"x","sku":{"capacity":"muchos"}}]}"#);
        assert_eq!(modelos.len(), 1);
        assert_eq!(ignorados, 0, "un campo raro no borra un despliegue que sí tiene nombre");
        assert_eq!(modelos[0].capacidad, None);
    }

    #[test]
    fn un_value_vacio_es_una_lista_vacia_y_no_un_error() {
        let (modelos, ignorados, siguiente) = leer(r#"{"value":[]}"#);
        assert!(modelos.is_empty());
        assert_eq!(ignorados, 0);
        assert!(siguiente.is_none());
    }

    #[test]
    fn un_nextlink_en_blanco_no_afirma_que_haya_mas() {
        let (_, _, siguiente) = leer(r#"{"value":[],"nextLink":"   "}"#);
        assert!(siguiente.is_none(), "un nextLink en blanco no es paginación");
    }

    // ── Traducción de errores ────────────────────────────────────────────────

    #[test]
    fn el_401_dice_que_hacer_y_trae_el_detalle() {
        let m = mensaje_de_arm(
            401,
            r#"{"error":{"code":"InvalidAuthenticationToken","message":"token vencido"}}"#,
        );
        assert!(m.contains("az login"), "tiene que decir qué correr: {m}");
        assert!(m.contains("InvalidAuthenticationToken"), "y traer el detalle: {m}");
    }

    #[test]
    fn cada_estado_de_arm_se_traduce_a_algo_accionable() {
        assert!(mensaje_de_arm(403, "{}").contains("Lector"));
        assert!(mensaje_de_arm(404, "{}").contains("no existe"));
        assert!(mensaje_de_arm(429, "{}").contains("limitando"));
        assert!(mensaje_de_arm(500, "{}").contains("500"));
    }

    #[test]
    fn un_error_sin_json_igual_da_un_mensaje_legible() {
        // Medido: una puerta corporativa puede contestar HTML en vez de JSON.
        let m = detalle_arm("<html><body>502 Bad Gateway</body></html>");
        assert!(m.contains("502 Bad Gateway"));
    }

    #[test]
    fn el_recorte_corta_el_html_largo_y_lo_marca() {
        let html = "x".repeat(4000);
        let r = recorte(&html);
        assert_eq!(r.chars().count(), 301, "300 + el carácter de corte");
        assert!(r.ends_with('…'));
        assert_eq!(recorte("  corto  "), "corto", "lo corto se recorta de espacios, no de largo");
    }
}

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
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string());
    Ok((modelos, ignorados, siguiente))
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

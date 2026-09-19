//! Motores de reconocimiento de voz (STT), **declarativos**.
//!
//! Por qué existe: el bucle hablado nació cableado a Speechmatics — URL, protocolo y emisión de token
//! dentro de `server.rs`. Eso ata el producto a un proveedor: cambiar de motor obligaba a tocar el
//! backend y el frontend. Acá el motor pasa a ser un **dato** (id, URL, protocolo, campo de clave,
//! idiomas soportados) y sumar uno nuevo es una entrada en `CATALOGO` más su rama en `abrir_sesion`.
//!
//! Dos reglas que no se negocian:
//!
//! 1. **La clave de cuenta nunca sale del backend.** El frontend recibe un token temporal de vida corta
//!    (`abrir_sesion`), igual que ya hace con Speechmatics.
//! 2. **Ningún idioma se promete si el proveedor no lo soporta.** `idioma_efectivo` degrada al idioma
//!    soportado y devuelve un **aviso explícito** en vez de fallar en silencio a mitad de una frase.
//!
//! Módulo puro salvo `abrir_sesion` (una llamada HTTP por sesión); el resto son funciones sin efectos
//! para poder testearlas sin red ni claves.

use serde_json::{json, Value};
use std::path::Path;

/// Un motor de voz disponible. Todos los campos son estáticos a propósito: el catálogo se compila y no
/// depende de configuración, así una instalación no puede quedar con un motor a medio declarar.
pub struct Proveedor {
    pub id: &'static str,
    pub etiqueta: &'static str,
    /// URL del WebSocket de reconocimiento.
    pub url: &'static str,
    /// Protocolo del WebSocket: decide qué cliente usa el frontend.
    pub protocolo: &'static str,
    /// Variables de entorno donde puede estar la clave (en este orden).
    pub clave_env: &'static [&'static str],
    /// Campos de `nodeflow.config.json` donde puede estar la clave (en este orden).
    pub clave_config: &'static [&'static str],
    /// `"*"` = multilingüe. Si no, la lista de idiomas soportados separados por coma (`"en"`).
    pub idiomas: &'static str,
    /// Lo que hay que decirle al usuario sobre este motor.
    pub nota: &'static str,
}

/// El catálogo. Agregar un motor es agregar una entrada acá y una rama en `abrir_sesion`.
pub const CATALOGO: &[Proveedor] = &[
    Proveedor {
        id: "speechmatics",
        etiqueta: "Speechmatics Realtime",
        url: "wss://eu.rt.speechmatics.com/v2",
        protocolo: "speechmatics-v2",
        clave_env: &["SPEECHMATICS_API_KEY", "SPEECHMATICS_KEY"],
        clave_config: &["speechmatics_api_key", "SPEECHMATICS_API_KEY"],
        idiomas: "*",
        nota: "Multilingüe (55+ idiomas) con parciales sub-segundo. Token temporal de 300 s.",
    },
    Proveedor {
        id: "assemblyai",
        etiqueta: "AssemblyAI Universal-Streaming",
        url: "wss://streaming.assemblyai.com/v3/ws",
        protocolo: "assemblyai-v3",
        clave_env: &["ASSEMBLYAI_API_KEY"],
        clave_config: &["assemblyai_api_key", "ASSEMBLYAI_API_KEY"],
        idiomas: "en, es, de, fr",
        nota: "Universal-Streaming multilingüe (en, es, de, fr). Verificado en vivo el 16/09 contra el WS v3: transcribió castellano tal cual, sin parámetro de idioma ni de modelo. Token temporal de hasta 600 s.",
    },
];

/// Motor por defecto cuando la configuración no dice nada.
pub const POR_DEFECTO: &str = "speechmatics";

/// Modelo por defecto de cada motor, en **un solo lugar**: lo usan la sesión real (`abrir_sesion`)
/// y lo que se muestra en el panel (`voz_estado`). Estaban duplicados y por eso el panel podía
/// anunciar el modelo de un motor mientras la sesión usaba el de otro (`enhanced` vs `u3-rt-pro`).
pub fn modelo_por_defecto(id: &str) -> &'static str {
    match id {
        "speechmatics" => "enhanced",
        "assemblyai" => "u3-rt-pro",
        _ => "",
    }
}

/// Formato de audio que usan los dos motores del catálogo (y el que captura el frontend).
pub const CODEC: &str = "pcm_s16le 16000 Hz";

/// Clave del `nodeflow.config.json` que guarda la elección (misma familia que `motor_activo`).
pub const CLAVE_CONFIG: &str = "stt_proveedor";

pub fn por_id(id: &str) -> Option<&'static Proveedor> {
    let id = id.trim().to_lowercase();
    CATALOGO.iter().find(|p| p.id == id)
}

/// El motor elegido en el config, o el de por defecto si no hay nada válido guardado.
pub fn seleccionado(data_dir: &Path) -> String {
    leer_config(data_dir)
        .and_then(|v| v[CLAVE_CONFIG].as_str().map(|s| s.trim().to_lowercase()))
        .filter(|id| por_id(id).is_some())
        .unwrap_or_else(|| POR_DEFECTO.to_string())
}

/// Guarda la elección **preservando el resto del config** (claves, tarifas, vault_path…).
/// Un id desconocido se rechaza: es mejor un error que dejar la app sin motor.
pub fn guardar_seleccion(data_dir: &Path, id: &str) -> Result<(), String> {
    let id = id.trim().to_lowercase();
    if por_id(&id).is_none() {
        return Err(format!(
            "Motor de voz desconocido: «{id}». Disponibles: {}",
            ids().join(", ")
        ));
    }
    let mut cfg = leer_config(data_dir).unwrap_or_else(|| json!({}));
    if !cfg.is_object() {
        cfg = json!({});
    }
    cfg[CLAVE_CONFIG] = json!(id);
    escribir_config(data_dir, &cfg)
}

pub fn ids() -> Vec<&'static str> {
    CATALOGO.iter().map(|p| p.id).collect()
}

/// El catálogo como JSON, para que el frontend no tenga que conocerlo de antemano.
///
/// Recibe `data_dir` a propósito: el flag `clave_configurada` tiene que mirar el MISMO config que usa
/// la emisión del token. Sin el directorio, la lista diría "sin clave" mientras el estado dice que sí.
pub fn catalogo_json(data_dir: &Path) -> Value {
    Value::Array(
        CATALOGO
            .iter()
            .map(|p| {
                json!({
                    "id": p.id,
                    "etiqueta": p.etiqueta,
                    "url": p.url,
                    "protocolo": p.protocolo,
                    "idiomas": p.idiomas,
                    "nota": p.nota,
                    "clave_configurada": clave_de(data_dir, p).is_some(),
                })
            })
            .collect(),
    )
}

/// Clave del motor: entorno → `.env` del proyecto (dev) → `nodeflow.config.json`.
/// Nunca se loguea el valor, sólo de dónde salió.
pub fn clave_de(data_dir: &Path, prov: &Proveedor) -> Option<String> {
    // Resolución **central** (`claves.rs`): entorno → `.env` → llavero del sistema → config.
    // Tiene que pasar por acá: si no, mover las claves al Credential Manager deja a la voz sin clave
    // (pasó el 15/09 — el test de persistencia la habría detectado, esta ruta no la estaba usando).
    for campo in prov.clave_config {
        if let Some(v) = crate::claves::obtener(campo, data_dir) {
            return Some(v);
        }
    }
    // El catálogo puede declarar variables de entorno que `claves` no conoce (proveedores nuevos).
    clave_de_entorno_o_disco(Some(data_dir), prov)
}

fn clave_de_entorno_o_disco(data_dir: Option<&Path>, prov: &Proveedor) -> Option<String> {
    for var in prov.clave_env {
        if let Ok(v) = std::env::var(var) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    for candidate in ["../.env", ".env"] {
        if let Ok(txt) = std::fs::read_to_string(candidate) {
            for line in txt.lines() {
                let line = line.trim();
                for campo in prov.clave_env {
                    if let Some(rest) = line.strip_prefix(&format!("{campo}=")) {
                        let v = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                        if !v.is_empty() {
                            log::info!("voz: clave de {} leída desde {candidate}", prov.id);
                            return Some(v);
                        }
                    }
                }
            }
        }
    }
    let data_dir = data_dir?;
    let txt = std::fs::read_to_string(data_dir.join("nodeflow.config.json")).ok()?;
    let v: Value = serde_json::from_str(&txt).ok()?;
    for campo in prov.clave_config {
        if let Some(k) = v[campo].as_str() {
            let k = k.trim().to_string();
            if !k.is_empty() {
                log::info!("voz: clave de {} leída desde nodeflow.config.json", prov.id);
                return Some(k);
            }
        }
    }
    None
}

/// ¿El motor soporta este idioma? `"*"` es multilingüe.
pub fn soporta(prov: &Proveedor, idioma: &str) -> bool {
    if prov.idiomas == "*" {
        return true;
    }
    let pedido = idioma.trim().to_lowercase();
    let base = pedido.split(['-', '_']).next().unwrap_or("").to_string();
    prov.idiomas
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .any(|s| s == pedido || s == base)
}

/// Idioma que se va a usar **de verdad**, con aviso cuando el pedido no se puede cumplir.
/// Devuelve `(idioma_efectivo, aviso)`.
pub fn idioma_efectivo(prov: &Proveedor, pedido: &str) -> (String, Option<String>) {
    let pedido = pedido.trim();
    let pedido = if pedido.is_empty() { "es" } else { pedido };
    if soporta(prov, pedido) {
        return (pedido.to_lowercase(), None);
    }
    let primero = prov
        .idiomas
        .split(',')
        .next()
        .unwrap_or("en")
        .trim()
        .to_lowercase();
    (
        primero.clone(),
        Some(format!(
            "{} todavía no transcribe «{}». La sesión se abre en «{}»: dictá en ese idioma o elegí otro motor.",
            prov.etiqueta, pedido, primero
        )),
    )
}

/// Una sesión de reconocimiento lista para que el frontend abra el WebSocket.
pub struct Sesion {
    pub proveedor: String,
    pub etiqueta: String,
    pub protocolo: String,
    pub token: String,
    pub url: String,
    pub idioma: String,
    pub modelo: String,
    pub expira_en_s: u64,
    pub codec: String,
    pub aviso: Option<String>,
}

impl Sesion {
    pub fn a_json(&self) -> Value {
        let mut v = json!({
            "success": true,
            "proveedor": self.proveedor,
            "etiqueta": self.etiqueta,
            "protocolo": self.protocolo,
            "token": self.token,
            // `jwt` se mantiene por compatibilidad con el cliente actual del panel de voz.
            "jwt": self.token,
            "url": self.url,
            "idioma": self.idioma,
            "modelo": self.modelo,
            "expira_en_s": self.expira_en_s,
            "codec": self.codec,
        });
        if let Some(a) = &self.aviso {
            v["aviso"] = json!(a);
        }
        v
    }
}

/// Pide el token temporal al motor y arma la sesión.
///
/// - **speechmatics**: `POST /v1/api_keys?type=rt` con `Authorization: Bearer <clave>` → `key_value`, 300 s.
/// - **assemblyai**: `GET /v3/token?expires_in_seconds=600` con `Authorization: <clave>` (**cruda**, sin
///   `Bearer`) → `token`, hasta 600 s.
pub async fn abrir_sesion(
    http: &reqwest::Client,
    prov: &Proveedor,
    clave: &str,
    idioma_pedido: &str,
    modelo_pedido: &str,
) -> Result<Sesion, String> {
    let (idioma, aviso) = idioma_efectivo(prov, idioma_pedido);
    let (token, expira_en_s) = match prov.id {
        "speechmatics" => {
            let r = http
                .post("https://mp.speechmatics.com/v1/api_keys?type=rt")
                .header("Authorization", format!("Bearer {clave}"))
                .json(&json!({ "ttl": 300 }))
                .send()
                .await
                .map_err(|e| format!("No pude hablar con Speechmatics: {e}"))?;
            let code = r.status();
            let body: Value = r.json().await.unwrap_or(json!({}));
            if !code.is_success() {
                let detalle = body["error"]
                    .as_str()
                    .or(body["message"].as_str())
                    .unwrap_or("sin detalle");
                return Err(format!("Speechmatics rechazó la clave ({code}): {detalle}"));
            }
            let t = body["key_value"].as_str().unwrap_or("").trim().to_string();
            if t.is_empty() {
                return Err("Speechmatics no devolvió token temporal.".to_string());
            }
            (t, 300)
        }
        "assemblyai" => {
            let r = http
                .get("https://streaming.assemblyai.com/v3/token?expires_in_seconds=600")
                .header("Authorization", clave)
                .send()
                .await
                .map_err(|e| format!("No pude hablar con AssemblyAI: {e}"))?;
            let code = r.status();
            let body: Value = r.json().await.unwrap_or(json!({}));
            if !code.is_success() {
                let detalle = body["error"]
                    .as_str()
                    .or(body["message"].as_str())
                    .unwrap_or("sin detalle");
                return Err(format!("AssemblyAI rechazó la clave ({code}): {detalle}"));
            }
            let t = body["token"].as_str().unwrap_or("").trim().to_string();
            if t.is_empty() {
                // Sin inventar el campo: se listan las claves que sí vinieron, para poder diagnosticar.
                let claves: Vec<String> = body
                    .as_object()
                    .map(|o| o.keys().cloned().collect())
                    .unwrap_or_default();
                return Err(format!(
                    "AssemblyAI no devolvió token temporal (campos: {}).",
                    if claves.is_empty() {
                        "ninguno".to_string()
                    } else {
                        claves.join(", ")
                    }
                ));
            }
            (t, 600)
        }
        otro => {
            return Err(format!(
                "El motor «{otro}» no tiene emisión de token implementada."
            ))
        }
    };

    let modelo = if modelo_pedido.trim().is_empty() {
        crate::stt::modelo_por_defecto(prov.id).to_string()
    } else {
        modelo_pedido.trim().to_lowercase()
    };

    Ok(Sesion {
        proveedor: prov.id.to_string(),
        etiqueta: prov.etiqueta.to_string(),
        protocolo: prov.protocolo.to_string(),
        token,
        url: prov.url.to_string(),
        idioma,
        modelo,
        expira_en_s,
        codec: "pcm_s16le 16000 Hz".to_string(),
        aviso,
    })
}

fn leer_config(data_dir: &Path) -> Option<Value> {
    let txt = std::fs::read_to_string(data_dir.join("nodeflow.config.json")).ok()?;
    serde_json::from_str(&txt).ok()
}

fn escribir_config(data_dir: &Path, cfg: &Value) -> Result<(), String> {
    let ruta = data_dir.join("nodeflow.config.json");
    let txt = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    crate::estado::escribir_atomico(&ruta, &txt).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_de_prueba(nombre: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("nodeflow-stt-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn el_catalogo_es_coherente() {
        let ids = ids();
        let mut unicos = ids.clone();
        unicos.sort();
        unicos.dedup();
        assert_eq!(unicos.len(), ids.len(), "ids repetidos en el catálogo");
        for p in CATALOGO {
            assert!(
                p.url.starts_with("wss://"),
                "{} no apunta a un WebSocket",
                p.id
            );
            assert!(!p.protocolo.is_empty(), "{} sin protocolo", p.id);
            assert!(por_id(p.id).is_some());
            assert!(!p.nota.is_empty(), "{} sin nota para el usuario", p.id);
        }
        assert!(
            por_id("SPEECHMATICS").is_some(),
            "el id debe resolverse sin importar mayúsculas"
        );
        assert!(por_id("no-existe").is_none());
    }

    #[test]
    fn sin_config_gana_el_motor_por_defecto() {
        let d = dir_de_prueba("defecto");
        assert_eq!(seleccionado(&d), POR_DEFECTO);
    }

    #[test]
    fn un_id_invalido_en_config_cae_al_por_defecto() {
        let d = dir_de_prueba("invalido");
        std::fs::write(
            d.join("nodeflow.config.json"),
            r#"{"stt_proveedor":"inventado"}"#,
        )
        .unwrap();
        assert_eq!(seleccionado(&d), POR_DEFECTO);
    }

    #[test]
    fn guardar_y_leer_conserva_el_resto_del_config() {
        let d = dir_de_prueba("preserva");
        std::fs::write(
            d.join("nodeflow.config.json"),
            r#"{"speechmatics_api_key":"NO_TOCAR","vault_path":"X"}"#,
        )
        .unwrap();
        guardar_seleccion(&d, "assemblyai").unwrap();
        assert_eq!(seleccionado(&d), "assemblyai");
        let v: Value =
            serde_json::from_str(&std::fs::read_to_string(d.join("nodeflow.config.json")).unwrap())
                .unwrap();
        assert_eq!(
            v["speechmatics_api_key"], "NO_TOCAR",
            "la clave ajena se preserva"
        );
        assert_eq!(v["vault_path"], "X");
        assert_eq!(v[CLAVE_CONFIG], "assemblyai");
    }

    #[test]
    fn guardar_rechaza_un_motor_desconocido() {
        let d = dir_de_prueba("rechaza");
        let e = guardar_seleccion(&d, "chatgpt").unwrap_err();
        assert!(
            e.contains("desconocido"),
            "el error debe explicar el id: {e}"
        );
        assert_eq!(
            seleccionado(&d),
            POR_DEFECTO,
            "no debe quedar guardado nada"
        );
    }

    #[test]
    fn speechmatics_es_multilingue() {
        let p = por_id("speechmatics").unwrap();
        assert_eq!(idioma_efectivo(p, "es"), ("es".to_string(), None));
        assert_eq!(idioma_efectivo(p, "en-US"), ("en-us".to_string(), None));
        assert!(soporta(p, "de"));
    }

    #[test]
    fn assemblyai_transcribe_castellano() {
        let p = por_id("assemblyai").unwrap();
        // Medido en vivo el 16/09 contra el WS v3 real: castellano, sin parámetro de idioma.
        assert_eq!(idioma_efectivo(p, "es"), ("es".to_string(), None));
        assert_eq!(idioma_efectivo(p, "es-AR"), ("es-ar".to_string(), None));
        assert_eq!(idioma_efectivo(p, "en"), ("en".to_string(), None));
        assert!(soporta(p, "de"));
        assert!(soporta(p, "fr"));
        // Un idioma fuera de la lista sigue degradando **con aviso**: nunca en silencio.
        let (idioma, aviso) = idioma_efectivo(p, "ja");
        assert_eq!(idioma, "en", "fuera de la lista debe degradar al primero");
        let aviso = aviso.expect("debe haber aviso: prometer japonés sería mentir");
        assert!(
            aviso.contains("todavía no transcribe"),
            "aviso poco claro: {aviso}"
        );
    }

    #[test]
    fn el_catalogo_json_marca_la_clave_y_no_la_expone() {
        let d = dir_de_prueba("catalogo");
        std::fs::write(
            d.join("nodeflow.config.json"),
            r#"{"speechmatics_api_key":"EXISTE"}"#,
        )
        .unwrap();
        let j = catalogo_json(&d);
        let arr = j.as_array().unwrap();
        assert_eq!(arr.len(), CATALOGO.len());
        for e in arr {
            assert!(e["id"].is_string());
            assert!(e["clave_configurada"].is_boolean());
            assert!(e.get("token").is_none(), "el catálogo nunca lleva secretos");
        }
        let spee = arr.iter().find(|e| e["id"] == "speechmatics").unwrap();
        let ass = arr.iter().find(|e| e["id"] == "assemblyai").unwrap();
        assert_eq!(
            spee["clave_configurada"], true,
            "con la clave en el config, la lista tiene que decir que sí"
        );
        // Sobre `assemblyai` NO se puede afirmar un valor: la resolución mira también el llavero del
        // sistema, así que en una máquina con la clave guardada vale `true` aunque este config de prueba
        // no la tenga (falló así el 16/09, con razón). El test mide el CONTRATO —booleano y sin secreto—
        // no el estado de la máquina.
        assert!(ass["clave_configurada"].is_boolean());
    }
}

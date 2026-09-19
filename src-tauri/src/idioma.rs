//! Idioma de la app: **una sola fuente** para la interfaz, la transcripción y la voz de salida.
//!
//! Por qué existe: si el idioma viviera sólo en el frontend, la voz seguiría dictando y respondiendo
//! en el idioma anterior —el motor de transcripción necesita el código de idioma, y el sintetizador
//! necesita la voz—. Acá se guarda una vez y lo leen los tres.
//!
//! Regla: un idioma desconocido **no se guarda** (mejor un error explícito que una app a medio traducir).

use std::path::Path;

use serde_json::{json, Value};

/// Idiomas soportados hoy.
pub const IDIOMAS: &[&str] = &["es", "en"];

/// Idioma por defecto cuando nadie eligió nada.
pub const POR_DEFECTO: &str = "es";

/// ¿Es un idioma que sabemos hablar?
pub fn es_valido(idioma: &str) -> bool {
    IDIOMAS
        .iter()
        .any(|i| i.eq_ignore_ascii_case(idioma.trim()))
}

fn normalizar(idioma: &str) -> String {
    idioma.trim().to_lowercase()
}

/// Voz de Kokoro que corresponde al idioma (voces nativas, no una voz con acento prestado).
pub fn voz_tts(idioma: &str) -> &'static str {
    match normalizar(idioma).as_str() {
        "en" => "af_bella",
        _ => "ef_dora",
    }
}

/// El config tal como está en disco (objeto vacío si no hay o está roto).
fn leer_cfg(data_dir: &Path) -> Value {
    std::fs::read_to_string(data_dir.join("nodeflow.config.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .unwrap_or_else(|| json!({}))
}

/// Idioma guardado en el config; si no hay o no es válido, el de por defecto.
pub fn actual(data_dir: &Path) -> String {
    let guardado = leer_cfg(data_dir)["idioma"]
        .as_str()
        .unwrap_or("")
        .to_string();
    if es_valido(&guardado) {
        normalizar(&guardado)
    } else {
        POR_DEFECTO.to_string()
    }
}

/// Idioma de la **voz**: con el que el motor de transcripción escucha y con el que responde el
/// sintetizador. No tiene por qué ser el de la interfaz —se puede leer la app en inglés y hablarle en
/// castellano, y al revés—, así que vive aparte en `voz.idioma` del config.
///
/// Sin valor propio, sigue al idioma de la interfaz (el comportamiento de siempre).
pub fn voz_idioma(data_dir: &Path) -> String {
    let guardado = leer_cfg(data_dir)["voz"]["idioma"]
        .as_str()
        .unwrap_or("")
        .to_string();
    if es_valido(&guardado) {
        normalizar(&guardado)
    } else {
        actual(data_dir)
    }
}

/// ¿La voz está siguiendo a la interfaz (sin idioma propio)?
pub fn voz_sigue_a_la_interfaz(data_dir: &Path) -> bool {
    !es_valido(leer_cfg(data_dir)["voz"]["idioma"].as_str().unwrap_or(""))
}

/// Guarda el idioma preservando el resto del config.
pub fn guardar(data_dir: &Path, idioma: &str) -> Result<Value, String> {
    if !es_valido(idioma) {
        return Err(format!(
            "Idioma desconocido: «{idioma}». Soportados: {}",
            IDIOMAS.join(", ")
        ));
    }
    let idioma = normalizar(idioma);
    let ruta = data_dir.join("nodeflow.config.json");
    let mut cfg: Value = std::fs::read_to_string(&ruta)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}));
    let Some(obj) = cfg.as_object_mut() else {
        return Err("config inválido".to_string());
    };
    obj.insert("idioma".into(), json!(idioma.clone()));
    let txt = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    crate::estado::escribir_atomico(&ruta, &txt).map_err(|e| format!("no pude escribir el config: {e}"))?;
    log::info!("idioma: guardado «{idioma}» · voz TTS {}", voz_tts(&idioma));
    Ok(json!({
        "ok": true,
        "idioma": idioma,
        "voz_tts": voz_tts(&idioma),
        "idiomas": IDIOMAS,
    }))
}

/// Guarda el **idioma de la voz** preservando el resto del config. Con `auto` (o vacío) la voz vuelve
/// a seguir al idioma de la interfaz, que es como se comportaba antes de que esta clave existiera.
pub fn guardar_voz_idioma(data_dir: &Path, idioma: &str) -> Result<Value, String> {
    let pedido = normalizar(idioma);
    let auto = pedido.is_empty() || pedido == "auto" || pedido == "interfaz";
    if !auto && !es_valido(&pedido) {
        return Err(format!(
            "Idioma de voz desconocido: «{idioma}». Soportados: {} (o «auto» para seguir a la interfaz)",
            IDIOMAS.join(", ")
        ));
    }
    let ruta = data_dir.join("nodeflow.config.json");
    let mut cfg = leer_cfg(data_dir);
    let Some(obj) = cfg.as_object_mut() else {
        return Err("config inválido".to_string());
    };
    let mut voz = obj
        .get("voz")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    if auto {
        voz.remove("idioma");
    } else {
        voz.insert("idioma".into(), json!(pedido));
    }
    obj.insert("voz".into(), Value::Object(voz));
    let txt = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    crate::estado::escribir_atomico(&ruta, &txt).map_err(|e| format!("no pude escribir el config: {e}"))?;
    let efectivo = voz_idioma(data_dir);
    log::info!("voz: idioma «{efectivo}» · voz TTS {}", voz_tts(&efectivo));
    Ok(json!({
        "ok": true,
        "voz_idioma": efectivo,
        "voz_tts": voz_tts(&efectivo),
        "auto": auto,
        "idiomas": IDIOMAS,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_voz_sigue_a_la_interfaz_si_no_tiene_idioma_propio() {
        let d = dir();
        std::fs::write(d.join("nodeflow.config.json"), r#"{"idioma":"en"}"#).unwrap();
        assert_eq!(
            voz_idioma(&d),
            "en",
            "sin voz.idioma propio, la voz sigue a la interfaz"
        );
        assert!(voz_sigue_a_la_interfaz(&d));
    }

    #[test]
    fn la_voz_puede_ir_por_su_lado() {
        let d = dir();
        std::fs::write(d.join("nodeflow.config.json"), r#"{"idioma":"en"}"#).unwrap();
        let r = guardar_voz_idioma(&d, "es").unwrap();
        assert_eq!(r["voz_idioma"], "es");
        assert_eq!(
            voz_idioma(&d),
            "es",
            "se puede leer la app en inglés y hablarle en castellano"
        );
        assert_eq!(actual(&d), "en", "el idioma de la interfaz no se toca");
        assert!(!voz_sigue_a_la_interfaz(&d));
        // «auto» la devuelve a seguir a la interfaz.
        let r = guardar_voz_idioma(&d, "auto").unwrap();
        assert_eq!(r["auto"], true);
        assert_eq!(voz_idioma(&d), "en");
        assert!(voz_sigue_a_la_interfaz(&d));
    }

    #[test]
    fn un_idioma_de_voz_invalido_no_se_guarda() {
        let d = dir();
        std::fs::write(d.join("nodeflow.config.json"), r#"{"idioma":"es"}"#).unwrap();
        assert!(guardar_voz_idioma(&d, "klingon").is_err());
        assert_eq!(voz_idioma(&d), "es");
    }

    #[test]
    fn el_idioma_de_voz_preserva_el_resto_del_config() {
        let d = dir();
        std::fs::write(
            d.join("nodeflow.config.json"),
            r#"{"vault_path":"X","cerebro":{"repo":"R"}}"#,
        )
        .unwrap();
        guardar_voz_idioma(&d, "en").unwrap();
        let cfg: Value =
            serde_json::from_str(&std::fs::read_to_string(d.join("nodeflow.config.json")).unwrap())
                .unwrap();
        assert_eq!(cfg["vault_path"], "X");
        assert_eq!(cfg["cerebro"]["repo"], "R", "el resto del config no se toca");
    }

    fn dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "nf-idioma-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn sin_config_gana_el_idioma_por_defecto() {
        assert_eq!(actual(&dir()), POR_DEFECTO);
    }

    #[test]
    fn guardar_y_leer_conserva_el_resto_del_config() {
        let d = dir();
        std::fs::write(
            d.join("nodeflow.config.json"),
            r#"{"vault_path":"X","idioma":"es"}"#,
        )
        .unwrap();
        guardar(&d, "en").unwrap();
        assert_eq!(actual(&d), "en");
        let cfg: Value =
            serde_json::from_str(&std::fs::read_to_string(d.join("nodeflow.config.json")).unwrap())
                .unwrap();
        assert_eq!(cfg["vault_path"], "X", "el resto del config no se toca");
    }

    #[test]
    fn un_idioma_invalido_no_se_guarda() {
        let d = dir();
        std::fs::write(d.join("nodeflow.config.json"), r#"{"idioma":"es"}"#).unwrap();
        assert!(guardar(&d, "klingon").is_err());
        assert_eq!(actual(&d), "es", "un idioma inventado no cambia nada");
    }

    #[test]
    fn un_idioma_invalido_en_el_config_cae_al_por_defecto() {
        let d = dir();
        std::fs::write(d.join("nodeflow.config.json"), r#"{"idioma":"xx"}"#).unwrap();
        assert_eq!(actual(&d), POR_DEFECTO);
    }

    #[test]
    fn cada_idioma_tiene_su_voz_nativa() {
        assert_eq!(voz_tts("es"), "ef_dora");
        assert_eq!(voz_tts("en"), "af_bella");
        assert_ne!(voz_tts("es"), voz_tts("en"));
    }

    #[test]
    fn tolera_mayusculas_y_espacios() {
        let d = dir();
        assert!(guardar(&d, "  EN ").is_ok());
        assert_eq!(actual(&d), "en");
    }
}

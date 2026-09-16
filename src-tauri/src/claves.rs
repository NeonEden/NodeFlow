//! Claves de API: **un solo lugar**, y ninguna en texto plano si se puede evitar.
//!
//! Por qué existe: cada módulo resolvía su clave por su cuenta — Gemini en `lib.rs`, los embeddings en
//! `semantica.rs`, Tavily en `investigacion.rs`, la voz en `stt.rs`, los proveedores en `server.rs` — y
//! todas terminaban leyendo el mismo `nodeflow.config.json` **en texto plano**. Eso deja la clave en
//! disco, legible por cualquier proceso, y hace imposible "mover las claves al llavero del sistema" sin
//! tocar cinco lugares distintos.
//!
//! Orden de resolución (gana el primero que responde):
//!   1. **entorno** — desarrollo y CI. Nunca se escribe nada.
//!   2. **`.env` del proyecto** — sólo modo desarrollo, como antes.
//!   3. **llavero del sistema operativo** — Windows Credential Manager. Destino de la migración.
//!   4. **config en texto plano** — legado: sigue funcionando, pero se reporta y se puede migrar.
//!
//! Regla que no se rompe: la clave **nunca** sale al frontend. `estado()` dice dónde está y publica una
//! huella no invertible; jamás el valor.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use serde_json::{json, Value};

/// Nombre del servicio en el llavero del sistema (Windows Credential Manager).
pub const SERVICIO: &str = "com.nodeflow.desktop";

/// Campos de clave conocidos. Los proveedores que el usuario agrega desde la UI generan campos
/// dinámicos (`<id>_api_key`) que también se reconocen, sin declararlos acá.
pub struct Campo {
    pub config: &'static str,
    pub etiqueta: &'static str,
    /// Nombres de variable de entorno alternativos, además del obvio (`CAMPO_MAYÚSCULAS`).
    pub env_extra: &'static [&'static str],
}

pub const CAMPOS: &[Campo] = &[
    Campo {
        config: "deepseek_api_key",
        etiqueta: "DeepSeek",
        env_extra: &[],
    },
    Campo {
        config: "gemini_api_key",
        etiqueta: "Gemini",
        env_extra: &["GOOGLE_API_KEY"],
    },
    Campo {
        config: "speechmatics_api_key",
        etiqueta: "Speechmatics",
        env_extra: &["SPEECHMATICS_KEY"],
    },
    Campo {
        config: "assemblyai_api_key",
        etiqueta: "AssemblyAI",
        env_extra: &[],
    },
    Campo {
        config: "tavily_api_key",
        etiqueta: "Tavily",
        env_extra: &[],
    },
];

/// ¿Este nombre de campo del config guarda una clave? (`deepseek_api_key`, `groq_api_key`, …)
pub fn es_campo_de_clave(nombre: &str) -> bool {
    let n = nombre.to_lowercase();
    n.ends_with("_api_key") || n.ends_with("_apikey") || n == "api_key"
}

/// Etiqueta legible de un campo (cae al propio nombre si es un proveedor agregado por el usuario).
pub fn etiqueta(campo: &str) -> String {
    CAMPOS
        .iter()
        .find(|c| c.config.eq_ignore_ascii_case(campo))
        .map(|c| c.etiqueta.to_string())
        .unwrap_or_else(|| campo.to_string())
}

/// Nombres de variable de entorno que se prueban, en orden.
pub fn env_names(campo: &str) -> Vec<String> {
    let mut v = vec![campo.to_uppercase()];
    if let Some(c) = CAMPOS.iter().find(|c| c.config.eq_ignore_ascii_case(campo)) {
        v.extend(c.env_extra.iter().map(|s| s.to_string()));
    }
    v
}

/// Dónde apareció la clave. Es información de diagnóstico, nunca el valor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origen {
    Entorno,
    EnvFile,
    Llavero,
    ConfigTextoPlano,
}

impl Origen {
    pub fn como_str(self) -> &'static str {
        match self {
            Origen::Entorno => "entorno",
            Origen::EnvFile => "env-file",
            Origen::Llavero => "llavero",
            Origen::ConfigTextoPlano => "texto-plano",
        }
    }
}

pub struct Resuelta {
    pub valor: String,
    pub origen: Origen,
}

/// Almacén de secretos. Se abstrae para poder testear sin tocar el llavero de la máquina.
pub trait Store: Send + Sync {
    fn leer(&self, campo: &str) -> Option<String>;
    fn escribir(&self, campo: &str, valor: &str) -> Result<(), String>;
    fn borrar(&self, campo: &str) -> Result<(), String>;
}

/// Implementación real: Windows Credential Manager (vía `keyring`).
pub struct Llavero;

impl Store for Llavero {
    fn leer(&self, campo: &str) -> Option<String> {
        let e = keyring::Entry::new(SERVICIO, campo).ok()?;
        match e.get_password() {
            Ok(v) => Some(v),
            Err(_) => None,
        }
    }

    fn escribir(&self, campo: &str, valor: &str) -> Result<(), String> {
        let e = keyring::Entry::new(SERVICIO, campo).map_err(|e| e.to_string())?;
        e.set_password(valor).map_err(|e| e.to_string())
    }

    fn borrar(&self, campo: &str) -> Result<(), String> {
        let e = keyring::Entry::new(SERVICIO, campo).map_err(|e| e.to_string())?;
        match e.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Almacén en memoria: para tests y para simulaciones sin tocar el sistema.
#[derive(Default)]
pub struct Memoria(pub Mutex<HashMap<String, String>>);

impl Memoria {
    pub fn nueva(pares: &[(&str, &str)]) -> Self {
        let m = Memoria::default();
        {
            let mut g = m.0.lock().unwrap();
            for (k, v) in pares {
                g.insert((*k).to_string(), (*v).to_string());
            }
        }
        m
    }
}

impl Store for Memoria {
    fn leer(&self, campo: &str) -> Option<String> {
        self.0.lock().unwrap().get(campo).cloned()
    }
    fn escribir(&self, campo: &str, valor: &str) -> Result<(), String> {
        self.0
            .lock()
            .unwrap()
            .insert(campo.to_string(), valor.to_string());
        Ok(())
    }
    fn borrar(&self, campo: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(campo);
        Ok(())
    }
}

/// Huella corta y no invertible de un secreto: sirve para verificar que el valor correcto llegó sin
/// publicarlo (comparar dos huellas confirma que es el mismo valor).
pub fn huella(valor: &str) -> String {
    // FNV-1a de 64 bits, colapsado a 32 para que sea corto.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in valor.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:08x}", (h ^ (h >> 32)) as u32)
}

fn leer_env_file(txt: &str, nombre: &str) -> Option<String> {
    let prefijo = format!("{nombre}=");
    for linea in txt.lines() {
        let l = linea.trim();
        if let Some(resto) = l.strip_prefix(&prefijo) {
            let v = resto
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

fn leer_config(data_dir: &Path) -> Value {
    std::fs::read_to_string(data_dir.join("nodeflow.config.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .unwrap_or_else(|| json!({}))
}

/// Valor de un campo de clave en el config en texto plano (incluye la variante en mayúsculas).
pub fn valor_config(data_dir: &Path, campo: &str) -> Option<String> {
    let cfg = leer_config(data_dir);
    for nombre in [campo.to_string(), campo.to_uppercase()] {
        if let Some(v) = cfg[nombre].as_str() {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

/// Resuelve una clave con un almacén dado (testeable).
pub fn obtener_con(campo: &str, data_dir: &Path, store: &dyn Store) -> Option<Resuelta> {
    for nombre in env_names(campo) {
        if let Ok(v) = std::env::var(&nombre) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Some(Resuelta {
                    valor: v,
                    origen: Origen::Entorno,
                });
            }
        }
    }
    for candidato in ["../.env", ".env"] {
        if let Ok(txt) = std::fs::read_to_string(candidato) {
            for nombre in env_names(campo) {
                if let Some(v) = leer_env_file(&txt, &nombre) {
                    return Some(Resuelta {
                        valor: v,
                        origen: Origen::EnvFile,
                    });
                }
            }
        }
    }
    if let Some(v) = store.leer(campo) {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return Some(Resuelta {
                valor: v,
                origen: Origen::Llavero,
            });
        }
    }
    valor_config(data_dir, campo).map(|valor| Resuelta {
        valor,
        origen: Origen::ConfigTextoPlano,
    })
}

/// Resuelve una clave con el llavero real.
pub fn obtener(campo: &str, data_dir: &Path) -> Option<String> {
    obtener_con(campo, data_dir, &Llavero).map(|r| r.valor)
}

/// ¿Hay una clave disponible para este campo, sea de donde sea?
pub fn disponible(campo: &str, data_dir: &Path) -> bool {
    obtener(campo, data_dir).is_some()
}

/// Mueve las claves del config en texto plano al llavero y las borra del archivo.
///
/// Es idempotente: lo que ya está en el llavero no se toca. Si el llavero falla, el valor **no** se
/// borra del config: nunca se pierde una clave por una migración fallida.
pub fn migrar(data_dir: &Path, store: &dyn Store) -> Result<Value, String> {
    let ruta = data_dir.join("nodeflow.config.json");
    let txt = match std::fs::read_to_string(&ruta) {
        Ok(t) => t,
        Err(e) => return Err(format!("no pude leer nodeflow.config.json: {e}")),
    };
    let mut cfg: Value = serde_json::from_str(&txt).map_err(|e| format!("config inválido: {e}"))?;

    // Campos declarados + cualquier campo dinámico de proveedor que esté en el archivo.
    let mut nombres: Vec<String> = CAMPOS.iter().map(|c| c.config.to_string()).collect();
    if let Some(obj) = cfg.as_object() {
        for k in obj.keys() {
            if es_campo_de_clave(k) && !nombres.iter().any(|n| n.eq_ignore_ascii_case(k)) {
                nombres.push(k.clone());
            }
        }
    }

    let mut migradas: Vec<String> = Vec::new();
    let mut ya_en_llavero: Vec<String> = Vec::new();
    let mut fallidas: Vec<Value> = Vec::new();
    let mut sin_valor: Vec<String> = Vec::new();

    for nombre in &nombres {
        let en_texto = valor_config(data_dir, nombre);
        let en_llave = store.leer(nombre).filter(|v| !v.trim().is_empty());
        match (en_texto, en_llave) {
            (Some(_), Some(_)) => {
                limpiar(&mut cfg, nombre);
                ya_en_llavero.push(nombre.clone());
            }
            (Some(v), None) => {
                // Se escribe y **se lee de vuelta**. Si el llavero no devuelve exactamente el mismo
                // valor, se considera fallido y la clave se QUEDA en el config.
                //
                // Por qué esta comprobación existe: un `set_password` que devuelve Ok sin persistir
                // (keyring compilado sin almacén de plataforma) borró 4 claves del único lugar donde
                // estaban. Confiar en el Ok de la escritura no alcanza: hay que verificar la lectura.
                match store.escribir(nombre, &v) {
                    Ok(()) => match store.leer(nombre) {
                        Some(leido) if leido == v => {
                            limpiar(&mut cfg, nombre);
                            migradas.push(nombre.clone());
                        }
                        otro => {
                            // Nunca se loguea el valor: sólo si vino algo, vacío o distinto.
                            let detalle = match otro {
                                None => "no devolvió nada".to_string(),
                                Some(x) if x.is_empty() => "devolvió vacío".to_string(),
                                Some(x) => {
                                    format!("devolvió un valor distinto ({} caracteres)", x.len())
                                }
                            };
                            log::error!("claves: {nombre} NON se migró — el llavero {detalle}");
                            fallidas.push(json!({
                                "campo": nombre,
                                "error": format!("el llavero no devolvió el valor escrito: {detalle}"),
                            }));
                        }
                    },
                    Err(e) => fallidas.push(json!({ "campo": nombre, "error": e })),
                }
            }
            (None, Some(_)) => ya_en_llavero.push(nombre.clone()),
            (None, None) => sin_valor.push(nombre.clone()),
        }
    }

    if !migradas.is_empty() {
        if let Some(obj) = cfg.as_object_mut() {
            obj.insert(
                "_nota_claves".into(),
                json!("Las claves de API viven en el llavero del sistema (Windows Credential Manager), \
                       servicio com.nodeflow.desktop. No se guardan en este archivo en texto plano. \
                       Configurar con: Panel de claves de la app, o variables de entorno."),
            );
        }
    }
    let salida = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(&ruta, salida).map_err(|e| format!("no pude escribir el config: {e}"))?;

    log::info!(
        "claves: {} migradas al llavero · {} ya estaban · {} fallidas",
        migradas.len(),
        ya_en_llavero.len(),
        fallidas.len()
    );
    Ok(json!({
        "ok": true,
        "servicio": SERVICIO,
        "migradas": migradas,
        "ya_en_llavero": ya_en_llavero,
        "fallidas": fallidas,
        "sin_valor": sin_valor,
    }))
}

fn limpiar(cfg: &mut Value, campo: &str) {
    if let Some(obj) = cfg.as_object_mut() {
        obj.remove(campo);
        obj.remove(&campo.to_uppercase());
    }
}

/// Estado de las claves: **dónde** está cada una y una huella, sin publicar ningún valor.
pub fn estado(data_dir: &Path, store: &dyn Store) -> Value {
    let campos: Vec<Value> = CAMPOS
        .iter()
        .map(|c| {
            let en_entorno = env_names(c.config).iter().any(|n| {
                std::env::var(n)
                    .map(|v| !v.trim().is_empty())
                    .unwrap_or(false)
            });
            let en_llave = store.leer(c.config).filter(|v| !v.trim().is_empty());
            let en_texto = valor_config(data_dir, c.config);
            let (origen, valor) = if en_entorno {
                ("entorno", None)
            } else if let Some(v) = en_llave {
                ("llavero", Some(v))
            } else if let Some(v) = en_texto {
                ("texto-plano", Some(v))
            } else {
                ("ausente", None)
            };
            json!({
                "campo": c.config,
                "etiqueta": c.etiqueta,
                "origen": origen,
                "huella": valor.as_deref().map(huella).unwrap_or_default(),
                "largo": valor.as_ref().map(|v| v.len()).unwrap_or(0),
            })
        })
        .collect();
    let en_texto_plano = campos
        .iter()
        .filter(|c| c["origen"] == "texto-plano")
        .count();
    let en_llavero = campos.iter().filter(|c| c["origen"] == "llavero").count();
    json!({
        "servicio": SERVICIO,
        "campos": campos,
        "en_llavero": en_llavero,
        "en_texto_plano": en_texto_plano,
        "seguro": en_texto_plano == 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "nf-claves-{}-{:?}",
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
    fn solo_reconoce_campos_de_clave() {
        assert!(es_campo_de_clave("deepseek_api_key"));
        assert!(es_campo_de_clave("groq_api_key"));
        assert!(es_campo_de_clave("API_KEY"));
        assert!(!es_campo_de_clave("vault_path"));
        assert!(!es_campo_de_clave("tarifas"));
    }

    #[test]
    fn el_llavero_gana_sobre_el_config_en_texto_plano() {
        let d = dir();
        std::fs::write(
            d.join("nodeflow.config.json"),
            r#"{"probando_api_key":"LA_DEL_CONFIG"}"#,
        )
        .unwrap();
        let store = Memoria::nueva(&[("probando_api_key", "LA_DEL_LLAVERO")]);
        let r = obtener_con("probando_api_key", &d, &store).unwrap();
        assert_eq!(r.valor, "LA_DEL_LLAVERO");
        assert_eq!(r.origen, Origen::Llavero);
    }

    #[test]
    fn sin_llavero_cae_al_config_y_lo_reporta_como_legado() {
        let d = dir();
        std::fs::write(
            d.join("nodeflow.config.json"),
            r#"{"probando_api_key":"LEGADO"}"#,
        )
        .unwrap();
        let store = Memoria::default();
        let r = obtener_con("probando_api_key", &d, &store).unwrap();
        assert_eq!(r.valor, "LEGADO");
        assert_eq!(r.origen, Origen::ConfigTextoPlano);
    }

    #[test]
    fn la_variante_en_mayusculas_tambien_se_lee() {
        let d = dir();
        std::fs::write(
            d.join("nodeflow.config.json"),
            r#"{"PROBANDO_API_KEY":"MAYUS"}"#,
        )
        .unwrap();
        assert_eq!(
            valor_config(&d, "probando_api_key").as_deref(),
            Some("MAYUS")
        );
    }

    #[test]
    fn migrar_mueve_al_llavero_y_borra_del_config() {
        let d = dir();
        std::fs::write(
            d.join("nodeflow.config.json"),
            r#"{"probando_api_key":"SECRETO","vault_path":"X"}"#,
        )
        .unwrap();
        let store = Memoria::default();
        let res = migrar(&d, &store).unwrap();
        assert_eq!(store.leer("probando_api_key").as_deref(), Some("SECRETO"));
        let cfg = leer_config(&d);
        assert!(
            cfg["probando_api_key"].is_null(),
            "la clave no puede quedar en texto plano"
        );
        assert_eq!(cfg["vault_path"], "X", "el resto del config no se toca");
        assert!(cfg["_nota_claves"].is_string());
        assert!(res["migradas"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "probando_api_key"));
    }

    #[test]
    fn migrar_es_idempotente_y_no_pisa_el_llavero() {
        let d = dir();
        std::fs::write(
            d.join("nodeflow.config.json"),
            r#"{"probando_api_key":"VIEJA"}"#,
        )
        .unwrap();
        let store = Memoria::nueva(&[("probando_api_key", "BUENA")]);
        let res = migrar(&d, &store).unwrap();
        assert_eq!(
            store.leer("probando_api_key").as_deref(),
            Some("BUENA"),
            "no se pisa lo que ya está"
        );
        assert!(res["ya_en_llavero"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "probando_api_key"));
        assert!(leer_config(&d)["probando_api_key"].is_null());
    }

    #[test]
    fn si_el_llavero_falla_la_clave_no_se_pierde() {
        struct Roto;
        impl Store for Roto {
            fn leer(&self, _c: &str) -> Option<String> {
                None
            }
            fn escribir(&self, _c: &str, _v: &str) -> Result<(), String> {
                Err("acceso denegado".into())
            }
            fn borrar(&self, _c: &str) -> Result<(), String> {
                Ok(())
            }
        }
        let d = dir();
        std::fs::write(
            d.join("nodeflow.config.json"),
            r#"{"probando_api_key":"NO_SE_PIERDE"}"#,
        )
        .unwrap();
        let res = migrar(&d, &Roto).unwrap();
        assert_eq!(leer_config(&d)["probando_api_key"], "NO_SE_PIERDE");
        assert_eq!(res["fallidas"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn el_estado_publica_huella_pero_nunca_el_valor() {
        let d = dir();
        std::fs::write(
            d.join("nodeflow.config.json"),
            r#"{"tavily_api_key":"tvly-secreto-largo"}"#,
        )
        .unwrap();
        let store = Memoria::default();
        let e = estado(&d, &store);
        let crudo = serde_json::to_string(&e).unwrap();
        assert!(
            !crudo.contains("tvly-secreto-largo"),
            "el estado jamás publica el valor"
        );
        let tavily = e["campos"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["campo"] == "tavily_api_key")
            .unwrap();
        assert_eq!(tavily["origen"], "texto-plano");
        assert_eq!(tavily["huella"].as_str().unwrap().len(), 8);
        assert_eq!(e["en_texto_plano"], 1);
        assert_eq!(e["seguro"], false);
    }

    #[test]
    fn la_huella_es_estable_y_distingue_valores() {
        assert_eq!(huella("abc"), huella("abc"));
        assert_ne!(huella("abc"), huella("abd"));
        assert_eq!(huella("abc").len(), 8);
    }

    /// **El candado.** Escribe en el llavero REAL del sistema, lee de vuelta y borra.
    ///
    /// Este test existe porque el 15/09/2026 una migración borró 4 claves: el crate `keyring` estaba
    /// compilado con las features por defecto, que **no incluyen ningún almacén de plataforma** —
    /// `set_password` devolvía Ok sin persistir nada y `get_password` devolvía vacío. Con la feature
    /// `windows-native` habilitada, este test pasa; sin ella, falla. Es la única forma de saber que el
    /// llavero existe de verdad antes de confiarle un secreto, y por eso corre siempre.
    #[test]
    fn el_llavero_real_persiste_de_verdad() {
        let campo = "prueba_de_persistencia_api_key";
        let valor = "VALOR-DE-PRUEBA-1234567890";
        let _ = Llavero.borrar(campo); // punto de partida limpio, sin importar corridas previas
        Llavero
            .escribir(campo, valor)
            .expect("escribir en el llavero del sistema tiene que funcionar");
        let leido = Llavero
            .leer(campo)
            .expect("el llavero tiene que devolver lo que se acaba de escribir");
        assert_eq!(
            leido, valor,
            "el valor leído tiene que ser idéntico al escrito"
        );
        Llavero.borrar(campo).expect("borrar tiene que funcionar");
        assert!(
            Llavero.leer(campo).is_none(),
            "después de borrar no puede quedar nada"
        );
    }
}

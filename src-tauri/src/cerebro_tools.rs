//! Fase 5.3 — el **registro de herramientas del cerebro**.
//!
//! El pedido del usuario: *«darte espacio para que crees tus herramientas»*. Hasta acá mis herramientas
//! eran las de Hermes más las 22 del lienzo; no podía crear una y que quedara disponible en el próximo
//! turno. Acá vive el registro:
//!
//! ```text
//! <bóveda>/cerebro/herramientas/<nombre>/tool.json   ← la ficha (nombre, descripción, params, riesgo)
//! <bóveda>/cerebro/herramientas/<nombre>/run.py      ← el código (recibe los params por stdin en JSON)
//! ```
//!
//! Decisiones que vienen del docx del usuario y de las que acordamos:
//! - **La creación pasa por la cola de propuestas**: una herramienta nueva entra como propuesta y el
//!   humano la aprueba. Recién ahí queda en el registro y disponible.
//! - **Jaula**: el nombre es un slug (nada de `..` ni rutas), el proceso corre con `cwd` en la bóveda, con
//!   tope de tiempo y salida acotada. El código lo escribe el agente, así que se trata como no confiable.
//! - **Ejecución en el backend** (no en el servidor MCP ni en el WebView): es donde ya viven la bóveda,
//!   el log y la auditoría; el servidor MCP queda como adaptador fino.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// Carpeta del registro, dentro de `cerebro/`.
pub const CARPETA: &str = "herramientas";
/// Tope de tiempo de una herramienta. Si se pasa, se mata: una tool colgada no puede colgar el turno.
pub const TOPE_S: u64 = 30;
/// Tope de salida que se devuelve (el resto se corta: la salida entra al contexto del modelo).
pub const TOPE_SALIDA: usize = 8_000;

const RIESGOS: [&str; 3] = ["lectura", "escritura", "destructiva"];

#[derive(Clone, Debug, PartialEq)]
pub struct Herramienta {
    pub nombre: String,
    pub descripcion: String,
    /// JSON-Schema de los parámetros (lo que el modelo ve para llamarla).
    pub parametros: Value,
    /// `lectura` · `escritura` · `destructiva` (informativo: el humano lo ve al aprobarla).
    pub riesgo: String,
}

/// ¿Sirve como nombre de herramienta? Slug corto: es un nombre de carpeta y viaja en rutas.
pub fn nombre_valido(nombre: &str) -> bool {
    let n = nombre.trim();
    (3..=40).contains(&n.chars().count())
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && n.chars().next().is_some_and(|c| c.is_ascii_lowercase())
}

/// La carpeta del registro dentro de una bóveda.
pub fn dir_registro(raiz_boveda: &Path) -> PathBuf {
    raiz_boveda.join(crate::cerebro::CARPETA).join(CARPETA)
}

/// La carpeta de una herramienta, **enjaulada**: sólo un nombre válido, siempre dentro del registro.
pub fn dir_herramienta(raiz_boveda: &Path, nombre: &str) -> Result<PathBuf, String> {
    if !nombre_valido(nombre) {
        return Err(format!(
            "nombre de herramienta inválido: «{nombre}» (minúsculas, números y _ , 3-40 chars)"
        ));
    }
    Ok(dir_registro(raiz_boveda).join(nombre.trim()))
}

/// Lee la ficha de una herramienta.
pub fn parsear(texto: &str) -> Result<Herramienta, String> {
    let v: Value = serde_json::from_str(texto).map_err(|e| format!("tool.json ilegible: {e}"))?;
    let nombre = v["nombre"].as_str().unwrap_or("").trim().to_string();
    if !nombre_valido(&nombre) {
        return Err(format!("nombre inválido en la ficha: «{nombre}»"));
    }
    let descripcion = v["descripcion"].as_str().unwrap_or("").trim().to_string();
    if descripcion.chars().count() < 8 {
        return Err(
            "la ficha necesita una descripción (es lo que el modelo lee para decidir)".into(),
        );
    }
    let parametros = v
        .get("parametros")
        .cloned()
        .unwrap_or_else(|| json!({"type": "object"}));
    if !parametros.is_object() {
        return Err("`parametros` tiene que ser un objeto JSON-Schema".into());
    }
    let riesgo = v["riesgo"]
        .as_str()
        .unwrap_or("lectura")
        .trim()
        .to_lowercase();
    let riesgo = if RIESGOS.contains(&riesgo.as_str()) {
        riesgo
    } else {
        return Err(format!(
            "riesgo desconocido: «{riesgo}» (lectura · escritura · destructiva)"
        ));
    };
    Ok(Herramienta {
        nombre,
        descripcion,
        parametros,
        riesgo,
    })
}

/// Todas las herramientas válidas del registro (una ficha ilegible se saltea, no rompe la lista).
pub fn listar(raiz_boveda: &Path) -> Vec<Herramienta> {
    let dir = dir_registro(raiz_boveda);
    let Ok(entradas) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<Herramienta> = entradas
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| std::fs::read_to_string(e.path().join("tool.json")).ok())
        .filter_map(|texto| match parsear(&texto) {
            Ok(h) => Some(h),
            Err(e) => {
                log::warn!("herramientas: ficha salteada ({e})");
                None
            }
        })
        .collect();
    out.sort_by(|a, b| a.nombre.cmp(&b.nombre));
    out
}

/// El intérprete con el que corren las herramientas. El mismo del servidor MCP por defecto.
pub fn python() -> String {
    std::env::var("NODEFLOW_PYTHON").unwrap_or_else(|_| "python".to_string())
}

/// Corre una herramienta y devuelve `(salida, ms)`.
///
/// Los parámetros van por **stdin** en JSON (nada de armar líneas de comando con lo que dijo el modelo),
/// el proceso corre con `cwd` en la bóveda, con tope de tiempo y salida acotada.
pub fn ejecutar(
    raiz_boveda: &Path,
    nombre: &str,
    parametros: &Value,
) -> Result<(String, u64), String> {
    let dir = dir_herramienta(raiz_boveda, nombre)?;
    let guion = dir.join("run.py");
    if !guion.exists() {
        return Err(format!("la herramienta «{nombre}» no tiene run.py"));
    }
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(python());
    cmd.arg(&guion)
        .current_dir(raiz_boveda)
        .env("NODEFLOW_BOVEDA", raiz_boveda)
        .env("NODEFLOW_HERRAMIENTA", nombre)
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let inicio = Instant::now();
    let mut hijo = cmd
        .spawn()
        .map_err(|e| format!("no pude ejecutar «{nombre}»: {e} (¿está `python` en el PATH?)"))?;
    if let Some(si) = hijo.stdin.as_mut() {
        let _ = si.write_all(parametros.to_string().as_bytes());
    }
    // Se cierra el stdin: el guion puede leer todo hasta EOF.
    drop(hijo.stdin.take());
    // Tope de tiempo **de verdad**: se espera en tramos y se mata si se pasa. Un `wait_with_output()`
    // pelado espera para siempre, y una herramienta colgada colgaría el turno entero.
    let salida = loop {
        match hijo.try_wait() {
            Ok(Some(_)) => {
                break hijo
                    .wait_with_output()
                    .map_err(|e| format!("no pude leer «{nombre}»: {e}"))?
            }
            Ok(None) => {
                if inicio.elapsed() > Duration::from_secs(TOPE_S) {
                    let _ = hijo.kill();
                    let _ = hijo.wait();
                    return Err(format!("«{nombre}» tardó más de {TOPE_S} s y lo corté"));
                }
                std::thread::sleep(Duration::from_millis(120));
            }
            Err(e) => return Err(format!("no pude seguir a «{nombre}»: {e}")),
        }
    };
    let ms = inicio.elapsed().as_millis() as u64;
    let texto = String::from_utf8_lossy(&salida.stdout).trim().to_string();
    let err = String::from_utf8_lossy(&salida.stderr).trim().to_string();
    if !salida.status.success() {
        return Err(format!(
            "«{nombre}» falló ({}): {}",
            salida.status.code().unwrap_or(-1),
            recorta(&if err.is_empty() { texto.clone() } else { err })
        ));
    }
    Ok((recorta(&texto), ms))
}

/// Escribe una herramienta en el registro (lo llama la **aplicación** de la propuesta, o sea después de
/// que el humano aprobó). Crea la carpeta y los dos archivos.
pub fn escribir(raiz_boveda: &Path, h: &Herramienta, codigo: &str) -> Result<Vec<PathBuf>, String> {
    if codigo.trim().chars().count() < 10 {
        return Err("la herramienta necesita código (run.py)".into());
    }
    let dir = dir_herramienta(raiz_boveda, &h.nombre)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("no pude crear {dir:?}: {e}"))?;
    let ficha = dir.join("tool.json");
    let guion = dir.join("run.py");
    let json = serde_json::to_string_pretty(&json!({
        "nombre": h.nombre,
        "descripcion": h.descripcion,
        "parametros": h.parametros,
        "riesgo": h.riesgo,
    }))
    .map_err(|e| e.to_string())?;
    let ficha_txt = format!("{json}\n");
    crate::estado::escribir_atomico(&ficha, &ficha_txt)
        .map_err(|e| format!("no pude escribir la ficha: {e}"))?;
    crate::estado::escribir_atomico(&guion, codigo).map_err(|e| format!("no pude escribir run.py: {e}"))?;
    Ok(vec![ficha, guion])
}

fn recorta(s: &str) -> String {
    if s.chars().count() <= TOPE_SALIDA {
        s.to_string()
    } else {
        let corte: String = s.chars().take(TOPE_SALIDA).collect();
        format!("{corte}\n…(salida cortada a {TOPE_SALIDA} chars)")
    }
}

#[cfg(test)]
mod tests_registro {
    use super::*;

    #[test]
    fn el_nombre_es_un_slug_y_no_una_ruta() {
        for bueno in ["resumen_lienzo", "contar_nodos", "leer_nota2"] {
            assert!(nombre_valido(bueno), "{bueno} debería servir");
        }
        for malo in [
            "../evil",
            "con espacio",
            "Mayusculas",
            "a",
            "_empieza_raro",
            "ñandu",
            "",
        ] {
            assert!(!nombre_valido(malo), "{malo} no debería servir");
        }
    }

    #[test]
    fn la_carpeta_no_se_puede_escapar_del_registro() {
        let raiz = std::path::Path::new("C:/boveda");
        assert!(dir_herramienta(raiz, "resumen_lienzo").is_ok());
        for malo in ["../../etc", "..", "a/b", "C:/windows"] {
            assert!(
                dir_herramienta(raiz, malo).is_err(),
                "«{malo}» tiene que rebotar"
            );
        }
    }

    #[test]
    fn la_ficha_necesita_descripcion_y_riesgo_conocido() {
        let ok = r#"{"nombre":"resumen_lienzo","descripcion":"Cuenta nodos por categoría","riesgo":"lectura"}"#;
        let h = parsear(ok).unwrap();
        assert_eq!(h.nombre, "resumen_lienzo");
        assert_eq!(h.riesgo, "lectura");
        assert_eq!(
            h.parametros,
            json!({"type": "object"}),
            "sin params, un objeto vacío"
        );

        assert!(
            parsear(r#"{"nombre":"x","descripcion":"algo largo"}"#).is_err(),
            "nombre inválido"
        );
        assert!(
            parsear(r#"{"nombre":"valido_1","descripcion":"corto"}"#).is_err(),
            "descripción pobre"
        );
        assert!(
            parsear(r#"{"nombre":"valido_1","descripcion":"una descripcion","riesgo":"malisimo"}"#)
                .is_err(),
            "riesgo inventado"
        );
    }

    #[test]
    fn escribe_y_lista_el_registro_salteando_fichas_rotas() {
        let base = std::env::temp_dir().join(format!("nf-tools-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let h = Herramienta {
            nombre: "resumen_lienzo".into(),
            descripcion: "Cuenta los nodos por categoría del lienzo".into(),
            parametros: json!({"type": "object", "properties": {"tope": {"type": "number"}}}),
            riesgo: "lectura".into(),
        };
        let escritos = escribir(&base, &h, "print('hola')\n").unwrap();
        assert_eq!(escritos.len(), 2, "ficha + guion");
        // una ficha rota no rompe la lista
        let rota = dir_registro(&base).join("rota_x");
        std::fs::create_dir_all(&rota).unwrap();
        std::fs::write(rota.join("tool.json"), "{ no soy json").unwrap();

        let lista = listar(&base);
        assert_eq!(lista.len(), 1, "sólo la válida");
        assert_eq!(lista[0].nombre, "resumen_lienzo");
        assert_eq!(lista[0].riesgo, "lectura");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn sin_codigo_no_se_escribe_nada() {
        let base = std::env::temp_dir().join(format!("nf-tools-sin-codigo-{}", std::process::id()));
        let h = Herramienta {
            nombre: "vacia_1".into(),
            descripcion: "una herramienta sin código".into(),
            parametros: json!({"type": "object"}),
            riesgo: "lectura".into(),
        };
        assert!(escribir(&base, &h, "  ").is_err());
        assert!(!dir_registro(&base).join("vacia_1").exists());
    }
}

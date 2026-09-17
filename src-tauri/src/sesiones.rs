//! Sesiones del lienzo guardadas **en la bóveda** (`.nodeflow/sesiones/`).
//!
//! Por qué en disco y no en el `localStorage` del WebView: una sesión es «un conjunto de nodos al que
//! quiero poder volver». Guardada en la app, no viaja en el respaldo, no la ve el otro perfil y se
//! pierde si se limpia el perfil del WebView. En la bóveda es un archivo más: entra al `backup.sh`,
//! sobrevive a la reinstalación y se puede mirar con cualquier editor.
//!
//! Formato: un archivo por sesión (`<id>.json`, misma forma canónica que `.nodeflow/state.json` para
//! que una sesión pueda restaurarse tal cual) y un `index.json` liviano — sin nodos ni aristas — que es
//! lo que lee el listado. Si el índice falta o quedó desincronizado, se **reconstruye** leyendo los
//! archivos: el índice es una caché, no la fuente de verdad.
//!
//! La sesión `auto` (rodante) es única y se sobrescribe: es la red de seguridad de «volver a como
//! estaba hace un rato», sin acumular copias.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Carpeta de las sesiones, relativa a la bóveda.
pub const CARPETA: &str = ".nodeflow/sesiones";
/// Cuántas sesiones se conservan. Al pasarse se podan las más viejas (la rodante nunca se poda).
pub const MAX_SESIONES: usize = 40;
/// Tope defensivo del cuerpo de una sesión (un lienzo grande ronda los 200 KB).
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
/// Id fijo de la sesión rodante.
pub const ID_RODANTE: &str = "auto";
const MAX_NOMBRE: usize = 80;

pub fn carpeta(boveda: &Path) -> PathBuf {
    boveda.join(CARPETA.replace('/', std::path::MAIN_SEPARATOR_STR))
}

/// Escribe `tmp` + `rename` para que nadie lea un archivo a medio escribir (misma regla que la bóveda).
fn escribir_atomico(path: &Path, contenido: &str) -> Result<usize, String> {
    if let Some(padre) = path.parent() {
        std::fs::create_dir_all(padre).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("tmp-nf");
    std::fs::write(&tmp, contenido.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(contenido.len())
}

/// Valida el id de una sesión: es un **nombre de archivo** que viaja en una ruta.
/// Sólo minúsculas, dígitos y guiones; 1..=40; nunca `..`, `/`, `\`, `:` ni mayúsculas.
pub fn id_valido(texto: &str) -> Option<String> {
    let t = texto.trim();
    if t.is_empty() || t.len() > 40 {
        return None;
    }
    if !t
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return None;
    }
    if t.starts_with('-') || t.ends_with('-') {
        return None;
    }
    Some(t.to_string())
}

/// Id nuevo a partir del instante (mismo espíritu que el resto de la app: sin azar, ordenable).
pub fn id_nuevo(ms: u128) -> String {
    format!("s-{ms}")
}

fn limpiar_nombre(nombre: &str) -> String {
    let t = nombre.trim();
    if t.is_empty() {
        return "Sesión sin nombre".to_string();
    }
    t.chars().take(MAX_NOMBRE).collect()
}

fn ahora_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Una sesión tal como queda en el índice (sin nodos ni aristas).
fn ficha(sesion: &Value, bytes: u64) -> Value {
    json!({
        "id": sesion["id"],
        "nombre": sesion["nombre"],
        "creado": sesion["creado"],
        "actualizado": sesion["actualizado"],
        "nodos": sesion["nodos"],
        "aristas": sesion["aristas"],
        "mapa": sesion["mapa"],
        "appearance": sesion["appearance"],
        "rodante": sesion["id"] == ID_RODANTE,
        "bytes": bytes,
    })
}

pub struct Sesiones {
    raiz: PathBuf,
}

impl Sesiones {
    pub fn nueva(boveda: &Path) -> Self {
        Self {
            raiz: carpeta(boveda),
        }
    }

    fn archivo(&self, id: &str) -> PathBuf {
        self.raiz.join(format!("{id}.json"))
    }

    fn indice(&self) -> PathBuf {
        self.raiz.join("index.json")
    }

    fn leer_archivo(&self, id: &str) -> Option<Value> {
        let txt = std::fs::read_to_string(self.archivo(id)).ok()?;
        serde_json::from_str(&txt).ok()
    }

    /// Listado ordenado de la más nueva a la más vieja. El índice es una caché: si no está o no
    /// coincide con los archivos que hay, se reconstruye.
    pub fn listar(&self) -> Vec<Value> {
        let archivos: Vec<Value> = std::fs::read_dir(&self.raiz)
            .map(|it| {
                it.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
                    .filter(|p| p.file_name().map(|n| n != "index.json").unwrap_or(false))
                    .filter_map(|p| {
                        let id = p.file_stem()?.to_str()?.to_string();
                        let id = id_valido(&id)?;
                        let s = self.leer_archivo(&id)?;
                        let bytes = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                        Some(ficha(&s, bytes))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut ordenadas = archivos;
        // La rodante va siempre primera: es la que mirás cuando algo salió mal.
        ordenadas.sort_by(|a, b| {
            let (ra, rb) = (
                a["rodante"].as_bool().unwrap_or(false),
                b["rodante"].as_bool().unwrap_or(false),
            );
            rb.cmp(&ra).then(
                b["creado"]
                    .as_u64()
                    .unwrap_or(0)
                    .cmp(&a["creado"].as_u64().unwrap_or(0)),
            )
        });

        self.escribir_indice(&ordenadas);
        ordenadas
    }

    fn escribir_indice(&self, sesiones: &[Value]) {
        let idx = json!({
            "version": 1,
            "carpeta": CARPETA,
            "sesiones": sesiones,
        });
        if let Ok(txt) = serde_json::to_string_pretty(&idx) {
            let _ = escribir_atomico(&self.indice(), &txt);
        }
    }

    /// Guarda (o sobrescribe) una sesión con el lienzo tal cual viene del frontend.
    pub fn guardar(&self, req: &Value) -> Result<Value, String> {
        let nodes = req["nodes"].as_array().ok_or("falta `nodes`")?.clone();
        let edges = req["edges"].as_array().cloned().unwrap_or_default();
        let cuerpo = serde_json::to_string(req).map_err(|e| e.to_string())?;
        if cuerpo.len() > MAX_BYTES {
            return Err(format!(
                "la sesión pesa {} bytes y el tope es {}",
                cuerpo.len(),
                MAX_BYTES
            ));
        }

        let id = match req["id"].as_str().and_then(id_valido) {
            Some(id) => id,
            None => id_nuevo(ahora_ms() as u128),
        };
        let anterior = self.leer_archivo(&id);
        let sesion = json!({
            "version": 1,
            "id": id,
            "nombre": limpiar_nombre(req["nombre"].as_str().unwrap_or("")),
            "creado": req["creado"].as_u64().unwrap_or_else(ahora_ms),
            "mapa": req["mapa"].as_str().unwrap_or(""),
            "nodos": nodes.len(),
            "aristas": edges.len(),
            "appearance": req["appearance"].clone(),
            "templateId": req["templateId"].clone(),
            "nodes": nodes,
            "edges": edges,
        });
        // El `creado` de una sesión que ya existía no se pisa: es cuándo nació, no cuándo se actualizó.
        let sesion = if let Some(prev) = anterior {
            let mut s = sesion;
            s["creado"] = prev["creado"].clone();
            s["actualizado"] = json!(ahora_ms());
            s
        } else {
            sesion
        };

        let texto = serde_json::to_string_pretty(&sesion).map_err(|e| e.to_string())?;
        let bytes = escribir_atomico(&self.archivo(&id), &texto)?;
        self.podar();

        Ok(json!({
            "ok": true,
            "sesion": ficha(&sesion, bytes as u64),
            "carpeta": CARPETA,
        }))
    }

    /// Contenido completo de una sesión (lo que el lienzo necesita para volver).
    pub fn leer(&self, id: &str) -> Result<Value, String> {
        let id = id_valido(id).ok_or("id de sesión inválido")?;
        self.leer_archivo(&id)
            .ok_or_else(|| format!("no encontré la sesión «{id}»"))
    }

    pub fn borrar(&self, id: &str) -> Result<Value, String> {
        let id = id_valido(id).ok_or("id de sesión inválido")?;
        let path = self.archivo(&id);
        if !path.exists() {
            return Err(format!("no encontré la sesión «{id}»"));
        }
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        let restantes = self.listar();
        Ok(json!({ "ok": true, "borrada": id, "sesiones": restantes.len() }))
    }

    /// Poda las más viejas cuando se pasa el tope. La rodante queda siempre.
    fn podar(&self) {
        let mut lista = self.listar();
        if lista.len() <= MAX_SESIONES {
            return;
        }
        lista.retain(|s| !s["rodante"].as_bool().unwrap_or(false));
        // `listar` viene de la más nueva a la más vieja: lo que sobra está al final.
        for s in lista.iter().skip(MAX_SESIONES.saturating_sub(1)) {
            if let Some(id) = s["id"].as_str() {
                let _ = std::fs::remove_file(self.archivo(id));
            }
        }
        self.listar();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn base(nombre: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("nf-sesiones-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    fn lienzo(n: usize) -> Value {
        let nodes: Vec<Value> = (0..n)
            .map(|i| json!({"id": format!("n-{i}"), "data": {"title": format!("Nodo {i}")}}))
            .collect();
        json!({ "nodes": nodes, "edges": [{"id": "e-0", "source": "n-0", "target": "n-1"}] })
    }

    #[test]
    fn el_id_es_un_nombre_de_archivo_y_no_un_camino() {
        assert_eq!(id_valido("s-123"), Some("s-123".into()));
        assert_eq!(id_valido("auto"), Some("auto".into()));
        assert_eq!(id_valido("  s-9  "), Some("s-9".into()));
        for malo in [
            "../fuera",
            "..",
            "a/b",
            "a\\b",
            "C:",
            "S-1",
            "-x",
            "x-",
            "",
            "  ",
            &"a".repeat(41),
        ] {
            assert!(id_valido(malo).is_none(), "debería rechazar {malo:?}");
        }
    }

    #[test]
    fn guardar_listar_leer_borrar() {
        let b = base("crud");
        let s = Sesiones::nueva(&b);

        let mut payload = lienzo(3);
        payload["nombre"] = json!("Arquitectura v1");
        payload["mapa"] = json!("mapa-de-prueba");
        payload["id"] = json!("s-1000");
        let r = s.guardar(&payload).unwrap();
        assert_eq!(r["sesion"]["nodos"], 3);
        assert_eq!(r["sesion"]["aristas"], 1);
        assert_eq!(r["sesion"]["nombre"], "Arquitectura v1");

        let lista = s.listar();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0]["id"], "s-1000");

        let leida = s.leer("s-1000").unwrap();
        assert_eq!(leida["nodes"].as_array().unwrap().len(), 3);
        assert_eq!(leida["mapa"], "mapa-de-prueba");

        let del = s.borrar("s-1000").unwrap();
        assert_eq!(del["sesiones"], 0);
        assert!(s.leer("s-1000").is_err());
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn guardar_dos_veces_el_mismo_id_actualiza_y_no_duplica() {
        let b = base("update");
        let s = Sesiones::nueva(&b);
        let mut p = lienzo(2);
        p["id"] = json!("s-2000");
        p["nombre"] = json!("Primera");
        s.guardar(&p).unwrap();
        let nacimiento = s.leer("s-2000").unwrap()["creado"].as_u64().unwrap();

        p["nombre"] = json!("Actualizada");
        p["nodes"] = lienzo(5)["nodes"].clone();
        s.guardar(&p).unwrap();

        let lista = s.listar();
        assert_eq!(lista.len(), 1, "mismo id = misma sesión");
        assert_eq!(lista[0]["nombre"], "Actualizada");
        assert_eq!(lista[0]["nodos"], 5);
        // El nacimiento no se pisa al actualizar.
        assert_eq!(
            s.leer("s-2000").unwrap()["creado"].as_u64().unwrap(),
            nacimiento
        );
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn la_rodante_no_se_poda_y_las_viejas_si() {
        let b = base("poda");
        let s = Sesiones::nueva(&b);
        let mut auto_s = lienzo(1);
        auto_s["id"] = json!(ID_RODANTE);
        auto_s["nombre"] = json!("Automática");
        s.guardar(&auto_s).unwrap();

        for i in 0..(MAX_SESIONES + 5) {
            let mut p = lienzo(2);
            p["id"] = json!(format!("s-{}", 1000 + i));
            p["creado"] = json!(1000 + i);
            s.guardar(&p).unwrap();
        }

        let lista = s.listar();
        assert!(lista.len() <= MAX_SESIONES, "quedaron {}", lista.len());
        assert_eq!(lista[0]["id"], ID_RODANTE, "la rodante va primera");
        assert!(
            lista.iter().any(|x| x["id"] == ID_RODANTE),
            "la rodante nunca se poda"
        );
        // Lo que sobrevive es lo más nuevo.
        assert!(lista
            .iter()
            .any(|x| x["id"] == json!(format!("s-{}", 1000 + MAX_SESIONES + 4))));
        assert!(!lista.iter().any(|x| x["id"] == json!("s-1000")));
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn el_indice_es_una_cache_se_reconstruye_de_los_archivos() {
        let b = base("indice");
        let s = Sesiones::nueva(&b);
        let mut p = lienzo(2);
        p["id"] = json!("s-3000");
        s.guardar(&p).unwrap();
        // Se borra el índice a mano: el listado tiene que reconstruirlo.
        let _ = std::fs::remove_file(s.indice());
        let lista = s.listar();
        assert_eq!(lista.len(), 1);
        assert!(s.indice().exists(), "el índice se reescribe solo");
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn una_sesion_sin_nombre_no_queda_vacia_y_los_ids_salvajes_se_rechazan() {
        let b = base("nombre");
        let s = Sesiones::nueva(&b);
        let mut p = lienzo(1);
        p["nombre"] = json!("   ");
        let r = s.guardar(&p).unwrap();
        assert_eq!(r["sesion"]["nombre"], "Sesión sin nombre");
        assert!(r["sesion"]["id"].as_str().unwrap().starts_with("s-"));

        assert!(s.leer("../../secretos").is_err());
        assert!(s.borrar("../secretos").is_err());
        let _ = std::fs::remove_dir_all(&b);
    }
}

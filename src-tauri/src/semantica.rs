//! Caché semántica: la segunda capa.
//!
//! La caché de `costo.rs` es **exacta** (misma clave = misma respuesta). Esta capa busca una respuesta que
//! ya se generó para un pedido **parecido** y la reusa: "investigá sobre sensores de humedad" y "investigá
//! sensores de humedad de suelo" son el mismo trabajo pagado dos veces.
//!
//! Dos caminos, en orden de calidad:
//! 1. **Vectorial** (embeddings de Gemini, si hay clave): compara por significado. Cuesta una llamada
//!    barata por pedido nuevo (no por acierto) y se guarda con la entrada.
//! 2. **Local** (sin clave, o si el embedding falla): tokens normalizados —minúsculas, sin acentos,
//!    ordenados— y similitud de Jaccard. Cero red, cero dependencias.
//!
//! Comparar por el **pedido del usuario** y no por el prompt entero es a propósito: el prompt lleva el
//! lienzo, que cambia todo el tiempo, y eso ensuciaría cualquier comparación.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};

/// Si los embeddings fallan una vez (sin cuota, sin red, clave inválida), no se vuelven a intentar en
/// este proceso: un timeout de 3 s en **cada** pedido nuevo sería peor que no tener caché semántica.
static EMBEDDINGS_CAIDOS: AtomicBool = AtomicBool::new(false);

/// Cuánto tienen que parecerse dos pedidos para reusar la respuesta. Alto a propósito: un falso positivo
/// devuelve una respuesta equivocada, y eso cuesta más que un token.
pub const UMBRAL_VECTOR: f32 = 0.92;
/// Para el camino local (sin embeddings) el umbral es de **contención**, no de Jaccard: pedidos cortos
/// nunca llegan a 0.9 de Jaccard aunque sean el mismo pedido con una palabra más (medido: 0.78 con
/// "investigá sensores de humedad de suelo" vs la misma frase con "automático"). Contención = cuánto del
/// pedido **más corto** aparece en el más largo.
pub const UMBRAL_LOCAL: f32 = 0.90;
/// Además de contenerse, los dos pedidos tienen que ser de largo comparable: si el nuevo agrega la mitad
/// de una consigna, la respuesta vieja ya no responde la nueva. 1.8 = "hasta un 80% más largo".
pub const LARGO_MAX: f32 = 1.8;

/// Texto normalizado para comparar sin depender de mayúsculas, acentos ni orden de palabras.
pub fn normalizar(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    for ch in t.to_lowercase().chars() {
        let c = match ch {
            'á' | 'à' | 'ä' | 'â' | 'ã' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            otro => otro,
        };
        s.push(if c.is_alphanumeric() { c } else { ' ' });
    }
    let mut toks: Vec<&str> = s.split_whitespace().collect();
    toks.sort_unstable();
    toks.dedup();
    toks.join(" ")
}

/// Similitud de Jaccard sobre los tokens normalizados (0 = nada en común, 1 = idénticos).
pub fn jaccard(a: &str, b: &str) -> f32 {
    let sa: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let sb: std::collections::HashSet<&str> = b.split_whitespace().collect();
    if sa.is_empty() || sb.is_empty() {
        return 0.0;
    }
    let inter = sa.intersection(&sb).count() as f32;
    let union = sa.union(&sb).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// Contención: qué fracción del pedido **más corto** aparece en el más largo (1 = uno contiene al otro).
pub fn contencion(a: &str, b: &str) -> (f32, f32) {
    let sa: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let sb: std::collections::HashSet<&str> = b.split_whitespace().collect();
    if sa.is_empty() || sb.is_empty() {
        return (0.0, 0.0);
    }
    let inter = sa.intersection(&sb).count() as f32;
    let (corto, largo) = (sa.len().min(sb.len()) as f32, sa.len().max(sb.len()) as f32);
    (inter / corto, largo / corto)
}

/// Similitud por coseno entre dos vectores (0 = ortogonales, 1 = iguales).
pub fn coseno(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

/// La clave del proveedor de embeddings: una sola resolución para toda la app (`claves.rs`), que
/// incluye el **llavero del sistema** además del entorno y el config en texto plano.
fn clave_embeddings(st: &crate::server::AppState) -> Option<String> {
    crate::claves::obtener("gemini_api_key", &st.data_dir)
}

/// Embeddings de Gemini. Los nombres de modelo cambian sin aviso: `text-embedding-004` respondía 404 y
/// el vigente es `gemini-embedding-001` (3072 dimensiones). Se prueban en orden y se **recuerda** el que
/// funcione en este proceso, así el próximo pedido no paga la prueba.
const MODELOS: [&str; 3] = ["gemini-embedding-001", "text-embedding-005", "text-embedding-004"];
static MODELO_ACTIVO: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Un intento contra un modelo concreto. `None` = este modelo no sirve (nombre viejo, 404), probar el
/// siguiente; un fallo de red/cuota sí corta el circuito, porque no se arregla probando otro nombre.
async fn pedir_embedding(
    st: &crate::server::AppState,
    clave: &str,
    modelo: &str,
    texto: &str,
) -> Option<Vec<f32>> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{modelo}:embedContent?key={clave}"
    );
    let cuerpo = json!({
        "model": format!("models/{modelo}"),
        "content": { "parts": [{ "text": texto.chars().take(2000).collect::<String>() }] }
    });
    let r = st
        .http
        .post(&url)
        .timeout(std::time::Duration::from_secs(4))
        .json(&cuerpo)
        .send()
        .await
        .ok()?;
    if r.status() == reqwest::StatusCode::NOT_FOUND {
        return None; // el modelo no existe con ese nombre
    }
    if !r.status().is_success() {
        log::warn!("semántica: embeddings devolvió {}; se desactivan por esta corrida", r.status());
        EMBEDDINGS_CAIDOS.store(true, Ordering::Relaxed);
        return None;
    }
    let v: Value = match r.json().await {
        Ok(v) => v,
        Err(e) => {
            log::warn!("semántica: respuesta de embeddings ilegible ({e}); se desactivan por esta corrida");
            EMBEDDINGS_CAIDOS.store(true, Ordering::Relaxed);
            return None;
        }
    };
    let vals = v["embedding"]["values"].as_array()?;
    let out: Vec<f32> = vals.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect();
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Embedding del pedido. Devuelve `None` si no hay clave, si ningún nombre de modelo responde o si ya se
/// falló antes: la caché semántica **nunca** puede romper una generación ni cobrar latencia dos veces.
pub async fn vector(st: &crate::server::AppState, texto: &str) -> Option<Vec<f32>> {
    if EMBEDDINGS_CAIDOS.load(Ordering::Relaxed) {
        return None;
    }
    let clave = clave_embeddings(st)?;
    let recordado = MODELO_ACTIVO.lock().ok().and_then(|m| m.clone());
    let candidatos: Vec<String> = match recordado {
        Some(m) => vec![m],
        None => MODELOS.iter().map(|s| s.to_string()).collect(),
    };
    for modelo in candidatos {
        if let Some(v) = pedir_embedding(st, &clave, &modelo, texto).await {
            if let Ok(mut g) = MODELO_ACTIVO.lock() {
                if g.as_deref() != Some(modelo.as_str()) {
                    log::info!("semántica: embeddings con {modelo} ({} dimensiones)", v.len());
                    *g = Some(modelo.clone());
                }
            }
            return Some(v);
        }
    }
    log::warn!("semántica: ningún nombre de modelo de embeddings respondió; se usa la comparación por texto");
    EMBEDDINGS_CAIDOS.store(true, Ordering::Relaxed);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizar_ignora_mayusculas_acentos_y_orden() {
        assert_eq!(
            normalizar("Investiga Sensores de Humedad"),
            normalizar("humedad de sensores investiga")
        );
        assert_eq!(normalizar("riego automático"), normalizar("riego automatico"));
        assert_eq!(normalizar("¿Sondas, de suelo?"), "de sondas suelo");
    }

    fn a_antes_de_agregar_una_consigna() -> String {
        // El mismo pedido con media consigna nueva: ya no es el mismo trabajo.
        "investigá sensores de humedad de suelo para riego y además compará precios de importación          con alternativas nacionales y armá un plan de compra por etapas".to_string()
    }

    #[test]
    fn el_pedido_parecido_supera_el_umbral_y_el_distinto_no() {
        let a = normalizar("investigá sensores de humedad de suelo para riego");
        let b = normalizar("investigá sobre sensores de humedad de suelo para riego automático");
        let c = normalizar("creá tres ramas sobre monetización del producto");
        let (contenido, largo) = contencion(&a, &b);
        let (ajeno, _) = contencion(&a, &c);
        assert!(
            contenido >= UMBRAL_LOCAL && largo <= LARGO_MAX,
            "el pedido parecido debe acreditar: contención {contenido}, largo {largo}"
        );
        assert!(ajeno < UMBRAL_LOCAL, "uno distinto no: {ajeno}");
        // Y el guardián de largo: si el nuevo agrega otra consigna, no acredita.
        let d = normalizar(&a_antes_de_agregar_una_consigna());
        let (_, l) = contencion(&a, &d);
        assert!(l > LARGO_MAX, "un pedido mucho más largo no debe reusar: {l}");
    }

    #[test]
    fn coseno_mide_lo_que_promete() {
        let a = vec![1.0, 0.0, 0.0];
        assert!((coseno(&a, &a) - 1.0).abs() < 1e-6, "idénticos = 1");
        assert!(coseno(&a, &[0.0, 1.0, 0.0]).abs() < 1e-6, "ortogonales = 0");
        assert_eq!(coseno(&a, &[1.0, 2.0]), 0.0, "distinta dimensión = 0");
    }
}

//! Fase 5b — Memoria semántica del vault.
//!
//! Indexa la bóveda de Obsidian completa (notas `.md` + los `.canvas`) con BM25 en memoria:
//! sin dependencias, sin red, sin modelos. Sirve para que el lienzo se nutra de todo lo que el
//! usuario ya escribió y no solo del grafo.
//!
//! Decisiones:
//! - **Plegado de acentos** (`información` ≡ `informacion`) y minúsculas: en castellano es la
//!   diferencia entre encontrar una nota y no encontrarla.
//! - **Boost de título** (×2.5): un término en el título pesa más que en el cuerpo.
//! - El índice se construye en el primer uso y se refresca solo si pasó `REFRESCO` (o si se pide
//!   `POST /api/vault/reindex`), así arrancar la app no cuesta nada.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_BYTES: u64 = 1_500_000;
const REFRESCO: Duration = Duration::from_secs(30);
const MIN_TOKEN: usize = 3;
const K1: f32 = 1.2;
const B: f32 = 0.75;
const BOOST_TITULO: f32 = 2.5;

#[derive(Clone)]
struct Doc {
    ruta: String,
    titulo: String,
    texto: String,
    plano: String,
    es_nodo: bool,
    modificado_ms: u64,
}

struct Estado {
    docs: Vec<Doc>,
    tf: Vec<HashMap<String, u32>>,
    df: HashMap<String, u32>,
    largo: Vec<usize>,
    prom: f64,
    construido: Option<Instant>,
    construido_ms: u64,
    errores: usize,
}

pub struct Memoria {
    raiz: PathBuf,
    /// Prefijo relativo de las notas que ya son nodos del lienzo (ej. `NodeFlow/nodos/`).
    prefijo_nodos: String,
    inner: Mutex<Estado>,
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Minúscula + sin acentos + ñ→n. Necesario para que buscar `informacion` encuentre `información`.
pub(crate) fn plegar(c: char) -> char {
    let l = c.to_lowercase().next().unwrap_or(c);
    match l {
        'á' | 'à' | 'ä' | 'â' | 'ã' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        otro => otro,
    }
}

pub(crate) fn tokenizar(texto: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in texto.chars() {
        let c = plegar(ch);
        if c.is_alphanumeric() {
            cur.push(c);
        } else if !cur.is_empty() {
            if cur.chars().count() >= MIN_TOKEN {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        }
    }
    if cur.chars().count() >= MIN_TOKEN {
        out.push(cur);
    }
    out
}

/// Raíz de la memoria: `memoria_path` del config, o el padre del vault de NodeFlow (la bóveda entera).
fn resolve_raiz(data_dir: &Path) -> PathBuf {
    let cfg = data_dir.join("nodeflow.config.json");
    if let Ok(txt) = std::fs::read_to_string(&cfg) {
        if let Ok(v) = serde_json::from_str::<Value>(&txt) {
            if let Some(p) = v["memoria_path"].as_str() {
                if !p.trim().is_empty() {
                    return PathBuf::from(p);
                }
            }
            if let Some(vp) = v["vault_path"].as_str() {
                if let Some(padre) = PathBuf::from(vp).parent() {
                    return padre.to_path_buf();
                }
            }
        }
    }
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join("Documents").join("Obsidian Vault")
}

fn prefijo_nodos(data_dir: &Path) -> String {
    let cfg = data_dir.join("nodeflow.config.json");
    let vp = std::fs::read_to_string(&cfg)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["vault_path"].as_str().map(String::from))
        .unwrap_or_default();
    if vp.is_empty() {
        return "nodos/".into();
    }
    // Relativo a la raíz REAL del índice (memoria_path o el padre del vault).
    match PathBuf::from(&vp).strip_prefix(resolve_raiz(data_dir)) {
        Ok(rel) => format!("{}/nodos/", rel.to_string_lossy().replace('\\', "/")),
        Err(_) => "nodos/".into(),
    }
}

fn es_dir_ignorada(nombre: &str) -> bool {
    nombre.starts_with('.')
        || matches!(
            nombre,
            "node_modules" | ".obsidian" | ".trash" | ".git" | "_backups" | "target" | "dist"
        )
}

fn titulo_de(texto: &str, ruta: &Path) -> String {
    for l in texto.lines().take(40) {
        let t = l.trim();
        if let Some(h) = t.strip_prefix("# ") {
            return h.trim().to_string();
        }
    }
    for l in texto.lines().take(40) {
        let t = l.trim();
        if let Some(h) = t.strip_prefix("title:") {
            return h.trim().trim_matches('"').to_string();
        }
    }
    ruta.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "(sin título)".into())
}

/// Extrae el texto de un `.canvas` (nodos de tipo texto).
fn texto_de_canvas(contenido: &str) -> Option<String> {
    let v: Value = serde_json::from_str(contenido).ok()?;
    let mut out = String::new();
    for n in v["nodes"].as_array()? {
        if let Some(t) = n["text"].as_str() {
            out.push_str(t);
            out.push('\n');
        }
    }
    Some(out)
}

impl Memoria {
    pub fn new(data_dir: &Path) -> Arc<Self> {
        let raiz = resolve_raiz(data_dir);
        let prefijo_nodos = prefijo_nodos(data_dir);
        log::info!(
            "memoria: raíz = {} (nodos: {prefijo_nodos})",
            raiz.display()
        );
        Arc::new(Self {
            raiz,
            prefijo_nodos,
            inner: Mutex::new(Estado {
                docs: Vec::new(),
                tf: Vec::new(),
                df: HashMap::new(),
                largo: Vec::new(),
                prom: 0.0,
                construido: None,
                construido_ms: 0,
                errores: 0,
            }),
        })
    }

    fn juntar(&self, dir: &Path, prof: usize, docs: &mut Vec<Doc>, errores: &mut usize) {
        if prof > 8 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.filter_map(|e| e.ok()) {
            let p = e.path();
            let nombre = e.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                if !es_dir_ignorada(&nombre) {
                    self.juntar(&p, prof + 1, docs, errores);
                }
                continue;
            }
            let ext = p
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext != "md" && ext != "canvas" {
                continue;
            }
            let meta = match std::fs::metadata(&p) {
                Ok(m) => m,
                Err(_) => {
                    *errores += 1;
                    continue;
                }
            };
            if meta.len() > MAX_BYTES {
                continue;
            }
            let crudo = match std::fs::read_to_string(&p) {
                Ok(t) => t,
                Err(_) => {
                    *errores += 1;
                    continue;
                }
            };
            let texto = if ext == "canvas" {
                match texto_de_canvas(&crudo) {
                    Some(t) => t,
                    None => continue,
                }
            } else {
                crudo.clone()
            };
            if texto.trim().is_empty() {
                continue;
            }
            let rel = p
                .strip_prefix(&self.raiz)
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| nombre.clone());
            let titulo = if ext == "canvas" {
                format!("{nombre} (canvas)")
            } else {
                titulo_de(&texto, &p)
            };
            let modificado_ms = meta
                .modified()
                .ok()
                .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            docs.push(Doc {
                es_nodo: rel.starts_with(&self.prefijo_nodos) || rel.contains("/nodos/"),
                ruta: rel,
                titulo,
                plano: texto.chars().map(plegar).collect(),
                texto,
                modificado_ms,
            });
        }
    }

    /// Construye (o reconstruye) el índice completo. Devuelve cuántas notas indexó.
    pub fn indexar(&self) -> usize {
        let mut docs: Vec<Doc> = Vec::new();
        let mut errores = 0usize;
        self.juntar(&self.raiz, 0, &mut docs, &mut errores);
        docs.sort_by(|a, b| a.ruta.cmp(&b.ruta));

        let mut tf: Vec<HashMap<String, u32>> = Vec::with_capacity(docs.len());
        let mut df: HashMap<String, u32> = HashMap::new();
        let mut largo: Vec<usize> = Vec::with_capacity(docs.len());
        for d in &docs {
            // El título entra dos veces en el bolsillo de términos: así el boost es real.
            let mut tokens = tokenizar(&d.texto);
            let del_titulo = tokenizar(&d.titulo);
            tokens.extend(del_titulo.iter().cloned());
            let mut m: HashMap<String, u32> = HashMap::new();
            for t in &tokens {
                *m.entry(t.clone()).or_insert(0) += 1;
            }
            largo.push(tokens.len());
            for t in m.keys() {
                *df.entry(t.clone()).or_insert(0) += 1;
            }
            tf.push(m);
        }
        let tot: usize = largo.iter().sum();
        let prom = if largo.is_empty() {
            0.0
        } else {
            tot as f64 / largo.len() as f64
        };
        let n = docs.len();

        let mut st = self.inner.lock().unwrap();
        st.docs = docs;
        st.tf = tf;
        st.df = df;
        st.largo = largo;
        st.prom = prom;
        st.errores = errores;
        st.construido = Some(Instant::now());
        st.construido_ms = epoch_ms();
        log::info!("memoria: {n} nota(s) indexada(s) · {tot} términos · {errores} ilegible(s)");
        n
    }

    fn asegurar(&self) {
        let vencido = {
            let st = self.inner.lock().unwrap();
            match st.construido {
                None => true,
                Some(t) => t.elapsed() > REFRESCO,
            }
        };
        if vencido {
            self.indexar();
        }
    }

    /// Fragmento alrededor de la primera coincidencia (mismo índice de caracteres que el texto
    /// original: el plegado conserva la cantidad de caracteres).
    fn fragmento(doc: &Doc, tokens: &[String], ancho: usize) -> String {
        let plano: Vec<char> = doc.plano.chars().collect();
        let mut pos: Option<usize> = None;
        for t in tokens {
            let tc: Vec<char> = t.chars().collect();
            if tc.is_empty() || tc.len() > plano.len() {
                continue;
            }
            let mut i = 0;
            while i + tc.len() <= plano.len() {
                if plano[i..i + tc.len()] == tc[..] {
                    pos = Some(i);
                    break;
                }
                i += 1;
            }
            if pos.is_some() {
                break;
            }
        }
        let chars: Vec<char> = doc.texto.chars().collect();
        let (ini, fin) = match pos {
            Some(p) => (p.saturating_sub(ancho), (p + ancho).min(chars.len())),
            None => (0, ancho * 2.min(chars.len())),
        };
        let mut s = String::new();
        if ini > 0 {
            s.push('…');
        }
        s.extend(&chars[ini..fin]);
        if fin < chars.len() {
            s.push('…');
        }
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Búsqueda BM25. `limite` acota los resultados.
    pub fn buscar(&self, consulta: &str, limite: usize) -> Value {
        let q = consulta.trim();
        if q.is_empty() {
            return json!({"ok": false, "error": "falta `q` (consulta)"});
        }
        self.asegurar();
        let tokens = tokenizar(q);
        if tokens.is_empty() {
            return json!({"ok": false, "error": "la consulta no tiene términos útiles (mínimo 3 letras)"});
        }

        let st = self.inner.lock().unwrap();
        let n = st.docs.len() as f32;
        let prom = if st.prom > 0.0 { st.prom as f32 } else { 1.0 };
        let mut puntajes: Vec<(usize, f32)> = Vec::new();

        for (i, m) in st.tf.iter().enumerate() {
            let largo = *st.largo.get(i).unwrap_or(&1) as f32;
            let mut score = 0.0f32;
            let titulo_tokens: Vec<String> = tokenizar(&st.docs[i].titulo);
            for t in &tokens {
                let f = match m.get(t) {
                    Some(v) => *v as f32,
                    None => continue,
                };
                let df = *st.df.get(t).unwrap_or(&0) as f32;
                if df <= 0.0 {
                    continue;
                }
                let idf = (((n - df + 0.5) / (df + 0.5)) + 1.0).ln();
                let denom = f + K1 * (1.0 - B + B * (largo / prom));
                let mut s = idf * (f * (K1 + 1.0)) / denom;
                if titulo_tokens.iter().any(|tt| tt == t) {
                    s *= BOOST_TITULO;
                }
                score += s;
            }
            if score > 0.0 {
                puntajes.push((i, score));
            }
        }
        puntajes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let limite = limite.clamp(1, 40);
        let resultados: Vec<Value> = puntajes
            .iter()
            .take(limite)
            .map(|(i, s)| {
                let d = &st.docs[*i];
                json!({
                    "ruta": d.ruta,
                    "titulo": d.titulo,
                    "puntaje": ((*s as f64 * 100.0).round() / 100.0),
                    "fragmento": Self::fragmento(d, &tokens, 140),
                    "ya_en_el_lienzo": d.es_nodo,
                    "modificado_ms": d.modificado_ms,
                })
            })
            .collect();

        json!({
            "ok": true,
            "consulta": q,
            "terminos": tokens,
            "raiz": self.raiz.to_string_lossy(),
            "docs_indexados": st.docs.len(),
            "coincidencias": puntajes.len(),
            "resultados": resultados,
        })
    }

    /// Lee una nota de la bóveda por su ruta relativa. Con guarda: nunca sale de la raíz.
    pub fn leer_nota(&self, rel: &str) -> Result<Value, String> {
        let limpio = rel.trim().trim_start_matches(['/', '\\']);
        if limpio.is_empty() || limpio.contains("..") {
            return Err("ruta inválida".into());
        }
        let candidato = self
            .raiz
            .join(limpio.replace('/', std::path::MAIN_SEPARATOR_STR));
        let canon =
            std::fs::canonicalize(&candidato).map_err(|_| format!("no encontré la nota: {rel}"))?;
        let raiz_canon = std::fs::canonicalize(&self.raiz).map_err(|e| e.to_string())?;
        if !canon.starts_with(&raiz_canon) {
            return Err("la ruta sale de la bóveda".into());
        }
        let texto = std::fs::read_to_string(&canon).map_err(|e| e.to_string())?;
        if texto.len() > MAX_BYTES as usize {
            return Err("la nota es demasiado grande para leerla entera".into());
        }
        let titulo = titulo_de(&texto, &canon);
        Ok(json!({
            "ok": true,
            "ruta": limpio,
            "titulo": titulo,
            "caracteres": texto.len(),
            "texto": texto,
        }))
    }

    pub fn estado(&self) -> Value {
        self.asegurar();
        let st = self.inner.lock().unwrap();
        let notas = st.docs.iter().filter(|d| !d.es_nodo).count();
        json!({
            "raiz": self.raiz.to_string_lossy(),
            "docs_indexados": st.docs.len(),
            "notas_del_vault_del_usuario": notas,
            "nodos_del_lienzo": st.docs.len() - notas,
            "terminos_unicos": st.df.len(),
            "ilegibles": st.errores,
            "construido_ms": st.construido_ms,
            "edad_segundos": st.construido.map(|t| t.elapsed().as_secs()),
            "refresco_automatico_segundos": REFRESCO.as_secs(),
        })
    }
}

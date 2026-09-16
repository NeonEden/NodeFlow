//! Fase 9 — costo visible y caché de las llamadas a IA.
//!
//! Módulo **puro y sin dependencias nuevas**: parsea el `usage` de cada proveedor, aplica tarifas
//! declaradas por el usuario, estima por caracteres cuando el proveedor no reporta uso, y calcula
//! la clave de caché con un hash propio (FNV-1a). `std::hash::DefaultHasher` **no** garantiza el
//! mismo valor entre versiones de Rust, y una clave guardada en disco no puede cambiar sola.
//!
//! Reglas del sistema:
//! - **Medido o estimado, nunca disfrazado.** Si el proveedor no trae `usage`, el consumo se estima
//!   por caracteres y la traza lo declara (`estimado: true`).
//! - **La caché no miente.** Un `hit` devuelve el artefacto ya generado, gasta 0 tokens y declara
//!   cuántos evitó.
//! - **Ningún precio inventado.** Las tarifas se declaran en `nodeflow.config.json`
//!   (`"tarifas": { "gemini-3.6-flash": [0.30, 2.50] }`, USD por 1M tokens de entrada/salida) o se
//!   declaran gratis por proveedor (`"gratis": ["ollama"]`). Sin declaración, el sistema informa los
//!   tokens y dice «sin tarifa declarada» en vez de inventar un dólar.
//! - La clave de caché incluye **nodo + prompt + proveedor + esquema + `CACHE_VER`**. No lleva el
//!   modelo porque se elige recién dentro de la llamada (cascada `CANDIDATE_MODELS`): si cambiás esa
//!   lista, subí `CACHE_VER` para invalidar lo guardado.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Versión del contrato de la caché. Subila para invalidar todas las entradas guardadas.
/// v2: la clave ahora incluye el **motor** (proveedor@modelo), no sólo el proveedor.
/// v3: los motores de Ollama pasaron a la API nativa con el esquema como gramática — las respuestas
///     viejas tenían la forma equivocada (un objeto donde el contrato pide una lista) y no se reusan.
pub const CACHE_VER: u32 = 3;

/// Tope de entradas en disco (una entrada ≈ un artefacto generado).
pub const CACHE_TOPE: usize = 300;

// ─────────────────────────────────────────────────────────────────────────────
// Consumo
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Consumo {
    pub prompt: u64,
    pub completion: u64,
    /// Tokens de entrada que el **proveedor** sirvió desde su caché de prefijo (DeepSeek lo reporta como
    /// `prompt_cache_hit_tokens`). Cuestan una fracción del precio normal: es el ahorro que el documento
    /// del usuario llama "prompt caching" — y sin medirlo no se sabe si el orden del prompt sirve.
    pub cache_hit: u64,
}

impl Consumo {
    pub fn total(&self) -> u64 {
        self.prompt + self.completion
    }

    pub fn sumar(&mut self, otro: &Consumo) {
        self.prompt += otro.prompt;
        self.completion += otro.completion;
        self.cache_hit += otro.cache_hit;
    }

    pub fn json(&self) -> Value {
        json!({
            "prompt": self.prompt,
            "completion": self.completion,
            "total": self.total(),
            "cache_hit": self.cache_hit,
            "cache_miss": self.prompt.saturating_sub(self.cache_hit),
        })
    }
}

/// Lo que devolvió **un** proveedor, antes de resolver consumo, costo y caché.
#[derive(Clone, Debug)]
pub struct Respuesta {
    pub valor: Value,
    pub modelo: String,
    /// `None` = el proveedor no reportó `usage` (aguas arriba se estima y se marca).
    pub consumo: Option<Consumo>,
}

/// Una llamada ya resuelta: es lo que se muestra en la traza.
#[derive(Clone, Debug)]
pub struct Llamada {
    pub valor: Value,
    pub proveedor: String,
    pub modelo: String,
    pub consumo: Consumo,
    /// `true` = el consumo se estimó por caracteres, no lo reportó el proveedor.
    pub estimado: bool,
    pub cache: bool,
    /// Tokens que NO se gastaron por venir de caché (0 en un miss).
    pub tokens_evitados: u64,
    pub ms: u128,
}

impl Llamada {
    pub fn uso_json(&self, tarifas: &Tarifas) -> Value {
        json!({
            "proveedor": self.proveedor,
            "modelo": self.modelo,
            "tokens": self.consumo.json(),
            "estimado": self.estimado,
            "costo_usd": tarifas.costo(&self.modelo, &self.proveedor, &self.consumo),
            "cache": if self.cache { "hit" } else { "miss" },
            "tokens_evitados": self.tokens_evitados,
            "cache_proveedor": self.consumo.cache_hit,
            "ms": self.ms,
        })
    }
}

/// Tokens de la respuesta de Gemini (`usageMetadata`).
pub fn consumo_gemini(v: &Value) -> Option<Consumo> {
    let u = v.get("usageMetadata")?;
    let prompt = u.get("promptTokenCount").and_then(|x| x.as_u64())?;
    let completion = u
        .get("candidatesTokenCount")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    Some(Consumo {
        prompt,
        completion,
        cache_hit: 0,
    })
}

/// Tokens de la API nativa de Ollama (`/api/chat`): `prompt_eval_count` + `eval_count`.
pub fn consumo_ollama_nativo(v: &Value) -> Option<Consumo> {
    let prompt = v.get("prompt_eval_count").and_then(|x| x.as_u64())?;
    let completion = v.get("eval_count").and_then(|x| x.as_u64()).unwrap_or(0);
    Some(Consumo {
        prompt,
        completion,
        cache_hit: 0,
    })
}

/// Tokens de una respuesta compatible con OpenAI (el daemon local de Ollama los reporta así).
pub fn consumo_openai(v: &Value) -> Option<Consumo> {
    let u = v.get("usage")?;
    let prompt = u.get("prompt_tokens").and_then(|x| x.as_u64())?;
    let completion = u
        .get("completion_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    // DeepSeek: `prompt_cache_hit_tokens`. OpenAI: `prompt_tokens_details.cached_tokens`.
    let cache_hit = u
        .get("prompt_cache_hit_tokens")
        .and_then(|x| x.as_u64())
        .or_else(|| {
            u.get("prompt_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
                .and_then(|x| x.as_u64())
        })
        .unwrap_or(0);
    Some(Consumo {
        prompt,
        completion,
        cache_hit,
    })
}

/// Estimación ≈4 caracteres por token, para cuando el proveedor no reporta `usage`.
pub fn aprox_tokens(texto: &str) -> u64 {
    (texto.chars().count() as u64 + 3) / 4
}

pub fn consumo_estimado(prompt: &str, salida: &str) -> Consumo {
    Consumo {
        prompt: aprox_tokens(prompt),
        completion: aprox_tokens(salida),
        cache_hit: 0,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tarifas
// ─────────────────────────────────────────────────────────────────────────────

/// Tarifas declaradas por el usuario: USD por 1M tokens (entrada, salida) por modelo, y proveedores
/// declarados gratis. Sin tarifa declarada el costo es `None`, no `0.0`: son cosas distintas.
#[derive(Clone, Debug, Default)]
pub struct Tarifas {
    modelos: HashMap<String, (f64, f64)>,
    gratis: Vec<String>,
}

impl Tarifas {
    /// `{"tarifas": {"modelo": [entrada, salida]}, "gratis": ["ollama"]}` desde el config del vault.
    pub fn desde_config(cfg: Option<&Value>) -> Tarifas {
        let Some(cfg) = cfg else {
            return Tarifas::default();
        };
        let mut modelos = HashMap::new();
        if let Some(obj) = cfg.get("tarifas").and_then(|t| t.as_object()) {
            for (modelo, par) in obj {
                let nums: Option<(f64, f64)> = par
                    .as_array()
                    .and_then(|a| match a.len() {
                        2 => match (a[0].as_f64(), a[1].as_f64()) {
                            (Some(e), Some(s)) => Some((e, s)),
                            _ => None,
                        },
                        _ => None,
                    })
                    .or_else(|| {
                        // también acepta {"entrada": x, "salida": y}
                        let e = par.get("entrada").and_then(|v| v.as_f64())?;
                        let s = par.get("salida").and_then(|v| v.as_f64())?;
                        Some((e, s))
                    });
                if let Some(par) = nums {
                    modelos.insert(modelo.to_lowercase(), par);
                }
            }
        }
        let gratis = cfg
            .get("gratis")
            .and_then(|g| g.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_lowercase())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec!["ollama".to_string()]);
        Tarifas { modelos, gratis }
    }

    /// Match exacto y, si no, por prefijo más largo (`gemini-3.6-flash` cubre `gemini-3.6-flash-001`).
    pub fn tarifa_de(&self, modelo: &str) -> Option<(f64, f64)> {
        let m = modelo.to_lowercase();
        if let Some(t) = self.modelos.get(&m) {
            return Some(*t);
        }
        let mut mejor: Option<(usize, (f64, f64))> = None;
        for (clave, t) in &self.modelos {
            if m.starts_with(clave.as_str()) {
                let largo = clave.len();
                if mejor.map(|(l, _)| largo > l).unwrap_or(true) {
                    mejor = Some((largo, *t));
                }
            }
        }
        mejor.map(|(_, t)| t)
    }

    /// `proveedor` puede venir como `proveedor@modelo` (la etiqueta del motor): alcanza con que
    /// empiece con un proveedor declarado como gratuito. Sin esto, un motor de Ollama quedaba con
    /// costo `None` en vez de `$0`.
    pub fn es_gratis(&self, proveedor: &str) -> bool {
        let p = proveedor.to_lowercase();
        self.gratis.iter().any(|g| {
            let g = g.to_lowercase();
            p == g || p.starts_with(&format!("{g}@")) || p.starts_with(&format!("{g}:"))
        })
    }

    /// Costo en USD. `None` = no declarado (ni tarifa ni gratis).
    pub fn costo(&self, modelo: &str, proveedor: &str, c: &Consumo) -> Option<f64> {
        if self.es_gratis(proveedor) {
            return Some(0.0);
        }
        let (entrada, salida) = self.tarifa_de(modelo)?;
        let v =
            c.prompt as f64 * entrada / 1_000_000.0 + c.completion as f64 * salida / 1_000_000.0;
        Some((v * 1_000_000.0).round() / 1_000_000.0)
    }

    pub fn modelos_declarados(&self) -> Vec<String> {
        let mut v: Vec<String> = self.modelos.keys().cloned().collect();
        v.sort();
        v
    }
}

/// Texto humano del costo, el mismo que ve el panel del orquestador (`res.costo`).
pub fn texto_costo(
    c: &Consumo,
    costo: Option<f64>,
    estimado: bool,
    cache_hits: u64,
    tokens_evitados: u64,
) -> String {
    let mut partes = vec![format!(
        "{} tokens ({} in / {} out{})",
        c.total(),
        c.prompt,
        c.completion,
        if estimado { ", estimado" } else { "" }
    )];
    partes.push(match costo {
        Some(v) => format!("US$ {v:.6}"),
        None => "sin tarifa declarada".to_string(),
    });
    if cache_hits > 0 {
        partes.push(format!(
            "{cache_hits} caché HIT ({tokens_evitados} tokens evitados)"
        ));
    }
    partes.join(" · ")
}

// ─────────────────────────────────────────────────────────────────────────────
// Hash y clave
// ─────────────────────────────────────────────────────────────────────────────

/// FNV-1a de 64 bits en hexadecimal: determinista entre corridas y versiones.
pub fn hash_estable(texto: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in texto.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Clave de caché. Separador `\u{1}` para que `nodo="a", prompt="b"` no colisione con
/// `nodo="ab", prompt=""`.
pub fn clave_cache(nodo: &str, prompt: &str, proveedor: &str, schema: &Value) -> String {
    let material = format!(
        "v{CACHE_VER}\u{1}{nodo}\u{1}{proveedor}\u{1}{}\u{1}{prompt}",
        hash_estable(&schema.to_string())
    );
    hash_estable(&material)
}

fn ahora_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Caché en disco
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Entrada {
    pub valor: Value,
    pub proveedor: String,
    pub modelo: String,
    /// Tokens que costó generarla: es lo que se declaró como evitado en cada `hit`.
    pub tokens: u64,
    pub ts: u64,
    /// El pedido normalizado que la generó. Es lo que compara la caché semántica: el prompt entero no
    /// sirve porque lleva el lienzo, que cambia en cada pedido. `serde(default)` para que las entradas
    /// ya guardadas (sin este campo) sigan siendo válidas y no haya que subir CACHE_VER.
    #[serde(default)]
    pub semilla: String,
    /// Embedding del pedido, si se pudo calcular. Sin él, la comparación cae al texto normalizado.
    #[serde(default)]
    pub vector: Option<Vec<f32>>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Contenido {
    #[serde(default)]
    ver: u32,
    #[serde(default)]
    entradas: HashMap<String, Entrada>,
    #[serde(default)]
    hits: u64,
    #[serde(default)]
    misses: u64,
    #[serde(default)]
    tokens_evitados: u64,
    /// Aciertos por parecido (no idénticos) y los tokens que evitaron.
    #[serde(default)]
    hits_semanticos: u64,
    #[serde(default)]
    tokens_evitados_semanticos: u64,
}

/// Caché de respuestas de IA, **en disco** (`.nodeflow/ai-cache.json`): repetir una generación no
/// cuesta ni un token, incluso después de reiniciar la app.
pub struct Cache {
    ruta: PathBuf,
    tope: usize,
    c: Mutex<Contenido>,
}

impl Cache {
    pub fn cargar(ruta: PathBuf) -> Cache {
        Cache::cargar_con_tope(ruta, CACHE_TOPE)
    }

    pub fn cargar_con_tope(ruta: PathBuf, tope: usize) -> Cache {
        let contenido = std::fs::read_to_string(&ruta)
            .ok()
            .and_then(|t| serde_json::from_str::<Contenido>(&t).ok())
            .filter(|c| c.ver == CACHE_VER) // otra versión del contrato: se descarta entera
            .unwrap_or_default();
        Cache {
            ruta,
            tope,
            c: Mutex::new(contenido),
        }
    }

    /// Un `hit` cuenta y acumula los tokens evitados; un `miss` solo cuenta.
    pub fn get(&self, clave: &str) -> Option<Entrada> {
        let mut c = match self.c.lock() {
            Ok(c) => c,
            Err(_) => return None,
        };
        match c.entradas.get(clave).cloned() {
            Some(e) => {
                c.hits += 1;
                c.tokens_evitados += e.tokens;
                Some(e)
            }
            None => {
                c.misses += 1;
                None
            }
        }
    }

    /// Busca una respuesta generada para un pedido **parecido** (no idéntico al de la clave exacta).
    ///
    /// Sólo compara contra entradas del **mismo proveedor y del mismo nodo**: reusar la respuesta de otra
    /// acción sería un error silencioso. Con vectores compara por coseno; sin ellos (o si la entrada vieja
    /// no tiene vector) cae al texto normalizado. Devuelve la mejor candidata que supere el umbral.
    pub fn buscar_parecido(
        &self,
        nodo: &str,
        proveedor: &str,
        pedido: &str,
        vector: Option<&[f32]>,
    ) -> Option<(String, Entrada, f32, &'static str)> {
        let busq = crate::semantica::normalizar(pedido);
        if busq.is_empty() {
            return None;
        }
        let c = self.c.lock().ok()?;
        let mut mejor: Option<(String, Entrada, f32, &'static str)> = None;
        for (clave, e) in c.entradas.iter() {
            if e.proveedor != proveedor || e.semilla.is_empty() {
                continue;
            }
            // La clave es `v3\x01nodo\x01proveedor\x01schema\x01prompt`: el nodo va en el segundo campo.
            let partes: Vec<&str> = clave.split('\u{1}').collect();
            if partes.len() >= 2 && partes[1] != nodo {
                continue;
            }
            let (sim, como) = match (vector, e.vector.as_deref()) {
                (Some(a), Some(b)) => (crate::semantica::coseno(a, b), "vectorial"),
                _ => {
                    // Sin embeddings: contención de tokens + guardián de largo (no reusar cuando el
                    // pedido nuevo agrega otra consigna).
                    let (con, largo) = crate::semantica::contencion(&e.semilla, &busq);
                    let sim = if largo <= crate::semantica::LARGO_MAX {
                        con
                    } else {
                        0.0
                    };
                    (sim, "por texto")
                }
            };
            let umbral = if como == "vectorial" {
                crate::semantica::UMBRAL_VECTOR
            } else {
                crate::semantica::UMBRAL_LOCAL
            };
            if sim >= umbral && mejor.as_ref().map(|m| sim > m.2).unwrap_or(true) {
                mejor = Some((clave.clone(), e.clone(), sim, como));
            }
        }
        mejor
    }

    /// Anota un acierto semántico. Se persiste en disco para que el panel pueda mostrarlo.
    pub fn registrar_semantico(&self, tokens: u64) {
        if let Ok(mut c) = self.c.lock() {
            c.hits_semanticos += 1;
            c.tokens_evitados_semanticos += tokens;
            c.ver = CACHE_VER;
            let _ = self.guardar(&c);
        }
    }

    pub fn put(&self, clave: &str, entrada: Entrada) {
        let mut c = match self.c.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        if c.entradas.len() >= self.tope && !c.entradas.contains_key(clave) {
            // Desaloja la más vieja por `ts`: sin dependencias y suficiente a esta escala.
            if let Some(k) = c
                .entradas
                .iter()
                .min_by_key(|(_, v)| v.ts)
                .map(|(k, _)| k.clone())
            {
                c.entradas.remove(&k);
            }
        }
        c.entradas.insert(clave.to_string(), entrada);
        c.ver = CACHE_VER;
        let _ = self.guardar(&c);
    }

    fn guardar(&self, c: &Contenido) -> std::io::Result<()> {
        if let Some(dir) = self.ruta.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let txt = serde_json::to_string(c)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&self.ruta, txt)
    }

    /// Para verificar la caché desde la API, sin adivinar.
    pub fn stats(&self) -> Value {
        let c = match self.c.lock() {
            Ok(c) => c,
            Err(_) => return json!({ "error": "caché no disponible" }),
        };
        json!({
            "ruta": self.ruta.to_string_lossy(),
            "ver": c.ver,
            "entradas": c.entradas.len(),
            "tope": self.tope,
            "hits": c.hits,
            "hits_semanticos": c.hits_semanticos,
            "tokens_evitados_semanticos": c.tokens_evitados_semanticos,
            "misses": c.misses,
            "tokens_evitados": c.tokens_evitados,
        })
    }
}

pub fn entrada_nueva(
    valor: Value,
    proveedor: &str,
    modelo: &str,
    tokens: u64,
    semilla: &str,
    vector: Option<Vec<f32>>,
) -> Entrada {
    Entrada {
        valor,
        proveedor: proveedor.to_string(),
        modelo: modelo.to_string(),
        tokens,
        ts: ahora_ms(),
        semilla: semilla.to_string(),
        vector,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(nombre: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("nodeflow-costo-{nombre}-{}", ahora_ms()));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn usage_de_gemini_y_de_openai() {
        let g = json!({
            "candidates": [{"content": {"parts": [{"text": "{}"}]}}],
            "usageMetadata": { "promptTokenCount": 812, "candidatesTokenCount": 430 }
        });
        assert_eq!(
            consumo_gemini(&g),
            Some(Consumo {
                prompt: 812,
                completion: 430,
                cache_hit: 0
            })
        );

        let o = json!({ "usage": { "prompt_tokens": 120, "completion_tokens": 80, "total_tokens": 200 } });
        assert_eq!(
            consumo_openai(&o),
            Some(Consumo {
                prompt: 120,
                completion: 80,
                cache_hit: 0
            })
        );

        // Sin `usage` no se inventa: None, para que aguas arriba se estime y se declare.
        assert_eq!(consumo_gemini(&json!({ "candidates": [] })), None);
        assert_eq!(consumo_openai(&json!({ "choices": [] })), None);
        // `completion_tokens` ausente no invalida el prompt medido.
        assert_eq!(
            consumo_openai(&json!({ "usage": { "prompt_tokens": 7 } })),
            Some(Consumo {
                prompt: 7,
                completion: 0,
                cache_hit: 0
            })
        );
    }

    #[test]
    fn consumo_estimado_por_caracteres() {
        assert_eq!(aprox_tokens(""), 0);
        assert_eq!(aprox_tokens("abcd"), 1);
        assert_eq!(aprox_tokens("abcde"), 2);
        let c = consumo_estimado("abcd", "abcdefgh");
        assert_eq!(
            c,
            Consumo {
                prompt: 1,
                completion: 2,
                cache_hit: 0
            }
        );
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn hash_estable_es_determinista_y_distingue() {
        let a = hash_estable("nodeflow");
        assert_eq!(a, hash_estable("nodeflow"));
        assert_ne!(a, hash_estable("nodeflow "));
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        // Valor de referencia de FNV-1a 64 de "abc" (no puede cambiar sin cambiar el algoritmo).
        assert_eq!(hash_estable("abc"), "e71fa2190541574b");
    }

    #[test]
    fn la_clave_distingue_nodo_prompt_proveedor_y_esquema() {
        let schema = json!({ "type": "OBJECT", "properties": { "a": { "type": "STRING" } } });
        let base = clave_cache("n-1", "prompt", "gemini", &schema);
        assert_eq!(base, clave_cache("n-1", "prompt", "gemini", &schema));
        assert_ne!(base, clave_cache("n-2", "prompt", "gemini", &schema));
        assert_ne!(base, clave_cache("n-1", "otro prompt", "gemini", &schema));
        assert_ne!(base, clave_cache("n-1", "prompt", "ollama", &schema));
        assert_ne!(
            base,
            clave_cache("n-1", "prompt", "gemini", &json!({ "type": "OBJECT" }))
        );
        // El separador impide la colisión por concatenación.
        assert_ne!(
            clave_cache("a", "b", "gemini", &schema),
            clave_cache("ab", "", "gemini", &schema)
        );
    }

    #[test]
    fn las_tarifas_se_declaran_no_se_inventan() {
        let cfg = json!({
            "tarifas": { "gemini-3.6-flash": [0.30, 2.50], "otro": {"entrada": 1.0, "salida": 3.0} },
            "gratis": ["ollama"]
        });
        let t = Tarifas::desde_config(Some(&cfg));
        assert_eq!(t.tarifa_de("gemini-3.6-flash"), Some((0.30, 2.50)));
        assert_eq!(t.tarifa_de("otro"), Some((1.0, 3.0)));
        // Prefijo: una variante del mismo modelo hereda la tarifa declarada.
        assert_eq!(t.tarifa_de("gemini-3.6-flash-001"), Some((0.30, 2.50)));
        assert_eq!(t.tarifa_de("modelo-desconocido"), None);

        let un_millon = Consumo {
            prompt: 1_000_000,
            completion: 0,
            cache_hit: 0,
        };
        assert_eq!(
            t.costo("gemini-3.6-flash", "gemini", &un_millon),
            Some(0.30)
        );
        let mixto = Consumo {
            prompt: 500_000,
            completion: 200_000,
            cache_hit: 0,
        };
        assert_eq!(t.costo("gemini-3.6-flash", "gemini", &mixto), Some(0.65));
        // Gratis se declara, y se declara por proveedor: 0.0 es un dato, no una ausencia.
        assert_eq!(t.costo("cualquiera", "ollama", &mixto), Some(0.0));
        // Sin declaración: None (no 0.0), y el texto lo dice.
        assert_eq!(t.costo("desconocido", "otro-prov", &mixto), None);
        assert!(texto_costo(&mixto, None, false, 0, 0).contains("sin tarifa declarada"));
    }

    #[test]
    fn el_texto_del_costo_marca_estimado_y_cache() {
        let c = Consumo {
            prompt: 100,
            completion: 20,
            cache_hit: 0,
        };
        let t = texto_costo(&c, Some(0.000123), false, 0, 0);
        assert!(t.contains("120 tokens (100 in / 20 out)"));
        assert!(t.contains("US$ 0.000123"));
        let t2 = texto_costo(&c, None, true, 1, 120);
        assert!(t2.contains("estimado"));
        assert!(t2.contains("1 caché HIT (120 tokens evitados)"));
    }

    #[test]
    fn la_cache_guarda_relee_y_cuenta() {
        let ruta = tmp("guarda");
        let c = Cache::cargar(ruta.clone());
        let k = "k1";
        assert!(c.get(k).is_none(), "miss en caché vacía");
        c.put(
            k,
            entrada_nueva(
                json!({ "artefacto": 1 }),
                "gemini",
                "gemini-3.6-flash",
                1234,
                "",
                None,
            ),
        );
        let e = c.get(k).expect("hit tras guardar");
        assert_eq!(e.valor["artefacto"], 1);
        assert_eq!(e.tokens, 1234);
        let s = c.stats();
        assert_eq!(s["hits"], 1);
        assert_eq!(s["misses"], 1);
        assert_eq!(s["tokens_evitados"], 1234);
        assert_eq!(s["entradas"], 1);

        // Sobrevive al reinicio: la evidencia de una caché es que otro proceso la lea.
        let c2 = Cache::cargar(ruta.clone());
        assert_eq!(c2.get(k).expect("hit tras recargar").tokens, 1234);
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn una_version_distinta_del_contrato_descarta_todo() {
        let ruta = tmp("ver");
        let c = Cache::cargar(ruta.clone());
        c.put(
            "k",
            entrada_nueva(json!({ "a": 1 }), "ollama", "m", 5, "", None),
        );
        let mut disco: Value =
            serde_json::from_str(&std::fs::read_to_string(&ruta).unwrap()).unwrap();
        disco["ver"] = json!(CACHE_VER + 1);
        std::fs::write(&ruta, disco.to_string()).unwrap();
        let c2 = Cache::cargar(ruta.clone());
        assert_eq!(c2.stats()["entradas"], 0);
        assert!(c2.get("k").is_none());
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn la_cache_desaloja_la_mas_vieja_al_llegar_al_tope() {
        let ruta = tmp("tope");
        let c = Cache::cargar_con_tope(ruta.clone(), 3);
        for i in 0..4 {
            let mut e = entrada_nueva(json!({ "i": i }), "ollama", "m", 1, "", None);
            e.ts = 1000 + i as u64; // ts explícito: el desalojo es por antigüedad, no por hash
            c.put(&format!("k{i}"), e);
        }
        assert_eq!(c.stats()["entradas"], 3);
        assert!(c.get("k0").is_none(), "la más vieja se fue");
        assert!(c.get("k3").is_some(), "la última entró");
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn los_tokens_de_la_api_nativa_de_ollama_se_leen() {
        let v = serde_json::json!({"prompt_eval_count": 1200, "eval_count": 340});
        let c = consumo_ollama_nativo(&v).unwrap();
        assert_eq!(c.prompt, 1200);
        assert_eq!(c.completion, 340);
        assert_eq!(c.total(), 1540);
        assert!(consumo_ollama_nativo(&serde_json::json!({})).is_none());
    }

    #[test]
    fn un_motor_gratuito_cuesta_cero_aunque_la_etiqueta_lleve_el_modelo() {
        let mut tarifas = Tarifas::default();
        tarifas.gratis = vec!["ollama".to_string()];
        assert!(tarifas.es_gratis("ollama"));
        assert!(tarifas.es_gratis("ollama@granite3.3:2b"));
        assert!(tarifas.es_gratis("OLLAMA@qwen2.5vl:7b"));
        assert!(!tarifas.es_gratis("gemini@gemini-3.6-flash"));
        let c = Consumo {
            prompt: 1000,
            completion: 1000,
            cache_hit: 0,
        };
        assert_eq!(
            tarifas.costo("granite3.3:2b", "ollama@granite3.3:2b", &c),
            Some(0.0)
        );
    }

    #[test]
    fn un_archivo_corrupto_no_rompe_el_arranque() {
        let ruta = tmp("corrupto");
        std::fs::write(&ruta, "{no es json").unwrap();
        let c = Cache::cargar(ruta.clone());
        assert_eq!(c.stats()["entradas"], 0);
        c.put(
            "k",
            entrada_nueva(json!({ "ok": true }), "ollama", "m", 1, "", None),
        );
        assert!(c.get("k").is_some());
        let _ = std::fs::remove_file(&ruta);
    }
}

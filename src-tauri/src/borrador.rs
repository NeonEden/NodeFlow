//! Fase 10 — borradores con el modelo local (`granite3.3:2b`) y **validador en Rust aguas abajo**.
//!
//! Principio: **el modelo propone, el código valida**. El 2B local es barato y rápido en caliente
//! (medido: 6,6 s en frío, 2,4 s en caliente con `keep_alive`), pero la gramática de Ollama garantiza
//! la *forma* del JSON, no la *verdad* de los valores: se midió `madurez: 100` con un JSON
//! perfectamente válido. Por eso acá nada de lo que devuelve el modelo se usa sin pasar por una regla
//! verificable, y lo que no pasa se reemplaza por el valor determinista —declarándolo en `problemas`.
//!
//! Qué NO hace este módulo: segmentar el texto ni armar títulos desde cero. Eso ya está resuelto con
//! código determinista y tests en `conocimiento.rs` (y meter un modelo donde ya hay tests es un
//! retroceso). El modelo aporta **clasificación** (categoría, madurez, tags) y, si su título pasa las
//! mismas reglas que el heurístico, un título mejor.
//!
//! Este archivo es puro: el HTTP al daemon vive en `server.rs`.

use serde_json::{json, Value};

/// Categorías canónicas: los presets del editor (`NodeEditModal.tsx`) + las que el resto de la app ya
/// escribe (`CONOCIMIENTO` en la captura, `MEMORIA` en el flujo de la bóveda, `NÚCLEO` en el lienzo).
/// Una categoría inventada por el modelo se rechaza: el valor por defecto del sistema es más honesto
/// que una etiqueta nueva que ninguna vista conoce.
pub const CATEGORIAS: [&str; 26] = [
    "IDEA",
    "ESTRATEGIA",
    "TECNOLOGÍA",
    "INVESTIGACIÓN",
    "PRODUCTO",
    "HIPÓTESIS",
    "MERCADO",
    "DRAFT",
    "CONOCIMIENTO",
    "MEMORIA",
    "NÚCLEO",
    // El lienzo real usa su propio vocabulario (medido el 12/09 en los 42 nodos): sin estas, el
    // validador habría rechazado 15 de las 16 categorías que el usuario ya tiene escritas.
    "ARQUITECTURA",
    "GOBERNANZA",
    "DATOS",
    "EJECUCIÓN",
    "UX",
    "SEGURIDAD",
    "SISTEMAS",
    "ALGORITMOS",
    "ECOSISTEMA",
    "VISUALES",
    "DISEÑO",
    "IA",
    "NEGOCIO",
    "CREATIVO",
    "REFERENCIA",
];

pub const MIN_TITULO: usize = 4;
pub const MAX_TITULO: usize = 62;
pub const MIN_TAGS: usize = 2;
pub const MAX_TAGS: usize = 5;
const MIN_TAG: usize = 3;
const MAX_TAG: usize = 24;
pub const MADUREZ_MIN: i64 = 1;
pub const MADUREZ_MAX: i64 = 5;
/// Recorte del cuerpo que se le manda al modelo: un 2B no aprovecha más y el prompt se paga en tiempo.
const RECORTE_CUERPO: usize = 1200;

/// Parámetros del microservicio local. Prioridad: variable de entorno → `nodeflow.config.json`
/// (`"borrador": {...}`) → default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Endpoint nativo del daemon (`/api/chat`), sin el `/v1` compatible con OpenAI.
    pub url: String,
    pub modelo: String,
    /// `keep_alive` **por request** (medido: una variable global en 0 s recarga el modelo cada vez y
    /// convierte 12 ms en 13 s). "5m" mientras se trabaja; "0" cuando la GPU hace falta para otra cosa.
    pub keep_alive: String,
    /// Cuántos candidatos se mandan al modelo por pedido (el resto queda con el camino determinista).
    pub tope_por_pedido: usize,
    /// Tope de tokens **de salida** por generación (`num_predict` / `max_tokens`). Sin tope el modelo
    /// se explaya, cruza el contexto y el pedido muere: medido 15/09, 6.238 tokens generados,
    /// `slot context shift (n_discard = 2045)`, HTTP 500 a los 1m49s y el daemon abajo.
    pub num_predict: usize,
    /// Ventana de contexto explícita (`num_ctx`). Se fija por **latencia objetivo**, no por el máximo
    /// del modelo: 16k con un 7B en esta placa son 426 s de TTFT.
    pub num_ctx: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            url: "http://localhost:11434".to_string(),
            modelo: "granite3.3:2b".to_string(),
            keep_alive: "5m".to_string(),
            tope_por_pedido: 5,
            num_predict: 1024,
            num_ctx: 4096,
        }
    }
}

impl Config {
    pub fn desde(cfg: Option<&Value>) -> Config {
        let mut c = Config::default();
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        if let Some(seccion) = cfg {
            if let Some(v) = seccion.get("modelo").and_then(|v| v.as_str()) {
                c.modelo = v.to_string();
            }
            if let Some(v) = seccion.get("keep_alive").and_then(|v| v.as_str()) {
                c.keep_alive = v.to_string();
            }
            if let Some(v) = seccion.get("url").and_then(|v| v.as_str()) {
                c.url = v.to_string();
            }
            if let Some(v) = seccion.get("tope_por_pedido").and_then(|v| v.as_u64()) {
                c.tope_por_pedido = (v as usize).clamp(1, 15);
            }
            if let Some(v) = seccion.get("num_predict").and_then(|v| v.as_u64()) {
                c.num_predict = (v as usize).clamp(64, 8192);
            }
            if let Some(v) = seccion.get("num_ctx").and_then(|v| v.as_u64()) {
                c.num_ctx = (v as usize).clamp(1024, 32768);
            }
        }
        // El daemon compatible con OpenAI se anuncia con `/v1`; el nativo no lo lleva.
        if let Some(u) = env("NODEFLOW_DRAFT_URL").or_else(|| env("NODEFLOW_OLLAMA_URL")) {
            c.url = u.trim_end_matches('/').trim_end_matches("/v1").to_string();
        }
        if let Some(m) = env("NODEFLOW_DRAFT_MODEL").or_else(|| env("NODEFLOW_OLLAMA_MODEL")) {
            c.modelo = m;
        }
        if let Some(k) = env("NODEFLOW_DRAFT_KEEP_ALIVE") {
            c.keep_alive = k;
        }
        if let Some(n) = env("NODEFLOW_NUM_PREDICT").and_then(|v| v.trim().parse::<usize>().ok()) {
            c.num_predict = n.clamp(64, 8192);
        }
        if let Some(n) = env("NODEFLOW_NUM_CTX").and_then(|v| v.trim().parse::<usize>().ok()) {
            c.num_ctx = n.clamp(1024, 32768);
        }
        c
    }
}

/// Plegado de acentos y mayúsculas, para comparar etiquetas escritas por un humano o por un modelo
/// («tecnología» ≡ «TECNOLOGIA»).
pub fn plano(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.trim().to_lowercase().chars() {
        out.push(match ch {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            otro => otro,
        });
    }
    out
}

/// Esquema que se pasa como gramática (`format`) al daemon. Garantiza la forma; **no** la verdad.
pub fn esquema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "titulo": { "type": "string", "minLength": MIN_TITULO, "maxLength": MAX_TITULO },
            "categoria": { "type": "string", "enum": CATEGORIAS },
            "madurez": { "type": "integer", "minimum": MADUREZ_MIN, "maximum": MADUREZ_MAX },
            "tags": {
                "type": "array",
                "items": { "type": "string" },
                "minItems": MIN_TAGS,
                "maxItems": MAX_TAGS
            }
        },
        "required": ["titulo", "categoria", "madurez", "tags"]
    })
}

/// Instrucción para el 2B. Corta y con las reglas que el validador va a exigir, para que un rechazo
/// sea una excepción y no la norma.
pub fn prompt(titulo_heuristico: &str, cuerpo: &str) -> String {
    let cuerpo: String = cuerpo.chars().take(RECORTE_CUERPO).collect();
    format!(
        "Clasificás un fragmento para un mapa mental. Devolvés SOLO el JSON del esquema, sin texto extra.\n\
         CATEGORÍA: elegí una de esta lista, sin inventar ninguna: {}.\n\
         MADUREZ: entero de {MADUREZ_MIN} a {MADUREZ_MAX} ({MADUREZ_MIN} = idea suelta, {MADUREZ_MAX} = \
         validada en la práctica).\n\
         TAGS: entre {MIN_TAGS} y {MAX_TAGS}, en minúsculas, sin símbolos ni números sueltos, \
         entre {MIN_TAG} y {MAX_TAG} caracteres.\n\
         TITULO: entre {MIN_TITULO} y {MAX_TITULO} caracteres, concreto, sin comillas ni markdown.\n\n\
         TÍTULO PROPUESTO POR EL SISTEMA: {titulo_heuristico}\n\
         FRAGMENTO:\n{cuerpo}",
        CATEGORIAS.join(", ")
    )
}

/// Un borrador ya validado: lo que el sistema acepta usar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Borrador {
    pub titulo: String,
    pub categoria: String,
    pub madurez: i64,
    pub tags: Vec<String>,
    /// Todo lo que el modelo propuso y el validador rechazó, con el motivo y el reemplazo.
    pub problemas: Vec<String>,
    /// `local` = el modelo aportó todos los campos; `mixto` = aportó algunos; `heuristica` = ninguno
    /// (el modelo no respondió o todo lo suyo se rechazó).
    pub fuente: String,
}

impl Borrador {
    pub fn json(&self) -> Value {
        json!({
            "titulo": self.titulo,
            "categoria": self.categoria,
            "madurez": self.madurez,
            "tags": self.tags,
            "problemas": self.problemas,
            "fuente": self.fuente,
        })
    }
}

fn titulo_valido(t: &str) -> Result<String, String> {
    let limpio = t
        .trim()
        .trim_matches(|c: char| c == '"' || c == '.' || c == '#' || c == '*')
        .trim()
        .replace('\n', " ");
    let largo = limpio.chars().count();
    if largo < MIN_TITULO {
        return Err(format!(
            "título de {largo} caracteres (mínimo {MIN_TITULO})"
        ));
    }
    if largo > MAX_TITULO {
        return Err(format!(
            "título de {largo} caracteres (máximo {MAX_TITULO})"
        ));
    }
    if !limpio.chars().any(|c| c.is_alphanumeric()) {
        return Err("título sin caracteres alfanuméricos".to_string());
    }
    Ok(limpio)
}

fn categoria_valida(c: &str, extra: &[String]) -> Option<String> {
    let p = plano(c);
    // Vocabulario dinámico primero: las categorías que el lienzo ya usa son verdad medida, y así el
    // validador se auto-repara cuando el usuario inventa una categoría nueva en la app.
    if let Some(k) = extra.iter().find(|k| plano(k) == p) {
        return Some(k.clone());
    }
    CATEGORIAS
        .iter()
        .find(|k| plano(k) == p)
        .map(|k| (*k).to_string())
}

fn tag_limpio(t: &str) -> Option<String> {
    let limpio: String = t
        .trim()
        .trim_start_matches('#')
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect();
    let largo = limpio.chars().count();
    if largo < MIN_TAG || largo > MAX_TAG {
        return None;
    }
    if !limpio.chars().any(|c| c.is_alphabetic()) {
        return None;
    }
    Some(limpio)
}

/// Devuelve los tags usables **y cuántos se descartaron**. Descartar alguno no invalida el campo:
/// la regla es que queden al menos `MIN_TAGS` válidos (si no, el campo entero cae al del sistema).
fn tags_validos(v: &Value) -> Result<(Vec<String>, usize), String> {
    let Some(arr) = v.as_array() else {
        return Err("tags no es una lista".to_string());
    };
    let mut out: Vec<String> = Vec::new();
    let mut descartados = 0usize;
    for t in arr {
        match t.as_str().and_then(tag_limpio) {
            Some(tag) if !out.contains(&tag) => out.push(tag),
            _ => descartados += 1,
        }
    }
    if out.len() > MAX_TAGS {
        descartados += out.len() - MAX_TAGS;
        out.truncate(MAX_TAGS);
    }
    if out.len() < MIN_TAGS {
        return Err(format!(
            "quedaron {} tag(s) válidos de {} (se piden {MIN_TAGS}-{MAX_TAGS})",
            out.len(),
            arr.len()
        ));
    }
    Ok((out, descartados))
}

/// **El corazón de la fase**: convierte lo que devolvió el modelo en algo usable, rechazando campo por
/// campo y cayendo al valor determinista. Nunca devuelve un borrador con un valor sin verificar.
/// Envoltorio sin vocabulario dinámico: lo usan los tests y cualquier llamador que no tenga el grafo
/// a mano. El camino de producción usa `validar_con` con las categorías vivas del lienzo.
#[allow(dead_code)]
pub fn validar(crudo: Option<&Value>, titulo_fallback: &str, categoria_fallback: &str) -> Borrador {
    validar_con(crudo, titulo_fallback, categoria_fallback, &[])
}

/// Igual que `validar`, pero además acepta las categorías que el lienzo ya tiene (`extra`).
pub fn validar_con(
    crudo: Option<&Value>,
    titulo_fallback: &str,
    categoria_fallback: &str,
    extra: &[String],
) -> Borrador {
    let mut problemas: Vec<String> = Vec::new();
    let mut aportados = 0usize;

    // Título
    let titulo = match crudo.and_then(|c| c.get("titulo")).and_then(|v| v.as_str()) {
        Some(t) => match titulo_valido(t) {
            Ok(t) => {
                aportados += 1;
                t
            }
            Err(e) => {
                problemas.push(format!("{e} → uso el título del sistema"));
                titulo_fallback.to_string()
            }
        },
        None => {
            problemas.push("sin título del modelo → uso el título del sistema".to_string());
            titulo_fallback.to_string()
        }
    };

    // Categoría
    let categoria = match crudo.and_then(|c| c.get("categoria")).and_then(|v| v.as_str()) {
        Some(c) => match categoria_valida(c, extra) {
            Some(c) => {
                aportados += 1;
                c
            }
            None => {
                problemas.push(format!(
                    "categoría «{c}» fuera del catálogo → uso {categoria_fallback}"
                ));
                categoria_fallback.to_string()
            }
        },
        None => {
            problemas.push(format!(
                "sin categoría del modelo → uso {categoria_fallback}"
            ));
            categoria_fallback.to_string()
        }
    };

    // Madurez (el pitfall medido: JSON válido con madurez 100)
    let madurez = match crudo.and_then(|c| c.get("madurez")).and_then(|v| v.as_i64()) {
        Some(m) if (MADUREZ_MIN..=MADUREZ_MAX).contains(&m) => {
            aportados += 1;
            m
        }
        Some(m) => {
            problemas.push(format!(
                "madurez {m} fuera de {MADUREZ_MIN}-{MADUREZ_MAX} → uso 2"
            ));
            2
        }
        None => {
            problemas.push("sin madurez del modelo → uso 2".to_string());
            2
        }
    };

    // Tags
    let tags = match crudo.and_then(|c| c.get("tags")) {
        Some(v) => match tags_validos(v) {
            Ok((t, descartados)) => {
                aportados += 1;
                if descartados > 0 {
                    problemas.push(format!(
                        "{descartados} tag(s) descartados por forma; se usan los válidos"
                    ));
                }
                t
            }
            Err(e) => {
                problemas.push(format!("{e} → uso la etiqueta del sistema"));
                vec!["conocimiento".to_string()]
            }
        },
        None => {
            problemas.push("sin tags del modelo → uso la etiqueta del sistema".to_string());
            vec!["conocimiento".to_string()]
        }
    };

    let fuente = match aportados {
        0 => "heuristica",
        4 => "local",
        _ => "mixto",
    };

    Borrador { titulo, categoria, madurez, tags, problemas, fuente: fuente.to_string() }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn completo(titulo: &str, categoria: &str, madurez: i64, tags: Value) -> Value {
        json!({ "titulo": titulo, "categoria": categoria, "madurez": madurez, "tags": tags })
    }

    #[test]
    fn un_borrador_completo_y_valido_se_acepta() {
        let crudo = completo("Puente OSC entre TouchDesigner y Ableton", "TECNOLOGÍA", 3, json!(["osc", "touchdesigner"]));
        let b = validar(Some(&crudo), "heurístico", "CONOCIMIENTO");
        assert_eq!(b.fuente, "local");
        assert!(b.problemas.is_empty(), "{:?}", b.problemas);
        assert_eq!(b.categoria, "TECNOLOGÍA");
        assert_eq!(b.madurez, 3);
        assert_eq!(b.tags, vec!["osc", "touchdesigner"]);
    }

    #[test]
    fn la_madurez_fuera_de_rango_se_rechaza_aunque_el_json_sea_valido() {
        // El pitfall medido: la gramática garantiza la forma, no el valor.
        for m in [0, 100, -3] {
            let crudo = completo("Título razonable", "IDEA", m, json!(["uno", "dos"]));
            let b = validar(Some(&crudo), "heurístico", "CONOCIMIENTO");
            assert_eq!(b.madurez, 2, "madurez {m} debía rechazarse");
            assert!(b.problemas.iter().any(|p| p.contains("madurez")), "{:?}", b.problemas);
        }
    }

    #[test]
    fn una_categoria_inventada_se_rechaza_y_cae_al_valor_del_sistema() {
        let crudo = completo("Título razonable", "MARKETING CUANTICO", 3, json!(["uno", "dos"]));
        let b = validar(Some(&crudo), "heurístico", "CONOCIMIENTO");
        assert_eq!(b.categoria, "CONOCIMIENTO");
        assert!(b.problemas.iter().any(|p| p.contains("fuera del catálogo")));
    }

    #[test]
    fn la_categoria_se_normaliza_sin_acentos_ni_mayusculas() {
        let crudo = completo("Título razonable", "tecnologia", 2, json!(["uno", "dos"]));
        assert_eq!(validar(Some(&crudo), "h", "CONOCIMIENTO").categoria, "TECNOLOGÍA");
        let crudo = completo("Título razonable", "  investigación ", 2, json!(["uno", "dos"]));
        assert_eq!(validar(Some(&crudo), "h", "CONOCIMIENTO").categoria, "INVESTIGACIÓN");
    }

    #[test]
    fn los_titulos_imposibles_se_rechazan_y_usan_el_heurístico() {
        let caso = |t: &str| {
            let crudo = completo(t, "IDEA", 2, json!(["uno", "dos"]));
            validar(Some(&crudo), "Título del sistema", "CONOCIMIENTO")
        };
        // demasiado corto
        let b = caso("ab");
        assert_eq!(b.titulo, "Título del sistema");
        assert!(b.problemas.iter().any(|p| p.contains("mínimo")));
        // demasiado largo
        let largo = "x".repeat(MAX_TITULO + 5);
        assert_eq!(caso(&largo).titulo, "Título del sistema");
        // sin alfanuméricos
        assert_eq!(caso("。。。").titulo, "Título del sistema");
        // con comillas y markdown alrededor: se limpia, no se rechaza
        let b = caso("\"## Un título con markdown\"");
        assert_eq!(b.titulo, "Un título con markdown");
        assert!(b.problemas.is_empty() || !b.problemas.iter().any(|p| p.contains("título")), "{:?}", b.problemas);
    }

    #[test]
    fn los_tags_basura_se_filtran_y_sin_minimo_se_usa_el_del_sistema() {
        // dos válidos y basura alrededor: se usan los válidos (y se declara el descarte)
        let crudo = completo("Título razonable", "IDEA", 2, json!(["!!", "a", "rag", "MCP", "2026", "un-tag-demasiado-largo-para-el-limite"]));
        let b = validar(Some(&crudo), "h", "CONOCIMIENTO");
        assert_eq!(b.tags, vec!["rag", "mcp"]);
        assert!(b.problemas.iter().any(|p| p.contains("descartados")), "{:?}", b.problemas);
        // uno solo válido: no alcanza el mínimo ⇒ etiqueta del sistema
        let crudo = completo("Título razonable", "IDEA", 2, json!(["rag", "!!!"]));
        let b = validar(Some(&crudo), "h", "CONOCIMIENTO");
        assert_eq!(b.tags, vec!["conocimiento"]);
        assert!(b.problemas.iter().any(|p| p.contains("tag(s) válidos")));
        // tags que no son lista
        let crudo = completo("Título razonable", "IDEA", 2, json!("rag,mcp"));
        assert_eq!(validar(Some(&crudo), "h", "CONOCIMIENTO").tags, vec!["conocimiento"]);
    }

    #[test]
    fn el_tope_de_tags_recorta_y_declara() {
        let crudo = completo("Título razonable", "IDEA", 2, json!(["uno", "dos", "tres", "cuatro", "cinco", "seis", "siete"]));
        let b = validar(Some(&crudo), "h", "CONOCIMIENTO");
        assert_eq!(b.tags.len(), MAX_TAGS);
        assert!(b.problemas.iter().any(|p| p.contains("descartados")));
    }

    #[test]
    fn sin_respuesta_del_modelo_el_borrador_es_determinista_y_lo_declara() {
        let b = validar(None, "Título del sistema", "MEMORIA");
        assert_eq!(b.fuente, "heuristica");
        assert_eq!(b.titulo, "Título del sistema");
        assert_eq!(b.categoria, "MEMORIA");
        assert_eq!(b.madurez, 2);
        assert_eq!(b.tags, vec!["conocimiento"]);
        assert_eq!(b.problemas.len(), 4, "{:?}", b.problemas);
    }

    #[test]
    fn un_borrador_a_medias_se_marca_mixto() {
        // título y categoría buenos, madurez y tags rechazados
        let crudo = completo("Un título bueno", "PRODUCTO", 9, json!(["!"]));
        let b = validar(Some(&crudo), "h", "CONOCIMIENTO");
        assert_eq!(b.fuente, "mixto");
        assert_eq!(b.titulo, "Un título bueno");
        assert_eq!(b.categoria, "PRODUCTO");
        assert_eq!(b.madurez, 2);
        assert_eq!(b.tags, vec!["conocimiento"]);
    }

    #[test]
    fn el_esquema_y_el_prompt_dicen_lo_que_el_validador_exige() {
        let e = esquema();
        assert_eq!(e["properties"]["categoria"]["enum"].as_array().unwrap().len(), CATEGORIAS.len());
        assert_eq!(e["properties"]["madurez"]["maximum"], MADUREZ_MAX);
        assert_eq!(e["properties"]["madurez"]["minimum"], MADUREZ_MIN);
        assert_eq!(e["properties"]["tags"]["maxItems"], MAX_TAGS);
        let p = prompt("Título del sistema", &"cuerpo ".repeat(400));
        for c in CATEGORIAS {
            assert!(p.contains(c), "el prompt tiene que listar {c}");
        }
        assert!(p.contains(&MADUREZ_MAX.to_string()));
        assert!(p.chars().count() < RECORTE_CUERPO + 900, "el prompt no se estira de más");
    }

    #[test]
    fn el_vocabulario_del_lienzo_manda_sobre_la_lista_fija() {
        // Una categoría nueva que el lienzo ya usa se acepta cuando se la pasa como vocabulario
        // medido; sin eso, el validador la rechaza (no se inventan etiquetas que ninguna vista conoce).
        let crudo = completo("Título razonable", "CATEGORÍA NUEVA", 3, json!(["uno", "dos"]));
        assert_eq!(
            validar(Some(&crudo), "h", "CONOCIMIENTO").categoria,
            "CONOCIMIENTO"
        );
        let extra = vec!["CATEGORÍA NUEVA".to_string()];
        assert_eq!(
            validar_con(Some(&crudo), "h", "CONOCIMIENTO", &extra).categoria,
            "CATEGORÍA NUEVA"
        );
        // El vocabulario medido del lienzo (16 categorías reales) está en la lista base: si no,
        // el validador habría rechazado 15 de las 16 en la primera corrida real.
        for real in ["ARQUITECTURA", "GOBERNANZA", "DATOS", "EJECUCIÓN", "UX", "SEGURIDAD", "SISTEMAS", "ALGORITMOS", "ECOSISTEMA", "VISUALES", "DISEÑO", "IA", "NEGOCIO", "CREATIVO", "REFERENCIA"] {
            assert!(CATEGORIAS.contains(&real), "falta {real} del vocabulario del lienzo");
        }
        // y las del catálogo base siguen entrando sin `extra`
        for c in CATEGORIAS {
            let crudo = completo("Título razonable", c, 3, json!(["uno", "dos"]));
            assert_eq!(
                validar(Some(&crudo), "h", "CONOCIMIENTO").categoria,
                c,
                "no se aceptó {c}"
            );
        }
    }

    #[test]
    fn la_config_prioriza_entorno_config_y_default() {
        let cfg = json!({ "modelo": "otro:2b", "keep_alive": "0", "tope_por_pedido": 99, "num_predict": 99999, "num_ctx": 64 });
        let c = Config::desde(Some(&cfg));
        assert_eq!(c.modelo, "otro:2b");
        assert_eq!(c.keep_alive, "0");
        assert_eq!(c.tope_por_pedido, 15, "el tope se acota al máximo razonable");
        // Los topes de generación se acotan: sin cota el modelo se explaya, cruza el contexto y el
        // pedido muere con 500 (medido 15/09).
        assert_eq!(c.num_predict, 8192, "el tope de salida no pasa de 8192");
        assert_eq!(c.num_ctx, 1024, "la ventana no baja de 1024");
        let d = Config::desde(None);
        assert_eq!(d.modelo, "granite3.3:2b");
        assert_eq!(d.keep_alive, "5m");
        assert_eq!(d.num_predict, 1024);
        assert_eq!(d.num_ctx, 4096);
        assert_eq!(d.url, "http://localhost:11434");
        // el endpoint compatible con OpenAI se normaliza al nativo
        assert_eq!(Config::desde(None).url.trim_end_matches("/v1"), "http://localhost:11434");
    }
}

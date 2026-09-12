//! Fase 8 — Captura de conocimiento: convertir texto crudo en nodos PROPUESTOS.
//!
//! Principio de diseño: **capturar sin curar es envenenar la memoria**. La bóveda alimenta los
//! prompts de generación (top-3 por BM25), así que una nota de bajo valor empeora los resultados y
//! el síntoma queda a tres capas de la causa. Por eso acá solo se PROPONE: el humano aprueba en el
//! panel y recién entonces el nodo entra al grafo y a la bóveda.
//!
//! La segmentación es local, determinista y sin IA: corta por secciones (headings, líneas en blanco,
//! viñetas), descarta lo que no tiene densidad suficiente y arma títulos legibles. Nada de magia:
//! si el texto es malo, la vista previa lo muestra y el humano no lo aprueba.

use serde_json::{json, Value};

/// Un bloque necesita esta densidad mínima para ser candidato a nodo.
const MIN_CHARS: usize = 70;
/// Tope de candidatos por captura: capturar 40 nodos de un pegado es engordar, no alimentar.
const MAX_NODOS: usize = 15;
/// Largo máximo del título de un nodo.
const MAX_TITULO: usize = 62;
/// Largo mínimo del título: una sigla real ("RAG", "MCP") es un título legítimo, una sola letra no.
const MIN_TITULO: usize = 4;

fn limpiar_markdown(t: &str) -> String {
    let mut s = t.trim().to_string();
    // Sacar marcadores de lista y énfasis del arranque.
    while let Some(rest) = s
        .strip_prefix("- ")
        .or_else(|| s.strip_prefix("* "))
        .or_else(|| s.strip_prefix("+ "))
        .or_else(|| s.strip_prefix("> "))
    {
        s = rest.trim().to_string();
    }
    s = s
        .trim_start_matches('#')
        .trim()
        .replace("**", "")
        .replace("__", "")
        .replace('`', "");
    s.trim().to_string()
}

/// Título legible: el heading si lo hay; si no, la primera oración recortada a `MAX_TITULO`.
fn titulo_de(bloque: &str) -> String {
    let primera = bloque.lines().next().unwrap_or("").trim();
    if primera.starts_with('#') {
        let t = limpiar_markdown(primera);
        if !t.is_empty() {
            return recortar(&t);
        }
    }
    let limpio = limpiar_markdown(bloque);
    // Cortar en el primer separador fuerte de oración.
    for sep in ['.', ':', ';', '?', '!', '\n'] {
        if let Some(i) = limpio.find(sep) {
            let corte = limpio[..i].trim();
            if corte.chars().count() >= 18 {
                return recortar(corte);
            }
        }
    }
    recortar(&limpio)
}

fn recortar(t: &str) -> String {
    let limpio = t.trim().trim_end_matches(['.', ':', ';', ',']);
    if limpio.chars().count() <= MAX_TITULO {
        return limpio.to_string();
    }
    let corto: String = limpio.chars().take(MAX_TITULO).collect();
    match corto.rfind(' ') {
        Some(i) if i > 24 => format!("{}…", corto[..i].trim()),
        _ => format!("{corto}…"),
    }
}

/// Corta el texto en bloques: un heading o una línea en blanco separan secciones.
fn bloques(texto: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut actual: Vec<String> = Vec::new();
    let mut dentro_de_codigo = false;
    for linea in texto.lines() {
        let l = linea.trim_end();
        if l.trim_start().starts_with("```") {
            dentro_de_codigo = !dentro_de_codigo;
        }
        if dentro_de_codigo {
            actual.push(l.to_string());
            continue;
        }
        let es_heading = l.trim_start().starts_with("# ") || l.trim_start().starts_with("## ");
        let vacia = l.trim().is_empty();
        // Un heading SOLO (todavía sin cuerpo) no cierra bloque: si no, la línea en blanco que sigue
        // lo separaría de su propio texto y el título saldría del cuerpo, no del heading.
        let solo_heading = actual.len() == 1 && actual[0].trim_start().starts_with('#');
        if (es_heading || vacia) && !actual.is_empty() && !solo_heading {
            let b = actual.join("\n").trim().to_string();
            if !b.is_empty() {
                out.push(b);
            }
            actual.clear();
            if es_heading {
                actual.push(l.to_string());
            }
            continue;
        }
        if !vacia {
            actual.push(l.to_string());
        }
    }
    let b = actual.join("\n").trim().to_string();
    if !b.is_empty() {
        out.push(b);
    }
    out
}

/// Convierte texto crudo en candidatos a nodo. Determinista: mismo texto ⇒ mismos candidatos.
pub fn segmentar(texto: &str, min_chars: Option<usize>, max_nodos: Option<usize>) -> Vec<Value> {
    let min = min_chars.unwrap_or(MIN_CHARS);
    let max = max_nodos.unwrap_or(MAX_NODOS);
    let mut out: Vec<Value> = Vec::new();
    let mut vistos: std::collections::HashSet<String> = std::collections::HashSet::new();
    for b in bloques(texto) {
        let limpio = b.trim().to_string();
        if limpio.chars().count() < min {
            continue;
        }
        let titulo = titulo_de(&limpio);
        let clave = titulo.to_lowercase();
        if titulo.chars().count() < MIN_TITULO || !vistos.insert(clave) {
            continue; // sin título útil o repetido
        }
        // El cuerpo va sin el heading inicial (ya está en el título).
        let cuerpo = if limpio.lines().next().unwrap_or("").trim_start().starts_with('#') {
            limpio.lines().skip(1).collect::<Vec<_>>().join("\n").trim().to_string()
        } else {
            limpio.clone()
        };
        out.push(json!({
            "titulo": titulo,
            "descripcion": if cuerpo.is_empty() { limpio.clone() } else { cuerpo },
            "categoria": "CONOCIMIENTO",
            "madurez": 2,
            "caracteres": limpio.chars().count(),
        }));
        if out.len() >= max {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corta_por_secciones_y_arma_titulos() {
        let texto = "# Puente HTTP /api/graph\n\nExpone lectura y escritura del grafo en localhost:37371. \
Sostiene el MCP, el panel de propuestas y el autoguardado del lienzo.\n\n# Politica de invariantes\n\n\
El borde de escritura repara en silencio las piezas rotas; el agente en cambio recibe el error antes de \
escribir, porque su llamada es atomica y la puede corregir.";
        let c = segmentar(texto, None, None);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0]["titulo"], "Puente HTTP /api/graph");
        assert_eq!(c[1]["titulo"], "Politica de invariantes");
        // El cuerpo no repite el heading
        assert!(!c[0]["descripcion"].as_str().unwrap().starts_with('#'));
        assert!(c[0]["descripcion"].as_str().unwrap().contains("37371"));
    }

    #[test]
    fn descarta_bloques_sin_densidad() {
        let texto = "# A\n\ncorto\n\n# Bloque bueno\n\nEste bloque si tiene la densidad minima \
necesaria para convertirse en un candidato a nodo de conocimiento utilizable.";
        let c = segmentar(texto, None, None);
        assert_eq!(c.len(), 1, "el bloque corto no debe ser candidato");
        assert_eq!(c[0]["titulo"], "Bloque bueno");
        // Un título de una sola letra no es un título
        assert!(segmentar("# X\n\nContenido suficientemente largo para pasar el filtro de densidad minima exigido.", None, None).is_empty());
    }

    #[test]
    fn respeta_el_tope_de_nodos() {
        let mut texto = String::new();
        for i in 0..30 {
            texto.push_str(&format!(
                "\n# Tema {i}\n\nContenido suficientemente largo del tema {i} para pasar el filtro de densidad minima.\n"
            ));
        }
        let c = segmentar(&texto, None, Some(5));
        assert_eq!(c.len(), 5, "no puede inundar el lienzo: hay tope");
    }

    #[test]
    fn titulo_sin_heading_se_recorta_en_la_primera_oracion() {
        let texto = "La memoria semantica de la boveda se implementa con BM25 lexico en Rust, sin \
modelos ni red, y con plegado de acentos para que buscar informacion encuentre información.";
        let c = segmentar(texto, None, None);
        assert_eq!(c.len(), 1);
        let t = c[0]["titulo"].as_str().unwrap();
        assert!(t.chars().count() <= MAX_TITULO + 1, "el titulo se recorta: {t}");
        assert!(!t.ends_with('.'), "sin punto final");
    }

    #[test]
    fn es_determinista_y_dedupea() {
        let texto = "# Mismo tema\n\nContenido largo y suficiente para pasar el filtro de densidad minima del segmentador.\n\n\
# Mismo tema\n\nOtro contenido igual de largo para pasar el mismo filtro de densidad minima exigido.";
        let a = segmentar(texto, None, None);
        let b = segmentar(texto, None, None);
        assert_eq!(a, b, "mismo texto ⇒ mismos candidatos");
        assert_eq!(a.len(), 1, "titulos repetidos se descartan");
    }

    #[test]
    fn texto_vacio_no_produce_nada() {
        assert!(segmentar("", None, None).is_empty());
        assert!(segmentar("   \n\n  ", None, None).is_empty());
    }
}

//! Slice 1 — Registro de Expertos.
//!
//! Un **Experto** es una nota de la bóveda. El frontmatter declara qué es y qué produce; el cuerpo
//! del archivo es el **system prompt**. Se refinan escribiendo, como cualquier nota — no programando.
//!
//! Esto reemplaza la idea de automatizar sesiones web: un "Gem" es, en su esencia, un system prompt,
//! y el activo reutilizable es el prompt, no la sesión de Chrome.

use serde_json::{json, Value};
use std::path::Path;

/// Carpeta dentro del vault donde viven los expertos.
pub const CARPETA: &str = "expertos";

/// Parsea el frontmatter simple (`clave: valor` entre dos líneas `---`) y devuelve el cuerpo.
pub fn frontmatter(txt: &str) -> (Vec<(String, String)>, String) {
    let mut pares: Vec<(String, String)> = Vec::new();
    let mut lineas = txt.lines();
    let primera = lineas.next().unwrap_or("");
    if primera.trim() != "---" {
        return (pares, txt.trim().to_string());
    }
    let mut cuerpo: Vec<&str> = Vec::new();
    let mut cerrado = false;
    for l in lineas {
        if !cerrado && l.trim() == "---" {
            cerrado = true;
            continue;
        }
        if cerrado {
            cuerpo.push(l);
            continue;
        }
        if let Some((k, v)) = l.split_once(':') {
            let k = k.trim().to_lowercase();
            let v = v
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string();
            if !k.is_empty() && !v.is_empty() {
                pares.push((k, v));
            }
        }
    }
    (pares, cuerpo.join("\n").trim().to_string())
}

fn campo(pares: &[(String, String)], clave: &str) -> String {
    pares
        .iter()
        .find(|(k, _)| k == clave)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

/// Arma el archivo de un experto: frontmatter canónico + cuerpo (el system prompt, tal cual se pegó).
///
/// Es pura y se prueba sola: es la única pieza del guardado que puede meter basura en la bóveda.
/// Los campos opcionales vacíos **no se escriben** (un `rol: ` vacío ensucia el frontmatter).
pub fn md_desde(nombre: &str, tipo: &str, campos: &[(&str, &str)], system: &str) -> String {
    let limpiar = |s: &str| {
        s.trim()
            .lines()
            .collect::<Vec<_>>()
            .join(" ")
            .replace('"', "'")
    };
    let mut txt = String::from("---\n");
    txt.push_str(&format!("experto: \"{}\"\n", limpiar(nombre)));
    for (k, v) in campos {
        let v = limpiar(v);
        if !v.is_empty() {
            txt.push_str(&format!("{k}: {v}\n"));
        }
    }
    txt.push_str(&format!(
        "tipo_artefacto: {}\n---\n\n{}\n",
        limpiar(tipo),
        system.trim()
    ));
    txt
}

/// Lee los expertos de `<vault>/expertos/*.md`. Un archivo sin frontmatter o sin tipo válido se
/// devuelve igual pero marcado, para que el panel pueda avisar en vez de esconderlo.
pub fn cargar(dir: &Path) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entradas.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("md") {
            continue;
        }
        let Ok(txt) = std::fs::read_to_string(&p) else {
            continue;
        };
        let (pares, system) = frontmatter(&txt);
        let archivo = p
            .file_stem()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_string();
        let nombre = {
            let n = campo(&pares, "experto");
            if n.is_empty() {
                archivo.clone()
            } else {
                n
            }
        };
        let tipo = campo(&pares, "tipo_artefacto");
        let valido = crate::artefactos::schema(&tipo).is_some();
        if system.is_empty() {
            continue; // sin system prompt no hay experto
        }
        out.push(json!({
            "slug": archivo,
            "nombre": nombre,
            "tipo_artefacto": tipo,
            "valido": valido,
            "modelo": campo(&pares, "modelo"),
            "proveedor": campo(&pares, "proveedor"),
            "descripcion": campo(&pares, "descripcion"),
            "rol": campo(&pares, "rol"),
            "system": system,
            "caracteres_system": system.chars().count(),
        }));
    }
    out.sort_by_key(|a| a["nombre"].as_str().unwrap_or("").to_string());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_archivo_que_escribimos_se_vuelve_a_leer_igual() {
        let md = md_desde(
            "Prompt Visual",
            "prompt_visual",
            &[
                ("rol", "curador de arte digital"),
                ("descripcion", ""),
                ("proveedor", "gemini"),
            ],
            "  Sos curador. Regla: no inventes marcas.  ",
        );
        let (pares, cuerpo) = frontmatter(&md);
        assert_eq!(campo(&pares, "experto"), "Prompt Visual");
        assert_eq!(campo(&pares, "tipo_artefacto"), "prompt_visual");
        assert_eq!(campo(&pares, "proveedor"), "gemini");
        // Un campo vacío no se escribe (no deja `descripcion: ` colgado en el frontmatter).
        assert!(campo(&pares, "descripcion").is_empty());
        assert!(!md.contains("descripcion:"));
        // El cuerpo es el prompt, sin los espacios de los bordes.
        assert_eq!(cuerpo, "Sos curador. Regla: no inventes marcas.");
        // Las comillas del nombre no rompen el frontmatter.
        let md2 = md_desde("Mi \"marca\" visual", "critica", &[], "x");
        assert_eq!(campo(&frontmatter(&md2).0, "experto"), "Mi 'marca' visual");
    }

    #[test]
    fn parsea_frontmatter_y_cuerpo() {
        let txt = "---\nexperto: \"Prompt Visual\"\ntipo_artefacto: prompt_visual\ndescripcion: convierte ideas en prompts\n---\n\nSos un director de fotografía.\nRegla: no inventes marcas.";
        let (pares, cuerpo) = frontmatter(txt);
        assert_eq!(campo(&pares, "experto"), "Prompt Visual");
        assert_eq!(campo(&pares, "tipo_artefacto"), "prompt_visual");
        assert_eq!(campo(&pares, "descripcion"), "convierte ideas en prompts");
        assert!(cuerpo.starts_with("Sos un director de fotografía."));
        assert!(cuerpo.contains("Regla: no inventes marcas."));
        assert!(!cuerpo.contains("---"));
    }

    #[test]
    fn sin_frontmatter_el_texto_entero_es_cuerpo() {
        let (pares, cuerpo) = frontmatter("Solo un prompt suelto.");
        assert!(pares.is_empty());
        assert_eq!(cuerpo, "Solo un prompt suelto.");
    }

    #[test]
    fn tolera_valores_con_dos_puntos_y_comillas() {
        let txt =
            "---\ndescripcion: \"guía: arte y musica\"\nmodelo: 'gemini-2.5-flash'\n---\ncuerpo";
        let (pares, _) = frontmatter(txt);
        assert_eq!(campo(&pares, "descripcion"), "guía: arte y musica");
        assert_eq!(campo(&pares, "modelo"), "gemini-2.5-flash");
    }

    #[test]
    fn cargar_marca_los_tipos_invalidos_y_ordena() {
        let dir = std::env::temp_dir().join(format!("nf_expertos_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("visual.md"),
            "---\nexperto: Visual\ntipo_artefacto: prompt_visual\n---\nSos director de foto.",
        )
        .unwrap();
        std::fs::write(
            dir.join("raro.md"),
            "---\nexperto: Aaa Raro\ntipo_artefacto: magia\n---\nHace magia.",
        )
        .unwrap();
        std::fs::write(
            dir.join("sin_prompt.md"),
            "---\nexperto: Vacío\ntipo_artefacto: spec_td\n---\n",
        )
        .unwrap();
        let e = cargar(&dir);
        assert_eq!(e.len(), 2, "el experto sin system prompt se descarta");
        assert_eq!(e[0]["nombre"], "Aaa Raro");
        assert_eq!(e[0]["valido"], false);
        assert_eq!(e[1]["nombre"], "Visual");
        assert_eq!(e[1]["valido"], true);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

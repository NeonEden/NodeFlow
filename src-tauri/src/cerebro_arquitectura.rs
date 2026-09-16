//! Fase 5.4 — el **contexto de la app**: la arquitectura de NodeFlow leída del árbol real.
//!
//! El pedido: *«mi turno cita la arquitectura sin leerla a mano»*. Hasta acá, para hablar del código yo
//! tenía que grepear el repo en cada turno (caro y frágil). Acá se **inventaría** el proyecto —módulos,
//! tamaños, roles (el `//!` de cada archivo), rutas HTTP, herramientas MCP— y se escribe una nota en la
//! bóveda (`cerebro/arquitectura.md`) con un bloque **mapa** acotado que el briefing del turno antepone.
//!
//! Dos reglas:
//! - **Se deriva del árbol**: nada de listas escritas a mano que envejezcan mintiendo. Si un módulo
//!   desaparece, desaparece del mapa.
//! - **Nunca falla por un archivo ausente**: se inventaría lo que se pueda y se dice qué faltó.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// Nombre de la nota que se deja en la bóveda (dentro de `cerebro/`).
pub const NOTA: &str = "arquitectura.md";

/// Marcas del bloque acotado que el briefing del turno extrae (y que el humano puede leer igual).
pub const MAPA_INICIO: &str = "<!-- mapa -->";
pub const MAPA_FIN: &str = "<!-- /mapa -->";

/// Un archivo del proyecto con su rol y tamaño.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pieza {
    pub ruta: String,
    pub lineas: usize,
    pub rol: String,
}

/// Lo que se pudo ver del proyecto. Todo lo que falte queda vacío (nunca es un error fatal).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventario {
    pub rust: Vec<Pieza>,
    pub front: Vec<Pieza>,
    pub rutas: Vec<(String, String)>, // (método, ruta)
    pub tools_mcp: Vec<String>,
    pub faltantes: Vec<String>,
}

impl Inventario {
    pub fn lineas_rust(&self) -> usize {
        self.rust.iter().map(|p| p.lineas).sum()
    }
    pub fn lineas_front(&self) -> usize {
        self.front.iter().map(|p| p.lineas).sum()
    }
    /// Rutas agrupadas por **área**: dos tramos (`/api/cerebro`, `/api/graph`, …), que es como está
    /// organizada la API. Agrupar por tres haría un cajón por ruta y el mapa dejaría de decir algo.
    pub fn por_area(&self) -> BTreeMap<String, Vec<(String, String)>> {
        let mut m: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        for (metodo, ruta) in &self.rutas {
            let tramos: Vec<&str> = ruta.split('/').filter(|t| !t.is_empty()).collect();
            let area = match tramos.len() {
                0 => "/".to_string(),
                1 => format!("/{}", tramos[0]),
                _ => format!("/{}/{}", tramos[0], tramos[1]),
            };
            m.entry(area)
                .or_default()
                .push((metodo.clone(), ruta.clone()));
        }
        m
    }
}

/// El rol de un módulo: la primera línea del doc-comment `//!`, sin el prefijo. Es la voz del que lo
/// escribió — mejor que cualquier resumen mío.
fn rol_de(texto: &str) -> String {
    for l in texto.lines().take(12) {
        let l = l.trim();
        if let Some(resto) = l.strip_prefix("//!") {
            let limpio = resto.trim().trim_start_matches(['—', '-', '·']).trim();
            if !limpio.is_empty() {
                return recortar(limpio, 150);
            }
        }
        // El doc-comment termina cuando empieza el código: no buscar más abajo.
        if !l.is_empty() && !l.starts_with("//") && !l.starts_with("#![") {
            break;
        }
    }
    String::new()
}

fn recortar(t: &str, tope: usize) -> String {
    if t.chars().count() <= tope {
        return t.to_string();
    }
    let corte: String = t.chars().take(tope).collect();
    format!("{}…", corte.trim_end())
}

/// Las rutas HTTP declaradas en `server.rs`, con su método. Es un **escaneo** del `.route("…", …)`:
/// si alguien cambia de forma la declaración, esto se degrada (no rompe) y el doc lo va a mostrar.
pub fn rutas_de(texto: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut resto = texto;
    while let Some(i) = resto.find(".route(\"") {
        let desde = &resto[i + 8..];
        let Some(fin) = desde.find('"') else { break };
        let ruta = desde[..fin].to_string();
        let cola: String = desde[fin..].chars().take(160).collect();
        let mut metodos: Vec<String> = ["get", "post", "put", "patch", "delete"]
            .iter()
            .filter(|m| cola.contains(&format!("{m}(")))
            .map(|m| m.to_uppercase())
            .collect();
        if metodos.is_empty() {
            metodos.push("GET".to_string());
        }
        for m in metodos {
            out.push((m, ruta.clone()));
        }
        resto = &desde[fin..];
    }
    out
}

/// Los nombres de las herramientas MCP: sólo lo declarado dentro de la lista `TOOLS = [ … ]` del
/// servidor. El resto del archivo también tiene `"name"` (la respuesta `serverInfo`), y confundirlos
/// hace que el inventario mienta — el peor defecto posible en un documento que se cita solo.
pub fn tools_de(texto: &str) -> Vec<String> {
    let Some(ini) = texto.find("TOOLS = [") else {
        return Vec::new();
    };
    let desde = &texto[ini..];
    let fin = desde.find("\n]").unwrap_or(desde.len());
    let bloque = &desde[..fin];
    let mut out = Vec::new();
    for (i, _) in bloque.match_indices("\"name\":") {
        let cola: String = bloque[i + 7..].chars().take(120).collect();
        let cola = cola.trim_start();
        let Some(resto) = cola.strip_prefix('"') else {
            continue;
        };
        let Some(fin_nombre) = resto.find('"') else {
            continue;
        };
        let nombre = &resto[..fin_nombre];
        if !nombre.is_empty()
            && nombre
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !out.contains(&nombre.to_string())
        {
            out.push(nombre.to_string());
        }
    }
    out
}

fn piezas_de(dir: &Path, filtro: &dyn Fn(&Path) -> bool) -> Vec<Pieza> {
    let mut out: Vec<Pieza> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.filter_map(|e| e.ok()) {
            let p = e.path();
            if !p.is_file() || !filtro(&p) {
                continue;
            }
            let Ok(texto) = std::fs::read_to_string(&p) else {
                continue;
            };
            let nombre = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string();
            out.push(Pieza {
                ruta: nombre,
                lineas: texto.lines().count(),
                rol: rol_de(&texto),
            });
        }
    }
    out.sort_by(|a, b| a.ruta.cmp(&b.ruta));
    out
}

/// Busca la raíz del repo a partir de una pista (la config, el entorno o la ubicación del ejecutable):
/// sube por los ancestros hasta encontrar `src-tauri/src/server.rs`. Así el cerebro encuentra el código
/// aunque lo llamen desde el binario instalado en `%LOCALAPPDATA%`.
pub fn buscar_repo(pista: &Path) -> Option<std::path::PathBuf> {
    let arranque = if pista.is_dir() {
        pista.to_path_buf()
    } else {
        pista.parent()?.to_path_buf()
    };
    arranque
        .ancestors()
        .find(|d| d.join("src-tauri").join("src").join("server.rs").is_file())
        .map(|d| d.to_path_buf())
}

/// El frontend: la raíz de `src/` (entrada y tipos) más `components/` y `services/`. La carpeta va en el
/// nombre (`components/CerebroPanel.tsx`) para que el documento diga dónde vive cada cosa.
fn piezas_front(fe: &Path) -> Vec<Pieza> {
    let mut out = piezas_de(fe, &|p| {
        matches!(p.extension().and_then(|x| x.to_str()), Some("tsx" | "ts"))
    });
    for sub in ["components", "services"] {
        let dir = fe.join(sub);
        for mut p in piezas_de(&dir, &|p| {
            matches!(p.extension().and_then(|x| x.to_str()), Some("tsx" | "ts"))
        }) {
            p.ruta = format!("{sub}/{}", p.ruta);
            out.push(p);
        }
    }
    out.sort_by(|a, b| a.ruta.cmp(&b.ruta));
    out
}

/// Inventaría el proyecto. Se le pasa la raíz del repo; lo que no exista se anota en `faltantes`.
pub fn inventariar(repo: &Path) -> Inventario {
    let mut inv = Inventario::default();
    let rs = repo.join("src-tauri").join("src");
    inv.rust = piezas_de(&rs, &|p| {
        p.extension().and_then(|x| x.to_str()) == Some("rs")
    });
    if inv.rust.is_empty() {
        inv.faltantes
            .push(format!("no encontré módulos Rust en {}", rs.display()));
    }
    let fe = repo.join("src");
    inv.front = piezas_front(&fe);
    if inv.front.is_empty() {
        inv.faltantes
            .push(format!("no encontré componentes en {}", fe.display()));
    }
    if let Ok(server) = std::fs::read_to_string(rs.join("server.rs")) {
        inv.rutas = rutas_de(&server);
    } else {
        inv.faltantes
            .push("no pude leer src-tauri/src/server.rs (rutas HTTP)".to_string());
    }
    if let Ok(mcp) = std::fs::read_to_string(repo.join("mcp-server").join("nodeflow_mcp.py")) {
        inv.tools_mcp = tools_de(&mcp);
    } else {
        inv.faltantes
            .push("no pude leer mcp-server/nodeflow_mcp.py (tools MCP)".to_string());
    }
    inv
}

/// El bloque **mapa**: lo compacto que el turno antepone. Va acotado a propósito — se paga en cada turno.
pub fn mapa(inv: &Inventario, sello: &str, boveda: &str) -> String {
    let mut s = String::new();
    let areas = inv.por_area();
    let _ = writeln!(
        s,
        "- **Qué es**: app de escritorio Tauri v2 (Rust) + React/TS (Vite). API local en `127.0.0.1:37371`; el lienzo lo sirve el propio binario.",
    );
    let _ = writeln!(
        s,
        "- **Rust** (`src-tauri/src`): {} módulos · {} líneas. Los más grandes: {}.",
        inv.rust.len(),
        inv.lineas_rust(),
        mas_grandes(&inv.rust, 4)
    );
    let _ = writeln!(
        s,
        "- **Frontend** (`src`): {} archivos · {} líneas (componentes y servicios).",
        inv.front.len(),
        inv.lineas_front()
    );
    let _ = writeln!(
        s,
        "- **API HTTP**: {} rutas. {}",
        inv.rutas.len(),
        resumen_areas(&areas)
    );
    let _ = writeln!(
        s,
        "- **MCP propio** (`mcp-server/nodeflow_mcp.py`): {} tools.",
        inv.tools_mcp.len()
    );
    let _ = writeln!(
        s,
        "- **Bóveda**: `{}` — el estado del lienzo vive en `.nodeflow/state.json` (la app lo coalesce); la config, en `%APPDATA%/com.nodeflow.desktop/nodeflow.config.json`.",
        boveda
    );
    let _ = writeln!(
        s,
        "- **Inventariado**: {sello} (se regenera con `POST /api/cerebro/arquitectura/generar`)."
    );
    if !inv.faltantes.is_empty() {
        let _ = writeln!(s, "- **No pude ver**: {}.", inv.faltantes.join(" · "));
    }
    s
}

fn mas_grandes(piezas: &[Pieza], n: usize) -> String {
    let mut v: Vec<&Pieza> = piezas.iter().collect();
    v.sort_by(|a, b| b.lineas.cmp(&a.lineas));
    v.iter()
        .take(n)
        .map(|p| format!("`{}` ({} l)", p.ruta, p.lineas))
        .collect::<Vec<_>>()
        .join(", ")
}

fn resumen_areas(areas: &BTreeMap<String, Vec<(String, String)>>) -> String {
    let mut v: Vec<(&String, usize)> = areas.iter().map(|(k, r)| (k, r.len())).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v.iter()
        .take(6)
        .map(|(k, n)| format!("`{k}` ({n})"))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// El documento completo: el mapa + el detalle (módulos con su rol, rutas por área, herramientas).
pub fn redactar(inv: &Inventario, sello: &str, boveda: &str, repo: &str, version: &str) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "---");
    let _ = writeln!(s, "titulo: \"Arquitectura de NodeFlow\"");
    let _ = writeln!(s, "tipo: arquitectura");
    let _ = writeln!(s, "generado: \"{sello}\"");
    let _ = writeln!(s, "generado_por: cerebro");
    let _ = writeln!(s, "repo: \"{repo}\"");
    let _ = writeln!(s, "version: \"{version}\"");
    let _ = writeln!(s, "---");
    let _ = writeln!(s);
    let _ = writeln!(s, "# Arquitectura de NodeFlow");
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "Inventario del árbol real ({} · versión {}). Lo genera la app: si un módulo aparece o desaparece, acá se ve. No se edita a mano.",
        repo, version
    );
    let _ = writeln!(s);
    let _ = writeln!(s, "{MAPA_INICIO}");
    let _ = writeln!(s, "## Mapa");
    let _ = write!(s, "{}", mapa(inv, sello, boveda));
    let _ = writeln!(s, "{MAPA_FIN}");
    let _ = writeln!(s);

    let _ = writeln!(s, "## Módulos de Rust (rol y tamaño)");
    let _ = writeln!(s);
    let _ = writeln!(s, "| módulo | líneas | rol |");
    let _ = writeln!(s, "|---|---:|---|");
    for p in &inv.rust {
        let rol = if p.rol.is_empty() {
            "—".to_string()
        } else {
            p.rol.replace('|', "\\|")
        };
        let _ = writeln!(s, "| `{}` | {} | {} |", p.ruta, p.lineas, rol);
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "## Frontend (componentes y servicios)");
    let _ = writeln!(s);
    for p in &inv.front {
        let _ = writeln!(s, "- `{}` — {} líneas", p.ruta, p.lineas);
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "## API HTTP (por área)");
    let _ = writeln!(s);
    for (area, rutas) in inv.por_area() {
        let _ = writeln!(s, "### {area} ({})", rutas.len());
        let _ = writeln!(s);
        for (m, r) in rutas {
            let _ = writeln!(s, "- `{m} {r}`");
        }
        let _ = writeln!(s);
    }
    let _ = writeln!(s, "## Herramientas MCP ({})", inv.tools_mcp.len());
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "{}",
        inv.tools_mcp
            .iter()
            .map(|t| format!("`{t}`"))
            .collect::<Vec<_>>()
            .join(" · ")
    );
    let _ = writeln!(s);
    let _ = writeln!(s, "## La bóveda (dónde vive qué)");
    let _ = writeln!(s);
    let _ = writeln!(s, "- `.nodeflow/state.json` — **la verdad del lienzo** (nodos, aristas, revisión). La app coalesce las escrituras: una de cada dos se pierde si no se reintenta.");
    let _ = writeln!(s, "- `cerebro/*-turno.md` — una nota por turno del cerebro (qué pidió el humano, qué contexto se le dio).");
    let _ = writeln!(
        s,
        "- `cerebro/bitacora.md` y `cerebro/planes/` — mi criterio acumulado (Fase 5.2)."
    );
    let _ = writeln!(
        s,
        "- `cerebro/herramientas/<nombre>/` — mis herramientas (`tool.json` + `run.py`, Fase 5.3)."
    );
    let _ = writeln!(s, "- `cerebro/arquitectura.md` — **este** documento.");
    let _ = writeln!(s);
    let _ = writeln!(s, "## Invariantes (no negociables)");
    let _ = writeln!(s);
    let _ = writeln!(s, "- Nada toca el lienzo ni crea herramientas **sin aprobación humana**: todo eso nace por la cola de propuestas.");
    let _ = writeln!(
        s,
        "- La lógica es `src-tauri/src/**`; el front no decide estado, lo muestra."
    );
    let _ = writeln!(
        s,
        "- El estado canónico es la bóveda, no `%APPDATA%` (ahí sólo la config)."
    );
    let _ = writeln!(s, "- Se compila en release con la UI embebida (`npx tauri build`): la app **no** depende de Vite.");
    let _ = writeln!(
        s,
        "- Cada cambio de código cierra con `cargo test --lib` (verde), `tsc` limpio y commit."
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_rol_sale_del_doc_comment_del_modulo() {
        let t = "//! Fase 5.4 — el **contexto de la app**.\n//!\n//! Otra línea.\n\nuse std::path::Path;\n";
        assert!(rol_de(t).starts_with("Fase 5.4"));
        assert!(
            rol_de("use std::path::Path;\n").is_empty(),
            "sin doc-comment no inventa un rol"
        );
    }

    #[test]
    fn las_rutas_se_escanean_con_su_metodo() {
        let t = r#"
            .route("/api/health", get(health))
            .route("/api/cerebro/espacio/nota", post(cerebro_espacio_nota))
            .route("/api/vault/note", get(vault_note).post(vault_note_w))
        "#;
        let r = rutas_de(t);
        assert!(r.contains(&("GET".to_string(), "/api/health".to_string())));
        assert!(r.contains(&("POST".to_string(), "/api/cerebro/espacio/nota".to_string())));
        let nota: Vec<&(String, String)> =
            r.iter().filter(|(_, p)| p == "/api/vault/note").collect();
        assert_eq!(
            nota.len(),
            2,
            "una ruta con dos métodos cuenta dos veces: {r:?}"
        );
    }

    #[test]
    fn las_tools_mcp_salen_de_la_lista_y_no_del_serverinfo() {
        let t = "TOOLS = [\n    {\n        \"name\": \"canvas_summary\",\n        \"x\": 1,\n    },\n    {\n        \"name\": \"mi_espacio\",\n    },\n]\n\ndef manojo():\n    return {\"serverInfo\": {\"name\": \"nodeflow\"}}\n";
        let tools = tools_de(t);
        assert_eq!(
            tools,
            vec!["canvas_summary".to_string(), "mi_espacio".to_string()],
            "{tools:?}"
        );
        assert!(
            !tools.iter().any(|t| t == "nodeflow"),
            "serverInfo no es una tool"
        );
        assert!(!tools.iter().any(|t| t == "name"), "el valor, no la clave");
        assert!(tools_de("sin lista").is_empty());
    }

    #[test]
    fn el_mapa_se_agrupa_por_area_y_no_se_pasa_de_largo() {
        let inv = Inventario {
            rust: vec![Pieza {
                ruta: "server.rs".into(),
                lineas: 4500,
                rol: "API".into(),
            }],
            front: vec![Pieza {
                ruta: "App.tsx".into(),
                lineas: 900,
                rol: String::new(),
            }],
            rutas: vec![
                ("GET".into(), "/api/graph/state".into()),
                ("POST".into(), "/api/graph/node".into()),
                ("GET".into(), "/api/cerebro/espacio".into()),
            ],
            tools_mcp: vec!["canvas_summary".into(), "mi_espacio".into()],
            faltantes: vec![],
        };
        let m = mapa(&inv, "2026-09-16T11:40", "C:\\boveda");
        assert!(m.contains("`/api/graph` (2)"), "agrupa por área: {m}");
        assert!(m.contains("`/api/cerebro` (1)"));
        assert!(m.contains("2 tools"));
        assert!(
            m.chars().count() < 1800,
            "el mapa se paga cada turno: {}",
            m.chars().count()
        );
        let doc = redactar(&inv, "2026-09-16T11:40", "C:\\boveda", "C:\\repo", "0.3.5");
        assert!(doc.contains(MAPA_INICIO) && doc.contains(MAPA_FIN));
        assert!(doc.contains("| `server.rs` | 4500 | API |"));
        assert!(doc.contains("## Invariantes"));
    }

    #[test]
    fn el_repo_se_encuentra_subiendo_por_los_ancestros() {
        let base = std::env::temp_dir().join("nf-arq-buscar");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("src-tauri").join("src")).unwrap();
        std::fs::write(
            base.join("src-tauri").join("src").join("server.rs"),
            "//! x",
        )
        .unwrap();
        let exe = base.join("src-tauri").join("target").join("release");
        std::fs::create_dir_all(&exe).unwrap();
        let encontrado = buscar_repo(&exe.join("app.exe"));
        assert_eq!(
            encontrado.as_deref(),
            Some(base.as_path()),
            "sube desde el exe hasta la raíz"
        );
        assert!(buscar_repo(&std::env::temp_dir().join("nf-arq-buscar-nada")).is_none());
    }

    #[test]
    fn un_proyecto_vacio_no_rompe_y_dice_que_falto() {
        let tmp = std::env::temp_dir().join("nf-arq-vacio");
        let _ = std::fs::create_dir_all(&tmp);
        let inv = inventariar(&tmp);
        assert!(inv.rust.is_empty() && inv.rutas.is_empty());
        assert!(!inv.faltantes.is_empty(), "avisa qué no pudo ver");
        let doc = redactar(&inv, "sello", "boveda", "repo", "0");
        assert!(
            doc.contains("No pude ver") || doc.contains("no encontré"),
            "el doc declara lo que falta"
        );
    }
}

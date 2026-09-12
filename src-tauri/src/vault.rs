//! Fase 3 — El vault en disco como fuente de verdad.
//!
//! Qué escribe (todo dentro de `<vault>`):
//!   `<mapa>.canvas`              → Obsidian Canvas nativo (geometría + texto), regenerado
//!   `<mapa>.md`                  → nota índice con frontmatter + [[wikilinks]], regenerada
//!   `nodos/<slug>.md`            → UNA nota por nodo, con frontmatter (id, madurez, tags).
//!                                  ESTA es la superficie de edición desde Obsidian.
//!   `.nodeflow/state.json`       → estado canónico lossless (round-trip exacto de la app)
//!   `.nodeflow/backup-N.json`    → respaldos rotativos (últimos 5)
//!
//! Dirección Obsidian → app: un watcher sobre el árbol detecta cambios externos en `nodos/*.md`,
//! fusiona frontmatter + cuerpo en el estado canónico y sube `revision` para que la app lo tome.
//! Los archivos generados (`<mapa>.canvas`, `<mapa>.md`, `state.json`) NO se fusionan: si se
//! editaran a mano, se regenerarían en el siguiente guardado y entraríamos en un bucle.

use notify::Watcher;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Ventana en la que se ignoran eventos del sistema de archivos (son nuestros propios writes).
const WRITE_GRACE: Duration = Duration::from_millis(2500);
const WATCH_INTERVAL: Duration = Duration::from_millis(500);
const MAX_BACKUPS: usize = 5;
/// Madurez por defecto si una nota editada a mano no la declara.
const DEFAULT_MATURITY: i64 = 1;

pub struct Vault {
    root: PathBuf,
    inner: Mutex<Inner>,
}

struct Inner {
    revision: u64,
    updated_at: u64,
    last_write: Option<Instant>,
    hashes: HashMap<String, u64>,
    external: Vec<String>,
    map_name: String,
    /// Fase 4: ids (nodos o aristas) que escribió el agente, con la revisión en que lo hizo.
    /// Protegen esa escritura hasta que el lienzo la incorpore y la devuelva en un autoguardado.
    agent_writes: HashMap<String, u64>,
    /// Fase 5a: escrituras del agente esperando aprobación humana (persisten en disco).
    pendientes: Vec<Value>,
    pending_rev: u64,
    /// Fase 7b: métrica de valor — sesiones de trabajo y su T0→T1.
    metricas: Value,
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn hash_bytes(b: &[u8]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    b.hash(&mut h);
    h.finish()
}

/// Nombre de archivo seguro: sin separadores ni caracteres prohibidos, en minúsculas.
pub(crate) fn slug(raw: &str) -> String {
    let mut out = String::new();
    for c in raw.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if matches!(c, ' ' | '-' | '_' | '/' | '.') {
            out.push('-');
        }
    }
    let mut collapsed = String::new();
    let mut prev_dash = false;
    for c in out.chars() {
        if c == '-' {
            if !prev_dash {
                collapsed.push(c);
            }
            prev_dash = true;
        } else {
            collapsed.push(c);
            prev_dash = false;
        }
    }
    let trimmed = collapsed.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "nodo".into()
    } else {
        trimmed.chars().take(60).collect()
    }
}

/// Escapa un string para meterlo entre comillas dobles en YAML/markdown.
fn yaml_escape(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " ")
}

/// Resuelve la raíz del vault: config del usuario → bóveda de Obsidian → Documents\nodeflow.
fn resolve_root(data_dir: &Path) -> PathBuf {
    let cfg_path = data_dir.join("nodeflow.config.json");
    if let Ok(txt) = std::fs::read_to_string(&cfg_path) {
        if let Ok(v) = serde_json::from_str::<Value>(&txt) {
            if let Some(p) = v["vault_path"].as_str() {
                if !p.trim().is_empty() {
                    return PathBuf::from(p);
                }
            }
        }
    }
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
    let obsidian = PathBuf::from(&home).join("Documents").join("Obsidian Vault");
    let root = if obsidian.is_dir() {
        obsidian.join("NodeFlow")
    } else {
        PathBuf::from(&home).join("Documents").join("NodeFlow")
    };
    let cfg = json!({
        "vault_path": root.to_string_lossy(),
        "map_name": "nodeflow",
        "_comentario": "vault_path = dónde vive el grafo en disco. Si apuntás dentro de tu bóveda de Obsidian, las notas aparecen ahí al instante."
    });
    let _ = std::fs::write(
        &cfg_path,
        serde_json::to_string_pretty(&cfg).unwrap_or_default(),
    );
    log::info!("vault: config creada en {} → {}", cfg_path.display(), root.display());
    root
}

impl Vault {
    pub fn new(data_dir: &Path) -> Arc<Self> {
        let root = resolve_root(data_dir);
        for sub in ["nodos", ".nodeflow"] {
            let _ = std::fs::create_dir_all(root.join(sub));
        }
        let map_name = std::fs::read_to_string(data_dir.join("nodeflow.config.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v["map_name"].as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "nodeflow".into());
        log::info!("vault: raíz = {}", root.display());
        // Fase 5a: propuestas que quedaron sin resolver antes del cierre anterior.
        let pendientes: Vec<Value> =
            std::fs::read_to_string(root.join(".nodeflow").join("pending.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|v| v["pendientes"].as_array().cloned())
                .unwrap_or_default();
        if !pendientes.is_empty() {
            log::info!(
                "vault: {} propuesta(s) del agente esperando aprobación",
                pendientes.len()
            );
        }
        // Fase 7b: el cronómetro de conversión sobrevive reinicios (se lee antes de mover `root`).
        let metricas = std::fs::read_to_string(root.join(".nodeflow").join("metricas.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .unwrap_or_else(|| json!({"version": 1, "sesiones": [], "abierta": Value::Null}));
        Arc::new(Self {
            root,
            inner: Mutex::new(Inner {
                revision: 0,
                updated_at: 0,
                last_write: None,
                hashes: HashMap::new(),
                external: Vec::new(),
                map_name,
                agent_writes: HashMap::new(),
                pendientes,
                pending_rev: 0,
                metricas,
            }),
        })
    }

    fn map_name(&self) -> String {
        self.inner.lock().unwrap().map_name.clone()
    }

    /// Escritura atómica (tmp + rename) para que Obsidian nunca lea un archivo a medio escribir.
    fn write_atomic(&self, rel: &str, content: &str) -> Result<usize, String> {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("tmp-nf");
        std::fs::write(&tmp, content.as_bytes()).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        let bytes = content.len();
        self.inner
            .lock()
            .unwrap()
            .hashes
            .insert(rel.replace('\\', "/"), hash_bytes(content.as_bytes()));
        Ok(bytes)
    }

    // ── serializadores (mismo formato que el export de la app) ────────────────

    fn canvas_json(&self, name: &str, nodes: &[Value], edges: &[Value]) -> String {
        let cn: Vec<Value> = nodes
            .iter()
            .map(|n| {
                let d = &n["data"];
                let title = d["title"].as_str().or_else(|| d["label"].as_str()).unwrap_or("Concepto");
                let is_root = d["isRoot"].as_bool().unwrap_or(false);
                let category = d["category"].as_str().unwrap_or(if is_root { "Núcleo Central" } else { "Concepto" });
                let desc = d["description"].as_str().unwrap_or("");
                let tags: Vec<String> = d["tags"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|t| t.as_str())
                            .map(|t| if t.starts_with('#') { t.to_string() } else { format!("#{t}") })
                            .collect()
                    })
                    .unwrap_or_default();
                let tags_txt = if tags.is_empty() { String::new() } else { format!("\n\n{}", tags.join(" ")) };
                let text = format!("### [[{title}]]\n*{category}*\n\n{desc}{tags_txt}");
                let height = (120 + desc.len() / 2).clamp(160, 320) as i64;
                let color = d["colorAccent"]
                    .as_str()
                    .unwrap_or(if is_root { "#4f46e5" } else { "#059669" })
                    .to_string();
                json!({
                    "id": n["id"],
                    "type": "text",
                    "text": text.trim(),
                    "x": n["position"]["x"].as_f64().unwrap_or(0.0).round() as i64,
                    "y": n["position"]["y"].as_f64().unwrap_or(0.0).round() as i64,
                    "width": 280,
                    "height": height,
                    "color": color,
                })
            })
            .collect();

        let ce: Vec<Value> = edges
            .iter()
            .filter(|e| {
                let s = e["source"].as_str().unwrap_or("");
                let t = e["target"].as_str().unwrap_or("");
                nodes.iter().any(|n| n["id"].as_str() == Some(s))
                    && nodes.iter().any(|n| n["id"].as_str() == Some(t))
            })
            .map(|e| {
                let side = |h: Option<&str>, default: &str| -> String {
                    match h {
                        Some(h) => {
                            for s in ["left", "top", "bottom", "right"] {
                                if h.contains(s) {
                                    return s.into();
                                }
                            }
                            default.into()
                        }
                        None => default.into(),
                    }
                };
                let mut obj = json!({
                    "id": e["id"],
                    "fromNode": e["source"],
                    "fromSide": side(e["sourceHandle"].as_str(), "right"),
                    "toNode": e["target"],
                    "toSide": side(e["targetHandle"].as_str(), "left"),
                });
                if let Some(l) = e["label"].as_str() {
                    if !l.is_empty() {
                        obj["label"] = json!(l);
                    }
                }
                if let Some(s) = e["style"]["stroke"].as_str() {
                    obj["color"] = json!(s);
                }
                obj
            })
            .collect();

        serde_json::to_string_pretty(&json!({
            "_nodeflow": {
                "generado": true,
                "mapa": name,
                "aviso": "Archivo generado por NodeFlow. Se regenera en cada guardado; para editar, usá las notas de nodos/."
            },
            "nodes": cn,
            "edges": ce
        }))
        .unwrap_or_default()
    }

    fn map_markdown(&self, name: &str, nodes: &[Value], edges: &[Value]) -> String {
        let mut md = String::from("---\n");
        md += &format!("titulo: \"{}\"\n", yaml_escape(name));
        md += &format!("actualizado_ms: {}\n", epoch_ms());
        md += "tags:\n  - nodeflow\n  - mapa-conceptual\n";
        md += &format!("nodos_totales: {}\n", nodes.len());
        md += &format!("conexiones_totales: {}\n", edges.len());
        md += "generador: NodeFlow\n";
        md += "generado: true\n";
        md += "---\n\n";
        md += &format!("# {name}\n\n");
        md += "> [!warning] Archivo generado\n";
        md += "> Este índice y `<mapa>.canvas` los regenera NodeFlow en cada guardado. Para editar\n";
        md += "> contenido que la app respete, modificá las notas de la carpeta `nodos/`.\n\n";
        md += "## Nodos del grafo\n\n";
        for n in nodes {
            let d = &n["data"];
            let title = d["title"].as_str().or_else(|| d["label"].as_str()).unwrap_or("Concepto");
            let is_root = d["isRoot"].as_bool().unwrap_or(false);
            let category = d["category"].as_str().unwrap_or(if is_root { "Núcleo Central" } else { "Concepto" });
            md += &format!("### [[{title}]]\n");
            md += &format!("* **Tipo/Categoría:** {category}\n");
            if let Some(m) = d["maturity"].as_i64() {
                md += &format!("* **Madurez:** {m}/5\n");
            }
            if let Some(tags) = d["tags"].as_array() {
                let t: Vec<String> = tags
                    .iter()
                    .filter_map(|x| x.as_str())
                    .map(|x| if x.starts_with('#') { x.into() } else { format!("#{x}") })
                    .collect();
                if !t.is_empty() {
                    md += &format!("* **Tags:** {}\n", t.join(" "));
                }
            }
            md += "\n";
            md += d["description"].as_str().unwrap_or("Sin descripción.");
            md += "\n\n";
            let id = n["id"].as_str().unwrap_or("");
            let out: Vec<&Value> = edges.iter().filter(|e| e["source"].as_str() == Some(id)).collect();
            if !out.is_empty() {
                md += "**Conexiones:**\n";
                for e in out {
                    let tid = e["target"].as_str().unwrap_or("");
                    if let Some(t) = nodes.iter().find(|x| x["id"].as_str() == Some(tid)) {
                        let tt = t["data"]["title"]
                            .as_str()
                            .or_else(|| t["data"]["label"].as_str())
                            .unwrap_or("Concepto");
                        let rel = match e["label"].as_str() {
                            Some(l) if !l.is_empty() => format!("_({l})_"),
                            _ => "→".into(),
                        };
                        md += &format!("- {rel} [[{tt}]]\n");
                    }
                }
                md += "\n";
            }
        }
        md
    }

    /// Nota individual de un nodo: la superficie que se puede editar desde Obsidian.
    fn node_markdown(&self, node: &Value, nodes: &[Value], edges: &[Value], map: &str) -> String {
        let d = &node["data"];
        let id = node["id"].as_str().unwrap_or("");
        let title = d["title"].as_str().or_else(|| d["label"].as_str()).unwrap_or("Concepto");
        let is_root = d["isRoot"].as_bool().unwrap_or(false);
        let category = d["category"].as_str().unwrap_or(if is_root { "Núcleo Central" } else { "Concepto" });
        let mut tags: Vec<String> = d["tags"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.trim_start_matches('#').to_string()).collect())
            .unwrap_or_default();
        if let Some(o) = d["aiOrigin"].as_str() {
            tags.push(format!("ai:{o}"));
        }
        if is_root {
            tags.push("nucleo".into());
        }
        let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".into());

        let mut md = String::from("---\n");
        md += &format!("id: \"{}\"\n", yaml_escape(id));
        md += &format!("title: \"{}\"\n", yaml_escape(title));
        md += &format!("categoria: \"{}\"\n", yaml_escape(category));
        md += &format!("madurez: {}\n", d["maturity"].as_i64().unwrap_or(DEFAULT_MATURITY));
        md += &format!("tags: {tags_json}\n");
        md += &format!("mapa: \"{}\"\n", yaml_escape(map));
        md += &format!("es_nucleo: {}\n", is_root);
        md += &format!("actualizado_ms: {}\n", epoch_ms());
        md += "generado_por: NodeFlow\n";
        md += "editable: true\n";
        md += "---\n\n";
        md += &format!("# {title}\n\n");
        md += d["description"].as_str().unwrap_or("Sin descripción.");
        md += "\n\n## Conexiones\n\n";
        let mut any = false;
        for e in edges.iter().filter(|e| e["source"].as_str() == Some(id)) {
            if let Some(t) = nodes
                .iter()
                .find(|x| x["id"].as_str() == e["target"].as_str())
            {
                let tt = t["data"]["title"]
                    .as_str()
                    .or_else(|| t["data"]["label"].as_str())
                    .unwrap_or("Concepto");
                let rel = match e["label"].as_str() {
                    Some(l) if !l.is_empty() => format!("_({l})_"),
                    _ => "→".into(),
                };
                md += &format!("- {rel} [[{tt}]]\n");
                any = true;
            }
        }
        for e in edges.iter().filter(|e| e["target"].as_str() == Some(id)) {
            if let Some(s) = nodes
                .iter()
                .find(|x| x["id"].as_str() == e["source"].as_str())
            {
                let st = s["data"]["title"]
                    .as_str()
                    .or_else(|| s["data"]["label"].as_str())
                    .unwrap_or("Concepto");
                md += &format!("- ← [[{st}]]\n");
                any = true;
            }
        }
        if !any {
            md += "_Sin conexiones todavía._\n";
        }
        md += "\n> [!tip] Editable desde Obsidian\n";
        md += "> Cambiá el título, la `madurez` o el texto de esta nota y NodeFlow lo refleja solo.\n";
        md
    }

    // ── API principal ────────────────────────────────────────────────────────

    /// Guarda el estado completo: canónico + artefactos de Obsidian. Devuelve el resumen.
    /// Respaldo rotativo del canónico anterior (últimos MAX_BACKUPS).
    fn backup_canonico(&self) {
        let state_path = self.root.join(".nodeflow").join("state.json");
        if !state_path.is_file() {
            return;
        }
        let dir = self.root.join(".nodeflow");
        let mut backups: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name()
                            .and_then(|s| s.to_str())
                            .map(|s| s.starts_with("backup-"))
                            .unwrap_or(false)
                    })
                    .collect()
            })
            .unwrap_or_default();
        backups.sort();
        if backups.len() >= MAX_BACKUPS {
            for old in backups.iter().take(backups.len() - MAX_BACKUPS + 1) {
                let _ = std::fs::remove_file(old);
            }
        }
        let _ = std::fs::copy(&state_path, dir.join(format!("backup-{}.json", epoch_ms())));
    }

    /// Escribe el canónico + todos los artefactos de Obsidian y sube la revisión.
    fn write_all(
        &self,
        nodes: &[Value],
        edges: &[Value],
        appearance: &Value,
        template_id: &Value,
        map: &str,
    ) -> Result<Value, String> {
        // Reparación silenciosa en el borde de escritura: si algo llegó roto (una arista huérfana,
        // un id repetido), se descarta ESA pieza y se escribe el resto. Nunca se pierde el trabajo
        // del humano por un invariante; las mutaciones del AGENTE sí se rechazan antes (ver add_edge).
        let mut ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        let nodes_ok: Vec<Value> = nodes
            .iter()
            .filter(|n| {
                let id = crate::grafo::id_de(n);
                !id.is_empty() && ids.insert(id)
            })
            .cloned()
            .collect();
        let antes_aristas = edges.len();
        let edges_ok: Vec<Value> = edges
            .iter()
            .filter(|e| {
                let s = e["source"].as_str().unwrap_or("");
                let t = e["target"].as_str().unwrap_or("");
                !s.is_empty() && !t.is_empty() && ids.contains(s) && ids.contains(t)
            })
            .cloned()
            .collect();
        let reparadas = (nodes.len() - nodes_ok.len()) + (antes_aristas - edges_ok.len());
        if reparadas > 0 {
            log::warn!(
                "integridad: descarté {reparadas} pieza(s) rota(s) al escribir ({} nodos huérfanos de id, {} aristas colgadas)",
                nodes.len() - nodes_ok.len(),
                antes_aristas - edges_ok.len()
            );
        }
        let nodes: &[Value] = &nodes_ok;
        let edges: &[Value] = &edges_ok;
        let mut files: Vec<Value> = Vec::new();
        let mut written_slugs: HashSet<String> = HashSet::new();
        {
            let mut inner = self.inner.lock().unwrap();
            inner.last_write = Some(Instant::now());
            inner.map_name = map.to_string();
        }

        let canonical = json!({
            "version": 1,
            "name": map,
            "updated_at": epoch_ms(),
            "templateId": template_id,
            "appearance": appearance,
            "nodes": nodes,
            "edges": edges,
        });
        let state_txt = serde_json::to_string_pretty(&canonical).map_err(|e| e.to_string())?;
        let bytes_state = self.write_atomic(".nodeflow/state.json", &state_txt)?;
        files.push(json!({"ruta": ".nodeflow/state.json", "bytes": bytes_state, "tipo": "canónico"}));

        for n in nodes {
            let d = &n["data"];
            let title = d["title"]
                .as_str()
                .or_else(|| d["label"].as_str())
                .unwrap_or("Concepto");
            let mut s = slug(title);
            if written_slugs.contains(&s) {
                let id = n["id"].as_str().unwrap_or("");
                let tail: String = id
                    .chars()
                    .rev()
                    .take(6)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                s = format!("{s}-{}", slug(&tail));
            }
            written_slugs.insert(s.clone());
            let rel = format!("nodos/{s}.md");
            let md = self.node_markdown(n, nodes, edges, map);
            let bytes = self.write_atomic(&rel, &md)?;
            files.push(json!({"ruta": rel, "bytes": bytes, "tipo": "nota editable"}));
        }

        let canvas = self.canvas_json(map, nodes, edges);
        let b1 = self.write_atomic(&format!("{map}.canvas"), &canvas)?;
        files.push(
            json!({"ruta": format!("{map}.canvas"), "bytes": b1, "tipo": "Obsidian Canvas (generado)"}),
        );
        let idx = self.map_markdown(map, nodes, edges);
        let b2 = self.write_atomic(&format!("{map}.md"), &idx)?;
        files.push(json!({"ruta": format!("{map}.md"), "bytes": b2, "tipo": "índice (generado)"}));

        let mut huerfanos = 0;
        if let Ok(rd) = std::fs::read_dir(self.root.join("nodos")) {
            for e in rd.filter_map(|e| e.ok()) {
                let name = e.file_name().to_string_lossy().to_string();
                if !name.ends_with(".md") {
                    continue;
                }
                let stem = name.trim_end_matches(".md").to_string();
                if !written_slugs.contains(&stem) {
                    let _ = std::fs::remove_file(e.path());
                    self.inner
                        .lock()
                        .unwrap()
                        .hashes
                        .remove(&format!("nodos/{name}"));
                    huerfanos += 1;
                }
            }
        }

        let (revision, updated_at) = {
            let mut inner = self.inner.lock().unwrap();
            inner.revision += 1;
            inner.updated_at = epoch_ms();
            inner.external.clear();
            inner.last_write = Some(Instant::now());
            (inner.revision, inner.updated_at)
        };

        Ok(json!({
            "ok": true,
            "revision": revision,
            "updated_at": updated_at,
            "mapa": map,
            "vault": self.root.to_string_lossy(),
            "nodos": nodes.len(),
            "aristas": edges.len(),
            "notas_borradas": huerfanos,
            "piezas_descartadas": reparadas,
            "archivos": files,
        }))
    }

    /// Guarda el estado que manda el lienzo. Antes reconcilia: `base_revision` es la revisión que el
    /// lienzo conocía, así que si el agente escribió en el medio esa escritura no se pierde.
    pub fn save(&self, payload: &Value) -> Result<Value, String> {
        let mut nodes = payload["nodes"].as_array().cloned().unwrap_or_default();
        let mut edges = payload["edges"].as_array().cloned().unwrap_or_default();
        if nodes.is_empty() {
            return Err("el estado no tiene nodos".into());
        }
        let map = payload["name"]
            .as_str()
            .map(|s| slug(s))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.map_name());
        let map = slug(&map);

        if let Some(base) = payload["base_revision"].as_u64() {
            let rev = self.inner.lock().unwrap().revision;
            if base < rev {
                let (n2, e2) = self.reconcile(&nodes, &edges, base);
                if n2.len() > nodes.len() || e2.len() > edges.len() {
                    log::info!(
                        "vault: reconciliado (lienzo en rev {base}, disco en rev {rev}) → +{} nodos, +{} aristas rescatados",
                        n2.len() - nodes.len(),
                        e2.len() - edges.len()
                    );
                }
                nodes = n2;
                edges = e2;
            }
        }
        // El lienzo ya devolvió estos ids: dejan de estar protegidos.
        {
            let mut conocidos: HashSet<String> = HashSet::new();
            conocidos.extend(nodes.iter().filter_map(|n| n["id"].as_str().map(String::from)));
            conocidos.extend(edges.iter().filter_map(|e| e["id"].as_str().map(String::from)));
            self.inner
                .lock()
                .unwrap()
                .agent_writes
                .retain(|k, _| !conocidos.contains(k));
        }

        // Fase 7b: la escritura humana abre/refresca la sesión de trabajo (T0 = la primera).
        self.marcar_actividad("lienzo");
        self.backup_canonico();
        let appearance = payload
            .get("appearance")
            .or_else(|| payload.get("edgeAppearance"))
            .cloned()
            .unwrap_or(Value::Null);
        let template_id = payload.get("templateId").cloned().unwrap_or(Value::Null);
        self.write_all(&nodes, &edges, &appearance, &template_id, &map)
    }

    /// Une el estado del lienzo con el canónico: rescata lo que el agente escribió después de la
    /// revisión que el lienzo conoce, para que un autoguardado no lo borre.
    fn reconcile(
        &self,
        incoming_nodes: &[Value],
        incoming_edges: &[Value],
        base: u64,
    ) -> (Vec<Value>, Vec<Value>) {
        let agent_writes = self.inner.lock().unwrap().agent_writes.clone();
        let Some(canon) = self.read_state() else {
            return (incoming_nodes.to_vec(), incoming_edges.to_vec());
        };
        let node_ids: HashSet<String> = incoming_nodes
            .iter()
            .filter_map(|n| n["id"].as_str().map(String::from))
            .collect();
        let mut nodos = incoming_nodes.to_vec();
        for n in canon["nodes"].as_array().cloned().unwrap_or_default() {
            let id = n["id"].as_str().unwrap_or("").to_string();
            if node_ids.contains(&id) {
                continue;
            }
            if agent_writes.get(&id).map(|r| *r > base).unwrap_or(false) {
                nodos.push(n);
            }
        }
        let vivos: HashSet<String> = nodos
            .iter()
            .filter_map(|n| n["id"].as_str().map(String::from))
            .collect();
        let edge_ids: HashSet<String> = incoming_edges
            .iter()
            .filter_map(|e| e["id"].as_str().map(String::from))
            .collect();
        let mut aristas = incoming_edges.to_vec();
        for e in canon["edges"].as_array().cloned().unwrap_or_default() {
            let id = e["id"].as_str().unwrap_or("").to_string();
            if edge_ids.contains(&id) {
                continue;
            }
            let s = e["source"].as_str().unwrap_or("").to_string();
            let t = e["target"].as_str().unwrap_or("").to_string();
            let nueva = agent_writes.get(&id).map(|r| *r > base).unwrap_or(false);
            if nueva && vivos.contains(&s) && vivos.contains(&t) {
                aristas.push(e);
            }
        }
        (nodos, aristas)
    }

    // ── Fase 4: el agente lee y escribe el lienzo ────────────────────────────

    /// Resuelve un nodo por id exacto, id aproximado, título exacto o título contenido.
    fn resolve(&self, nodes: &[Value], needle: &str) -> Option<String> {
        let n = needle.trim();
        if n.is_empty() {
            return None;
        }
        let low = n.to_lowercase();
        let tid = |x: &Value| x["id"].as_str().map(String::from);
        let titulo = |x: &Value| -> String {
            x["data"]["title"]
                .as_str()
                .or_else(|| x["data"]["label"].as_str())
                .unwrap_or("")
                .to_string()
        };
        if let Some(h) = nodes.iter().find(|x| x["id"].as_str() == Some(n)) {
            return tid(h);
        }
        if let Some(h) = nodes
            .iter()
            .find(|x| x["id"].as_str().map(|s| s.to_lowercase()) == Some(low.clone()))
        {
            return tid(h);
        }
        if let Some(h) = nodes.iter().find(|x| titulo(x).to_lowercase() == low) {
            return tid(h);
        }
        if let Some(h) = nodes.iter().find(|x| titulo(x).to_lowercase().contains(&low)) {
            return tid(h);
        }
        None
    }

    fn nuevo_id(&self, nodes: &[Value], title: &str) -> String {
        let base = format!("n-ag-{}", slug(title));
        if !nodes.iter().any(|n| n["id"].as_str() == Some(base.as_str())) {
            return base;
        }
        let mut i = 2;
        loop {
            let cand = format!("{base}-{i}");
            if !nodes.iter().any(|n| n["id"].as_str() == Some(cand.as_str())) {
                return cand;
            }
            i += 1;
        }
    }

    /// Base para las operaciones del agente: estado canónico actual.
    fn estado_base(&self) -> Result<(Value, Vec<Value>, Vec<Value>, String), String> {
        let state = self
            .read_state()
            .ok_or("todavía no hay estado en disco — abrí NodeFlow y esperá el primer autoguardado")?;
        let nodes = state["nodes"].as_array().cloned().unwrap_or_default();
        let edges = state["edges"].as_array().cloned().unwrap_or_default();
        let map = state["name"].as_str().unwrap_or("nodeflow").to_string();
        Ok((state, nodes, edges, map))
    }

    /// Crea un nodo (o actualiza el existente, si el título o el id coinciden) y, si hay `parent`,
    /// lo conecta. Devuelve el id para que el agente pueda seguir encadenando.
    pub fn upsert_node(&self, req: &Value) -> Result<Value, String> {
        let title = req["title"]
            .as_str()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or("falta `title` (título del nodo)")?;
        let (state, mut nodes, mut edges, map) = self.estado_base()?;
        let appearance = state.get("appearance").cloned().unwrap_or(Value::Null);
        let template_id = state.get("templateId").cloned().unwrap_or(Value::Null);

        let existente = req["id"]
            .as_str()
            .and_then(|i| self.resolve(&nodes, i))
            .or_else(|| self.resolve(&nodes, &title));

        if let Some(id) = existente {
            let mut cambios: Vec<String> = Vec::new();
            for n in nodes.iter_mut() {
                if n["id"].as_str() != Some(id.as_str()) {
                    continue;
                }
                if n["data"]["title"].as_str() != Some(title.as_str()) {
                    n["data"]["title"] = json!(title);
                    if n["data"]["label"].is_string() {
                        n["data"]["label"] = json!(title);
                    }
                    cambios.push("título".into());
                }
                if let Some(d) = req["description"].as_str() {
                    if n["data"]["description"].as_str() != Some(d) {
                        n["data"]["description"] = json!(d);
                        cambios.push("descripción".into());
                    }
                }
                if let Some(c) = req["category"].as_str() {
                    n["data"]["category"] = json!(c);
                    cambios.push("categoría".into());
                }
                if let Some(m) = req["maturity"].as_i64() {
                    n["data"]["maturity"] = json!(m);
                    cambios.push("madurez".into());
                }
                if let Some(t) = req["tags"].as_array() {
                    n["data"]["tags"] = json!(t);
                    cambios.push("tags".into());
                }
                if let Some(x) = req["x"].as_f64() {
                    n["position"]["x"] = json!(x);
                    cambios.push("x".into());
                }
                if let Some(y) = req["y"].as_f64() {
                    n["position"]["y"] = json!(y);
                    cambios.push("y".into());
                }
            }
            if cambios.is_empty() {
                return Ok(json!({"ok": true, "accion": "sin_cambios", "id": id}));
            }
            let mut res = self.write_all(&nodes, &edges, &appearance, &template_id, &map)?;
            {
                let rev = res["revision"].as_u64().unwrap_or(0);
                self.inner.lock().unwrap().agent_writes.insert(id.clone(), rev);
            }
            res["accion"] = json!("actualizado");
            res["id"] = json!(id);
            res["campos"] = json!(cambios);
            return Ok(res);
        }

        let id = self.nuevo_id(&nodes, &title);
        let parent_id = req["parent"].as_str().and_then(|p| self.resolve(&nodes, p));
        let mut x = nodes
            .iter()
            .filter_map(|n| n["position"]["x"].as_f64())
            .fold(0.0, f64::max)
            + 340.0;
        let mut y = 120.0;
        if let Some(pid) = &parent_id {
            if let Some(p) = nodes.iter().find(|n| n["id"].as_str() == Some(pid.as_str())) {
                x = p["position"]["x"].as_f64().unwrap_or(0.0) + 340.0;
                y = p["position"]["y"].as_f64().unwrap_or(0.0) + 40.0;
            }
        }
        if let Some(rx) = req["x"].as_f64() {
            x = rx;
        }
        if let Some(ry) = req["y"].as_f64() {
            y = ry;
        }
        // Colocación por niveles (reemplaza el offset ciego): a la derecha del padre y debajo
        // de sus hijos ya existentes, con padding constante. Si aún choca, baja en pasos de nivel.
        if let Some(pid) = &parent_id {
            let hijos_y: Vec<f64> = edges
                .iter()
                .filter(|e| e["source"].as_str() == Some(pid.as_str()))
                .filter_map(|e| {
                    let tid = e["target"].as_str().unwrap_or("");
                    nodes.iter().find(|n| crate::grafo::id_de(n) == tid)
                })
                .map(|n| n["position"]["y"].as_f64().unwrap_or(0.0))
                .collect();
            let base = hijos_y.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            if base.is_finite() {
                y = base + 380.0;
            }
        }
        let mut guard = 0;
        while guard < 40
            && nodes.iter().any(|n| {
                (n["position"]["x"].as_f64().unwrap_or(-9999.0) - x).abs() < 100.0
                    && (n["position"]["y"].as_f64().unwrap_or(-9999.0) - y).abs() < 200.0
            })
        {
            y += 380.0;
            guard += 1;
        }
        let category = req["category"].as_str().unwrap_or("AGENTE").to_string();
        nodes.push(json!({
            "id": id,
            "type": "ideaNode",
            "position": { "x": x, "y": y },
            "width": 290,
            "height": 230,
            "dragging": false,
            "selected": false,
            "data": {
                "id": id,
                "title": title,
                "label": category,
                "category": category,
                "description": req["description"].as_str().unwrap_or(""),
                "tags": req["tags"].clone(),
                "colorAccent": req["colorAccent"].as_str().unwrap_or("#8b5cf6"),
                "isRoot": false,
                "isEditing": false,
                "maturity": req["maturity"].as_i64().unwrap_or(1),
                "aiOrigin": {
                    "actionType": "hermes",
                    "createdAt": epoch_ms(),
                    "promptOriginal": req["prompt_original"].as_str().unwrap_or("(creado por el agente)")
                }
            }
        }));

        let mut edge_id = Value::Null;
        if let Some(pid) = &parent_id {
            let eid = format!("e-ag-{}-{}", slug(pid), slug(&id));
            let label = req["link_label"].as_str().unwrap_or("").to_string();
            edges.push(json!({
                "id": eid,
                "source": pid,
                "target": id,
                "sourceHandle": "right",
                "targetHandle": "left",
                "type": "smoothstep",
                "animated": true,
                "label": if label.is_empty() { Value::Null } else { json!(label) },
                "style": { "stroke": "#8b5cf6", "strokeWidth": 2 }
            }));
            edge_id = json!(eid);
        }

        let mut res = self.write_all(&nodes, &edges, &appearance, &template_id, &map)?;
        {
            let rev = res["revision"].as_u64().unwrap_or(0);
            let mut inner = self.inner.lock().unwrap();
            inner.agent_writes.insert(id.clone(), rev);
            if let Some(e) = edge_id.as_str() {
                inner.agent_writes.insert(e.to_string(), rev);
            }
        }
        res["accion"] = json!("creado");
        res["id"] = json!(id);
        res["padre"] = json!(parent_id);
        res["arista"] = edge_id;
        Ok(res)
    }

    /// Conecta dos nodos (por id o título). `direction: "<-"` invierte el sentido.
    pub fn add_edge(&self, req: &Value) -> Result<Value, String> {
        let (state, nodes, mut edges, map) = self.estado_base()?;
        let appearance = state.get("appearance").cloned().unwrap_or(Value::Null);
        let template_id = state.get("templateId").cloned().unwrap_or(Value::Null);
        let src = req["source"]
            .as_str()
            .ok_or("falta `source` (id o título del nodo origen)")?;
        let dst = req["target"]
            .as_str()
            .ok_or("falta `target` (id o título del nodo destino)")?;
        let s = self
            .resolve(&nodes, src)
            .ok_or_else(|| format!("no encontré el nodo origen: {src}"))?;
        let t = self
            .resolve(&nodes, dst)
            .ok_or_else(|| format!("no encontré el nodo destino: {dst}"))?;
        if s == t {
            return Err("origen y destino son el mismo nodo".into());
        }
        // Invariante preventivo: la arista se valida ANTES de tocar el grafo. El agente recibe
        // el error acá y puede corregir la llamada; el camino humano nunca pasa por este punto.
        crate::grafo::arista_valida(&nodes, &s, &t)?;

        let (a, b) = if req["direction"].as_str() == Some("<-") {
            (t.clone(), s.clone())
        } else {
            (s.clone(), t.clone())
        };
        if edges
            .iter()
            .any(|e| e["source"].as_str() == Some(a.as_str()) && e["target"].as_str() == Some(b.as_str()))
        {
            return Ok(json!({"ok": true, "accion": "ya_existia", "origen": a, "destino": b}));
        }
        let eid = format!("e-ag-{}-{}", slug(&a), slug(&b));
        let label = req["label"].as_str().unwrap_or("").to_string();
        edges.push(json!({
            "id": eid,
            "source": a,
            "target": b,
            "sourceHandle": "right",
            "targetHandle": "left",
            "type": "smoothstep",
            "animated": true,
            "label": if label.is_empty() { Value::Null } else { json!(label) },
            "style": { "stroke": "#8b5cf6", "strokeWidth": 2 }
        }));
        let mut res = self.write_all(&nodes, &edges, &appearance, &template_id, &map)?;
        {
            let rev = res["revision"].as_u64().unwrap_or(0);
            self.inner.lock().unwrap().agent_writes.insert(eid.clone(), rev);
        }
        res["accion"] = json!("conectados");
        res["id"] = json!(eid);
        res["origen"] = json!(a);
        res["destino"] = json!(b);
        Ok(res)
    }

    /// Borra un nodo y sus aristas. Nunca borra el núcleo raíz.
    pub fn delete_node(&self, req: &Value) -> Result<Value, String> {
        let needle = req["id"]
            .as_str()
            .or_else(|| req["title"].as_str())
            .ok_or("falta `id` (o `title`)")?;
        let (state, nodes, edges, map) = self.estado_base()?;
        let appearance = state.get("appearance").cloned().unwrap_or(Value::Null);
        let template_id = state.get("templateId").cloned().unwrap_or(Value::Null);
        let id = self
            .resolve(&nodes, needle)
            .ok_or_else(|| format!("no encontré el nodo: {needle}"))?;
        let victima = nodes
            .iter()
            .find(|n| n["id"].as_str() == Some(id.as_str()))
            .cloned()
            .unwrap_or(Value::Null);
        if victima["data"]["isRoot"].as_bool().unwrap_or(false) {
            return Err("no borro el nodo núcleo (es la raíz del mapa)".into());
        }
        let nodos: Vec<Value> = nodes
            .iter()
            .filter(|n| n["id"].as_str() != Some(id.as_str()))
            .cloned()
            .collect();
        let aristas: Vec<Value> = edges
            .iter()
            .filter(|e| {
                e["source"].as_str() != Some(id.as_str()) && e["target"].as_str() != Some(id.as_str())
            })
            .cloned()
            .collect();
        let quitadas = edges.len() - aristas.len();
        let mut res = self.write_all(&nodos, &aristas, &appearance, &template_id, &map)?;
        self.inner.lock().unwrap().agent_writes.remove(&id);
        res["accion"] = json!("borrado");
        res["id"] = json!(id);
        res["titulo"] = json!(victima["data"]["title"]);
        res["aristas_borradas"] = json!(quitadas);
        Ok(res)
    }

    /// Saca las aristas que apuntan a nodos inexistentes (integridad referencial del grafo).
    pub fn prune(&self, _req: &Value) -> Result<Value, String> {
        let (state, nodes, edges, map) = self.estado_base()?;
        let appearance = state.get("appearance").cloned().unwrap_or(Value::Null);
        let template_id = state.get("templateId").cloned().unwrap_or(Value::Null);
        let ids: HashSet<String> = nodes
            .iter()
            .filter_map(|n| n["id"].as_str().map(String::from))
            .collect();
        let antes = edges.len();
        let aristas: Vec<Value> = edges
            .iter()
            .filter(|e| {
                ids.contains(e["source"].as_str().unwrap_or(""))
                    && ids.contains(e["target"].as_str().unwrap_or(""))
            })
            .cloned()
            .collect();
        let quitadas = antes - aristas.len();
        if quitadas == 0 {
            return Ok(json!({
                "ok": true, "accion": "nada_que_limpiar",
                "aristas": antes, "nodos": nodes.len(),
            }));
        }
        let mut res = self.write_all(&nodes, &aristas, &appearance, &template_id, &map)?;
        res["accion"] = json!("saneado");
        res["aristas_quitadas"] = json!(quitadas);
        res["aristas_antes"] = json!(antes);
        res["aristas"] = json!(aristas.len());
        Ok(res)
    }

    /// Vista compacta del lienzo para el agente (leer antes de escribir).
    pub fn summary(&self) -> Value {
        let info = self.info();
        let Some(state) = self.read_state() else {
            return json!({
                "ok": false,
                "info": info,
                "mensaje": "todavía no hay estado en disco: abrí NodeFlow y esperá el primer autoguardado"
            });
        };
        let nodes = state["nodes"].as_array().cloned().unwrap_or_default();
        let edges = state["edges"].as_array().cloned().unwrap_or_default();
        let ids: HashSet<String> = nodes
            .iter()
            .filter_map(|n| n["id"].as_str().map(String::from))
            .collect();
        let colgadas = edges
            .iter()
            .filter(|e| {
                let s = e["source"].as_str().unwrap_or("");
                let t = e["target"].as_str().unwrap_or("");
                !ids.contains(s) || !ids.contains(t)
            })
            .count();
        let mut sin_conexiones = 0;
        let mut madurez_total = 0i64;
        let mut con_madurez = 0i64;
        let lista: Vec<Value> = nodes
            .iter()
            .map(|n| {
                let id = n["id"].as_str().unwrap_or("");
                let conns: Vec<Value> = edges
                    .iter()
                    .filter(|e| e["source"].as_str() == Some(id) || e["target"].as_str() == Some(id))
                    .map(|e| {
                        let (otro, dir) = if e["source"].as_str() == Some(id) {
                            (e["target"].as_str().unwrap_or(""), "→")
                        } else {
                            (e["source"].as_str().unwrap_or(""), "←")
                        };
                        let titulo = nodes
                            .iter()
                            .find(|x| x["id"].as_str() == Some(otro))
                            .and_then(|x| x["data"]["title"].as_str().or_else(|| x["data"]["label"].as_str()))
                            .unwrap_or("?");
                        json!({"dir": dir, "titulo": titulo, "label": e["label"]})
                    })
                    .collect();
                if conns.is_empty() {
                    sin_conexiones += 1;
                }
                if let Some(m) = n["data"]["maturity"].as_i64() {
                    madurez_total += m;
                    con_madurez += 1;
                }
                json!({
                    "id": id,
                    "titulo": n["data"]["title"].as_str().or_else(|| n["data"]["label"].as_str()).unwrap_or(""),
                    "categoria": n["data"]["category"],
                    "madurez": n["data"]["maturity"],
                    "nucleo": n["data"]["isRoot"].as_bool().unwrap_or(false),
                    "descripcion": n["data"]["description"],
                    "tags": n["data"]["tags"],
                    "creado_por": n["data"]["aiOrigin"]["actionType"],
                    "conexiones": conns,
                })
            })
            .collect();
        json!({
            "ok": true,
            "mapa": state["name"],
            "vault": self.root.to_string_lossy(),
            "revision": info["revision"],
            "stats": {
                "nodos": nodes.len(),
                "aristas": edges.len(),
                "aristas_colgadas": colgadas,
                "nodos_sin_conexiones": sin_conexiones,
                "madurez_promedio": if con_madurez > 0 {
                    ((madurez_total as f64 / con_madurez as f64) * 10.0).round() / 10.0
                } else { 0.0 },
                "notas_en_disco": info["notas"],
            },
            "nodos": lista,
        })
    }


    pub fn read_state(&self) -> Option<Value> {
        let txt = std::fs::read_to_string(self.root.join(".nodeflow").join("state.json")).ok()?;
        serde_json::from_str(&txt).ok()
    }

    pub fn info(&self) -> Value {
        let inner = self.inner.lock().unwrap();
        let notas = std::fs::read_dir(self.root.join("nodos"))
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_string_lossy().ends_with(".md"))
                    .count()
            })
            .unwrap_or(0);
        let canonico = self.read_state();
        json!({
            "vault": self.root.to_string_lossy(),
            "mapa": inner.map_name,
            "revision": inner.revision,
            "updated_at": inner.updated_at,
            "notas": notas,
            "nodos_en_disco": canonico.as_ref().map(|c| c["nodes"].as_array().map(|a| a.len()).unwrap_or(0)),
            "ultimos_cambios_externos": inner.external,
            "tiene_estado": canonico.is_some(),
        })
    }

    // ── dirección Obsidian → app ─────────────────────────────────────────────

    /// Fusiona una nota `nodos/*.md` editada por fuera en el estado canónico.
    fn merge_node_note(&self, rel: &str, text: &str) -> Result<(), String> {
        let (fm, body) = split_frontmatter(text);
        let id = fm
            .get("id")
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty());

        let mut state = self.read_state().ok_or("no hay estado canónico")?;
        let nodes = state["nodes"].as_array().cloned().ok_or("estado sin nodos")?;

        // Localizar el nodo: por id del frontmatter o, si falta, por el slug del archivo.
        let stem = Path::new(rel)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let target = nodes.iter().find(|n| {
            if let Some(id) = &id {
                n["id"].as_str() == Some(id.as_str())
            } else {
                let t = n["data"]["title"].as_str().unwrap_or("");
                slug(t) == stem
            }
        });
        let target_id = target
            .and_then(|n| n["id"].as_str())
            .ok_or("la nota no corresponde a ningún nodo")?
            .to_string();

        // Título: frontmatter > primer heading > actual
        let heading = body
            .lines()
            .find(|l| l.trim_start().starts_with("# "))
            .map(|l| l.trim_start_matches('#').trim().to_string());
        let nuevo_titulo = fm
            .get("title")
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
            .or(heading.filter(|s| !s.is_empty()));

        // Descripción: el texto entre el heading y "## Conexiones"
        let desc = body
            .split("## Conexiones")
            .next()
            .unwrap_or("")
            .lines()
            .skip_while(|l| !l.trim_start().starts_with("# "))
            .skip(1)
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();

        let madurez = fm
            .get("madurez")
            .or_else(|| fm.get("maturity"))
            .and_then(|v| v.trim().parse::<i64>().ok());
        let categoria = fm
            .get("categoria")
            .or_else(|| fm.get("category"))
            .or_else(|| fm.get("categoría"))
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty());
        let tags = fm.get("tags").and_then(|v| serde_json::from_str::<Vec<String>>(v).ok());

        let mut cambios: Vec<String> = Vec::new();
        let mut nodes_mut = nodes.clone();
        for n in nodes_mut.iter_mut() {
            if n["id"].as_str() != Some(target_id.as_str()) {
                continue;
            }
            if let Some(t) = &nuevo_titulo {
                if n["data"]["title"].as_str() != Some(t.as_str()) {
                    n["data"]["title"] = json!(t);
                    if n["data"]["label"].is_string() {
                        n["data"]["label"] = json!(t);
                    }
                    cambios.push(format!("título → {t}"));
                }
            }
            if !desc.is_empty() && n["data"]["description"].as_str().unwrap_or("") != desc.as_str() {
                n["data"]["description"] = json!(desc);
                cambios.push("descripción".into());
            }
            if let Some(m) = madurez {
                if n["data"]["maturity"].as_i64() != Some(m) {
                    n["data"]["maturity"] = json!(m);
                    cambios.push(format!("madurez → {m}"));
                }
            }
            if let Some(c) = &categoria {
                if n["data"]["category"].as_str() != Some(c.as_str()) {
                    n["data"]["category"] = json!(c);
                    cambios.push(format!("categoría → {c}"));
                }
            }
            if let Some(t) = &tags {
                let limpios: Vec<String> = t.iter().map(|s| s.trim_start_matches('#').to_string()).collect();
                n["data"]["tags"] = json!(limpios);
                cambios.push("tags".into());
            }
        }
        if cambios.is_empty() {
            return Ok(());
        }

        state["nodes"] = json!(nodes_mut);
        state["updated_at"] = json!(epoch_ms());
        state["ultimo_cambio_externo"] = json!({ "archivo": rel, "campos": cambios });
        let txt = serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?;
        let mut inner = self.inner.lock().unwrap();
        inner.last_write = Some(Instant::now());
        drop(inner);
        self.write_atomic(".nodeflow/state.json", &txt)?;
        log::info!("vault: {rel} fusionado desde Obsidian → {cambios:?}");
        Ok(())
    }

    /// Procesa un lote de rutas cambiadas; ignora lo nuestro (ventana + hash) y fusiona lo ajeno.
    fn process_external(&self, paths: Vec<PathBuf>) {
        let grace = {
            let i = self.inner.lock().unwrap();
            i.last_write
                .map(|t| t.elapsed() < WRITE_GRACE)
                .unwrap_or(false)
        };
        if grace {
            return;
        }
        let mut changed: Vec<String> = Vec::new();
        for p in paths {
            if !p.is_file() {
                continue;
            }
            let rel = match p.strip_prefix(&self.root) {
                Ok(r) => r.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };
            if rel.starts_with(".nodeflow") || rel.contains(".obsidian") || rel.contains(".git") {
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
            let content = match std::fs::read(&p) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let h = hash_bytes(&content);
            let known = { self.inner.lock().unwrap().hashes.get(&rel).copied() };
            if known == Some(h) {
                continue; // ya es nuestro / ya procesado
            }
            if rel.starts_with("nodos/") {
                let txt = String::from_utf8_lossy(&content).to_string();
                match self.merge_node_note(&rel, &txt) {
                    Ok(()) => changed.push(rel.clone()),
                    Err(e) => log::warn!("vault: no pude fusionar {rel}: {e}"),
                }
            } else {
                log::info!("vault: cambio externo en archivo generado ({rel}) — se regenerará al próximo guardado");
            }
            self.inner.lock().unwrap().hashes.insert(rel, h);
        }
        if !changed.is_empty() {
            let mut i = self.inner.lock().unwrap();
            i.revision += 1;
            i.external = changed;
            log::info!("vault: revisión {} por cambios desde Obsidian", i.revision);
        }
    }
    // ── Fase 5a: el agente propone, el humano aprueba ───────────────────────────



    fn save_pending(&self) {
        let doc = {
            let inner = self.inner.lock().unwrap();
            json!({ "version": 1, "pendientes": inner.pendientes })
        };
        if let Ok(txt) = serde_json::to_string_pretty(&doc) {
            let _ = self.write_atomic(".nodeflow/pending.json", &txt);
        }
    }

    pub fn count_pending(&self) -> usize {
        self.inner.lock().unwrap().pendientes.len()
    }

    /// Lista de propuestas. Con `since` responde barato cuando nada cambió (polling).
    pub fn pending_list(&self, since: Option<u64>) -> Value {
        let inner = self.inner.lock().unwrap();
        let revision = inner.pending_rev;
        if let Some(s) = since {
            if s == revision {
                return json!({"changed": false, "revision": revision, "total": inner.pendientes.len()});
            }
        }
        json!({
            "changed": true,
            "revision": revision,
            "total": inner.pendientes.len(),
            "pendientes": inner.pendientes,
        })
    }

    /// Guarda una propuesta: valida el pedido y arma la vista para el panel, SIN tocar el grafo.
    /// Validar acá es lo que le da al agente un error inmediato en vez de uno diferido.
    pub fn propose(&self, tipo: &str, req: &Value) -> Result<Value, String> {
        let vista = match tipo {
            "nodo" => self.preview_nodo(req)?,
            "conectar" => self.preview_conectar(req)?,
            "borrar" => self.preview_borrar(req)?,
            "sanear" => self.preview_sanear()?,
            "reacomodar" => self.preview_reacomodar(req)?,
            otro => return Err(format!("tipo de propuesta desconocido: {otro}")),
        };
        let resumen = vista["resumen"].as_str().unwrap_or("").to_string();

        let (id, ya_estaba, total) = {
            let mut inner = self.inner.lock().unwrap();
            if let Some(prev) = inner
                .pendientes
                .iter()
                .find(|p| p["vista"]["resumen"].as_str() == Some(resumen.as_str()))
            {
                let pid = prev["id"].clone();
                let t = inner.pendientes.len();
                (pid, true, t)
            } else {
                let id = format!("p-{}-{}", epoch_ms(), inner.pendientes.len() + 1);
                let op = json!({
                    "id": id,
                    "tipo": tipo,
                    "creado_ms": epoch_ms(),
                    "origen": req["origen"].as_str().unwrap_or("hermes"),
                    "motivo": req["motivo"].as_str().unwrap_or(""),
                    "payload": req,
                    "vista": vista,
                });
                inner.pendientes.push(op);
                inner.pending_rev += 1;
                let t = inner.pendientes.len();
                (json!(id), false, t)
            }
        };
        if !ya_estaba {
            self.save_pending();
            log::info!("agente: propuesta {id} · {resumen}");
        }
        Ok(json!({
            "ok": true,
            "accion": if ya_estaba { "ya_propuesto" } else { "propuesto" },
            "id_pendiente": id,
            "vista": vista,
            "pendientes": total,
            "nota": "Espera aprobación humana en el panel de la app (o aprobala por chat).",
        }))
    }

    /// Aprobar aplica la escritura; rechazar la descarta. Las que fallan quedan pendientes con su error.
    pub fn resolve_pending(
        &self,
        ids: &[String],
        todos: bool,
        aprobar: bool,
    ) -> Result<Value, String> {
        let pendientes: Vec<Value> = self.inner.lock().unwrap().pendientes.clone();
        if pendientes.is_empty() {
            return Ok(json!({"ok": true, "accion": "nada_pendiente", "cantidad": 0}));
        }
        let mut hechos = 0usize;
        let mut errores: Vec<Value> = Vec::new();
        let mut restantes: Vec<Value> = Vec::new();
        let mut revision = Value::Null;
        let mut detalle: Vec<Value> = Vec::new();

        for p in pendientes {
            let pid = p["id"].as_str().unwrap_or("").to_string();
            let elegido = todos || ids.iter().any(|x| x == &pid || x.trim_start_matches('#') == pid);
            if !elegido {
                restantes.push(p);
                continue;
            }
            if !aprobar {
                hechos += 1;
                detalle.push(json!({"id": pid, "resultado": "rechazado"}));
                continue;
            }
            let tipo = p["tipo"].as_str().unwrap_or("");
            let payload = &p["payload"];
            let res = match tipo {
                "nodo" => self.upsert_node(payload),
                "conectar" => self.add_edge(payload),
                "borrar" => self.delete_node(payload),
                "sanear" => self.prune(payload),
                "reacomodar" => self.aplicar_layout(payload),
                _ => Err(format!("tipo desconocido: {tipo}")),
            };
            match res {
                Ok(v) => {
                    hechos += 1;
                    revision = v["revision"].clone();
                    // Fase 7b: aprobar una creación/actualización del agente ES el artefacto (T1).
                    if tipo == "nodo" {
                        self.marcar_actividad("aprobacion_ia");
                    }
                    detalle.push(json!({
                        "id": pid,
                        "resultado": "aplicado",
                        "detalle": v["accion"],
                    }));
                }
                Err(e) => {
                    log::warn!("agente: no pude aplicar {pid}: {e}");
                    errores.push(json!({"id": pid, "error": e}));
                    restantes.push(p);
                }
            }
        }

        {
            let mut inner = self.inner.lock().unwrap();
            inner.pendientes = restantes;
            inner.pending_rev += 1;
        }
        self.save_pending();
        let quedan = self.count_pending();
        log::info!(
            "agente: {} {} propuesta(s) · quedan {quedan}",
            if aprobar { "aprobadas" } else { "rechazadas" },
            hechos
        );
        Ok(json!({
            "ok": true,
            "accion": if aprobar { "aprobado" } else { "rechazado" },
            "cantidad": hechos,
            "detalle": detalle,
            "errores": errores,
            "revision": revision,
            "pendientes": quedan,
        }))
    }

    // ── vistas previas (validan y describen, sin escribir) ──────────────────────

    fn preview_nodo(&self, req: &Value) -> Result<Value, String> {
        let title = req["title"]
            .as_str()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or("falta `title` (título del nodo)")?;
        let (_state, nodes, _edges, _map) = self.estado_base()?;
        let existente = req["id"]
            .as_str()
            .and_then(|i| self.resolve(&nodes, i))
            .or_else(|| self.resolve(&nodes, &title));
        let parent_id = match req["parent"].as_str() {
            Some(p) => Some(
                self.resolve(&nodes, p)
                    .ok_or_else(|| format!("no encontré el nodo padre: {p}"))?,
            ),
            None => None,
        };

        if let Some(id) = existente {
            let actual = nodes
                .iter()
                .find(|n| n["id"].as_str() == Some(id.as_str()))
                .cloned()
                .unwrap_or(Value::Null);
            let d = &actual["data"];
            let titulo_actual = d["title"]
                .as_str()
                .or_else(|| d["label"].as_str())
                .unwrap_or("")
                .to_string();
            let mut cambios: Vec<String> = Vec::new();
            if titulo_actual != title {
                cambios.push(format!("título: «{titulo_actual}» → «{title}»"));
            }
            if let Some(t) = req["description"].as_str() {
                if d["description"].as_str() != Some(t) {
                    cambios.push("descripción".into());
                }
            }
            if let Some(m) = req["maturity"].as_i64() {
                if d["maturity"].as_i64() != Some(m) {
                    cambios.push(format!(
                        "madurez: {} → {m}",
                        d["maturity"].as_i64().unwrap_or(0)
                    ));
                }
            }
            if let Some(c) = req["category"].as_str() {
                if d["category"].as_str() != Some(c) {
                    cambios.push(format!(
                        "categoría: {} → {c}",
                        d["category"].as_str().unwrap_or("-")
                    ));
                }
            }
            if req["tags"].is_array() {
                cambios.push("tags".into());
            }
            let resumen = if cambios.is_empty() {
                format!("Actualizar «{titulo_actual}»: sin cambios reales")
            } else {
                format!("Actualizar «{titulo_actual}»: {}", cambios.join(", "))
            };
            Ok(json!({
                "accion_legible": "Actualizar nodo",
                "titulo": titulo_actual,
                "resumen": resumen,
                "cambios": cambios,
                "nodo_id": id,
                "peligro": "bajo",
                "antes": {
                    "titulo": titulo_actual,
                    "categoria": d["category"],
                    "madurez": d["maturity"],
                    "descripcion": d["description"],
                },
                "despues": {
                    "titulo": title,
                    "categoria": req["category"],
                    "madurez": req["maturity"],
                    "descripcion": req["description"],
                },
            }))
        } else {
            let padre_titulo = parent_id.as_ref().and_then(|pid| {
                nodes
                    .iter()
                    .find(|n| n["id"].as_str() == Some(pid.as_str()))
                    .and_then(|n| {
                        n["data"]["title"]
                            .as_str()
                            .or_else(|| n["data"]["label"].as_str())
                            .map(String::from)
                    })
            });
            let cat = req["category"].as_str().unwrap_or("AGENTE").to_string();
            let mat = req["maturity"].as_i64().unwrap_or(1);
            let etiqueta = req["link_label"].as_str().unwrap_or("");
            let mut resumen = format!("Crear nodo «{title}» ({cat}, madurez {mat})");
            match &padre_titulo {
                Some(pt) => {
                    resumen.push_str(&format!(" colgado de «{pt}»"));
                    if !etiqueta.is_empty() {
                        resumen.push_str(&format!(" con la etiqueta «{etiqueta}»"));
                    }
                }
                None => resumen.push_str(" — sin conexión al árbol"),
            }
            Ok(json!({
                "accion_legible": "Crear nodo",
                "titulo": title,
                "resumen": resumen,
                "nodo_id": Value::Null,
                "padre_id": parent_id,
                "padre_titulo": padre_titulo,
                "peligro": "bajo",
                "despues": {
                    "titulo": title,
                    "categoria": cat,
                    "madurez": mat,
                    "descripcion": req["description"],
                },
            }))
        }
    }

    fn preview_conectar(&self, req: &Value) -> Result<Value, String> {
        let src = req["source"]
            .as_str()
            .ok_or("falta `source` (id o título del origen)")?;
        let dst = req["target"]
            .as_str()
            .ok_or("falta `target` (id o título del destino)")?;
        let (_state, nodes, edges, _map) = self.estado_base()?;
        let s = self
            .resolve(&nodes, src)
            .ok_or_else(|| format!("no encontré el nodo origen: {src}"))?;
        let t = self
            .resolve(&nodes, dst)
            .ok_or_else(|| format!("no encontré el nodo destino: {dst}"))?;
        if s == t {
            return Err("origen y destino son el mismo nodo".into());
        }
        let (a, b) = if req["direction"].as_str() == Some("<-") {
            (t, s)
        } else {
            (s, t)
        };
        let titulo = |id: &str| -> String {
            nodes
                .iter()
                .find(|n| n["id"].as_str() == Some(id))
                .and_then(|n| {
                    n["data"]["title"]
                        .as_str()
                        .or_else(|| n["data"]["label"].as_str())
                        .map(String::from)
                })
                .unwrap_or_else(|| id.to_string())
        };
        if edges.iter().any(|e| {
            e["source"].as_str() == Some(a.as_str()) && e["target"].as_str() == Some(b.as_str())
        }) {
            return Err("esa conexión ya existe".into());
        }
        let etiqueta = req["label"].as_str().unwrap_or("");
        Ok(json!({
            "accion_legible": "Conectar",
            "titulo": format!("{} → {}", titulo(&a), titulo(&b)),
            "resumen": format!(
                "Conectar «{}» → «{}»{}",
                titulo(&a),
                titulo(&b),
                if etiqueta.is_empty() { String::new() } else { format!(" con la etiqueta «{etiqueta}»") }
            ),
            "origen": a,
            "destino": b,
            "peligro": "bajo",
        }))
    }

    fn preview_borrar(&self, req: &Value) -> Result<Value, String> {
        let needle = req["id"]
            .as_str()
            .or_else(|| req["title"].as_str())
            .ok_or("falta `id` (o `title`)")?;
        let (_state, nodes, edges, _map) = self.estado_base()?;
        let id = self
            .resolve(&nodes, needle)
            .ok_or_else(|| format!("no encontré el nodo: {needle}"))?;
        let victima = nodes
            .iter()
            .find(|n| n["id"].as_str() == Some(id.as_str()))
            .cloned()
            .unwrap_or(Value::Null);
        if victima["data"]["isRoot"].as_bool().unwrap_or(false) {
            return Err("no borro el nodo núcleo (es la raíz del mapa)".into());
        }
        let titulo = victima["data"]["title"]
            .as_str()
            .or_else(|| victima["data"]["label"].as_str())
            .unwrap_or("")
            .to_string();
        let aristas = edges
            .iter()
            .filter(|e| {
                e["source"].as_str() == Some(id.as_str()) || e["target"].as_str() == Some(id.as_str())
            })
            .count();
        Ok(json!({
            "accion_legible": "Borrar nodo",
            "titulo": titulo,
            "resumen": format!("Borrar «{titulo}» y sus {aristas} conexión(es)"),
            "nodo_id": id,
            "aristas_afectadas": aristas,
            "peligro": "alto",
            "antes": {"titulo": titulo, "descripcion": victima["data"]["description"]},
        }))
    }

    fn preview_sanear(&self) -> Result<Value, String> {
        let (_state, nodes, edges, _map) = self.estado_base()?;
        let ids: HashSet<String> = nodes
            .iter()
            .filter_map(|n| n["id"].as_str().map(String::from))
            .collect();
        let colgadas = edges
            .iter()
            .filter(|e| {
                !ids.contains(e["source"].as_str().unwrap_or(""))
                    || !ids.contains(e["target"].as_str().unwrap_or(""))
            })
            .count();
        if colgadas == 0 {
            return Err("el grafo ya está sano: ninguna arista colgada".into());
        }
        Ok(json!({
            "accion_legible": "Sanear grafo",
            "titulo": "Integridad referencial",
            "resumen": format!("Quitar {colgadas} arista(s) que apuntan a nodos inexistentes"),
            "aristas_quitadas": colgadas,
            "peligro": "medio",
        }))
    }

    /// Nombre del mapa en disco (`<mapa>.md` / `<mapa>.canvas` son artefactos generados).
    pub fn nombre_mapa(&self) -> String {
        self.read_state()
            .and_then(|s| s["name"].as_str().map(String::from))
            .unwrap_or_else(|| "nodeflow".into())
    }

    /// Grafo canónico actual (nodos, aristas). Lo usa el contexto de la IA para el sesgo espacial.
    pub fn grafo_actual(&self) -> (Vec<Value>, Vec<Value>) {
        match self.estado_base() {
            Ok((_s, n, e, _m)) => (n, e),
            Err(_) => (Vec::new(), Vec::new()),
        }
    }

    // ── Fase 7a: agente jardín (diagnóstico + arreglo propuesto) ────────────────

    /// Diagnóstico del grafo: invariantes + hallazgos del jardín + padrinos sugeridos.
    /// Solo lectura: no toca nada.
    pub fn jardin_scan(&self) -> Value {
        match self.estado_base() {
            Ok((state, nodes, edges, _map)) => {
                let mut d = crate::grafo::diagnostico(&nodes, &edges);
                d["mapa"] = state["name"].clone();
                d["vault"] = json!(self.root.to_string_lossy());
                d["revision"] = json!(self.inner.lock().unwrap().revision);
                d["pendientes"] = json!(self.count_pending());
                d
            }
            Err(e) => json!({
                "sano": false, "error": e, "problemas": [], "padrinos": [], "stats": {}
            }),
        }
    }

    /// Convierte los hallazgos accionables en PROPUESTAS. No toca el lienzo: encola.
    pub fn jardin_proponer(&self, _req: &Value) -> Result<Value, String> {
        let (_state, nodes, edges, _map) = self.estado_base()?;
        let diag = crate::grafo::diagnostico(&nodes, &edges);
        let problemas = diag["problemas"].as_array().cloned().unwrap_or_default();
        let mut creadas: Vec<Value> = Vec::new();
        let mut motivos: Vec<String> = Vec::new();

        // 1) integridad (colgadas o duplicadas) → una sola propuesta de saneo
        if problemas.iter().any(|p| p["accion"] == "podar") {
            let r = self.propose(
                "sanear",
                &json!({"origen": "jardin", "motivo": "El jardín detectó aristas rotas o repetidas"}),
            )?;
            creadas.push(json!({"tipo": "sanear", "resultado": r["accion"], "id_pendiente": r["id_pendiente"], "resumen": r["vista"]["resumen"]}));
            motivos.push("integridad del grafo".into());
        }

        // 2) basura (archivos generados importados como nodos) → borrado, uno por nodo
        if let Some(p) = problemas.iter().find(|p| p["tipo"] == "nodos_basura") {
            for id in p["ids"].as_array().cloned().unwrap_or_default() {
                if let Some(id) = id.as_str() {
                    let r = self.propose(
                        "borrar",
                        &json!({"id": id, "origen": "jardin",
                                "motivo": "Es un archivo generado (índice o canvas), no un concepto"}),
                    )?;
                    creadas.push(json!({"tipo": "borrar", "id": id, "resultado": r["accion"], "id_pendiente": r["id_pendiente"], "resumen": r["vista"]["resumen"]}));
                }
            }
            motivos.push("nodos que eran archivos generados".into());
        }

        // 3) huérfanos e islas → conectar con el padrino más afín (similitud de contenido)
        for pad in diag["padrinos"].as_array().cloned().unwrap_or_default() {
            let nodo = pad["nodo"].as_str().unwrap_or("");
            let padre = pad["padre_sugerido"].as_str().unwrap_or("");
            if nodo.is_empty() || padre.is_empty() {
                continue;
            }
            let sim = pad["similitud"].as_f64().unwrap_or(0.0);
            let confianza = pad["confianza"].as_str().unwrap_or("baja");
            let r = self.propose(
                "conectar",
                &json!({
                    "source": padre,
                    "target": nodo,
                    // La etiqueta no puede prometer más de lo que la evidencia sostiene.
                    "label": if confianza == "alta" { "afín" } else { "revisar vínculo" },
                    "origen": "jardin",
                    "motivo": format!(
                        "El jardín los emparejó por similitud de contenido ({sim}, confianza {confianza}){}",
                        if confianza == "alta" { "" } else { " — verificá que la conexión tenga sentido" }
                    ),
                }),
            )?;
            creadas.push(json!({
                "tipo": "conectar", "nodo": nodo, "padre": padre, "similitud": sim,
                "resultado": r["accion"], "id_pendiente": r["id_pendiente"], "resumen": r["vista"]["resumen"]
            }));
        }
        if diag["padrinos"].as_array().map(|a| !a.is_empty()).unwrap_or(false) {
            motivos.push("conexiones sugeridas por afinidad".into());
        }

        Ok(json!({
            "ok": true,
            "creadas": creadas,
            "cantidad": creadas.len(),
            "motivos": motivos,
            "pendientes_totales": self.count_pending(),
            "nota": "Nada tocó el lienzo: son propuestas para aprobar en «Cambios del agente».",
        }))
    }

    /// Calcula el layout por niveles y lo PROPONE (reemplaza el despeje de colisiones a mano).
    pub fn tidy(&self, req: &Value) -> Result<Value, String> {
        let (_state, nodes, edges, _map) = self.estado_base()?;
        let pos = crate::grafo::layout_jerarquico(&nodes, &edges);
        let mut cambios: Vec<Value> = Vec::new();
        for n in &nodes {
            let id = crate::grafo::id_de(n);
            if let Some((x, y)) = pos.get(&id) {
                let ox = n["position"]["x"].as_f64().unwrap_or(0.0);
                let oy = n["position"]["y"].as_f64().unwrap_or(0.0);
                if (ox - x).abs() > 8.0 || (oy - y).abs() > 8.0 {
                    cambios.push(json!({"id": id, "x": x, "y": y}));
                }
            }
        }
        if cambios.is_empty() {
            return Ok(json!({"ok": true, "accion": "ya_ordenado",
                "mensaje": "El lienzo ya está en niveles: no hay nada que mover."}));
        }
        let mut payload = json!({
            "posiciones": cambios,
            "origen": req["origen"].as_str().unwrap_or("jardin"),
            "motivo": req["motivo"].as_str().unwrap_or("Layout por niveles"),
        });
        payload["origen"] = json!(req["origen"].as_str().unwrap_or("jardin"));
        let r = self.propose("reacomodar", &payload)?;
        Ok(json!({
            "ok": true,
            "accion": r["accion"],
            "id_pendiente": r["id_pendiente"],
            "vista": r["vista"],
            "moveria": cambios.len(),
            "de_total": nodes.len(),
        }))
    }

    /// Aplica el layout aprobado (una sola escritura con todas las posiciones).
    pub fn aplicar_layout(&self, req: &Value) -> Result<Value, String> {
        let (state, mut nodes, edges, map) = self.estado_base()?;
        let appearance = state.get("appearance").cloned().unwrap_or(Value::Null);
        let template_id = state.get("templateId").cloned().unwrap_or(Value::Null);
        let mut movidos = 0usize;
        for p in req["posiciones"].as_array().cloned().unwrap_or_default() {
            let id = p["id"].as_str().unwrap_or("");
            let (x, y) = (p["x"].as_f64(), p["y"].as_f64());
            if id.is_empty() || x.is_none() || y.is_none() {
                continue;
            }
            for n in nodes.iter_mut() {
                if crate::grafo::id_de(n) == id {
                    n["position"]["x"] = json!(x.unwrap());
                    n["position"]["y"] = json!(y.unwrap());
                    movidos += 1;
                }
            }
        }
        if movidos == 0 {
            return Err("ninguna posición coincidió con un nodo".into());
        }
        let mut res = self.write_all(&nodes, &edges, &appearance, &template_id, &map)?;
        res["accion"] = json!("reacomodado");
        res["nodos_movidos"] = json!(movidos);
        Ok(res)
    }

    /// Previsualización del reacomodo (para el panel): qué y cuánto se mueve.
    fn preview_reacomodar(&self, req: &Value) -> Result<Value, String> {
        let (_state, nodes, _edges, _map) = self.estado_base()?;
        let cambios = req["posiciones"].as_array().cloned().unwrap_or_default();
        if cambios.is_empty() {
            return Err("no hay posiciones para aplicar".into());
        }
        let mut ejemplos: Vec<String> = Vec::new();
        for c in cambios.iter().take(4) {
            let id = c["id"].as_str().unwrap_or("");
            let titulo = nodes
                .iter()
                .find(|n| crate::grafo::id_de(n) == id)
                .map(crate::grafo::titulo_de)
                .unwrap_or_else(|| id.to_string());
            ejemplos.push(titulo);
        }
        Ok(json!({
            "accion_legible": "Reacomodar lienzo",
            "titulo": "Layout por niveles",
            "resumen": format!(
                "Mover {} de {} nodos a una grilla por niveles (el árbol se lee de izquierda a derecha, sin solapamientos)",
                cambios.len(),
                nodes.len()
            ),
            "cambios": ejemplos,
            "peligro": "medio",
            "nodos_movidos": cambios.len(),
        }))
    }

    // ── Fase 7b: métrica de valor (T0 → T1) ─────────────────────────────────────

    /// Gap que cierra una sesión de trabajo (30 min sin actividad).
    const SESION_GAP_MS: u64 = 30 * 60 * 1000;

    fn guardar_metricas(&self, doc: &Value) {
        if let Ok(txt) = serde_json::to_string_pretty(doc) {
            let _ = self.write_atomic(".nodeflow/metricas.json", &txt);
        }
    }

    /// Registra actividad y, cuando corresponde, cierra la conversión de la sesión.
    /// T0 = la primera escritura HUMANA de la sesión (el brain dump aterriza en el lienzo).
    /// T1 = la primera propuesta de IA APROBADA (aprobar es decir «esto es un artefacto»).
    fn marcar_actividad(&self, evento: &str) {
        let ahora = epoch_ms();
        let mut doc = self.inner.lock().unwrap().metricas.clone();
        if !doc.is_object() {
            doc = json!({"version": 1, "sesiones": [], "abierta": Value::Null});
        }
        let mut sesiones = doc["sesiones"].as_array().cloned().unwrap_or_default();
        let mut abierta = doc["abierta"].clone();
        let abrir = match abierta["t0_ms"].as_u64() {
            None => true,
            Some(t0) => {
                ahora.saturating_sub(abierta["ultima_ms"].as_u64().unwrap_or(t0)) > Self::SESION_GAP_MS
            }
        };
        if abrir {
            if abierta["t1_ms"].as_u64().is_some() {
                sesiones.push(abierta.clone());
            }
            abierta = json!({
                "t0_ms": ahora,
                "ultima_ms": ahora,
                "t1_ms": Value::Null,
                "delta_min": Value::Null,
                "eventos": [],
            });
        }
        abierta["ultima_ms"] = json!(ahora);
        let mut eventos = abierta["eventos"].as_array().cloned().unwrap_or_default();
        eventos.push(json!({"ts": ahora, "que": evento}));
        if eventos.len() > 60 {
            eventos.drain(0..eventos.len() - 60);
        }
        abierta["eventos"] = json!(eventos);
        if evento == "aprobacion_ia" && abierta["t1_ms"].is_null() {
            let t0 = abierta["t0_ms"].as_u64().unwrap_or(ahora);
            let delta = ahora.saturating_sub(t0) as f64 / 60_000.0;
            abierta["t1_ms"] = json!(ahora);
            abierta["delta_min"] = json!((delta * 10.0).round() / 10.0);
            log::info!(
                "metrica: primera conversión de la sesión en {:.1} min (objetivo < 3)",
                delta
            );
        }
        doc["sesiones"] = json!(sesiones);
        doc["abierta"] = abierta;
        {
            self.inner.lock().unwrap().metricas = doc.clone();
        }
        self.guardar_metricas(&doc);
    }

    /// Estado de la métrica de valor: la última conversión, el promedio y la sesión abierta.
    pub fn metricas(&self) -> Value {
        let doc = self.inner.lock().unwrap().metricas.clone();
        let sesiones = doc["sesiones"].as_array().cloned().unwrap_or_default();
        let mut deltas: Vec<f64> = sesiones
            .iter()
            .filter_map(|s| s["delta_min"].as_f64())
            .collect();
        let abierta = doc["abierta"].clone();
        let en_curso = abierta["delta_min"].as_f64();
        if let Some(d) = en_curso {
            deltas.push(d);
        }
        let promedio = if deltas.is_empty() {
            Value::Null
        } else {
            json!(((deltas.iter().sum::<f64>() / deltas.len() as f64) * 10.0).round() / 10.0)
        };
        let abierta_activa = abierta["t0_ms"].as_u64().is_some()
            && epoch_ms().saturating_sub(abierta["ultima_ms"].as_u64().unwrap_or(0))
                < Self::SESION_GAP_MS;
        json!({
            "objetivo_min": 3.0,
            "promedio_min": promedio,
            "ultima_min": deltas.last().copied().map(|d| json!((d * 10.0).round() / 10.0)),
            "conversiones": deltas.len(),
            "sesion_activa": abierta_activa,
            "t0_ms": abierta["t0_ms"],
            "t1_ms": abierta["t1_ms"],
            "minutos_desde_t0": abierta["t0_ms"].as_u64().map(|t0| {
                (((epoch_ms().saturating_sub(t0)) as f64 / 60_000.0) * 10.0).round() / 10.0
            }),
            "sesiones_cerradas": sesiones.len(),
            "detalle": sesiones.iter().rev().take(5).collect::<Vec<_>>(),
        })
    }

}

/// Separa el frontmatter YAML (plano, key: value) del cuerpo.
fn split_frontmatter(text: &str) -> (HashMap<String, String>, String) {
    let mut fm: HashMap<String, String> = HashMap::new();
    let t = text.strip_prefix('\u{feff}').unwrap_or(text);
    if !t.trim_start().starts_with("---") {
        return (fm, t.to_string());
    }
    let after = &t[t.find("---").unwrap() + 3..];
    let Some(end) = after.find("\n---") else {
        return (fm, t.to_string());
    };
    let head = &after[..end];
    let body = after[end + 4..].to_string();
    for line in head.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            fm.insert(k.trim().to_lowercase(), v.trim().to_string());
        }
    }
    (fm, body)
}

/// Monitorea el vault y dispara la fusión de cambios externos (con debounce).
pub fn start_watcher(vault: Arc<Vault>) {
    let pending: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
    let pending_cb = pending.clone();
    let root = vault.root.clone();
    let mut watcher = match notify::recommended_watcher(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(ev) = res {
                if matches!(ev.kind, notify::EventKind::Access(_)) {
                    return;
                }
                if let Ok(mut set) = pending_cb.lock() {
                    for p in ev.paths {
                        set.insert(p);
                    }
                }
            }
        },
    ) {
        Ok(w) => w,
        Err(e) => {
            log::warn!("vault: no pude crear el watcher ({e})");
            return;
        }
    };
    if let Err(e) = watcher.watch(&root, notify::RecursiveMode::Recursive) {
        log::warn!("vault: no pude observar {} ({e})", root.display());
        return;
    }
    log::info!("vault: observando {} (bidireccional)", root.display());
    std::thread::spawn(move || {
        let _keep_alive = watcher;
        loop {
            std::thread::sleep(WATCH_INTERVAL);
            let batch: Vec<PathBuf> = {
                let mut s = match pending.lock() {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                if s.is_empty() {
                    continue;
                }
                s.drain().collect()
            };
            vault.process_external(batch);
        }
    });
}

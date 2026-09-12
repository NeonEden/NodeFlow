//! Fase 7a — Funciones PURAS sobre el grafo: invariantes, layout jerárquico y diagnóstico de jardín.
//!
//! Todo acá es determinista y sin I/O a propósito: es lo que se puede testear con `cargo test` y lo
//! que el backend necesita para validar una mutación ANTES de tocar el disco (el punto de control
//! preventivo que faltaba: rechazar la arista huérfana al entrar, en vez de limpiarla después).

use crate::memoria::tokenizar;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};

/// Separación horizontal entre niveles del árbol.
const X_PASO: f64 = 420.0;
/// Separación vertical dentro de un nivel (los nodos miden hasta ~320 px de alto).
const Y_PASO: f64 = 380.0;
/// Debajo de este puntaje de solapamiento no se sugiere conectar un huérfano.
const MIN_SIMILITUD: f32 = 0.28;

pub fn id_de(n: &Value) -> String {
    n["id"].as_str().unwrap_or("").to_string()
}

pub fn titulo_de(n: &Value) -> String {
    n["data"]["title"]
        .as_str()
        .or_else(|| n["data"]["label"].as_str())
        .or_else(|| n["title"].as_str())
        .unwrap_or("")
        .to_string()
}

pub fn descripcion_de(n: &Value) -> String {
    n["data"]["description"]
        .as_str()
        .or_else(|| n["description"].as_str())
        .unwrap_or("")
        .to_string()
}

fn es_nucleo(n: &Value) -> bool {
    n["data"]["isRoot"].as_bool().unwrap_or(false)
}

/// Grados (entrada/salida) por nodo, ignorando aristas que apuntan a ids inexistentes.
pub fn grados(nodes: &[Value], edges: &[Value]) -> HashMap<String, (usize, usize)> {
    let ids: HashSet<String> = nodes.iter().map(id_de).collect();
    let mut g: HashMap<String, (usize, usize)> = ids.iter().map(|i| (i.clone(), (0, 0))).collect();
    for e in edges {
        let s = e["source"].as_str().unwrap_or("");
        let t = e["target"].as_str().unwrap_or("");
        if !ids.contains(s) || !ids.contains(t) {
            continue;
        }
        g.entry(s.to_string()).or_insert((0, 0)).1 += 1;
        g.entry(t.to_string()).or_insert((0, 0)).0 += 1;
    }
    g
}

// ── Invariantes ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Gravedad {
    Alta,
    Media,
    Baja,
}

impl Gravedad {
    fn str(&self) -> &'static str {
        match self {
            Gravedad::Alta => "alta",
            Gravedad::Media => "media",
            Gravedad::Baja => "baja",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Problema {
    pub tipo: &'static str,
    pub gravedad: Gravedad,
    pub detalle: String,
    pub ids: Vec<String>,
    /// Acción que lo resuelve, si es automaticamente proponible: `podar` | `borrar` | `conectar` | `nada`.
    pub accion: &'static str,
}

/// Valida un grafo completo. Es la ÚNICA fuente de verdad sobre qué es un grafo sano:
/// la usan la validación previa a escribir (invariante), el panel y el jardín.
pub fn validar(nodes: &[Value], edges: &[Value]) -> Vec<Problema> {
    let mut out: Vec<Problema> = Vec::new();
    let ids: HashSet<String> = nodes.iter().map(id_de).collect();
    let titulos: HashMap<String, usize> = {
        let mut m = HashMap::new();
        for n in nodes {
            *m.entry(titulo_de(n).trim().to_lowercase()).or_insert(0) += 1;
        }
        m
    };

    // 1) ids duplicados (o vacíos) — rompe todo lo demás, va primero
    let mut vistos: HashSet<String> = HashSet::new();
    let mut dup: Vec<String> = Vec::new();
    for n in nodes {
        let id = id_de(n);
        if id.is_empty() || !vistos.insert(id.clone()) {
            dup.push(if id.is_empty() { "(sin id)".into() } else { id });
        }
    }
    if !dup.is_empty() {
        out.push(Problema {
            tipo: "ids_duplicados",
            gravedad: Gravedad::Alta,
            detalle: format!("{} id(s) repetidos o vacíos: {}", dup.len(), dup.join(", ")),
            ids: dup,
            accion: "nada",
        });
    }

    // 2) aristas colgadas
    let colgadas: Vec<String> = edges
        .iter()
        .filter(|e| {
            let s = e["source"].as_str().unwrap_or("");
            let t = e["target"].as_str().unwrap_or("");
            !ids.contains(s) || !ids.contains(t)
        })
        .map(|e| e["id"].as_str().unwrap_or("(sin id)").to_string())
        .collect();
    if !colgadas.is_empty() {
        out.push(Problema {
            tipo: "aristas_colgadas",
            gravedad: Gravedad::Alta,
            detalle: format!(
                "{} arista(s) apuntan a nodos que no existen: {}",
                colgadas.len(),
                colgadas.join(", ")
            ),
            ids: colgadas,
            accion: "podar",
        });
    }

    // 3) nodos huérfanos (sin ninguna arista): invisibles en la vista de árbol
    let grados = grados(nodes, edges);
    let huerfanos: Vec<String> = nodes
        .iter()
        .filter(|n| grados.get(&id_de(n)).map(|(i, o)| *i + *o == 0).unwrap_or(true))
        .map(|n| id_de(n))
        .collect();
    if !huerfanos.is_empty() {
        out.push(Problema {
            tipo: "huerfanos",
            gravedad: Gravedad::Media,
            detalle: format!(
                "{} nodo(s) sin una sola conexión: {}",
                huerfanos.len(),
                huerfanos
                    .iter()
                    .map(|id| {
                        nodes
                            .iter()
                            .find(|n| id_de(n) == *id)
                            .map(titulo_de)
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ids: huerfanos,
            accion: "conectar",
        });
    }

    // 4) aristas duplicadas (mismo origen y destino repetido)
    let mut pares: HashSet<String> = HashSet::new();
    let mut repetidas: Vec<String> = Vec::new();
    for e in edges {
        let k = format!(
            "{}->{}",
            e["source"].as_str().unwrap_or(""),
            e["target"].as_str().unwrap_or("")
        );
        if !pares.insert(k) {
            repetidas.push(e["id"].as_str().unwrap_or("(sin id)").to_string());
        }
    }
    if !repetidas.is_empty() {
        out.push(Problema {
            tipo: "aristas_duplicadas",
            gravedad: Gravedad::Media,
            detalle: format!("{} conexión(es) repetidas entre los mismos nodos", repetidas.len()),
            ids: repetidas,
            accion: "podar",
        });
    }

    // 5) títulos repetidos
    let repes: Vec<String> = titulos
        .iter()
        .filter(|(t, c)| **c > 1 && !t.is_empty())
        .map(|(t, _)| t.clone())
        .collect();
    if !repes.is_empty() {
        out.push(Problema {
            tipo: "titulos_repetidos",
            gravedad: Gravedad::Baja,
            detalle: format!("{} título(s) repetidos: {}", repes.len(), repes.join(", ")),
            ids: Vec::new(),
            accion: "nada",
        });
    }

    // 6) nodos basura: el nombre de un archivo generado, no un concepto
    let basura: Vec<String> = nodes
        .iter()
        .filter(|n| {
            let t = titulo_de(n).trim().to_lowercase();
            if t.is_empty() {
                return true;
            }
            let d = descripcion_de(n).to_lowercase();
            // Ojo: comparar siempre en minúsculas — "Desde el vault:" no es "desde el vault:".
            t.ends_with(".canvas")
                || t.ends_with(".md")
                || (d.contains("archivo generado") && d.contains("se regenera"))
                || (d.starts_with("desde el vault:")
                    && (d.contains(".canvas") || d.contains(".md")))
        })
        .map(|n| id_de(n))
        .collect();
    if !basura.is_empty() {
        out.push(Problema {
            tipo: "nodos_basura",
            gravedad: Gravedad::Alta,
            detalle: format!(
                "{} nodo(s) que son archivos generados, no conceptos: {}",
                basura.len(),
                basura
                    .iter()
                    .map(|id| {
                        nodes
                            .iter()
                            .find(|n| id_de(n) == *id)
                            .map(titulo_de)
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ids: basura,
            accion: "borrar",
        });
    }

    // 7) islas: componentes que no contienen al núcleo
    let mut ady: HashMap<String, Vec<String>> = HashMap::new();
    for e in edges {
        let s = e["source"].as_str().unwrap_or("");
        let t = e["target"].as_str().unwrap_or("");
        if ids.contains(s) && ids.contains(t) {
            ady.entry(s.to_string()).or_default().push(t.to_string());
            ady.entry(t.to_string()).or_default().push(s.to_string());
        }
    }
    let raiz = nodes
        .iter()
        .find(|n| es_nucleo(n))
        .map(id_de)
        .or_else(|| {
            grados
                .iter()
                .max_by_key(|(_, (i, o))| *i + *o)
                .map(|(k, _)| k.clone())
        });
    if let Some(raiz) = raiz {
        let mut alcanzables: HashSet<String> = HashSet::new();
        let mut cola = VecDeque::new();
        cola.push_back(raiz.clone());
        alcanzables.insert(raiz.clone());
        while let Some(a) = cola.pop_front() {
            for b in ady.get(&a).cloned().unwrap_or_default() {
                if alcanzables.insert(b.clone()) {
                    cola.push_back(b);
                }
            }
        }
        let islas: Vec<String> = nodes
            .iter()
            .map(id_de)
            .filter(|id| !alcanzables.contains(id))
            .collect();
        if !islas.is_empty() {
            out.push(Problema {
                tipo: "islas",
                gravedad: Gravedad::Media,
                detalle: format!(
                    "{} nodo(s) en racimos desconectados del núcleo: {}",
                    islas.len(),
                    islas
                        .iter()
                        .map(|id| nodes
                            .iter()
                            .find(|n| id_de(n) == *id)
                            .map(titulo_de)
                            .unwrap_or_default())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                ids: islas,
                accion: "conectar",
            });
        }
    }

    // 8) radar: mucha conexión con poca madurez (lo estructuralmente flojo)
    let sin_madurez: Vec<String> = nodes
        .iter()
        .filter(|n| n["data"]["maturity"].as_i64().is_none())
        .map(|n| id_de(n))
        .collect();
    if !sin_madurez.is_empty() {
        out.push(Problema {
            tipo: "sin_madurez",
            gravedad: Gravedad::Baja,
            detalle: format!("{} nodo(s) sin madurez declarada", sin_madurez.len()),
            ids: sin_madurez,
            accion: "nada",
        });
    }
    let hubs_flojos: Vec<String> = nodes
        .iter()
        .filter(|n| {
            let d = grados.get(&id_de(n)).map(|(i, o)| i + o).unwrap_or(0);
            let m = n["data"]["maturity"].as_i64().unwrap_or(99);
            d >= 5 && m <= 2
        })
        .map(|n| id_de(n))
        .collect();
    if !hubs_flojos.is_empty() {
        out.push(Problema {
            tipo: "hubs_inmaduros",
            gravedad: Gravedad::Baja,
            detalle: format!(
                "lo más conectado es lo menos maduro: {}",
                hubs_flojos
                    .iter()
                    .map(|id| nodes
                        .iter()
                        .find(|n| id_de(n) == *id)
                        .map(titulo_de)
                        .unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ids: hubs_flojos,
            accion: "nada",
        });
    }

    out
}

/// ¿Se puede escribir este grafo? Devuelve los problemas que lo IMPIDEN.
/// Solo bloquean los que rompen la integridad estructural (ids y aristas): lo demás se reporta.
/// La usa la validación previa del agente y los tests de invariantes.
#[cfg_attr(not(test), allow(dead_code))]
pub fn errores_bloqueantes(nodes: &[Value], edges: &[Value]) -> Vec<String> {
    validar(nodes, edges)
        .into_iter()
        .filter(|p| matches!(p.tipo, "ids_duplicados" | "aristas_colgadas"))
        .map(|p| format!("[{}] {}", p.tipo, p.detalle))
        .collect()
}

/// Chequeo previo a una mutación concreta: ¿esta arista puede existir?
pub fn arista_valida(nodes: &[Value], source: &str, target: &str) -> Result<(), String> {
    let ids: HashSet<String> = nodes.iter().map(id_de).collect();
    if source.is_empty() || target.is_empty() {
        return Err("la arista necesita origen y destino".into());
    }
    if source == target {
        return Err("origen y destino son el mismo nodo".into());
    }
    if !ids.contains(source) {
        return Err(format!("el nodo origen no existe: {source}"));
    }
    if !ids.contains(target) {
        return Err(format!("el nodo destino no existe: {target}"));
    }
    Ok(())
}

// ── Layout jerárquico ────────────────────────────────────────────────────────

/// Layout por niveles, determinista: BFS desde el núcleo (o el nodo más conectado), ordena cada
/// nivel por el orden de sus padres y por título, y separa los racimos desconectados a la derecha.
/// Reemplaza el despeje de colisiones a mano (`y += 220` en un bucle).
pub fn layout_jerarquico(nodes: &[Value], edges: &[Value]) -> HashMap<String, (f64, f64)> {
    let mut pos: HashMap<String, (f64, f64)> = HashMap::new();
    if nodes.is_empty() {
        return pos;
    }
    let ids: HashSet<String> = nodes.iter().map(id_de).collect();
    let mut hijos: HashMap<String, Vec<String>> = HashMap::new();
    let mut entrantes: HashMap<String, usize> = HashMap::new();
    for e in edges {
        let s = e["source"].as_str().unwrap_or("");
        let t = e["target"].as_str().unwrap_or("");
        if !ids.contains(s) || !ids.contains(t) {
            continue;
        }
        hijos.entry(s.to_string()).or_default().push(t.to_string());
        *entrantes.entry(t.to_string()).or_insert(0) += 1;
    }
    for v in hijos.values_mut() {
        v.sort();
        v.dedup();
    }

    let raiz = nodes
        .iter()
        .find(|n| es_nucleo(n))
        .map(id_de)
        .or_else(|| {
            let g = grados(nodes, edges);
            g.iter()
                .max_by_key(|(_, (i, o))| *i + *o)
                .map(|(k, _)| k.clone())
        })
        .unwrap_or_else(|| id_de(&nodes[0]));

    // BFS: nivel = profundidad en el árbol
    let mut nivel: HashMap<String, usize> = HashMap::new();
    let mut cola: VecDeque<String> = VecDeque::new();
    cola.push_back(raiz.clone());
    nivel.insert(raiz.clone(), 0);
    while let Some(a) = cola.pop_front() {
        let n = *nivel.get(&a).unwrap_or(&0);
        for h in hijos.get(&a).cloned().unwrap_or_default() {
            if !nivel.contains_key(&h) {
                nivel.insert(h.clone(), n + 1);
                cola.push_back(h);
            }
        }
    }

    // Los que no se alcanzaron desde la raíz van a una columna aparte, a la derecha.
    let max_nivel = nivel.values().copied().max().unwrap_or(0);
    let mut sueltos: Vec<String> = nodes
        .iter()
        .map(id_de)
        .filter(|id| !nivel.contains_key(id))
        .collect();
    sueltos.sort_by_key(|id| titulo_de(nodes.iter().find(|n| id_de(n) == *id).unwrap()).to_lowercase());
    for (i, id) in sueltos.iter().enumerate() {
        nivel.insert(id.clone(), max_nivel + 1 + (i / 8));
    }

    // Agrupar por nivel y ordenar dentro del nivel
    let mut por_nivel: HashMap<usize, Vec<String>> = HashMap::new();
    for n in nodes {
        let id = id_de(n);
        let nv = *nivel.get(&id).unwrap_or(&max_nivel);
        por_nivel.entry(nv).or_default().push(id);
    }
    for (_, v) in por_nivel.iter_mut() {
        v.sort_by(|a, b| {
            let ta = nodes.iter().find(|n| id_de(n) == *a).map(titulo_de).unwrap_or_default();
            let tb = nodes.iter().find(|n| id_de(n) == *b).map(titulo_de).unwrap_or_default();
            ta.to_lowercase().cmp(&tb.to_lowercase())
        });
    }

    let mut niveles: Vec<usize> = por_nivel.keys().copied().collect();
    niveles.sort();
    for nv in niveles {
        let miembros = por_nivel.get(&nv).cloned().unwrap_or_default();
        let total = miembros.len() as f64;
        for (i, id) in miembros.iter().enumerate() {
            let x = 120.0 + (nv as f64) * X_PASO;
            // Centrado vertical alrededor de 0, con paso constante: sin solapamientos.
            let y = 120.0 + (i as f64 - (total - 1.0) / 2.0) * Y_PASO;
            pos.insert(id.clone(), (x.round(), y.round()));
        }
    }
    pos
}

// ── Cercanía en el grafo (para el RAG espacial) ───────────────────────────────

/// Multiplicador de relevancia por distancia relacional al nodo enfocado:
/// vecino directo 1.5×, a dos saltos 1.2×, el resto 1.0×. Devuelve id → factor, sin incluir el foco.
pub fn cercania(nodes: &[Value], edges: &[Value], foco: &str, saltos: usize) -> HashMap<String, f32> {
    let ids: HashSet<String> = nodes.iter().map(id_de).collect();
    if !ids.contains(foco) {
        return HashMap::new();
    }
    let mut ady: HashMap<String, Vec<String>> = HashMap::new();
    for e in edges {
        let s = e["source"].as_str().unwrap_or("");
        let t = e["target"].as_str().unwrap_or("");
        if ids.contains(s) && ids.contains(t) {
            ady.entry(s.to_string()).or_default().push(t.to_string());
            ady.entry(t.to_string()).or_default().push(s.to_string());
        }
    }
    let mut out: HashMap<String, f32> = HashMap::new();
    let mut visitados: HashSet<String> = HashSet::new();
    visitados.insert(foco.to_string());
    let mut frontera: Vec<String> = vec![foco.to_string()];
    for nivel in 1..=saltos {
        let factor = match nivel {
            1 => 1.5,
            2 => 1.2,
            _ => 1.0,
        };
        let mut siguiente: Vec<String> = Vec::new();
        for a in &frontera {
            for b in ady.get(a).cloned().unwrap_or_default() {
                if visitados.insert(b.clone()) {
                    out.insert(b.clone(), factor);
                    siguiente.push(b);
                }
            }
        }
        if siguiente.is_empty() {
            break;
        }
        frontera = siguiente;
    }
    out
}

// ── Sugerencias de jardín ────────────────────────────────────────────────────

/// Solapamiento de términos entre dos textos (0..1), con el título pesando doble.
pub fn similitud(a_titulo: &str, a_desc: &str, b_titulo: &str, b_desc: &str) -> f32 {
    let mut ta = tokenizar(&format!("{a_titulo} {a_titulo} {a_desc}"));
    let mut tb = tokenizar(&format!("{b_titulo} {b_titulo} {b_desc}"));
    ta.sort();
    ta.dedup();
    tb.sort();
    tb.dedup();
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    // Jaccard ponderado: castiga títulos largos distintos, premia vocabulario compartido.
    let set_a: HashSet<&String> = ta.iter().collect();
    let set_b: HashSet<&String> = tb.iter().collect();
    let inter = set_a.intersection(&set_b).count() as f32;
    let union = set_a.union(&set_b).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// Para cada nodo huérfano o isla, el mejor candidato a padre por similitud (o `None`).
pub fn padrinos(nodes: &[Value], edges: &[Value]) -> Vec<Value> {
    let grados = grados(nodes, edges);
    let problemas = validar(nodes, edges);
    let mut candidatos: Vec<String> = Vec::new();
    for p in &problemas {
        if p.accion == "conectar" {
            candidatos.extend(p.ids.clone());
        }
    }
    candidatos.sort();
    candidatos.dedup();
    let mut out: Vec<Value> = Vec::new();
    for id in candidatos {
        let Some(h) = nodes.iter().find(|n| id_de(n) == id) else {
            continue;
        };
        let ht = titulo_de(h);
        let hd = descripcion_de(h);
        // Ya conectados: proponer una conexión que existe no arregla nada.
        let mut ya_conectados: HashSet<String> = HashSet::new();
        for e in edges {
            let (s, t) = (
                e["source"].as_str().unwrap_or(""),
                e["target"].as_str().unwrap_or(""),
            );
            if s == id {
                ya_conectados.insert(t.to_string());
            }
            if t == id {
                ya_conectados.insert(s.to_string());
            }
        }
        let mut mejor: Option<(String, String, f32)> = None;
        for otro in nodes {
            let oid = id_de(otro);
            if oid == id
                || ya_conectados.contains(&oid)
                || grados.get(&oid).map(|(i, o)| i + o).unwrap_or(0) == 0
            {
                continue; // no tejer huérfano con huérfano, ni repetir una conexión
            }
            let s = similitud(&ht, &hd, &titulo_de(otro), &descripcion_de(otro));
            if mejor.as_ref().map(|(_, _, bs)| s > *bs).unwrap_or(true) {
                mejor = Some((oid, titulo_de(otro), s));
            }
        }
        if let Some((pid, ptitulo, s)) = mejor {
            // Siempre se devuelve el MEJOR candidato: si la afinidad es baja se marca como tal y
            // decide el humano en el panel. Un jardín que no propone nada no sirve de nada.
            out.push(json!({
                "nodo": id,
                "titulo": ht,
                "padre_sugerido": pid,
                "padre_titulo": ptitulo,
                "similitud": ((s as f64 * 100.0).round() / 100.0),
                "confianza": if s >= MIN_SIMILITUD { "alta" } else { "baja" },
            }));
        }
    }
    out
}

/// Diagnóstico completo para el panel del jardín.
pub fn diagnostico(nodes: &[Value], edges: &[Value]) -> Value {
    let problemas = validar(nodes, edges);
    let grados = grados(nodes, edges);
    let sin_desc = nodes
        .iter()
        .filter(|n| descripcion_de(n).trim().len() < 20)
        .count();
    let sin_madurez = nodes
        .iter()
        .filter(|n| n["data"]["maturity"].as_i64().is_none())
        .count();
    let bloqueantes = problemas
        .iter()
        .filter(|p| matches!(p.tipo, "ids_duplicados" | "aristas_colgadas"))
        .count();
    json!({
        "sano": problemas.is_empty(),
        "bloqueantes": bloqueantes,
        "problemas": problemas.iter().map(|p| json!({
            "tipo": p.tipo,
            "gravedad": p.gravedad.str(),
            "detalle": p.detalle,
            "ids": p.ids,
            "accion": p.accion,
        })).collect::<Vec<_>>(),
        "padrinos": padrinos(nodes, edges),
        "stats": {
            "nodos": nodes.len(),
            "aristas": edges.len(),
            "sin_descripcion": sin_desc,
            "sin_madurez": sin_madurez,
            "huerfanos": grados.values().filter(|(i, o)| *i + *o == 0).count(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nodo(id: &str, titulo: &str, desc: &str) -> Value {
        json!({"id": id, "type": "ideaNode", "position": {"x": 0, "y": 0},
               "data": {"id": id, "title": titulo, "description": desc, "maturity": 3}})
    }
    fn raiz(id: &str, titulo: &str) -> Value {
        let mut n = nodo(id, titulo, "nucleo del mapa");
        n["data"]["isRoot"] = json!(true);
        n
    }
    fn arista(id: &str, s: &str, t: &str) -> Value {
        json!({"id": id, "source": s, "target": t})
    }

    #[test]
    fn arista_colgada_se_detecta_y_bloquea() {
        let nodes = vec![raiz("r", "Núcleo"), nodo("a", "Uno", "descripcion larga de prueba")];
        let edges = vec![arista("e1", "r", "a"), arista("e2", "r", "fantasma")];
        let problemas = validar(&nodes, &edges);
        assert!(
            problemas.iter().any(|p| p.tipo == "aristas_colgadas"),
            "debe detectar la arista colgada"
        );
        let bloqueantes = errores_bloqueantes(&nodes, &edges);
        assert_eq!(bloqueantes.len(), 1, "y debe bloquear la escritura");
    }

    #[test]
    fn grafo_sano_no_reporta_nada_bloqueante() {
        let nodes = vec![raiz("r", "Núcleo"), nodo("a", "Uno", "una descripcion suficientemente larga")];
        let edges = vec![arista("e1", "r", "a")];
        assert!(errores_bloqueantes(&nodes, &edges).is_empty());
    }

    #[test]
    fn ids_duplicados_bloquean() {
        let nodes = vec![raiz("r", "Núcleo"), nodo("r", "Otro", "descripcion de prueba larga")];
        let b = errores_bloqueantes(&nodes, &[]);
        assert!(b.iter().any(|s| s.contains("ids_duplicados")));
    }

    #[test]
    fn arista_a_si_mismo_o_inexistente_se_rechaza() {
        let nodes = vec![raiz("r", "Núcleo")];
        assert!(arista_valida(&nodes, "r", "r").is_err());
        assert!(arista_valida(&nodes, "r", "nada").is_err());
        assert!(arista_valida(&nodes, "r", "").is_err());
    }

    #[test]
    fn el_layout_es_determinista_y_sin_solapamientos() {
        let nodes = vec![
            raiz("r", "Núcleo"),
            nodo("a", "A", "descripcion larga uno"),
            nodo("b", "B", "descripcion larga dos"),
            nodo("c", "C", "descripcion larga tres"),
        ];
        let edges = vec![arista("e1", "r", "a"), arista("e2", "r", "b"), arista("e3", "a", "c")];
        let p1 = layout_jerarquico(&nodes, &edges);
        let p2 = layout_jerarquico(&nodes, &edges);
        assert_eq!(p1, p2, "el mismo grafo debe dar las mismas posiciones");
        // Nadie en la misma celda
        let mut vistos: HashSet<(i64, i64)> = HashSet::new();
        for (_, (x, y)) in p1.iter() {
            assert!(vistos.insert((*x as i64, *y as i64)), "dos nodos en la misma posición");
        }
        // El núcleo queda a la izquierda de sus hijos
        let (rx, _) = p1.get("r").unwrap();
        let (ax, _) = p1.get("a").unwrap();
        assert!(rx < ax, "el núcleo debe estar a la izquierda de sus hijos");
    }

    #[test]
    fn el_layout_ubica_los_desconectados_a_la_derecha() {
        let nodes = vec![raiz("r", "Núcleo"), nodo("a", "A", "descripcion larga"), nodo("x", "Isla", "otra descripcion larga")];
        let edges = vec![arista("e1", "r", "a")];
        let p = layout_jerarquico(&nodes, &edges);
        let (ax, _) = p.get("a").unwrap();
        let (xx, _) = p.get("x").unwrap();
        assert!(xx > ax, "la isla va a la derecha del árbol");
    }

    #[test]
    fn detecta_nodos_basura_archivos_generados() {
        let nodes = vec![
            raiz("r", "Núcleo"),
            nodo("junk", "agente-autónomo-multimodal", "Desde el vault: NodeFlow/x.md # titulo"),
            nodo("malo", "indice.canvas", "cualquier cosa"),
        ];
        let p = validar(&nodes, &[]);
        let basura = p.iter().find(|p| p.tipo == "nodos_basura").expect("debe detectar basura");
        assert!(basura.ids.contains(&"junk".to_string()));
        assert!(basura.ids.contains(&"malo".to_string()));
        assert_eq!(basura.accion, "borrar");
    }

    #[test]
    fn detecta_el_nodo_basura_real_de_la_app() {
        // El caso real: la MemoriaPanel trajo el indice generado y quedo como nodo suelto.
        let nodes = vec![
            raiz("r", "Núcleo"),
            nodo(
                "junk",
                "agente-autónomo-multimodal",
                "Desde el vault: NodeFlow/agente-autónomo-multimodal.md # agente-autónomo-multimodal > [!warning] Archivo generado > Este índice y <mapa>.canvas los regenera NodeFlow en cada guardado.",
            ),
        ];
        let p = validar(&nodes, &[]);
        let basura = p.iter().find(|p| p.tipo == "nodos_basura").expect("el indice importado es basura");
        assert!(basura.ids.contains(&"junk".to_string()));
    }

    #[test]
    fn no_marca_como_basura_un_nodo_que_menciona_archivos() {
        let nodes = vec![
            raiz("r", "Núcleo"),
            nodo(
                "ok",
                "Vault en Disco como Fuente de Verdad",
                "El estado vive en nodos/*.md y en el .canvas del mapa; el watcher los mantiene sincronizados.",
            ),
        ];
        let p = validar(&nodes, &[]);
        assert!(
            !p.iter().any(|p| p.tipo == "nodos_basura"),
            "mencionar archivos no convierte a un nodo en basura"
        );
    }

    #[test]
    fn detecta_isla_y_sugiere_padrino_por_similitud() {
        let nodes = vec![
            raiz("r", "Núcleo del Mapa"),
            nodo("a", "Visuales TouchDesigner", "render en vivo con difusion y latencia baja"),
            nodo("isla", "Render en vivo TouchDesigner", "latencia de difusion en visuales"),
        ];
        let edges = vec![arista("e1", "r", "a")];
        let p = validar(&nodes, &edges);
        assert!(p.iter().any(|p| p.tipo == "huerfanos" || p.tipo == "islas"));
        let pad = padrinos(&nodes, &edges);
        let par = pad.iter().find(|v| v["nodo"] == "isla").expect("debe encontrar padrino");
        assert_eq!(par["padre_sugerido"], "a", "el par mas parecido es el de TouchDesigner");
        assert!(par["similitud"].as_f64().unwrap() > 0.2);
    }

    #[test]
    fn la_cercania_pondera_por_saltos() {
        // r - a - b - c   => a(1.5), b(1.2), c fuera
        let nodes = vec![
            raiz("r", "Núcleo"),
            nodo("a", "A", "descripcion larga uno"),
            nodo("b", "B", "descripcion larga dos"),
            nodo("c", "C", "descripcion larga tres"),
        ];
        let edges = vec![arista("e1", "r", "a"), arista("e2", "a", "b"), arista("e3", "b", "c")];
        let c = cercania(&nodes, &edges, "r", 2);
        assert_eq!(c.get("a"), Some(&1.5));
        assert_eq!(c.get("b"), Some(&1.2));
        assert_eq!(c.get("c"), None, "a tres saltos no se pondera");
        assert_eq!(c.get("r"), None, "el foco no se pondera a si mismo");
    }

    #[test]
    fn la_cercania_sin_foco_no_hace_nada() {
        let nodes = vec![raiz("r", "Núcleo"), nodo("a", "A", "descripcion larga")];
        let edges = vec![arista("e1", "r", "a")];
        assert!(cercania(&nodes, &edges, "inexistente", 2).is_empty());
    }

    #[test]
    fn no_sugiere_padrino_ya_conectado() {
        // Dos nodos ya unidos entre si, ambos fuera del arbol: el padrino debe ser OTRO.
        let nodes = vec![
            raiz("r", "Núcleo"),
            nodo("a", "Guardrails y Auditoria Etica", "validacion de seguridad y gobernanza del agente"),
            nodo("i1", "Curaduria de Señales HITL", "filtrar señales de aprendizaje antes de inyectarlas"),
            nodo("i2", "Provenance de Nodos IA", "trazabilidad de validacion de los nodos generados"),
        ];
        let edges = vec![arista("e1", "r", "a"), arista("e2", "i1", "i2")];
        let pad = padrinos(&nodes, &edges);
        for p in &pad {
            let n = p["nodo"].as_str().unwrap_or("");
            let sug = p["padre_sugerido"].as_str().unwrap_or("");
            assert!(
                !(n == "i1" && sug == "i2") && !(n == "i2" && sug == "i1"),
                "no debe proponer una conexion que ya existe"
            );
        }
        assert!(!pad.is_empty(), "las islas deben recibir una sugerencia");
    }

    #[test]
    fn similitud_ordena_los_pares_correctamente() {
        let alta = similitud("Visuales TouchDesigner", "render en vivo", "TouchDesigner visuales", "render en vivo");
        let baja = similitud("Visuales TouchDesigner", "render en vivo", "Cold outreach", "prospeccion por instagram");
        assert!(alta > baja, "los textos afines deben puntuar mas alto");
        assert!(alta > MIN_SIMILITUD);
        assert!(baja < MIN_SIMILITUD);
    }

    #[test]
    fn el_diagnostico_sale_completo() {
        let nodes = vec![raiz("r", "Núcleo"), nodo("suelto", "Suelto", "")];
        let edges = vec![];
        let d = diagnostico(&nodes, &edges);
        assert_eq!(d["sano"], json!(false));
        assert!(d["stats"]["huerfanos"].as_u64().unwrap() >= 1);
        assert!(d["problemas"].as_array().unwrap().len() >= 2);
    }
}

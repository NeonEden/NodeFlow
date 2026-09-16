//! Fase 5.5 — el **curador del lienzo**: propuestas de fusión y poda, con motivo.
//!
//! El pedido: *«propongo fusionar/borrar nodos con motivo, aprobable en bloque»* — con el objetivo de que
//! el lienzo se mantenga limpio sin que lo haga el humano a mano.
//!
//! Dos capas, a propósito:
//! - **Esta (mecánica)**: reglas deterministas y explicables sobre el estado canónico — duplicados,
//!   casi-duplicados, títulos subsumidos, fragmentos vacíos y sueltos. Corre sola, sin modelo, y propone.
//! - **La del turno (juicio)**: lo que ninguna regla ve (dos títulos distintos con la misma idea, un nodo
//!   que ya quedó cubierto por otro). Eso lo decido yo leyendo el lienzo y lo propongo con la tool
//!   `fusionar_nodos`, por la misma cola.
//!
//! Reglas de convivencia con el humano:
//! - Nada se aplica acá: **todo entra a la cola de propuestas** y se aprueba (en bloque, si quiere).
//! - Lo que no se toca automáticamente se **declara**: el curador dice qué miró y por qué no lo tocó. Un
//!   curador que borra callado es peor que un lienzo sucio.
//! - La precisión manda sobre la cobertura: cada hallazgo trae su **motivo** y su **evidencia**.

use serde_json::{json, Value};

/// Lo que el curador encontró: una observación con su porqué y la propuesta lista para la cola.
#[derive(Clone, Debug, PartialEq)]
pub struct Hallazgo {
    /// Clase de hallazgo: `duplicado`, `casi_duplicado`, `subsumido`, `fragmento`, `suelto`.
    pub clase: &'static str,
    /// Qué nodo (o par) afecta, para leerlo de un vistazo.
    pub titulo: String,
    /// **Por qué** se propone: una frase concreta, sin adjetivos.
    pub motivo: String,
    /// Los números que lo sostienen (grados, largos, similitud).
    pub evidencia: String,
    /// `alta` (regla exacta) | `media` (parecido).
    pub confianza: &'static str,
    /// El payload tal como lo espera la cola: `{"tipo": ..., "payload": {...}}`.
    pub propuesta: Value,
}

impl Hallazgo {
    pub fn clase_legible(&self) -> &'static str {
        match self.clase {
            "duplicado" => "Duplicado",
            "casi_duplicado" => "Casi duplicado",
            "subsumido" => "Título cubierto por otro",
            "fragmento" => "Fragmento sin contenido",
            "suelto" => "Nodo suelto (sin conexiones)",
            otra => otra,
        }
    }
}

/// Por qué un nodo **no** se toca automáticamente. `None` = el curador puede proponer sobre él.
/// Es deliberadamente conservador: el núcleo, lo maduro, lo bien conectado y lo que tiene texto propio
/// sólo se tocan si el humano (o el turno) lo decide con la evidencia a la vista.
pub fn intocable(n: &Value, grado: usize) -> Option<String> {
    if n["data"]["isRoot"].as_bool().unwrap_or(false) {
        return Some("es el núcleo (raíz del mapa)".to_string());
    }
    let madurez = madurez_de(n);
    if madurez >= 4 {
        return Some(format!("madurez {madurez} (probado)"));
    }
    if grado >= 4 {
        return Some(format!("{grado} conexiones"));
    }
    let d = desc_de(n);
    if d.chars().count() >= 300 {
        return Some(format!("descripción de {} chars", d.chars().count()));
    }
    None
}

pub fn titulo_de(n: &Value) -> String {
    n["data"]["title"]
        .as_str()
        .or_else(|| n["data"]["label"].as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}
pub fn desc_de(n: &Value) -> String {
    n["data"]["description"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string()
}
pub fn madurez_de(n: &Value) -> i64 {
    n["data"]["maturity"].as_i64().unwrap_or(0)
}
pub fn categoria_de(n: &Value) -> String {
    n["data"]["category"].as_str().unwrap_or("?").to_string()
}
pub fn id_de(n: &Value) -> String {
    n["id"].as_str().unwrap_or("").to_string()
}

/// Normaliza para comparar: minúsculas, sin acentos, sin signos, espacios simples.
pub fn norm(t: &str) -> String {
    let plano: String = t
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c if c.is_ascii_alphanumeric() || c == ' ' => c,
            _ => ' ',
        })
        .collect();
    plano.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Raíz aproximada de una palabra: sin plural y recortada (para que «modelo» y «modelos» sean lo mismo).
fn raiz(p: &str) -> String {
    let p = p.trim_end_matches("es").trim_end_matches('s');
    p.chars().take(6).collect()
}

fn tokens(t: &str) -> Vec<String> {
    norm(t)
        .split_whitespace()
        .filter(|w| w.chars().count() > 2)
        .map(raiz)
        .collect()
}

/// Similitud de Jaccard sobre las raíces de los términos.
pub fn jaccard(a: &str, b: &str) -> f64 {
    let (ta, tb) = (tokens(a), tokens(b));
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    let (sa, sb): (std::collections::BTreeSet<_>, std::collections::BTreeSet<_>) =
        (ta.into_iter().collect(), tb.into_iter().collect());
    let inter = sa.intersection(&sb).count() as f64;
    let union = sa.union(&sb).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

fn grados(aristas: &[Value]) -> std::collections::BTreeMap<String, usize> {
    let mut g: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for e in aristas {
        for lado in ["source", "target"] {
            if let Some(id) = e[lado].as_str() {
                *g.entry(id.to_string()).or_insert(0) += 1;
            }
        }
    }
    g
}

/// Elige a quién conviene conservar de un par: más conexiones, después más texto, después más madurez.
/// Empate: el id más chico (determinista, para que dos corridas propongan lo mismo).
fn mejor(a: &Value, b: &Value, g: &std::collections::BTreeMap<String, usize>) -> (Value, Value) {
    let peso = |n: &Value| {
        (
            *g.get(&id_de(n)).unwrap_or(&0),
            desc_de(n).chars().count(),
            madurez_de(n) as usize,
        )
    };
    let (pa, pb) = (peso(a), peso(b));
    if pa > pb || (pa == pb && id_de(a) <= id_de(b)) {
        (a.clone(), b.clone())
    } else {
        (b.clone(), a.clone())
    }
}

/// Corre todas las reglas mecánicas. Devuelve hallazgos ordenados por confianza (alta primero).
pub fn curar(nodos: &[Value], aristas: &[Value]) -> Vec<Hallazgo> {
    let g = grados(aristas);
    let grado = |n: &Value| *g.get(&id_de(n)).unwrap_or(&0);
    let mut out: Vec<Hallazgo> = Vec::new();
    let mut ya_propuesto: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    // ── 1) Duplicados exactos de título (misma categoría) → fusionar el menor en el mayor ──
    let mut por_clave: std::collections::BTreeMap<(String, String), Vec<&Value>> =
        Default::default();
    for n in nodos {
        let k = (norm(&titulo_de(n)), norm(&categoria_de(n)));
        if k.0.is_empty() {
            continue;
        }
        por_clave.entry(k).or_default().push(n);
    }
    for ((titulo, categoria), grupo) in &por_clave {
        if grupo.len() < 2 {
            continue;
        }
        let mut orden: Vec<&Value> = grupo.clone();
        orden.sort_by_key(|n| {
            std::cmp::Reverse((grado(n), desc_de(n).chars().count(), madurez_de(n)))
        });
        let (destino, resto) = orden.split_at(1);
        let destino = destino[0];
        for origen in resto {
            let motivo = format!(
                "Mismo título que «{}» en la categoría {categoria}: son el mismo concepto. Fusiono para que no haya dos nodos del mismo tema.",
                titulo_de(destino)
            );
            if intocable(origen, grado(origen)).is_none() {
                out.push(Hallazgo {
                    clase: "duplicado",
                    titulo: format!("{} ≈ {}", titulo_de(origen), titulo_de(destino)),
                    motivo: motivo.clone(),
                    evidencia: format!(
                        "título idéntico normalizado «{titulo}» · origen: {} conexiones, {} chars, madurez {} · destino: {} conexiones, {} chars, madurez {}",
                        grado(origen),
                        desc_de(origen).chars().count(),
                        madurez_de(origen),
                        grado(destino),
                        desc_de(destino).chars().count(),
                        madurez_de(destino)
                    ),
                    confianza: "alta",
                    propuesta: fusion(id_de(origen), id_de(destino), &titulo_de(origen), &titulo_de(destino), &motivo),
                });
                ya_propuesto.insert(id_de(origen));
            }
        }
    }

    // ── 2) Casi duplicados: títulos parecidos en la misma categoría y con conexiones bajas ──
    for (i, a) in nodos.iter().enumerate() {
        for b in nodos.iter().skip(i + 1) {
            let (ida, idb) = (id_de(a), id_de(b));
            if ida == idb || ya_propuesto.contains(&ida) || ya_propuesto.contains(&idb) {
                continue;
            }
            if categoria_de(a) != categoria_de(b) {
                continue;
            }
            let sim = jaccard(&titulo_de(a), &titulo_de(b));
            // Se compara el título y también la primera línea de la descripción: dos nodos pueden
            // llamarse distinto y decir lo mismo.
            let sim = sim.max(jaccard(&desc_de(a), &desc_de(b)));
            if sim < 0.62 {
                continue;
            }
            let (destino, origen) = mejor(a, b, &g);
            if let Some(razon) = intocable(&origen, grado(&origen)) {
                // Se declara: hay parecido, pero el candidato a fusionar está protegido. El humano decide.
                out.push(Hallazgo {
                    clase: "casi_duplicado",
                    titulo: format!("{} ≈ {}", titulo_de(&origen), titulo_de(&destino)),
                    motivo: format!(
                        "«{}» y «{}» se parecen un {:.0}% en la categoría {}, pero NO propongo fusionarlos: {razon}.",
                        titulo_de(&origen),
                        titulo_de(&destino),
                        sim * 100.0,
                        categoria_de(a)
                    ),
                    evidencia: format!("similitud {sim:.2} · {} conexiones · {razon}", grado(&origen)),
                    confianza: "media",
                    propuesta: json!({ "tipo": "nada", "payload": {}, "declarado": true }),
                });
                ya_propuesto.insert(ida_del(&origen));
                continue;
            }
            let motivo = format!(
                "El título de «{}» se parece un {:.0}% al de «{}» en la categoría {}: probablemente sea el mismo tema escrito dos veces. Conservo el más conectado y con más texto.",
                titulo_de(&origen),
                sim * 100.0,
                titulo_de(&destino),
                categoria_de(a)
            );
            out.push(Hallazgo {
                clase: "casi_duplicado",
                titulo: format!("{} ≈ {}", titulo_de(&origen), titulo_de(&destino)),
                motivo: motivo.clone(),
                evidencia: format!(
                    "similitud {:.2} · origen: {} conexiones, {} chars, madurez {} · destino: {} conexiones, {} chars, madurez {}",
                    sim,
                    grado(&origen),
                    desc_de(&origen).chars().count(),
                    madurez_de(&origen),
                    grado(&destino),
                    desc_de(&destino).chars().count(),
                    madurez_de(&destino)
                ),
                confianza: "media",
                propuesta: fusion(ida_del(&origen), ida_del(&destino), &titulo_de(&origen), &titulo_de(&destino), &motivo),
            });
            ya_propuesto.insert(ida_del(&origen));
        }
    }

    // ── 3) Título subsumido: el de A está contenido en el de B (misma categoría) → A queda cubierto ──
    for (i, a) in nodos.iter().enumerate() {
        for b in nodos.iter().skip(i + 1) {
            let (ta, tb) = (norm(&titulo_de(a)), norm(&titulo_de(b)));
            if ta.is_empty() || tb.len() <= ta.len() || !tb.contains(&ta) || ta.chars().count() < 6
            {
                continue;
            }
            let ida = id_de(a);
            if ya_propuesto.contains(&ida) || categoria_de(a) != categoria_de(b) {
                continue;
            }
            if intocable(a, grado(a)).is_some() {
                continue;
            }
            let motivo = format!(
                "Todo lo que dice el título de «{}» ya está en el de «{}» (misma categoría {}): el segundo lo cubre. Fusiono para no dejar dos entradas del mismo tema.",
                titulo_de(a),
                titulo_de(b),
                categoria_de(a)
            );
            out.push(Hallazgo {
                clase: "subsumido",
                titulo: format!("{} ⊂ {}", titulo_de(a), titulo_de(b)),
                motivo: motivo.clone(),
                evidencia: format!(
                    "título «{ta}» contenido en «{tb}» · {} conexiones contra {}",
                    grado(a),
                    grado(b)
                ),
                confianza: "media",
                propuesta: fusion(ida.clone(), id_de(b), &titulo_de(a), &titulo_de(b), &motivo),
            });
            ya_propuesto.insert(ida);
        }
    }

    // ── 4) Fragmentos: sin contenido propio y sin conexiones → borrar ──
    for n in nodos {
        let id = id_de(n);
        if ya_propuesto.contains(&id) {
            continue;
        }
        let t = titulo_de(n);
        let d = desc_de(n);
        let vacio = d.chars().count() < 60 || norm(&d) == norm(&t);
        if grado(n) != 0 || !vacio || madurez_de(n) > 2 {
            continue;
        }
        if let Some(razon) = intocable(n, 0) {
            // Se declara: es un fragmento, pero tiene algo que lo protege.
            out.push(Hallazgo {
                clase: "fragmento",
                titulo: t.clone(),
                motivo: format!(
                    "Sin conexiones y sin contenido propio, pero NO lo propongo: {razon}."
                ),
                evidencia: format!(
                    "{} chars de descripción · madurez {}",
                    d.chars().count(),
                    madurez_de(n)
                ),
                confianza: "alta",
                propuesta: json!({ "tipo": "nada", "payload": {}, "declarado": true }),
            });
            continue;
        }
        let motivo = format!(
            "«{t}» no tiene conexiones y su descripción no agrega nada ({} chars, igual al título o casi): no aporta al mapa.",
            d.chars().count()
        );
        out.push(Hallazgo {
            clase: "fragmento",
            titulo: t.clone(),
            motivo: motivo.clone(),
            evidencia: format!(
                "0 conexiones · {} chars · madurez {}",
                d.chars().count(),
                madurez_de(n)
            ),
            confianza: "alta",
            propuesta: json!({
                "tipo": "borrar",
                "payload": { "id": id, "motivo": motivo },
            }),
        });
        ya_propuesto.insert(id);
    }

    // ── 5) Sueltos: sin conexiones pero con contenido real → proponer dónde colgarlos ──
    for n in nodos {
        let id = id_de(n);
        if ya_propuesto.contains(&id) || grado(n) != 0 || desc_de(n).chars().count() < 80 {
            continue;
        }
        // El mejor padre: el nodo con más conexiones de la misma categoría, o el más parecido.
        let mut candidatos: Vec<(&Value, f64)> = nodos
            .iter()
            .filter(|c| id_de(c) != id && grado(c) > 0)
            .map(|c| {
                let sim = jaccard(&desc_de(n), &desc_de(c)) + jaccard(&titulo_de(n), &titulo_de(c));
                let bonus = if categoria_de(c) == categoria_de(n) {
                    0.15
                } else {
                    0.0
                };
                (c, sim + bonus)
            })
            .collect();
        candidatos.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let Some((padre, sim)) = candidatos.first().copied() else {
            continue;
        };
        // Umbral bajo a propósito: la afinidad va en la evidencia y el humano decide. Igual, por debajo
        // de esto no hay ningún parentesco textual que justifique colgarlo de un lado y no de otro.
        if sim < 0.15 {
            continue;
        }
        let motivo = format!(
            "«{}» no está conectado a nada, pero tiene contenido ({} chars). Lo cuelgo de «{}», el nodo más cercano por texto en la categoría {}.",
            titulo_de(n),
            desc_de(n).chars().count(),
            titulo_de(padre),
            categoria_de(n)
        );
        out.push(Hallazgo {
            clase: "suelto",
            titulo: titulo_de(n),
            motivo: motivo.clone(),
            evidencia: format!("0 conexiones · afinidad {sim:.2} con el padre propuesto"),
            confianza: "media",
            propuesta: json!({
                "tipo": "conectar",
                "payload": { "source": id_de(padre), "target": id, "label": "contiene", "motivo": motivo },
            }),
        });
        ya_propuesto.insert(id);
    }

    out.sort_by_key(|h| if h.confianza == "alta" { 0 } else { 1 });
    out
}

fn ida_del(n: &Value) -> String {
    id_de(n)
}

fn fusion(origen: String, destino: String, t_origen: &str, t_destino: &str, motivo: &str) -> Value {
    json!({
        "tipo": "fusionar",
        "payload": {
            "origen": origen,
            "destino": destino,
            "motivo": motivo,
            "titulos": { "origen": t_origen, "destino": t_destino },
        },
    })
}

/// Resumen legible para el panel y para el turno: cuántos por clase, y qué no se tocó.
pub fn resumen(hallazgos: &[Hallazgo]) -> Value {
    let mut por_clase: std::collections::BTreeMap<&str, usize> = Default::default();
    let mut declarados = 0;
    for h in hallazgos {
        if h.propuesta
            .get("declarado")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            declarados += 1;
            continue;
        }
        *por_clase.entry(h.clase).or_default() += 1;
    }
    json!({
        "hallazgos": hallazgos.len(),
        "propuestas": hallazgos.len() - declarados,
        "declarados": declarados,
        "por_clase": por_clase,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn nodo(id: &str, titulo: &str, cat: &str, mad: i64, desc: &str) -> Value {
        json!({
            "id": id,
            "data": { "title": titulo, "label": cat, "category": cat, "maturity": mad, "description": desc, "isRoot": false }
        })
    }
    fn arista(a: &str, b: &str) -> Value {
        json!({ "id": format!("e-{a}-{b}"), "source": a, "target": b })
    }

    #[test]
    fn normaliza_y_compara_raices() {
        assert_eq!(norm("Caché  Semántica!"), "cache semantica");
        assert!(
            jaccard("modelo local", "modelos locales") > 0.9,
            "las raíces igualan plurales"
        );
        assert!(
            jaccard("voz de ida y vuelta", "caché semántica") < 0.2,
            "temas distintos no se parecen"
        );
    }

    #[test]
    fn encuentra_el_duplicado_exacto_y_conserva_el_mejor() {
        let nodos = vec![
            nodo("a", "Caché semántica", "DATOS", 2, "notas"),
            nodo(
                "b",
                "cache SEMANTICA",
                "DATOS",
                1,
                "otra cosa parecida pero distinta de largo",
            ),
        ];
        let aristas = vec![arista("a", "z"), arista("a", "y")];
        let h = curar(&nodos, &aristas);
        assert_eq!(h.len(), 1, "{h:?}");
        assert_eq!(h[0].clase, "duplicado");
        assert_eq!(
            h[0].propuesta["payload"]["origen"], "b",
            "el menos conectado se fusiona"
        );
        assert_eq!(h[0].propuesta["payload"]["destino"], "a");
        assert_eq!(h[0].confianza, "alta");
        assert!(h[0].motivo.contains("Mismo título"));
    }

    #[test]
    fn protege_el_nucleo_lo_maduro_y_lo_bien_conectado() {
        let nodos = vec![
            json!({"id": "root", "data": {"title": "Raíz", "category": "NÚCLEO", "maturity": 1, "description": "", "isRoot": true}}),
            nodo("mad", "Maduro", "DATOS", 5, ""),
            nodo("hub", "Hub", "DATOS", 1, ""),
            nodo("texto", "Con texto", "DATOS", 1, &"x".repeat(400)),
        ];
        let aristas = vec![
            arista("hub", "a"),
            arista("hub", "b"),
            arista("hub", "c"),
            arista("hub", "d"),
        ];
        assert!(intocable(&nodos[0], 0).unwrap().contains("núcleo"));
        assert!(intocable(&nodos[1], 0).unwrap().contains("madurez 5"));
        assert!(intocable(&nodos[2], 4).unwrap().contains("4 conexiones"));
        assert!(intocable(&nodos[3], 0).unwrap().contains("400 chars"));
        assert!(intocable(&nodo("x", "X", "DATOS", 1, "corto"), 0).is_none());
    }

    #[test]
    fn el_fragmento_protegido_se_declara_y_no_se_propone() {
        // El único fragmento que puede estar protegido es el núcleo: sin conexiones y sin texto, pero es
        // la raíz del mapa. El curador lo dice en vez de proponer borrarlo.
        let nodos = vec![json!({
            "id": "root",
            "data": { "title": "Raíz", "category": "NÚCLEO", "maturity": 1, "description": "", "isRoot": true }
        })];
        let h = curar(&nodos, &[]);
        assert_eq!(h.len(), 1);
        assert!(
            h[0].propuesta["declarado"].as_bool().unwrap_or(false),
            "se declara, no se borra: {h:?}"
        );
        assert!(h[0].motivo.contains("sin contenido propio"));
        let r = resumen(&h);
        assert_eq!(r["propuestas"], 0);
        assert_eq!(r["declarados"], 1);
    }

    #[test]
    fn el_fragmento_sin_proteccion_se_propone_borrar() {
        let nodos = vec![nodo("f", "nada", "PENDIENTE", 1, "nada")];
        let h = curar(&nodos, &[]);
        assert_eq!(h[0].clase, "fragmento");
        assert_eq!(h[0].propuesta["tipo"], "borrar");
        assert_eq!(h[0].propuesta["payload"]["id"], "f");
        assert!(h[0].propuesta["payload"]["motivo"]
            .as_str()
            .unwrap()
            .contains("no aporta"));
    }

    #[test]
    fn el_suelto_con_contenido_se_cuelga_del_mas_cercano() {
        // Afinidad medida con la misma regla del curador: 0,27 (por encima del umbral 0,15 y por debajo
        // del 0,62 que lo tomaría como casi-duplicado).
        let nodos = vec![
            nodo(
                "p",
                "Vault en disco",
                "DATOS",
                3,
                "la bóveda es la fuente de verdad del proyecto: guarda las notas y el estado del lienzo en disco",
            ),
            nodo(
                "s",
                "Nodo suelto",
                "DATOS",
                1,
                "las notas del proyecto viven en la bóveda y el lienzo se materializa desde ahí, pero este nodo quedó sin conectar a nada",
            ),
        ];
        let aristas = vec![arista("p", "otro")];
        let h = curar(&nodos, &aristas);
        let suelto = h
            .iter()
            .find(|x| x.clase == "suelto")
            .expect("debe proponer colgarlo");
        assert_eq!(suelto.propuesta["payload"]["source"], "p");
        assert_eq!(suelto.propuesta["payload"]["target"], "s");
        assert_eq!(suelto.propuesta["payload"]["label"], "contiene");
    }

    #[test]
    fn el_casi_duplicado_protegido_se_declara_pero_no_se_propone() {
        // Dos nodos muy parecidos, pero el candidato a fusionar tiene madurez 5: se declara y decide el humano.
        let desc = "investigación sobre modelos locales cuantizados que entren en la placa de doce gigas de memoria de video disponible";
        let nodos = vec![
            nodo(
                "a",
                "Modelo local cuantizado para la placa",
                "INVESTIGACIÓN",
                5,
                desc,
            ),
            nodo(
                "b",
                "Modelo local cuantizado que entra en la placa",
                "INVESTIGACIÓN",
                5,
                desc,
            ),
        ];
        let h = curar(&nodos, &[]);
        assert_eq!(h.len(), 1, "{h:?}");
        assert!(
            h[0].propuesta["declarado"].as_bool().unwrap_or(false),
            "no se propone: {h:?}"
        );
        assert!(
            h[0].motivo.contains("NO propongo"),
            "dice por qué: {}",
            h[0].motivo
        );
        assert!(h[0].motivo.contains("madurez 5"));
        assert_eq!(resumen(&h)["propuestas"], 0);
    }

    #[test]
    fn un_lienzo_limpio_no_propone_nada() {
        let nodos = vec![
            nodo("a", "Uno", "DATOS", 4, &"a".repeat(400)),
            nodo("b", "Dos", "UX", 4, &"b".repeat(400)),
        ];
        let aristas = vec![arista("a", "b")];
        assert!(
            curar(&nodos, &aristas).is_empty(),
            "sin ruido no hay propuestas"
        );
    }

    #[test]
    fn el_resumen_cuenta_por_clase() {
        let nodos = vec![
            nodo(
                "a",
                "Igual",
                "DATOS",
                1,
                "un texto de largo medio para que no lo tome como fragmento (más de sesenta chars)",
            ),
            nodo(
                "b",
                "igual",
                "DATOS",
                1,
                "otro texto distinto, también largo, para el segundo del par duplicado del caso",
            ),
            nodo("f", "nada", "PENDIENTE", 1, "nada"),
        ];
        let h = curar(&nodos, &[]);
        let r = resumen(&h);
        assert_eq!(r["hallazgos"], 2);
        assert_eq!(r["por_clase"]["duplicado"], 1);
        assert_eq!(r["por_clase"]["fragmento"], 1);
    }
}

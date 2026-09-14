//! Planilla de evaluación — **medir en vez de suponer**.
//!
//! Corre las tareas REALES de NodeFlow contra cada motor y verifica el resultado **en código**: no hay
//! juicio de un modelo sobre otro modelo. Lo que se mide es lo que el usuario vive: ¿el plan sirvió?,
//! ¿cuánto tardó?, ¿cuánto costó?
//!
//! Cada prueba usa el mismo camino que la app (`POST /api/ai/action`): el spec, la validación, la
//! caché y el costo son los reales. Y es **de sólo lectura**: `/api/ai/action` propone, nunca aplica
//! al lienzo (aplicar es del frontend, con aprobación).

use crate::server::{AppState, API_PORT};
use serde_json::{json, Value};
use std::path::Path;

/// Una prueba de la planilla: entrada fija y veredicto verificable.
pub struct Prueba {
    pub id: &'static str,
    pub titulo: &'static str,
    /// Acción del spec (`voz`, `condensar`, `braindump`).
    pub accion: &'static str,
    /// Lo que se le pide, tal como lo diría el usuario.
    pub entrada: &'static str,
}

/// Resultado de mirar una salida.
pub struct Veredicto {
    pub ok: bool,
    pub detalle: String,
}

fn si(d: impl Into<String>) -> Veredicto {
    Veredicto { ok: true, detalle: d.into() }
}
fn no(d: impl Into<String>) -> Veredicto {
    Veredicto { ok: false, detalle: d.into() }
}

/// Las pruebas. Son cinco porque cinco alcanzan para ver el perfil de un motor, y porque cada corrida
/// cuesta tiempo real del usuario.
pub fn pruebas() -> Vec<Prueba> {
    vec![
        Prueba {
            id: "voz-enfocar",
            titulo: "Entiende una orden de limpieza",
            accion: "voz",
            entrada: "limpiá el lienzo y dejá sólo lo que se conecta con el copiloto espacial",
        },
        Prueba {
            id: "voz-crear",
            titulo: "Suma ideas sin tocar lo que ya hay",
            accion: "voz",
            entrada: "sumá dos ideas nuevas: el ruteo por tarea y la planilla de evaluación",
        },
        Prueba {
            id: "voz-delegar",
            titulo: "Reconoce cuándo hace falta el motor profundo",
            accion: "voz",
            entrada: "averiguá en la web si el sensor SHT31 sigue fabricándose y decime alternativas",
        },
        Prueba {
            id: "condensar",
            titulo: "Condensa sin perder el contrato",
            accion: "condensar",
            entrada: "Cerrar el bucle de voz del copiloto espacial",
        },
        Prueba {
            id: "braindump",
            titulo: "Descompone una idea en un mapa",
            accion: "braindump",
            entrada: "Quiero un sistema de riego para la huerta que avise al celular cuando falte agua, \
                      mida la humedad del suelo y no dependa de internet.",
        },
    ]
}

/// El veredicto de una prueba. **Función pura**: se testea sin llamar a ningún modelo.
pub fn evaluar(id: &str, s: &Value) -> Veredicto {
    match id {
        "voz-enfocar" => {
            let cs = s["comandos"].as_array().cloned().unwrap_or_default();
            if cs.len() != 1 {
                return no(format!("devolvió {} comandos (se esperaba 1)", cs.len()));
            }
            if cs[0]["accion"] != "enfocar" {
                return no(format!("en vez de enfocar, propuso «{}»", cs[0]["accion"]));
            }
            let n = cs[0]["nodos"].as_array().map(|a| a.len()).unwrap_or(0);
            if !(1..=10).contains(&n) {
                return no(format!("enfocó {n} nodos (fuera de 1 a 10)"));
            }
            si(format!("enfocar {n} nodos"))
        }
        "voz-crear" => {
            let cs = s["comandos"].as_array().cloned().unwrap_or_default();
            if cs.iter().any(|c| c["accion"] == "enfocar") {
                return no("propuso enfocar cuando sólo se pedían ideas nuevas".to_string());
            }
            let creados: Vec<&Value> = cs.iter().filter(|c| c["accion"] == "crear").collect();
            if creados.is_empty() {
                return no("no creó ningún nodo".to_string());
            }
            if creados.iter().any(|c| c["titulo"].as_str().unwrap_or("").trim().len() < 3) {
                return no("creó un nodo sin título usable".to_string());
            }
            si(format!("creó {} nodos con título", creados.len()))
        }
        "voz-delegar" => {
            let cs = s["comandos"].as_array().cloned().unwrap_or_default();
            if cs.len() != 1 {
                return no(format!("devolvió {} comandos (se esperaba 1 delegación)", cs.len()));
            }
            if cs[0]["accion"] != "delegar" {
                return no(format!("no delegó: propuso «{}»", cs[0]["accion"]));
            }
            let pedido = cs[0]["pedido"].as_str().unwrap_or("").trim();
            if pedido.len() < 20 {
                return no(format!("el pedido quedó corto ({} caracteres)", pedido.len()));
            }
            si("delegó con el pedido completo")
        }
        "condensar" => {
            let titulo = s["title"].as_str().unwrap_or("").trim();
            if titulo.is_empty() {
                return no("macro-concepto sin título".to_string());
            }
            let desc = s["description"].as_str().unwrap_or("").trim();
            if desc.chars().count() < 30 {
                return no("descripción demasiado corta".to_string());
            }
            match s["match"].as_f64() {
                Some(m) if (0.0..=1.0).contains(&m) => si(format!("«{}» con match {m:.2}", recorta(titulo, 40))),
                Some(m) => no(format!("match fuera de 0 a 1: {m}")),
                None => no("sin match numérico".to_string()),
            }
        }
        "braindump" => {
            let raiz = s["root"]["title"].as_str().unwrap_or("").trim();
            if raiz.is_empty() {
                return no("sin idea nuclear".to_string());
            }
            let nodos = s["nodes"].as_array().cloned().unwrap_or_default();
            let con_titulo = nodos
                .iter()
                .filter(|n| n["title"].as_str().unwrap_or("").trim().len() >= 3)
                .count();
            if con_titulo < 3 {
                return no(format!("sólo {con_titulo} nodos con título (se esperaban 3 o más)"));
            }
            si(format!("«{}» + {con_titulo} nodos", recorta(raiz, 34)))
        }
        _ => no(format!("prueba desconocida: {id}")),
    }
}

fn recorta(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

/// Base del daemon local (sin el `/v1` que usa el camino compatible con OpenAI).
pub fn cat_local_base_url(_id: &str) -> String {
    std::env::var("NODEFLOW_OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string())
}

/// ¿Es multimodal? Un modelo de visión ocupa mucha VRAM y no aporta nada a estas pruebas: por
/// defecto la planilla lo saltea (en esta máquina el VL de 7b pesa 5,8 GB).
pub fn es_multimodal(modelo: &str) -> bool {
    let n = modelo.to_lowercase();
    n.contains("vl") || n.contains("vision") || n.contains("llava")
}

/// Arma el cuerpo de la petición para una prueba (mismo contrato que usa la app).
fn cuerpo(prueba: &Prueba, lienzo: &[Value]) -> Value {
    match prueba.accion {
        "voz" => json!({ "type": "voz", "texto": prueba.entrada, "sin_cache": true }),
        "braindump" => json!({ "type": "braindump", "rawText": prueba.entrada, "sin_cache": true }),
        "condensar" => {
            let muestra: Vec<Value> = lienzo.iter().take(4).cloned().collect();
            json!({ "type": "condensar", "objetivo": prueba.entrada, "selectedNodes": muestra, "sin_cache": true })
        }
        _ => json!({ "type": prueba.accion }),
    }
}

/// Corre la planilla completa sobre los motores indicados. Devuelve la tabla y la deja guardada.
///
/// La selección global se cambia prueba por prueba (así el ruteo no mete la cola) y **se restaura**
/// al terminar, pase lo que pase.
pub async fn correr(st: &AppState, modelos: Vec<String>) -> Value {
    let bandera = st.data_dir.join("evaluacion.corriendo");
    let _ = std::fs::write(&bandera, "1");
    // La elección del usuario se guarda APARTE antes de tocar nada: si una corrida se corta a mitad,
    // la próxima la recupera en vez de quedarse con el motor que la planilla estaba midiendo.
    let guardada = st.data_dir.join("evaluacion.seleccion.json");
    if let Some(previa) = std::fs::read_to_string(&guardada)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["seleccion"].as_str().map(String::from))
    {
        let _ = crate::motores::guardar_seleccion(&st.data_dir, Some(&previa));
        let _ = std::fs::remove_file(&guardada);
        log::info!("evaluación: restaurada la elección del usuario que quedó de una corrida cortada");
    }
    let anterior = crate::motores::seleccionado(&st.data_dir);
    let _ = std::fs::write(
        &guardada,
        serde_json::to_string(&json!({ "seleccion": anterior })).unwrap_or_default(),
    );
    let lienzo: Vec<Value> = st
        .vault
        .read_state()
        .unwrap_or(json!({}))["nodes"]
        .as_array()
        .map(|ns| {
            ns.iter()
                .map(|n| {
                    json!({
                        "id": n["id"],
                        "title": n["data"]["title"],
                        "description": n["data"]["description"],
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let url = format!("http://127.0.0.1:{}/api/ai/action", API_PORT);
    let mut filas: Vec<Value> = Vec::new();
    let mut por_motor: Vec<Value> = Vec::new();

    for id in modelos {
        let _ = crate::motores::guardar_seleccion(&st.data_dir, Some(&id));
        let mut pruebas_json: Vec<Value> = Vec::new();
        let (mut aciertos, mut ms_total, mut tokens_total, mut costo_total) = (0u64, 0u64, 0u64, 0.0f64);

        for p in pruebas() {
            let t0 = std::time::Instant::now();
            let respuesta = st
                .http
                .post(&url)
                .json(&cuerpo(&p, &lienzo))
                .timeout(std::time::Duration::from_secs(180))
                .send()
                .await;
            let ms = t0.elapsed().as_millis() as u64;
            let (veredicto, tokens, costo, modelo_real, cache) = match respuesta {
                Ok(r) => match r.json::<Value>().await {
                    Ok(d) => {
                        let uso = d["uso"].clone();
                        // Cada acción devuelve su payload en su propia clave (`voz`, `condensar`)…
                        // y el braindump lo anida en `structure`. Leer de la raíz daba falsos fallos.
                        let salida = if p.accion == "braindump" {
                            d["structure"].clone()
                        } else if d[&p.accion].is_null() {
                            d.clone()
                        } else {
                            d[&p.accion].clone()
                        };
                        (
                            evaluar(p.id, &salida),
                            uso["tokens"]["total"].as_u64().unwrap_or(0),
                            uso["costo_usd"].as_f64().unwrap_or(0.0),
                            uso["modelo"].as_str().unwrap_or("").to_string(),
                            uso["cache"].as_str().unwrap_or("?").to_string(),
                        )
                    }
                    Err(e) => (no(format!("respuesta ilegible: {e}")), 0, 0.0, String::new(), "?".into()),
                },
                Err(e) => (no(format!("sin respuesta: {e}")), 0, 0.0, String::new(), "?".into()),
            };
            if veredicto.ok {
                aciertos += 1;
            }
            ms_total += ms;
            tokens_total += tokens;
            costo_total += costo;
            log::info!(
                "evaluación · {} · {} · {} · {} ms",
                id,
                p.id,
                if veredicto.ok { "OK" } else { "falló" },
                ms
            );
            pruebas_json.push(json!({
                "id": p.id,
                "titulo": p.titulo,
                "ok": veredicto.ok,
                "detalle": veredicto.detalle,
                "ms": ms,
                "tokens": tokens,
                "costo_usd": costo,
                "modelo": modelo_real,
                "cache": cache,
            }));
        }

        // Descargar el modelo de la VRAM al terminar: medir no puede dejar la placa cargada.
        if id.starts_with("ollama:") {
            let base = cat_local_base_url(&id);
            let modelo = id.trim_start_matches("ollama:");
            let _ = st
                .http
                .post(format!("{base}/api/generate"))
                .json(&json!({ "model": modelo, "keep_alive": 0 }))
                .timeout(std::time::Duration::from_secs(30))
                .send()
                .await;
            log::info!("evaluación: {modelo} descargado de la VRAM");
        }

        let total = pruebas_json.len() as u64;
        por_motor.push(json!({
            "id": id,
            "aciertos": aciertos,
            "total": total,
            "ms_medio": if total > 0 { ms_total / total } else { 0 },
            "ms_total": ms_total,
            "tokens": tokens_total,
            "costo_usd": costo_total,
            "pruebas": pruebas_json,
        }));
        filas.push(json!({ "motor": id, "aciertos": aciertos, "total": total }));
    }

    // La selección del usuario vuelve como estaba: medir no tiene que cambiarle nada.
    let _ = crate::motores::guardar_seleccion(&st.data_dir, anterior.as_deref());
    let _ = std::fs::remove_file(&guardada);

    // Ganador por prueba, en aciertos y —a igualdad— en velocidad. Esto es lo que después puede
    // alimentar el ruteo: la evidencia, no la corazonada.
    let mut ganadores = serde_json::Map::new();
    for p in pruebas() {
        // (motor, ms, ok) — en este orden, y comparando lo que corresponde: aciertos primero y,
        // a igualdad, el más rápido. La versión anterior cruzaba posiciones (comparaba ms contra
        // tokens) y elegía mal; ahora los tests lo cubren.
        let mut mejor: Option<(String, u64, bool, u64)> = None;
        for m in &por_motor {
            let id = m["id"].as_str().unwrap_or("").to_string();
            if let Some(f) = m["pruebas"]
                .as_array()
                .and_then(|fs| fs.iter().find(|f| f["id"] == p.id))
            {
                let ok = f["ok"].as_bool().unwrap_or(false);
                let ms = f["ms"].as_u64().unwrap_or(u64::MAX);
                let tok = f["tokens"].as_u64().unwrap_or(0);
                let gana = match &mejor {
                    None => true,
                    Some((_, ms_mejor, ok_mejor, _)) => {
                        (ok && !*ok_mejor) || (ok == *ok_mejor && ms < *ms_mejor)
                    }
                };
                if gana {
                    mejor = Some((id, ms, ok, tok));
                }
            }
        }
        if let Some((id, ms, ok, tok)) = mejor {
            ganadores.insert(
                p.id.to_string(),
                json!({ "motor": id, "ms": ms, "ok": ok, "tokens": tok }),
            );
        }
    }

    let _ = std::fs::remove_file(&bandera);
    let tabla = json!({
        "cuando": fecha_iso(),
        "lienzo": { "nodos": lienzo.len() },
        "motores": por_motor,
        "ganador_por_prueba": ganadores,
    });
    let _ = guardar(st, &tabla);
    tabla
}

fn fecha_iso() -> String {
    // Sin dependencias de fecha: alcanza para ordenar corridas.
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{d}")
}

/// Guarda la última planilla y su versión legible en la carpeta de datos de la app.
pub fn guardar(st: &AppState, tabla: &Value) -> Result<(), String> {
    let txt = serde_json::to_string_pretty(tabla).map_err(|e| e.to_string())?;
    std::fs::write(st.data_dir.join("evaluacion.json"), txt).map_err(|e| e.to_string())?;
    let md = a_markdown(tabla);
    std::fs::write(st.data_dir.join("EVALUACION.md"), md).map_err(|e| e.to_string())
}

/// ¿Hay una corrida en curso? La app no bloquea la ventana mientras mide.
pub fn en_curso(data_dir: &Path) -> bool {
    data_dir.join("evaluacion.corriendo").exists()
}

/// Lee la última planilla guardada (para mostrarla sin volver a correr nada).
pub fn leer(st: &AppState) -> Option<Value> {
    std::fs::read_to_string(st.data_dir.join("evaluacion.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
}

/// La planilla como tabla markdown, para el repo y la bóveda.
pub fn a_markdown(tabla: &Value) -> String {
    let mut out = String::from("| Motor | Aciertos | Tiempo medio | Tokens | Costo |\n|---|---|---|---|---|\n");
    for m in tabla["motores"].as_array().cloned().unwrap_or_default() {
        let costo = m["costo_usd"].as_f64().unwrap_or(0.0);
        out.push_str(&format!(
            "| `{}` | {}/{} | {:.1} s | {} | {} |\n",
            m["id"].as_str().unwrap_or(""),
            m["aciertos"].as_u64().unwrap_or(0),
            m["total"].as_u64().unwrap_or(0),
            m["ms_medio"].as_u64().unwrap_or(0) as f64 / 1000.0,
            m["tokens"].as_u64().unwrap_or(0),
            if costo > 0.0 { format!("US${costo:.6}") } else { "US$0".to_string() },
        ));
    }
    out.push_str("\n### Ganador por prueba\n\n| Prueba | Motor | Tiempo |\n|---|---|---|\n");
    for (k, v) in tabla["ganador_por_prueba"].as_object().cloned().unwrap_or_default() {
        out.push_str(&format!(
            "| `{}` | `{}` | {:.1} s |\n",
            k,
            v["motor"].as_str().unwrap_or(""),
            v["ms"].as_u64().unwrap_or(0) as f64 / 1000.0
        ));
    }
    out
}

#[cfg(test)]
mod tests_planilla {
    use super::*;

    #[test]
    fn enfocar_bien_y_mal() {
        let bien = json!({"comandos": [{"accion": "enfocar", "nodos": ["a", "b", "c"]}]});
        assert!(evaluar("voz-enfocar", &bien).ok);
        let dos = json!({"comandos": [{"accion": "enfocar", "nodos": []}, {"accion": "enfocar", "nodos": ["a"]}]});
        assert!(!evaluar("voz-enfocar", &dos).ok, "dos comandos no es una orden de limpieza");
        let condensar_todo = json!({"comandos": [{"accion": "condensar", "nodos": ["a", "b"]}]});
        assert!(!evaluar("voz-enfocar", &condensar_todo).ok, "no enfocar es fallar la prueba");
    }

    #[test]
    fn crear_no_debe_enfocar() {
        assert!(evaluar("voz-crear", &json!({"comandos": [{"accion": "crear", "titulo": "Ruteo por tarea"}]})).ok);
        let se_paso_de_largo = json!({"comandos": [{"accion": "crear", "titulo": "X idea"}, {"accion": "enfocar", "nodos": ["a"]}]});
        assert!(!evaluar("voz-crear", &se_paso_de_largo).ok, "si enfoca, destruyó el lienzo sin que se lo pidan");
    }

    #[test]
    fn delegar_necesita_pedido_completo() {
        assert!(evaluar("voz-delegar", &json!({"comandos": [{"accion": "delegar", "pedido": "averiguá si el SHT31 sigue fabricándose"}]})).ok);
        assert!(!evaluar("voz-delegar", &json!({"comandos": [{"accion": "delegar", "pedido": "buscá"}]})).ok);
        assert!(!evaluar("voz-delegar", &json!({"comandos": [{"accion": "crear", "titulo": "Sensores"}]})).ok);
    }

    #[test]
    fn condensar_necesita_contrato_completo() {
        let ok = json!({"title": "Copiloto de voz local", "description": "Cierra el bucle hablado con Kokoro y Speechmatics sin salir de la máquina.", "match": 0.9});
        assert!(evaluar("condensar", &ok).ok);
        assert!(!evaluar("condensar", &json!({"title": "X", "description": "corta", "match": 0.9})).ok);
        assert!(!evaluar("condensar", &json!({"title": "X", "description": "una descripción suficientemente larga para pasar", "match": 92})).ok, "match 92 no es 0..1");
    }

    #[test]
    fn braindump_necesita_raiz_y_ramas() {
        let ok = json!({"root": {"title": "Riego que avisa"}, "nodes": [{"title": "Humedad de suelo"}, {"title": "Aviso al celular"}, {"title": "Sin internet"}]});
        assert!(evaluar("braindump", &ok).ok);
        assert!(!evaluar("braindump", &json!({"root": {"title": "Riego"}, "nodes": [{"title": "Una"}]})).ok);
    }

    /// El error que este test evita: comparar posiciones cruzadas elegía al más lento.
    #[test]
    fn entre_dos_que_aciertan_gana_el_mas_rapido() {
        let motores = vec![
            json!({"id": "lento", "pruebas": [{"id": "voz-enfocar", "ok": true, "ms": 22400, "tokens": 4000}]}),
            json!({"id": "rapido", "pruebas": [{"id": "voz-enfocar", "ok": true, "ms": 9400, "tokens": 8308}]}),
        ];
        let mut mejor: Option<(String, u64, bool)> = None;
        for m in &motores {
            let f = &m["pruebas"][0];
            let (ok, ms) = (f["ok"].as_bool().unwrap(), f["ms"].as_u64().unwrap());
            let gana = match &mejor {
                None => true,
                Some((_, ms_mejor, ok_mejor)) => (ok && !*ok_mejor) || (ok == *ok_mejor && ms < *ms_mejor),
            };
            if gana {
                mejor = Some((m["id"].as_str().unwrap().to_string(), ms, ok));
            }
        }
        assert_eq!(mejor.unwrap().0, "rapido");
    }

    #[test]
    fn la_tabla_se_exporta_a_markdown() {
        let t = json!({
            "motores": [{"id": "ollama:granite3.3:2b", "aciertos": 4, "total": 5, "ms_medio": 2400, "tokens": 8000, "costo_usd": 0.0}],
            "ganador_por_prueba": {"voz-enfocar": {"motor": "ollama:granite3.3:2b", "ms": 2400}}
        });
        let md = a_markdown(&t);
        assert!(md.contains("granite3.3:2b") && md.contains("4/5") && md.contains("US$0"));
        assert!(md.contains("voz-enfocar"));
    }
}

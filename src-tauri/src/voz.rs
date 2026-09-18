//! Voz → comandos sobre el lienzo.
//!
//! El usuario habla; un motor de razonamiento propone un PLAN de operaciones (crear nodos,
//! enlazarlos con los que ya existen, enfocar el lienzo en una idea, condensar el resto,
//! criticarlo). Acá NO se ejecuta nada: se VALIDA el plan contra el lienzo real.
//!
//! Regla de la casa (ADR 0005): el modelo propone, el código valida.
//! - Sólo acciones de `ACCIONES`.
//! - Sólo ids que existan de verdad en el lienzo (si no, se descartan).
//! - Topes de comandos y de nodos por comando, para que un dictado no arrase con todo.
//! El frontend recibe el plan limpio y **lo muestra antes de aplicar**: vos aprobás.

use serde_json::{json, Value};

/// Lo que la voz puede pedir. Todo lo demás se descarta.
pub const ACCIONES: [&str; 10] = [
    "crear",
    "enlazar",
    "enfocar",
    "condensar",
    "criticar",
    "delegar",
    "actualizar",
    // El ciclo del pensamiento también se opera hablando: cerrar una pregunta y decidir qué queda.
    "responder",
    "aceptar",
    "descartar",
];

/// Hermes como motor profundo de NodeFlow: cuando el pedido necesita lo que el modelo local no
/// tiene (buscar en la web, leer un repo, razonar largo), el plan trae un `delegar` y el backend
/// corre una pasada completa de Hermes con **sus** herramientas. La ruta es configurable
/// (`NODEFLOW_HERMES`) para no depender de un único lugar de instalación.
pub fn hermes_exe() -> String {
    std::env::var("NODEFLOW_HERMES").unwrap_or_else(|_| {
        let candidato = r"C:\Users\tomas\AppData\Local\hermes\hermes-agent\venv\Scripts\hermes.exe";
        if std::path::Path::new(candidato).exists() {
            candidato.to_string()
        } else {
            "hermes".to_string()
        }
    })
}

/// Prompt que se le manda a Hermes: contexto del lienzo + el pedido, y una respuesta corta
/// (se muestra en el panel y puede volverse un nodo).
/// Tope de operaciones por dictado. Medido: un pedido de "dejá sólo lo que se conecta con X"
/// llegó como 5 condensaciones y colapsó media lienzo, cuando la intención era UNA operación.
/// Una frase son pocas operaciones; si el motor propone más, no se aplica el excedente.
pub const MAX_COMANDOS: usize = 4;
/// Tope de nodos por comando (evita "limpiá todo" por accidente).
pub const MAX_NODOS: usize = 30;

/// Servidor de voz local (Kokoro TTS). Corre en la máquina del usuario: sin cuotas ni red externa.
/// Se puede mover con NODEFLOW_TTS_URL.
pub fn tts_url() -> String {
    std::env::var("NODEFLOW_TTS_URL").unwrap_or_else(|_| "http://127.0.0.1:8125".to_string())
}

/// ¿Esta respuesta merece voz?
///
/// La regla es la del manifiesto: la IA **actúa primero y habla sólo cuando aporta**. Crear o
/// enlazar nodos es visible y evidente (silencio); en cambio un cambio estructural —enfocar el
/// lienzo, condensar, cuestionar— o haber descartado algo que el motor propuso a medias son
/// hallazgos: eso se dice, en una frase.
pub fn debe_hablar(plan: &Value) -> bool {
    if plan["descartados"].as_u64().unwrap_or(0) > 0 {
        return true;
    }
    plan["comandos"]
        .as_array()
        .map(|cs| {
            cs.iter().any(|c| {
                matches!(
                    c["accion"].as_str().unwrap_or(""),
                    // Estructurales + los cierres del ciclo: responder una pregunta y decidir son
                    // hallazgos, se dicen. Crear o enlazar se ven en el lienzo: silencio.
                    "enfocar" | "condensar" | "criticar" | "responder" | "aceptar" | "descartar"
                )
            })
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests_voz_selectiva {
    use super::debe_hablar;
    use serde_json::json;

    #[test]
    fn actualizar_muta_un_nodo_que_existe() {
        let ids = vec!["n-1".to_string()];
        let plan = json!({"intencion": "comando", "comandos": [
            {"accion": "actualizar", "nodo": "n-1", "descripcion": "Ya pasó a fase de síntesis.", "maturity": 3}
        ]});
        let limpio = super::validar(&plan, &ids);
        let c = &limpio["comandos"][0];
        assert_eq!(c["accion"], "actualizar");
        assert_eq!(c["nodo"], "n-1");
        assert_eq!(c["maturity"], 3);
        assert!(c["descripcion"].as_str().unwrap().contains("síntesis"));
    }

    #[test]
    fn actualizar_rechaza_lo_que_no_sirve() {
        let ids = vec!["n-1".to_string()];
        // nodo inexistente
        let fuera = json!({"comandos": [{"accion": "actualizar", "nodo": "n-999", "titulo": "x"}]});
        assert_eq!(
            super::validar(&fuera, &ids)["comandos"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        // sin campos
        let vacio = json!({"comandos": [{"accion": "actualizar", "nodo": "n-1"}]});
        assert_eq!(
            super::validar(&vacio, &ids)["comandos"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        // maturity fuera de rango se acota, no se descarta
        let raro = json!({"comandos": [{"accion": "actualizar", "nodo": "n-1", "maturity": 99}]});
        assert_eq!(super::validar(&raro, &ids)["comandos"][0]["maturity"], 5);
    }

    #[test]
    fn delegar_solo_una_vez_por_plan() {
        let plan = json!({"intencion": "comando", "comandos": [
            {"accion": "delegar", "pedido": "buscá en la web precios de sensores de humedad"},
            {"accion": "delegar", "pedido": "y también compará con otro proveedor"}
        ]});
        let limpio = super::validar(&plan, &[]);
        assert_eq!(
            limpio["comandos"].as_array().unwrap().len(),
            1,
            "el segundo delegar no debe pasar"
        );
        assert_eq!(
            limpio["descartados"].as_u64().unwrap(),
            1,
            "debe quedar 1 descarte"
        );
        assert_eq!(limpio["motivo_descarte"].as_array().unwrap().len(), 1);
        assert!(limpio["motivo_descarte"][0]
            .as_str()
            .unwrap()
            .contains("delegar"));
    }

    #[test]
    fn delegar_sin_pedido_se_descarta() {
        let plan =
            json!({"intencion": "comando", "comandos": [{"accion": "delegar", "pedido": "ab"}]});
        let limpio = super::validar(&plan, &[]);
        assert_eq!(limpio["comandos"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn delegar_conserva_el_pedido() {
        let plan = json!({"intencion": "comando", "comandos": [
            {"accion": "delegar", "pedido": "averiguá si el sensor SHT31 está discontinuado"}
        ]});
        let limpio = super::validar(&plan, &[]);
        let c = &limpio["comandos"][0];
        assert_eq!(c["accion"], "delegar");
        assert!(c["pedido"].as_str().unwrap().contains("SHT31"));
    }

    #[test]
    fn responder_necesita_una_pregunta_del_lienzo_y_su_texto() {
        let ids = vec!["p-1".to_string()];
        let ok = json!({"comandos": [{"accion": "responder", "nodo": "p-1", "respuesta": "lo probé"}]});
        let out = super::validar(&ok, &ids);
        assert_eq!(out["comandos"][0]["nodo"], json!("p-1"));
        assert_eq!(out["comandos"][0]["respuesta"], json!("lo probé"));
        // id inventado
        let fantasma = json!({"comandos": [{"accion": "responder", "nodo": "p-9", "respuesta": "x"}]});
        assert_eq!(super::validar(&fantasma, &ids)["comandos"].as_array().unwrap().len(), 0);
        // sin texto
        let mudo = json!({"comandos": [{"accion": "responder", "nodo": "p-1", "respuesta": " "}]});
        assert_eq!(super::validar(&mudo, &ids)["comandos"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn aceptar_y_descartar_toman_ids_validos_y_no_borran_nada() {
        let ids = vec!["n-1".to_string(), "n-2".to_string()];
        for accion in ["aceptar", "descartar"] {
            let plan = json!({"comandos": [{"accion": accion, "nodos": ["n-1", "fantasma"]}]});
            let out = super::validar(&plan, &ids);
            assert_eq!(out["comandos"][0]["nodos"], json!(["n-1"]), "{accion}");
            // Ninguna acción nueva se llama «borrar»: el validador no tiene esa palabra.
            assert!(!out.to_string().contains("borrar"));
        }
        // con un solo nodo alcanza (condensar necesita 2; decidir, no)
        let uno = json!({"comandos": [{"accion": "descartar", "nodos": ["n-1"]}]});
        assert_eq!(super::validar(&uno, &ids)["comandos"][0]["nodos"], json!(["n-1"]));
        // sin ids válidos, se cae
        let nada = json!({"comandos": [{"accion": "aceptar", "nodos": ["x", "y"]}]});
        assert_eq!(super::validar(&nada, &ids)["comandos"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn los_cierres_del_ciclo_hablan() {
        for accion in ["responder", "aceptar", "descartar"] {
            assert!(super::debe_hablar(&json!({"comandos": [{"accion": accion}]})), "{accion}");
        }
    }

    #[test]
    fn crear_y_enlazar_son_silencio() {
        // Acción obvia: se ve en el lienzo, no hace falta narrarla.
        let plan =
            json!({"comandos": [{"accion": "crear", "titulo": "Sensor"}, {"accion": "enlazar"}]});
        assert!(!debe_hablar(&plan));
    }

    #[test]
    fn cambio_estructural_habla() {
        for accion in ["enfocar", "condensar", "criticar"] {
            let plan = json!({"comandos": [{"accion": accion}]});
            assert!(debe_hablar(&plan), "{accion} debería hablar");
        }
    }

    #[test]
    fn descartar_algo_habla() {
        let plan = json!({"comandos": [{"accion": "crear", "titulo": "X"}], "descartados": 2});
        assert!(debe_hablar(&plan));
    }

    #[test]
    fn sin_comandos_no_habla() {
        assert!(!debe_hablar(&json!({"comandos": []})));
        assert!(!debe_hablar(&json!({})));
    }
}

/// ¿Hay una investigación del motor profundo en curso?
pub fn delegacion_en_curso(data_dir: &std::path::Path) -> bool {
    data_dir.join("delegacion.corriendo").exists()
}

/// Guarda el resultado de una investigación del motor profundo (y baja la bandera de "en curso").
pub fn guardar_delegacion(
    data_dir: &std::path::Path,
    pedido: &str,
    ok: bool,
    texto: &str,
    ms: u64,
    contexto: &str,
) -> Result<(), String> {
    let v = serde_json::json!({
        "pedido": pedido,
        "ok": ok,
        "salida": texto,
        "ms": ms,
        // Qué contexto se le mandó al agente: el panel lo puede mostrar (no es una caja negra).
        "contexto": contexto,
        "cuando": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    });
    std::fs::write(
        data_dir.join("delegacion.json"),
        serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(data_dir.join("delegacion.corriendo"));
    Ok(())
}

/// La última investigación terminada (o `null` si nunca hubo).
pub fn leer_delegacion(data_dir: &std::path::Path) -> Value {
    std::fs::read_to_string(data_dir.join("delegacion.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null)
}

/// Ajustes del servicio de voz (endpoint, modelo, idioma). Por entorno, para poder cambiar de
/// región o de idioma sin recompilar: NODEFLOW_VOZ_URL / NODEFLOW_VOZ_MODELO / NODEFLOW_VOZ_IDIOMA.
pub fn ajustes() -> (String, String, String) {
    let url = std::env::var("NODEFLOW_VOZ_URL")
        .unwrap_or_else(|_| "wss://eu.rt.speechmatics.com/v2".to_string());
    let modelo = std::env::var("NODEFLOW_VOZ_MODELO").unwrap_or_else(|_| "enhanced".to_string());
    let idioma = std::env::var("NODEFLOW_VOZ_IDIOMA").unwrap_or_else(|_| "es".to_string());
    (url, modelo, idioma)
}

fn ids_del_comando(c: &Value) -> Vec<String> {
    c["nodos"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn recorta(v: &str, n: usize) -> String {
    v.chars().take(n).collect::<String>().trim().to_string()
}

/// Valida el plan del motor contra los ids reales del lienzo.
/// Devuelve el plan limpio (con `descartados` y `motivo_descarte`) listo para mostrar y aplicar.
pub fn validar(plan: &Value, ids_validos: &[String]) -> Value {
    let existe = |id: &str| ids_validos.iter().any(|x| x == id);
    let intencion = match plan["intencion"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_lowercase()
        .as_str()
    {
        "capturar" => "capturar",
        "comando" => "comando",
        _ => "comando",
    };

    let mut limpios: Vec<Value> = Vec::new();
    let mut descartados: Vec<String> = Vec::new();

    for c in plan["comandos"].as_array().cloned().unwrap_or_default() {
        if limpios.len() >= MAX_COMANDOS {
            descartados.push(format!("se superó el tope de {MAX_COMANDOS} comandos"));
            break;
        }
        let accion = c["accion"].as_str().unwrap_or("").trim().to_lowercase();
        if !ACCIONES.contains(&accion.as_str()) {
            descartados.push(format!("acción no permitida: «{}»", recorta(&accion, 24)));
            continue;
        }

        let mut limpio = json!({ "accion": accion });
        match accion.as_str() {
            "delegar" => {
                // Un `delegar` por plan: es la operación cara (una pasada completa del motor profundo).
                if limpios.iter().any(|c| c["accion"] == "delegar") {
                    descartados.push(
                        "un segundo «delegar» en el mismo plan (es la operación cara)".into(),
                    );
                    continue;
                }
                let pedido = recorta(c["pedido"].as_str().unwrap_or(""), 600);
                if pedido.chars().count() < 4 {
                    descartados.push("un «delegar» sin pedido".into());
                    continue;
                }
                limpio["pedido"] = json!(pedido);
            }
            "actualizar" => {
                // Mutar un nodo que YA existe: es lo que hace posible la evolución por fases (el nodo
                // de investigación cambia de estado sin crear otro con lo mismo).
                let nodo = c["nodo"].as_str().unwrap_or("").trim().to_string();
                if nodo.is_empty() || !existe(&nodo) {
                    descartados.push(
                        "un «actualizar» que apunta a un nodo que no está en el lienzo".into(),
                    );
                    continue;
                }
                let mut campos = serde_json::Map::new();
                for (clave, tope) in [
                    ("titulo", 140usize),
                    ("descripcion", 700),
                    ("categoria", 40),
                ] {
                    if let Some(v) = c[clave]
                        .as_str()
                        .map(|s| recorta(s, tope))
                        .filter(|s| !s.is_empty())
                    {
                        campos.insert(clave.to_string(), json!(v));
                    }
                }
                if let Some(m) = c["maturity"].as_u64() {
                    campos.insert("maturity".into(), json!(m.clamp(1, 5)));
                }
                if let Some(ts) = c["tags"].as_array() {
                    let limpios: Vec<String> = ts
                        .iter()
                        .filter_map(|t| t.as_str())
                        .map(|s| recorta(s, 24))
                        .filter(|s| !s.is_empty())
                        .take(8)
                        .collect();
                    if !limpios.is_empty() {
                        campos.insert("tags".into(), json!(limpios));
                    }
                }
                if campos.is_empty() {
                    descartados.push("un «actualizar» sin ningún campo para cambiar".into());
                    continue;
                }
                if let Some(obj) = limpio.as_object_mut() {
                    obj.insert("nodo".into(), json!(nodo));
                    for (k, valor) in campos {
                        obj.insert(k, valor);
                    }
                }
            }
            "crear" => {
                let titulo = recorta(c["titulo"].as_str().unwrap_or(""), 140);
                if titulo.is_empty() {
                    descartados.push("un «crear» sin título".into());
                    continue;
                }
                limpio["titulo"] = json!(titulo);
                limpio["descripcion"] =
                    json!(recorta(c["descripcion"].as_str().unwrap_or(""), 700));
                limpio["categoria"] = json!(recorta(c["categoria"].as_str().unwrap_or("VOZ"), 40));
            }
            "enfocar" => {
                let mut ids: Vec<String> = ids_del_comando(&c)
                    .into_iter()
                    .filter(|i| existe(i))
                    .collect();
                ids.dedup();
                if ids.is_empty() {
                    descartados.push("un «enfocar» sin ids válidos del lienzo".into());
                    continue;
                }
                ids.truncate(MAX_NODOS);
                limpio["nodos"] = json!(ids);
                limpio["criterio"] = json!(recorta(c["criterio"].as_str().unwrap_or(""), 200));
            }
            "condensar" | "criticar" => {
                let mut ids: Vec<String> = ids_del_comando(&c)
                    .into_iter()
                    .filter(|i| existe(i))
                    .collect();
                ids.dedup();
                if ids.len() < 2 {
                    descartados.push(format!("un «{accion}» con menos de 2 nodos válidos"));
                    continue;
                }
                ids.truncate(MAX_NODOS);
                limpio["nodos"] = json!(ids);
            }
            "responder" => {
                // Cierra una PREGUNTA del lienzo: la respuesta se crea como nodo enlazado y la
                // pregunta pasa a `respondida`.
                let nodo = c["nodo"].as_str().unwrap_or("").trim().to_string();
                if nodo.is_empty() || !existe(&nodo) {
                    descartados.push(
                        "un «responder» que apunta a una pregunta que no está en el lienzo".into(),
                    );
                    continue;
                }
                let respuesta = recorta(c["respuesta"].as_str().unwrap_or(""), 700);
                if respuesta.chars().count() < 2 {
                    descartados.push("un «responder» sin el texto de la respuesta".into());
                    continue;
                }
                limpio["nodo"] = json!(nodo);
                limpio["respuesta"] = json!(respuesta);
            }
            "aceptar" | "descartar" => {
                // Decidir NO borra: el nodo queda con su decisión y se puede volver atrás. Con un
                // nodo alcanza (a diferencia de condensar, que necesita 2 para tener sentido).
                let mut ids: Vec<String> = ids_del_comando(&c)
                    .into_iter()
                    .filter(|i| existe(i))
                    .collect();
                ids.dedup();
                if ids.is_empty() {
                    descartados.push(format!("un «{accion}» sin ids válidos del lienzo"));
                    continue;
                }
                ids.truncate(MAX_NODOS);
                limpio["nodos"] = json!(ids);
            }
            "enlazar" => {
                let desde = c["desde"].as_str().unwrap_or("").trim().to_string();
                let hasta = c["hasta"].as_str().unwrap_or("").trim().to_string();
                if desde.is_empty() || hasta.is_empty() || desde == hasta {
                    descartados.push("un «enlazar» sin dos extremos distintos".into());
                    continue;
                }
                // Uno de los dos puede ser un nodo que este mismo plan crea (se resuelve al aplicar).
                let titulos_nuevos: Vec<String> = limpios
                    .iter()
                    .filter(|x| x["accion"] == "crear")
                    .map(|x| x["titulo"].as_str().unwrap_or("").to_lowercase())
                    .collect();
                let es_nuevo = |s: &str| titulos_nuevos.iter().any(|t| t == &s.to_lowercase());
                if (!existe(&desde) && !es_nuevo(&desde)) || (!existe(&hasta) && !es_nuevo(&hasta))
                {
                    descartados
                        .push("un «enlazar» que apunta a algo que no está en el lienzo".into());
                    continue;
                }
                limpio["desde"] = json!(desde);
                limpio["hasta"] = json!(hasta);
            }
            _ => {}
        }
        limpios.push(limpio);
    }

    let descartados_n = descartados.len();
    json!({
        "intencion": intencion,
        "respuesta": recorta(plan["respuesta"].as_str().unwrap_or(""), 300),
        "motivo": recorta(plan["motivo"].as_str().unwrap_or(""), 300),
        "comandos": limpios,
        "descartados": descartados_n,
        "motivo_descarte": descartados,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lienzo() -> Vec<String> {
        vec!["n-1".into(), "n-2".into(), "n-3".into()]
    }

    #[test]
    fn descarta_ids_inventados() {
        let plan = json!({"intencion": "comando", "respuesta": "listo",
            "comandos": [{"accion": "condensar", "nodos": ["n-1", "n-2", "n-99"]}]});
        let out = validar(&plan, &lienzo());
        assert_eq!(out["comandos"][0]["nodos"], json!(["n-1", "n-2"]));
    }

    #[test]
    fn enfocar_sin_ids_validos_se_cae() {
        let plan = json!({"intencion": "comando", "respuesta": "x",
            "comandos": [{"accion": "enfocar", "nodos": ["inventado-1", "inventado-2"]}]});
        let out = validar(&plan, &lienzo());
        assert_eq!(out["comandos"].as_array().unwrap().len(), 0);
        assert_eq!(out["descartados"], json!(1));
    }

    #[test]
    fn accion_desconocida_se_cae() {
        let plan = json!({"intencion": "comando", "respuesta": "x",
            "comandos": [{"accion": "borrar_todo"}, {"accion": "criticar", "nodos": ["n-1", "n-2"]}]});
        let out = validar(&plan, &lienzo());
        assert_eq!(out["comandos"].as_array().unwrap().len(), 1);
        assert_eq!(out["comandos"][0]["accion"], json!("criticar"));
    }

    #[test]
    fn crear_sin_titulo_se_cae_y_enlaza_puede_apuntar_a_lo_nuevo() {
        let plan = json!({"intencion": "capturar", "respuesta": "x", "comandos": [
            {"accion": "crear", "titulo": ""},
            {"accion": "crear", "titulo": "Orquestador de voz", "categoria": "VOZ"},
            {"accion": "enlazar", "desde": "Orquestador de voz", "hasta": "n-1"}
        ]});
        let out = validar(&plan, &lienzo());
        let c = out["comandos"].as_array().unwrap();
        assert_eq!(c.len(), 2, "el crear vacío cae, los otros dos quedan");
        assert_eq!(c[1]["accion"], json!("enlazar"));
    }

    #[test]
    fn enlazar_a_lo_inexistente_se_cae() {
        let plan = json!({"intencion": "comando", "respuesta": "x",
            "comandos": [{"accion": "enlazar", "desde": "n-1", "hasta": "fantasma"}]});
        assert_eq!(
            validar(&plan, &lienzo())["comandos"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn topes_de_comandos_y_nodos() {
        let muchos: Vec<Value> = (0..20)
            .map(|i| json!({"accion": "crear", "titulo": format!("n{i}")}))
            .collect();
        let plan = json!({"intencion": "capturar", "respuesta": "", "comandos": muchos});
        assert_eq!(
            validar(&plan, &lienzo())["comandos"]
                .as_array()
                .unwrap()
                .len(),
            MAX_COMANDOS
        );

        let ids: Vec<String> = (0..50).map(|i| format!("n-{i}")).collect();
        let plan2 = json!({"intencion": "comando", "respuesta": "", "comandos": [{"accion": "condensar", "nodos": ids}]});
        assert_eq!(
            validar(&plan2, &ids)["comandos"][0]["nodos"]
                .as_array()
                .unwrap()
                .len(),
            MAX_NODOS
        );
    }

    #[test]
    fn plan_vacio_no_paniquea() {
        let out = validar(&json!({}), &[]);
        assert_eq!(out["comandos"].as_array().unwrap().len(), 0);
        assert_eq!(out["intencion"], json!("comando"));
    }
}

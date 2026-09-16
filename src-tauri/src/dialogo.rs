//! Hilo de diálogo — **la memoria de la conversación**.
//!
//! Sin esto, cada frase es un plan aislado: "¿y si lo damos vuelta?" no tiene referente, y el copiloto
//! no puede continuar un razonamiento. Con esto, el intérprete recibe los últimos intercambios y el foco
//! actual, así que puede resolver referencias, no repetir lo ya hecho y preguntar cuando no alcanza.
//!
//! Se guarda en `dialogo.json` (carpeta de datos de la app) y **expira solo**: si pasaron más de
//! `MINUTOS_DE_VIDA` desde el último turno, se empieza una sesión nueva. Una conversación que quedó
//! abierta ayer no debería condicionar la de hoy.

use serde_json::{json, Value};
use std::path::Path;

/// Cuántos turnos se guardan (los viejos se tiran: la conversación no puede crecer sin límite).
pub const MAX_TURNOS: usize = 12;
/// Cuántos se le muestran al intérprete en el prompt.
pub const SE_MUESTRAN: usize = 6;
/// Minutos de vida de la sesión sin actividad.
pub const MINUTOS_DE_VIDA: u64 = 30;

fn ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// ¿La sesión ya venció? (para no arrastrar una conversación de ayer)
pub fn vencida(ultimo: u64, ahora: u64) -> bool {
    ahora.saturating_sub(ultimo) > MINUTOS_DE_VIDA * 60
}

/// Los últimos turnos, sin los viejos.
pub fn recorta(turnos: &[Value]) -> Vec<Value> {
    let desde = turnos.len().saturating_sub(MAX_TURNOS);
    turnos[desde..].to_vec()
}

/// El hilo como texto para el prompt: lo más viejo arriba, lo último abajo.
pub fn como_texto(turnos: &[Value]) -> String {
    if turnos.is_empty() {
        return "(es el primer pedido de esta conversación)".to_string();
    }
    let desde = turnos.len().saturating_sub(SE_MUESTRAN);
    turnos[desde..]
        .iter()
        .map(|t| {
            let dijo = t["dijo"].as_str().unwrap_or("").trim();
            let hizo = t["hizo"].as_str().unwrap_or("").trim();
            let dijo_esto = t["dijo_esto"].as_str().unwrap_or("").trim();
            let accion = if hecho(t) {
                format!("[hizo: {hizo}] {dijo_esto}")
            } else {
                format!("[no tocó el lienzo] {dijo_esto}")
            };
            format!("Vos: {dijo}\nCopiloto: {accion}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn hecho(t: &Value) -> bool {
    t["comandos"].as_u64().unwrap_or(0) > 0
}

/// Qué queda "en foco" después de un plan: si enfocó, esos nodos; si no, se mantiene el foco anterior.
/// Es la hebra que convierte "reunir ideas → enfocar una dirección → ejecutar" en una sola conversación.
pub fn foco_del_plan(plan: &Value, titulos: &[String]) -> Vec<String> {
    let mut foco: Vec<String> = Vec::new();
    for c in plan["comandos"].as_array().into_iter().flatten() {
        if c["accion"] == "enfocar" {
            for id in c["nodos"].as_array().into_iter().flatten() {
                if let Some(id) = id.as_str() {
                    let nombre = titulos
                        .iter()
                        .find(|t| t.starts_with(id))
                        .cloned()
                        .unwrap_or_else(|| id.to_string());
                    foco.push(nombre);
                }
            }
        }
    }
    if foco.is_empty() {
        // Sin foco nuevo se conserva el anterior (lo pasa `registrar`).
        Vec::new()
    } else {
        foco.truncate(8);
        foco
    }
}

/// ¿Hay una **conversación** en curso? Con dos turnos o más, el pedido ya no es una orden suelta: es
/// parte de un razonamiento, y merece que lo lea un modelo a la altura (lo usa el ruteo por tarea).
pub fn tiene_hilo(data_dir: &Path) -> bool {
    leer(data_dir)["turnos"]
        .as_array()
        .map(|t| t.len() >= 2)
        .unwrap_or(false)
}

/// La sesión en curso (vacía si venció o si nunca hubo).
pub fn leer(data_dir: &Path) -> Value {
    let crudo = std::fs::read_to_string(data_dir.join("dialogo.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .unwrap_or_else(|| json!({ "turnos": [], "foco": [] }));
    let ultimo = crudo["turnos"]
        .as_array()
        .and_then(|t| t.last())
        .and_then(|t| t["cuando"].as_u64())
        .unwrap_or(0);
    if ultimo > 0 && vencida(ultimo, ahora()) {
        return json!({ "turnos": [], "foco": [] });
    }
    crudo
}

/// Anota un turno: lo que dijo el usuario, qué hizo el copiloto y en qué quedó el foco.
pub fn registrar(
    data_dir: &Path,
    dijo: &str,
    plan: &Value,
    titulos: &[String],
) -> Result<Value, String> {
    let previa = leer(data_dir);
    let mut turnos = previa["turnos"].as_array().cloned().unwrap_or_default();
    let comandos: Vec<String> = plan["comandos"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["accion"].as_str().map(String::from))
        .collect();
    let que_hizo = if comandos.is_empty() {
        "no tocó el lienzo".to_string()
    } else {
        comandos.join(" + ")
    };
    turnos.push(json!({
        "cuando": ahora(),
        "dijo": dijo.chars().take(300).collect::<String>(),
        "comandos": comandos.len(),
        "hizo": que_hizo,
        "dijo_esto": plan["respuesta"].as_str().unwrap_or("").chars().take(240).collect::<String>(),
    }));
    let foco_nuevo = foco_del_plan(plan, titulos);
    let foco = if foco_nuevo.is_empty() {
        previa["foco"].as_array().cloned().unwrap_or_default()
    } else {
        foco_nuevo.into_iter().map(Value::String).collect()
    };
    let sesion = json!({ "turnos": recorta(&turnos), "foco": foco });
    if let Some(padre) = data_dir.parent() {
        let _ = std::fs::create_dir_all(padre);
    }
    std::fs::write(
        data_dir.join("dialogo.json"),
        serde_json::to_string_pretty(&sesion).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(sesion)
}

#[cfg(test)]
mod tests_dialogo {
    use super::*;

    #[test]
    fn el_hilo_se_lee_como_conversacion() {
        let turnos = vec![
            json!({"dijo": "sumá estas ideas", "comandos": 2, "hizo": "crear", "dijo_esto": "Listo, dos nodos nuevos."}),
            json!({"dijo": "y si lo damos vuelta", "comandos": 0, "hizo": "no tocó el lienzo", "dijo_esto": "¿Te referís al riego sin internet?"}),
        ];
        let t = como_texto(&turnos);
        assert!(t.contains("Vos: sumá estas ideas"));
        assert!(t.contains("[hizo: crear]"));
        assert!(t.contains("[no tocó el lienzo]"));
        assert!(t.contains("¿Te referís"));
    }

    #[test]
    fn sin_turnos_lo_dice() {
        assert!(como_texto(&[]).contains("primer pedido"));
    }

    #[test]
    fn solo_se_muestran_los_ultimos() {
        let turnos: Vec<Value> = (0..10)
            .map(|i| json!({"dijo": format!("turno {i}"), "comandos": 1, "hizo": "crear", "dijo_esto": "ok"}))
            .collect();
        let t = como_texto(&turnos);
        assert!(!t.contains("turno 0"), "los viejos no van al prompt");
        assert!(t.contains("turno 9"), "el último sí");
    }

    #[test]
    fn la_sesion_vence_y_se_guarda_con_tope() {
        let ahora = 1_000_000;
        assert!(vencida(ahora - MINUTOS_DE_VIDA * 60 - 1, ahora));
        assert!(!vencida(ahora - 60, ahora));
        let muchos: Vec<Value> = (0..30).map(|i| json!({"dijo": i.to_string()})).collect();
        assert_eq!(recorta(&muchos).len(), MAX_TURNOS, "no crece sin límite");
        assert_eq!(recorta(&muchos)[0]["dijo"], "18", "se quedan los últimos");
    }

    #[test]
    fn el_foco_lo_fija_enfocar() {
        let plan = json!({"comandos": [{"accion": "enfocar", "nodos": ["n-1", "n-2"]}]});
        let titulos = vec!["n-1 · Riego".to_string(), "n-2 · Humedad".to_string()];
        let foco = foco_del_plan(&plan, &titulos);
        assert_eq!(foco.len(), 2);
        assert!(foco[0].starts_with("n-1"));
        // Un plan que no enfoca no cambia el foco (lo conserva `registrar`).
        assert!(foco_del_plan(
            &json!({"comandos": [{"accion": "crear", "titulo": "x"}]}),
            &titulos
        )
        .is_empty());
    }

    #[test]
    fn una_conversacion_necesita_dos_turnos() {
        let dir = std::env::temp_dir().join(format!("nf-hilo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let _ = std::fs::remove_file(dir.join("dialogo.json"));
        assert!(!tiene_hilo(&dir), "sin nada, no hay conversación");
        let plan = json!({"comandos": [{"accion": "crear"}], "respuesta": "ok"});
        registrar(&dir, "una idea", &plan, &[]).unwrap();
        assert!(!tiene_hilo(&dir), "un pedido suelto no es una conversación");
        registrar(&dir, "y ahora eso", &plan, &[]).unwrap();
        assert!(tiene_hilo(&dir), "con dos turnos ya hay hilo");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn registrar_guarda_y_conserva_el_foco() {
        let dir = std::env::temp_dir().join(format!("nf-dialogo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let _ = std::fs::remove_file(dir.join("dialogo.json"));
        let titulos = vec!["n-1 · Riego".to_string()];
        let s1 = registrar(&dir, "enfocá el riego", &json!({"comandos": [{"accion": "enfocar", "nodos": ["n-1"]}], "respuesta": "Enfoco el riego."}), &titulos).unwrap();
        assert_eq!(s1["turnos"].as_array().unwrap().len(), 1);
        assert_eq!(s1["foco"].as_array().unwrap().len(), 1);
        // Un turno que no enfoca conserva el foco anterior.
        let s2 = registrar(
            &dir,
            "sumá una idea",
            &json!({"comandos": [{"accion": "crear"}], "respuesta": "Listo."}),
            &titulos,
        )
        .unwrap();
        assert_eq!(s2["turnos"].as_array().unwrap().len(), 2);
        assert_eq!(
            s2["foco"].as_array().unwrap().len(),
            1,
            "el foco no se pierde"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

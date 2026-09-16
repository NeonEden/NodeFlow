//! Fase 12 — **selector global de motor de inferencia**.
//!
//! El usuario elige UN motor (dónde corre y con qué modelo) y esa elección vale para toda la app:
//! no se configura función por función. El catálogo se arma con lo que hay de verdad en la máquina:
//! los modelos del daemon local de Ollama (`/api/tags`), el proveedor de nube configurado y los
//! proveedores compatibles con OpenAI declarados en `nodeflow.config.json`.
//!
//! Reglas de diseño:
//! - **Nada inventado**: si un motor no está disponible (daemon apagado, sin clave), el catálogo lo
//!   dice en vez de ofrecerlo.
//! - **La elección persiste** en el config y la respetan la UI, la API y los tests.
//! - **El ruteo sigue siendo determinista** (ADR 0005): se usa el motor elegido. Si falla, la acción
//!   cae al fallback heurístico — nunca a otro modelo por sorpresa.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// Dónde corre el motor, de cara al usuario.
pub const EN_TU_PLACA: &str = "local";
pub const NUBE_GRATIS: &str = "gratis";
pub const NUBE_PAGA: &str = "pago";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Motor {
    /// Identificador estable: `proveedor:modelo`.
    pub id: String,
    /// Nombre para mostrar.
    pub etiqueta: String,
    /// `ollama` | `gemini` | `openai`
    pub proveedor: String,
    pub modelo: String,
    /// `local` | `gratis` | `pago`
    pub donde: String,
    /// Base URL para proveedores compatibles con OpenAI (Ollama `/v1`, DeepSeek, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    pub disponible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nota: Option<String>,
    /// **Nombre** de la variable de entorno o del campo del config que guarda la clave.
    /// Nunca el valor: el catálogo que ve la UI no puede llevar credenciales.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clave_ref: Option<String>,
}

impl Motor {
    pub fn nuevo(
        proveedor: &str,
        modelo: &str,
        donde: &str,
        etiqueta: Option<String>,
        base_url: Option<String>,
    ) -> Motor {
        Motor {
            id: format!("{proveedor}:{modelo}"),
            etiqueta: etiqueta.unwrap_or_else(|| match donde {
                EN_TU_PLACA => format!("{modelo} · en tu placa"),
                NUBE_GRATIS => format!("{modelo} · nube gratuita"),
                _ => format!("{modelo} · nube"),
            }),
            proveedor: proveedor.to_string(),
            modelo: modelo.to_string(),
            donde: donde.to_string(),
            base_url,
            disponible: true,
            nota: None,
            clave_ref: None,
        }
    }
}

/// Clasifica un modelo del daemon local: los `-cloud`/`:cloud` son nube gratuita servida por el
/// daemon; el resto corre en el hardware del usuario.
pub fn donde_corre(nombre_modelo: &str) -> &'static str {
    let n = nombre_modelo.to_lowercase();
    if n.contains("-cloud") || n.contains(":cloud") {
        NUBE_GRATIS
    } else {
        EN_TU_PLACA
    }
}

/// Orden de presentación: primero lo que corre en la máquina del usuario, después la nube gratuita
/// y al final la que se paga. Determinista, para que el catálogo no cambie de orden entre corridas.
pub fn rango(donde: &str) -> u8 {
    match donde {
        EN_TU_PLACA => 0,
        NUBE_GRATIS => 1,
        _ => 2,
    }
}

/// Motores que ofrece un daemon de Ollama a partir de su listado de tags.
pub fn motores_de_tags(tags: &[String], base_url: &str) -> Vec<Motor> {
    let mut v: Vec<Motor> = tags
        .iter()
        .filter(|t| !t.trim().is_empty())
        .map(|t| {
            Motor::nuevo(
                "ollama",
                t.trim(),
                donde_corre(t),
                None,
                Some(base_url.to_string()),
            )
        })
        .collect();
    v.sort_by(|a, b| (rango(&a.donde), a.modelo.clone()).cmp(&(rango(&b.donde), b.modelo.clone())));
    v
}

/// Convierte un esquema estilo Gemini (`"type": "OBJECT"`) al JSON Schema estándar que espera el
/// `format` de Ollama. Medido: sin gramática, los modelos locales devuelven un objeto donde el
/// contrato pide una lista y la función parece rota; con gramática la forma se cumple siempre.
pub fn esquema_para_ollama(v: &Value) -> Value {
    match v {
        Value::Object(o) => {
            let mut n = serde_json::Map::new();
            for (k, val) in o {
                if k == "type" {
                    if let Some(t) = val.as_str() {
                        n.insert(k.clone(), serde_json::json!(t.to_lowercase()));
                        continue;
                    }
                }
                n.insert(k.clone(), esquema_para_ollama(val));
            }
            Value::Object(n)
        }
        Value::Array(a) => Value::Array(a.iter().map(esquema_para_ollama).collect()),
        otro => otro.clone(),
    }
}

/// Modos automáticos: eligen dentro de un grupo y se resuelven en `plan`.
pub const AUTO_LOCAL: &str = "auto:local";
pub const AUTO_NUBE: &str = "auto:nube";

/// **Recomendado**: el motor se elige por tipo de tarea (ver `Tarea`). El bucle de la voz no espera
/// a nadie; lo que puede pensar despacio usa el modelo local más grande; lo que necesita el mundo
/// sube a la nube. El usuario siempre puede pisarlo eligiendo un motor a mano.
pub const AUTO_TAREA: &str = "auto:tarea";

/// Qué se le está pidiendo al motor. Se deduce de la acción del spec: **determinista**, no lo
/// adivina un modelo (medido: un router por LLM chico acierta 1 de 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tarea {
    /// El bucle interactivo: hablás y el lienzo responde. Tiene que ser rápido y gratis.
    Lienzo,
    /// Puede esperar y gana con razonamiento: condensar, cuestionar, sintetizar.
    Sintesis,
    /// Conversar largo.
    Dialogo,
    /// Necesita el mundo (web, archivos, terminal): el modelo local no puede.
    Herramientas,
}

impl Tarea {
    pub fn de_accion(accion: &str) -> Tarea {
        match accion.trim().to_lowercase().as_str() {
            "condensar" | "criticar" | "critique" | "socratic" | "hybrid" | "hybridize"
            | "sintesis" | "synthesize" | "resonar" => Tarea::Sintesis,
            "delegar" | "investigar" | "herramientas" | "buscar" => Tarea::Herramientas,
            "chat" | "conversar" | "dialogo" => Tarea::Dialogo,
            // voz, braindump, borradores, expansión: el bucle de todos los días.
            _ => Tarea::Lienzo,
        }
    }

    /// En una frase, para mostrarlo en la UI o en el log.
    pub fn etiqueta(self) -> &'static str {
        match self {
            Tarea::Lienzo => "bucle del lienzo (rápido y gratis)",
            Tarea::Sintesis => "pensar despacio (razonamiento, puede esperar)",
            Tarea::Dialogo => "conversación",
            Tarea::Herramientas => "necesita herramientas (nube)",
        }
    }
}

/// Tamaño que declara el nombre del modelo (`granite3.3:2b` → 2, `deepseek-r1:7b` → 7,
/// `nemotron-3-nano:30b-cloud` → 30). Sin dato devuelve `None`: el orden nunca queda indefinido.
pub fn tamano_b(modelo: &str) -> Option<f32> {
    modelo
        .to_lowercase()
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '.'))
        .filter_map(|p| p.strip_suffix('b'))
        .filter_map(|n| n.parse::<f32>().ok())
        .filter(|n| (0.1..=2000.0).contains(n))
        .fold(None, |acc: Option<f32>, n| {
            Some(acc.map_or(n, |a| a.max(n)))
        })
}

/// ¿Es un modelo de razonamiento, según su propio nombre? Entre locales del mismo tamaño, el que
/// piensa antes de responder gana para "pensar despacio"; para el bucle del lienzo perdería, porque
/// su cadena de pensamiento es justamente lo que lo hace lento (medido: 31 s contra 3,8 s).
pub fn es_razonador(modelo: &str) -> bool {
    let n = modelo.to_lowercase();
    [
        "deepseek-r1",
        "r1:",
        "-r1",
        "qwq",
        "reason",
        "think",
        "magistral",
    ]
    .iter()
    .any(|m| n.contains(m))
}

fn gracias_y_pagas(gratis: Vec<Motor>, paga: Vec<Motor>) -> Vec<Motor> {
    let mut v = gratis;
    v.extend(paga);
    v
}

/// Cadena de motores para una tarea, **de más barato a más caro**. El criterio es el costo: lo local
/// gana los empates y la nube es la red de seguridad, no el camino principal.
pub fn orden_para(tarea: Tarea, catalogo: &[Motor]) -> Vec<Motor> {
    let mut local: Vec<Motor> = catalogo
        .iter()
        .filter(|m| m.disponible && m.donde == EN_TU_PLACA)
        .cloned()
        .collect();
    // Del más chico al más grande (None = mediano, va en el medio del orden comparativo).
    local.sort_by(|a, b| {
        tamano_b(&a.modelo)
            .unwrap_or(6.0)
            .partial_cmp(&tamano_b(&b.modelo).unwrap_or(6.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let gratis: Vec<Motor> = catalogo
        .iter()
        .filter(|m| m.disponible && m.donde == NUBE_GRATIS)
        .cloned()
        .collect();
    let paga: Vec<Motor> = catalogo
        .iter()
        .filter(|m| m.disponible && m.donde == NUBE_PAGA)
        .cloned()
        .collect();

    match tarea {
        // Rápido y gratis: el local más chico; si no hay, la nube.
        Tarea::Lienzo => {
            let mut v = local;
            v.extend(gratis);
            v.extend(paga);
            v
        }
        // Conversación: acá la calidad manda sobre el ahorro, porque el hilo sólo sirve si el modelo
        // lo entiende. Nube primero (gratis y después paga) y el local queda como último recurso.
        Tarea::Dialogo => {
            let mut v = gracias_y_pagas(gratis, paga);
            local.reverse();
            v.extend(local);
            v
        }
        // Pensar despacio: el local más grande primero (sigue siendo gratis), y entre iguales, el que
        // razona; después la nube.
        Tarea::Sintesis => {
            local.sort_by(|a, b| {
                let (ta, tb) = (
                    tamano_b(&a.modelo).unwrap_or(6.0),
                    tamano_b(&b.modelo).unwrap_or(6.0),
                );
                tb.partial_cmp(&ta)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(es_razonador(&b.modelo).cmp(&es_razonador(&a.modelo)))
            });
            let mut v = local;
            v.extend(gratis);
            v.extend(paga);
            v
        }
        // Herramientas: el modelo local no puede tocar el mundo, así que arranca en la nube.
        Tarea::Herramientas => {
            let mut v = gratis;
            v.extend(paga);
            v
        }
    }
}

/// Qué pruebas de la planilla hablan de una acción del spec. Es el puente entre lo que se está
/// pidiendo ahora y lo que ya se midió.
pub fn pruebas_de_accion(accion: &str) -> Vec<&'static str> {
    match accion.trim().to_lowercase().as_str() {
        "voz" => vec!["voz-enfocar", "voz-crear", "voz-delegar"],
        "condensar" => vec!["condensar"],
        "braindump" | "capturar" => vec!["braindump"],
        _ => vec![],
    }
}

/// **El ruteo alimentado por la planilla**: quién ganó, medido, las pruebas de esta acción.
///
/// Sólo cuentan los ganadores que **acertaron**: si en una prueba no acertó nadie, esa prueba no
/// recomienda a nadie (y es una señal de que ahí hace falta más motor, no menos). A igualdad de
/// victorias gana el más rápido. Sin planilla devuelve `None` y manda la heurística por tamaño.
pub fn ganador_medido(accion: &str, planilla: Option<&Value>) -> Option<String> {
    let pruebas = pruebas_de_accion(accion);
    if pruebas.is_empty() {
        return None;
    }
    let g = planilla?["ganador_por_prueba"].as_object()?;
    let mut puntos: std::collections::HashMap<String, (u32, u64)> =
        std::collections::HashMap::new();
    for p in pruebas {
        if let Some(fila) = g.get(p) {
            if fila["ok"].as_bool().unwrap_or(false) {
                if let Some(id) = fila["motor"].as_str() {
                    let e = puntos.entry(id.to_string()).or_insert((0, 0));
                    e.0 += 1;
                    e.1 += fila["ms"].as_u64().unwrap_or(0);
                }
            }
        }
    }
    puntos
        .into_iter()
        .max_by(|a, b| a.1 .0.cmp(&b.1 .0).then(b.1 .1.cmp(&a.1 .1)))
        .map(|(id, _)| id)
}

/// Igual que `plan`, pero si la selección es `auto:tarea` la cadena se arma según el tipo de tarea
/// **y de lo que la planilla de evaluación ya midió**: el ganador de esa acción va primero.
pub fn plan_tarea(
    catalogo: &[Motor],
    seleccionado: Option<&str>,
    modo: Option<&str>,
    tarea: Tarea,
    accion: &str,
    planilla: Option<&Value>,
) -> Vec<Motor> {
    let es_tarea = seleccionado
        .map(|s| s.trim().to_lowercase() == AUTO_TAREA)
        .unwrap_or(false);
    if !es_tarea {
        return plan(catalogo, seleccionado, modo);
    }
    let mut orden = orden_para(tarea, catalogo);
    if let Some(id) = ganador_medido(accion, planilla) {
        if let Some(pos) = orden.iter().position(|m| m.id == id) {
            let m = orden.remove(pos);
            log::info!(
                "ruteo: «{accion}» → {} (ganador medido por la planilla)",
                m.id
            );
            orden.insert(0, m);
        }
    }
    orden
}

/// Plan de ejecución para una llamada: **un solo motor**, el elegido.
///
/// `modo` (opcional, por tarea) restringe el grupo: `local` → sólo hardware del usuario,
/// `nube` → nube gratuita o paga. Dentro del grupo manda el motor seleccionado si pertenece a él.
pub fn plan(catalogo: &[Motor], seleccionado: Option<&str>, modo: Option<&str>) -> Vec<Motor> {
    // `auto:local` / `auto:nube` son elecciones guardadas que eligen dentro de un grupo.
    let (modo, salto_de_id): (Option<&str>, bool) =
        match seleccionado.map(|s| s.trim().to_lowercase()).as_deref() {
            Some(AUTO_LOCAL) => (Some("local"), true),
            Some(AUTO_NUBE) => (Some("nube"), true),
            _ => (modo, false),
        };
    let grupo: Option<Vec<&str>> = match modo.map(|m| m.trim().to_lowercase()).as_deref() {
        Some("local") | Some("edge") => Some(vec![EN_TU_PLACA]),
        Some("nube") | Some("cloud") => Some(vec![NUBE_GRATIS, NUBE_PAGA]),
        _ => None,
    };
    let seleccionado = if salto_de_id { None } else { seleccionado };
    let disponible = |m: &&Motor| {
        m.disponible
            && grupo
                .as_ref()
                .map(|g| g.contains(&m.donde.as_str()))
                .unwrap_or(true)
    };

    // Automáticos: devuelven **todo el grupo** en orden (placa → gratis → pago). Así, si un motor
    // falla —típicamente 402 por créditos— la misma corrida sigue con el siguiente en vez de
    // devolverle un error al usuario.
    if salto_de_id {
        return catalogo.iter().filter(disponible).cloned().collect();
    }
    // 1) el elegido, si existe y entra en el grupo pedido
    if let Some(id) = seleccionado {
        if let Some(m) = catalogo.iter().find(|m| m.id == id).filter(disponible) {
            return vec![m.clone()];
        }
    }
    // 2) el primero disponible del grupo (determinista por orden del catálogo)
    catalogo
        .iter()
        .find(disponible)
        .cloned()
        .into_iter()
        .collect()
}

/// Lee la selección guardada (`motor_activo`) del config de la app.
pub fn seleccionado(data_dir: &Path) -> Option<String> {
    let txt = std::fs::read_to_string(data_dir.join("nodeflow.config.json")).ok()?;
    let v: Value = serde_json::from_str(&txt).ok()?;
    v["motor_activo"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Guarda la selección **conservando el resto del config** (la clave de API incluida).
/// `None` o "auto" borra la selección y vuelve a la cadena configurada.
/// ¿El id elegido existe de verdad? Los `auto*` (la cadena configurada y los ruteos por tarea) siempre
/// valen; cualquier otra cosa tiene que estar en el catálogo **real** de motores.
///
/// Lo encontró el QA de escritura: `POST /api/ai/motor` con un id inventado devolvía `ok: true` y lo
/// guardaba, así que la app quedaba apuntando a un motor que no existe y fallaba recién al usarlo.
pub fn motor_valido(id: &str, disponibles: &[String]) -> bool {
    let id = id.trim();
    id.is_empty() || id == "auto" || id.starts_with("auto:") || disponibles.iter().any(|d| d == id)
}

pub fn guardar_seleccion(data_dir: &Path, id: Option<&str>) -> Result<(), String> {
    let ruta = data_dir.join("nodeflow.config.json");
    let mut cfg: Value = std::fs::read_to_string(&ruta)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let obj = cfg
        .as_object_mut()
        .ok_or_else(|| "el config no es un objeto JSON".to_string())?;
    match id
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != "auto")
    {
        Some(id) => {
            obj.insert("motor_activo".into(), Value::String(id.to_string()));
        }
        None => {
            obj.remove("motor_activo");
        }
    }
    let txt = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(&ruta, txt).map_err(|e| format!("no pude escribir {}: {e}", ruta.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn cat() -> Vec<Motor> {
        let mut v = motores_de_tags(
            &[
                "granite3.3:2b".into(),
                "qwen2.5vl:7b".into(),
                "nemotron-3-nano:30b-cloud".into(),
            ],
            "http://localhost:11434/v1",
        );
        v.push(Motor::nuevo(
            "gemini",
            "gemini-3.6-flash",
            NUBE_PAGA,
            None,
            None,
        ));
        v
    }

    #[test]
    fn clasifica_cloud_y_placa() {
        assert_eq!(donde_corre("nemotron-3-nano:30b-cloud"), NUBE_GRATIS);
        assert_eq!(donde_corre("glm-5.3:cloud"), NUBE_GRATIS);
        assert_eq!(donde_corre("granite3.3:2b"), EN_TU_PLACA);
        assert_eq!(donde_corre("qwen2.5vl:7b"), EN_TU_PLACA);
    }

    #[test]
    fn el_catalogo_marca_donde_corre_cada_modelo() {
        let c = cat();
        let ids: Vec<&str> = c.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"ollama:granite3.3:2b"));
        assert!(ids.contains(&"ollama:nemotron-3-nano:30b-cloud"));
        assert_eq!(
            c.iter()
                .find(|m| m.modelo == "granite3.3:2b")
                .unwrap()
                .donde,
            EN_TU_PLACA
        );
        assert_eq!(
            c.iter()
                .find(|m| m.modelo == "nemotron-3-nano:30b-cloud")
                .unwrap()
                .donde,
            NUBE_GRATIS
        );
    }

    #[test]
    fn sin_seleccion_usa_el_primero_del_catalogo() {
        let c = cat();
        let p = plan(&c, None, None);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].id, c[0].id);
    }

    #[test]
    fn el_elegido_gana_si_esta_disponible() {
        let c = cat();
        let p = plan(&c, Some("gemini:gemini-3.6-flash"), None);
        assert_eq!(p[0].modelo, "gemini-3.6-flash");
    }

    #[test]
    fn el_modo_restringe_el_grupo_y_el_elegido_no_lo_viola() {
        let c = cat();
        // Elegido en la nube, pero la tarea pide local: se usa un motor de la placa.
        let p = plan(&c, Some("gemini:gemini-3.6-flash"), Some("local"));
        assert_eq!(p[0].donde, EN_TU_PLACA);
        // Elegido en la placa, pero la tarea pide nube: se usa uno de nube.
        let p = plan(&c, Some("ollama:granite3.3:2b"), Some("nube"));
        assert_ne!(p[0].donde, EN_TU_PLACA);
    }

    #[test]
    fn un_motor_no_disponible_no_se_elige() {
        let mut c = cat();
        c.push(Motor {
            disponible: false,
            ..Motor::nuevo("ollama", "fantasma:7b", EN_TU_PLACA, None, None)
        });
        let p = plan(&c, Some("ollama:fantasma:7b"), Some("local"));
        assert_ne!(
            p[0].modelo, "fantasma:7b",
            "no debe elegir un motor marcado como no disponible"
        );
    }

    #[test]
    fn el_esquema_de_gemini_se_traduce_a_json_schema_estandar() {
        let g = serde_json::json!({
            "type": "OBJECT",
            "properties": {
                "variations": { "type": "ARRAY", "items": { "type": "OBJECT",
                    "properties": { "title": { "type": "STRING" } } } }
            },
            "required": ["variations"]
        });
        let o = esquema_para_ollama(&g);
        assert_eq!(o["type"], "object");
        assert_eq!(o["properties"]["variations"]["type"], "array");
        assert_eq!(o["properties"]["variations"]["items"]["type"], "object");
        assert_eq!(
            o["properties"]["variations"]["items"]["properties"]["title"]["type"],
            "string"
        );
        assert_eq!(o["required"][0], "variations");
    }

    #[test]
    fn auto_local_devuelve_solo_la_placa_y_auto_nube_solo_la_nube() {
        let c = cat();
        let l = plan(&c, Some(AUTO_LOCAL), None);
        assert!(!l.is_empty());
        assert!(
            l.iter().all(|m| m.donde == EN_TU_PLACA),
            "local no debe incluir nube"
        );
        let n = plan(&c, Some("auto:nube"), None);
        assert!(
            n.iter().all(|m| m.donde != EN_TU_PLACA),
            "nube no debe incluir la placa"
        );
    }

    #[test]
    fn los_automaticos_traen_respaldo_para_una_misma_corrida() {
        let c = cat();
        let n = plan(&c, Some(AUTO_NUBE), None);
        assert!(n.len() >= 2, "si el primero falla hay con qué seguir");
        assert_eq!(n[0].donde, NUBE_GRATIS);
    }

    #[test]
    fn auto_nube_prefiere_lo_gratuito_sobre_lo_pago() {
        let c = cat();
        let n = plan(&c, Some(AUTO_NUBE), None);
        assert_eq!(
            n[0].donde, NUBE_GRATIS,
            "con nube gratis disponible no gasta en la paga"
        );
    }

    #[test]
    fn guardar_y_leer_la_seleccion_conserva_el_resto_del_config() {
        let dir = std::env::temp_dir().join(format!("nf-motores-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("nodeflow.config.json");
        std::fs::write(&ruta, r#"{"gemini_api_key":"NO_TOCAR","vault_path":"X"}"#).unwrap();

        guardar_seleccion(&dir, Some("ollama:granite3.3:2b")).unwrap();
        assert_eq!(seleccionado(&dir).as_deref(), Some("ollama:granite3.3:2b"));
        let txt = std::fs::read_to_string(&ruta).unwrap();
        assert!(txt.contains("NO_TOCAR"), "la clave de API no se toca");
        assert!(txt.contains("vault_path"), "el resto del config sigue ahí");

        guardar_seleccion(&dir, None).unwrap();
        assert_eq!(seleccionado(&dir), None);

        std::fs::remove_dir_all(&dir).ok();
    }
}

#[cfg(test)]
mod tests_ruteo_por_tarea {
    use super::*;

    fn cat() -> Vec<Motor> {
        // El catálogo real de esta máquina, en miniatura.
        vec![
            Motor::nuevo("ollama", "granite3.3:2b", EN_TU_PLACA, None, None),
            Motor::nuevo("ollama", "deepseek-r1:7b", EN_TU_PLACA, None, None),
            Motor::nuevo("ollama", "gpt-oss:120b-cloud", NUBE_GRATIS, None, None),
            Motor::nuevo("deepseek", "deepseek-flash", NUBE_PAGA, None, None),
        ]
    }

    fn ids(ms: &[Motor]) -> Vec<&str> {
        ms.iter().map(|m| m.id.as_str()).collect()
    }

    #[test]
    fn la_tarea_se_deduce_de_la_accion() {
        assert_eq!(Tarea::de_accion("voz"), Tarea::Lienzo);
        assert_eq!(Tarea::de_accion("braindump"), Tarea::Lienzo);
        assert_eq!(Tarea::de_accion("condensar"), Tarea::Sintesis);
        assert_eq!(Tarea::de_accion("criticar"), Tarea::Sintesis);
        assert_eq!(Tarea::de_accion("delegar"), Tarea::Herramientas);
        assert_eq!(
            Tarea::de_accion("  VOZ "),
            Tarea::Lienzo,
            "no distingue mayúsculas ni espacios"
        );
    }

    #[test]
    fn el_tamano_se_lee_del_nombre() {
        assert_eq!(tamano_b("granite3.3:2b"), Some(2.0));
        assert_eq!(tamano_b("deepseek-r1:7b"), Some(7.0));
        assert_eq!(tamano_b("nemotron-3-nano:30b-cloud"), Some(30.0));
        assert_eq!(tamano_b("qwen2.5vl:7b"), Some(7.0));
        assert_eq!(
            tamano_b("glm-5.3-flash:cloud"),
            None,
            "sin tamaño declarado"
        );
    }

    #[test]
    fn el_bucle_del_lienzo_prefiere_el_local_mas_chico() {
        let orden = orden_para(Tarea::Lienzo, &cat());
        assert_eq!(
            ids(&orden)[0],
            "ollama:granite3.3:2b",
            "rápido y gratis primero"
        );
        assert_eq!(
            ids(&orden)[1],
            "ollama:deepseek-r1:7b",
            "después el local grande"
        );
        assert!(
            ids(&orden)[2].contains("cloud"),
            "la nube es la red de seguridad"
        );
    }

    #[test]
    fn pensar_despacio_prefiere_el_local_mas_grande() {
        let orden = orden_para(Tarea::Sintesis, &cat());
        assert_eq!(ids(&orden)[0], "ollama:deepseek-r1:7b");
        assert_eq!(ids(&orden)[1], "ollama:granite3.3:2b");
    }

    #[test]
    fn entre_locales_iguales_gana_el_que_razona() {
        let c = vec![
            Motor::nuevo("ollama", "qwen2.5vl:7b", EN_TU_PLACA, None, None),
            Motor::nuevo("ollama", "deepseek-r1:7b", EN_TU_PLACA, None, None),
        ];
        let orden = orden_para(Tarea::Sintesis, &c);
        assert_eq!(
            ids(&orden)[0],
            "ollama:deepseek-r1:7b",
            "piensa antes de responder"
        );
        // Pero en el bucle del lienzo el razonador NO va primero: la cadena de pensamiento es lenta.
        let rapido = orden_para(Tarea::Lienzo, &c);
        assert!(rapido
            .iter()
            .any(|m| m.modelo == "qwen2.5vl:7b" || m.modelo == "deepseek-r1:7b"));
        assert!(super::es_razonador("deepseek-r1:7b") && !super::es_razonador("granite3.3:2b"));
    }

    #[test]
    fn la_conversacion_prioriza_la_nube() {
        let orden = orden_para(Tarea::Dialogo, &cat());
        assert!(
            orden[0].donde != EN_TU_PLACA,
            "el hilo lo lee un modelo a la altura"
        );
        assert_eq!(
            ids(&orden)[0],
            "ollama:gpt-oss:120b-cloud",
            "nube gratuita primero"
        );
        assert_eq!(
            ids(&orden).last().map(|s| *s),
            Some("ollama:granite3.3:2b"),
            "el local queda de último recurso"
        );
    }

    #[test]
    fn sin_local_el_lienzo_igual_tiene_red() {
        let solo_nube: Vec<Motor> = cat()
            .into_iter()
            .filter(|m| m.donde != EN_TU_PLACA)
            .collect();
        let orden = orden_para(Tarea::Lienzo, &solo_nube);
        assert!(!orden.is_empty(), "nunca se queda sin motor");
        assert!(orden.iter().all(|m| m.donde != EN_TU_PLACA));
    }

    #[test]
    fn las_herramientas_no_usan_el_local() {
        let orden = orden_para(Tarea::Herramientas, &cat());
        assert!(
            orden.iter().all(|m| m.donde != EN_TU_PLACA),
            "el local no puede tocar el mundo"
        );
        assert_eq!(ids(&orden)[0], "ollama:gpt-oss:120b-cloud");
    }

    #[test]
    fn un_motor_no_disponible_no_entra() {
        let mut c = cat();
        c[1].disponible = false; // el R1 se cayó
        let orden = orden_para(Tarea::Sintesis, &c);
        assert_eq!(ids(&orden)[0], "ollama:granite3.3:2b", "usa el que queda");
    }

    fn planilla_de_prueba() -> Value {
        // Lo que devolvió la corrida real: el chico gana enfocar, el grande gana crear,
        // delegar no lo acierta ninguno.
        serde_json::json!({
            "ganador_por_prueba": {
                "voz-enfocar":  {"motor": "ollama:granite3.3:2b", "ms": 9400,  "ok": true},
                "voz-crear":    {"motor": "ollama:deepseek-r1:7b", "ms": 16200, "ok": true},
                "voz-delegar":  {"motor": "ollama:deepseek-r1:7b", "ms": 13100, "ok": false},
                "condensar":    {"motor": "ollama:granite3.3:2b", "ms": 2300,  "ok": true},
                "braindump":    {"motor": "ollama:granite3.3:2b", "ms": 5100,  "ok": true}
            }
        })
    }

    #[test]
    fn la_planilla_recomienda_por_accion() {
        let p = planilla_de_prueba();
        // El ganador medido de condensar es el chico: la planilla corrige a la heurística, que
        // mandaba el más grande por ser "pensar despacio".
        assert_eq!(
            ganador_medido("condensar", Some(&p)).unwrap(),
            "ollama:granite3.3:2b"
        );
        assert_eq!(
            ganador_medido("braindump", Some(&p)).unwrap(),
            "ollama:granite3.3:2b"
        );
        // En voz, empatan enfocar (chico) y crear (grande): gana el más rápido de los dos.
        assert_eq!(
            ganador_medido("voz", Some(&p)).unwrap(),
            "ollama:granite3.3:2b"
        );
        // Delegar no lo acertó nadie: no recomienda a nadie.
        assert_eq!(ganador_medido("delegar", Some(&p)), None);
        assert_eq!(
            ganador_medido("voz", None),
            None,
            "sin planilla manda la heurística"
        );
    }

    #[test]
    fn el_ganador_medido_va_primero_y_el_resto_queda_de_respaldo() {
        let p = planilla_de_prueba();
        // Sin planilla, condensar arranca por el local más grande (heurística).
        let sin = plan_tarea(
            &cat(),
            Some(AUTO_TAREA),
            None,
            Tarea::Sintesis,
            "condensar",
            None,
        );
        assert_eq!(ids(&sin)[0], "ollama:deepseek-r1:7b");
        // Con planilla, arranca por el que ganó midiendo… y el resto sigue ahí por si falla.
        let con = plan_tarea(
            &cat(),
            Some(AUTO_TAREA),
            None,
            Tarea::Sintesis,
            "condensar",
            Some(&p),
        );
        assert_eq!(ids(&con)[0], "ollama:granite3.3:2b");
        assert_eq!(
            ids(&con).len(),
            ids(&sin).len(),
            "cambia el orden, no la red de seguridad"
        );
        assert!(ids(&con).contains(&"ollama:deepseek-r1:7b"));
    }

    #[test]
    fn auto_tarea_usa_el_ruteo_y_un_motor_a_mano_manda() {
        let elegido = plan_tarea(&cat(), Some(AUTO_TAREA), None, Tarea::Lienzo, "voz", None);
        assert_eq!(ids(&elegido)[0], "ollama:granite3.3:2b");
        // Si el usuario eligió uno a mano, su elección gana: el ruteo no lo pisa.
        let manual = plan_tarea(
            &cat(),
            Some("deepseek:deepseek-flash"),
            None,
            Tarea::Lienzo,
            "voz",
            None,
        );
        assert_eq!(ids(&manual), vec!["deepseek:deepseek-flash"]);
    }

    #[test]
    fn un_motor_inventado_no_es_valido() {
        let cat = vec!["granite3.3:2b".to_string(), "openai:deepseek".to_string()];
        assert!(motor_valido("granite3.3:2b", &cat), "uno del catálogo vale");
        assert!(motor_valido("openai:deepseek", &cat));
        assert!(motor_valido("auto", &cat), "la cadena configurada vale");
        assert!(motor_valido("auto:tarea", &cat), "y los ruteos por tarea");
        assert!(motor_valido("", &cat), "vacío = volver atrás");
        assert!(
            !motor_valido("motor-que-no-existe", &cat),
            "un inventado NO vale"
        );
        assert!(!motor_valido("granite3.3:3b", &cat), "ni uno parecido");
    }
}

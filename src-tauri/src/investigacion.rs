//! Investigación por fases — el nodo que crece mientras investiga.
//!
//! Cuatro fases, cada una con su trabajo y su mutación sobre el lienzo (el ciclo que el usuario
//! diseñó): 🌱 **Semilla** (nace el nodo), ⚔️ **Fricción** (fuentes reales, una por nodo),
//! 🧪 **Cápsula** (síntesis y poda de lo que sobró), 🚀 **Hexágono** (cristaliza en la bóveda).
//!
//! Reparto de trabajo medido: **Hermes** sale al mundo (buscar ✓, verificado con datos reales ✓) y
//! **DeepSeek** razona la síntesis (5/5 en la planilla ✓, centavos ✓). Todo corre de fondo: la ventana
//! nunca se congela y cada fase deja su paso en `investigacion.json` para que el panel lo muestre.
//!
//! Es **sólo lectura sobre el lienzo**: emite comandos (crear/enlazar/actualizar/condensar) en el mismo
//! formato que el plan de voz, y quien los aplica es la app, con el deshacer disponible.

use crate::server::AppState;
use serde_json::{json, Value};
use std::path::Path;

/// Las fases, en orden. El emoji es el mismo del documento del usuario.
pub const FASES: [(&str, &str, &str); 4] = [
    ("semilla", "Semilla", "🌱"),
    ("friccion", "Fricción", "⚔️"),
    ("capsula", "Cápsula", "🧪"),
    ("hexagono", "Hexágono", "🚀"),
];

fn archivo(dir: &Path) -> std::path::PathBuf {
    dir.join("investigacion.json")
}

/// El estado de la investigación en curso (o `null`).
pub fn leer(data_dir: &Path) -> Value {
    std::fs::read_to_string(archivo(data_dir))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .unwrap_or(Value::Null)
}

/// Cuánto puede vivir la bandera de «en curso» sin que nadie la renueve.
///
/// Una investigación que termina borra su bandera sola, pero un proceso que muere a mitad de
/// camino (kill, crash, equipo apagado) la deja puesta — y entonces la app cree para siempre que
/// hay una corriendo y **rechaza toda investigación nueva**. Pasado el tope, la bandera es basura
/// y se limpia sola: la misma idea que el `expira solo a los 30 min` del hilo de diálogo.
const TOPE_CORRIENDO_S: u64 = 900;

/// Segundos desde la época (0 si el reloj no coopera).
fn ahora_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// ¿Hay una investigación corriendo **de verdad**? Una bandera vencida o ilegible se descarta y se
/// borra: el «está corriendo» no puede sobrevivir a la corrida que lo escribió.
pub fn en_curso(data_dir: &Path) -> bool {
    let bandera = data_dir.join("investigacion.corriendo");
    let Ok(texto) = std::fs::read_to_string(&bandera) else {
        return false;
    };
    let inicio: u64 = texto.trim().parse().unwrap_or(0);
    let edad = ahora_s().saturating_sub(inicio);
    if inicio == 0 || edad > TOPE_CORRIENDO_S {
        let _ = std::fs::remove_file(&bandera);
        return false;
    }
    true
}

/// Un paso de la investigación: qué fase, qué hizo y qué comandos deja para el lienzo.
fn anotar(data_dir: &Path, fase: &str, que: &str, comandos: Vec<Value>) {
    let previo = leer(data_dir);
    let mut pasos = previo["pasos"].as_array().cloned().unwrap_or_default();
    let (_, titulo, emoji) = FASES.iter().find(|(id, _, _)| *id == fase).copied().unwrap_or(("", "", ""));
    pasos.push(json!({
        "fase": fase,
        "titulo": titulo,
        "emoji": emoji,
        "que": que,
        "comandos": comandos,
        "cuando": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    }));
    let estado = json!({
        "pedido": previo["pedido"].clone(),
        "fase": fase,
        "pasos": pasos,
        "terminado": false,
    });
    let _ = std::fs::write(archivo(data_dir), serde_json::to_string_pretty(&estado).unwrap_or_default());
}

fn terminar(data_dir: &Path, resumen: &str, ok: bool) {
    let mut estado = leer(data_dir);
    if let Some(obj) = estado.as_object_mut() {
        obj.insert("terminado".into(), json!(true));
        obj.insert("ok".into(), json!(ok));
        obj.insert("salida".into(), json!(resumen));
    }
    let _ = std::fs::write(archivo(data_dir), serde_json::to_string_pretty(&estado).unwrap_or_default());
    let _ = std::fs::remove_file(data_dir.join("investigacion.corriendo"));
}

/// Extrae el primer objeto JSON de un texto (los modelos suelen agregar prosa alrededor).
pub fn primer_json(texto: &str) -> Option<Value> {
    let inicio = texto.find('{')?;
    let fin = texto.rfind('}')?;
    if fin <= inicio {
        return None;
    }
    serde_json::from_str(&texto[inicio..=fin]).ok()
}

/// Las fuentes que devolvió el motor del mundo, **validadas**: con título y url, y nunca más de 5.
pub fn fuentes_validas(v: &Value) -> Vec<Value> {
    v["fuentes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let titulo = f["titulo"].as_str().unwrap_or("").trim();
            let url = f["url"].as_str().unwrap_or("").trim();
            if titulo.len() < 3 || !url.starts_with("http") {
                return None;
            }
            Some(json!({
                "titulo": titulo.chars().take(120).collect::<String>(),
                "url": url.chars().take(300).collect::<String>(),
                "por_que": f["por_que"].as_str().unwrap_or("").chars().take(200).collect::<String>(),
            }))
        })
        .take(5)
        .collect()
}

/// Los comandos que hacen crecer el lienzo con las fuentes halladas.
pub fn comandos_de_fuentes(pedido: &str, fuentes: &[Value]) -> Vec<Value> {
    // Ojo: el nodo central **ya lo creó la fase Semilla**. Repetir el `crear` acá duplicaría la
    // investigación en el lienzo. Los enlaces funcionan igual porque el ejecutor resuelve por título.
    let mut comandos: Vec<Value> = Vec::new();
    let _ = pedido;
    for f in fuentes {
        let titulo = f["titulo"].as_str().unwrap_or("");
        comandos.push(json!({
            "accion": "crear",
            "titulo": titulo,
            "descripcion": format!("{}\n{}", f["url"].as_str().unwrap_or(""), f["por_que"].as_str().unwrap_or("")),
            "categoria": "FUENTE",
            "maturity": 2,
        }));
        comandos.push(json!({
            "accion": "enlazar",
            "desde": format!("Investigación: {}", recorta(pedido, 60)),
            "hasta": titulo,
        }));
    }
    comandos
}

/// La mutación de la fase Cápsula: el nodo central queda con la síntesis y sube de fase.
pub fn comandos_de_sintesis(titulo_nodo: &str, resumen: &str, principio: &str, descartar: &[String]) -> Vec<Value> {
    let mut comandos = vec![json!({
        "accion": "actualizar",
        "titulo": titulo_nodo,
        "descripcion": format!("{}\n\nPrincipio: {}", recorta(resumen, 700), recorta(principio, 200)),
        "maturity": 3,
        "tags": ["investigación", "síntesis"],
    })];
    if descartar.len() >= 2 {
        comandos.push(json!({
            "accion": "condensar",
            "nodos": descartar.to_vec(),
        }));
    }
    comandos
}

fn recorta(s: &str, n: usize) -> String {
    let limpio = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if limpio.chars().count() <= n {
        limpio
    } else {
        format!("{}…", limpio.chars().take(n).collect::<String>())
    }
}

/// Lee el JSON de la síntesis. Tolerante a propósito: si el motor lo envolvió en prosa o en markdown,
/// se busca el objeto adentro. Devuelve vacío si no hay resumen — y eso el llamador lo trata como fallo,
/// no como éxito silencioso.
pub fn leer_sintesis(texto: &str) -> (String, String, Vec<String>) {
    let v = match primer_json(texto) {
        Some(v) => v,
        None => return (String::new(), String::new(), Vec::new()),
    };
    (
        v["resumen"].as_str().unwrap_or("").trim().to_string(),
        v["principio"].as_str().unwrap_or("").trim().to_string(),
        v["descartar"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|d| d.as_str().map(String::from))
            .collect(),
    )
}

/// **Fallback declarado** para la síntesis: primero el motor barato y medido (DeepSeek) y, si falla o no
/// devuelve algo usable, Hermes — que además corre en la sesión del cerebro (ve el lienzo y tiene
/// herramientas). Antes un proveedor caído dejaba la investigación a mitad de camino **en silencio**:
/// el error se descartaba con `Err(_)` y la fase terminaba sin decir por qué (pasado el 15/09 con
/// DeepSeek inestable, la investigación quedaba inservible sin explicación).
async fn sintetizar(st: &AppState, prompt: &str) -> Result<(String, String, Vec<String>), String> {
    match correr_deepseek(st, prompt, 180).await {
        Ok(texto) => {
            let (resumen, principio, descartar) = leer_sintesis(&texto);
            if !resumen.is_empty() {
                return Ok((resumen, principio, descartar));
            }
            log::warn!("investigación: DeepSeek respondió sin resumen usable; la síntesis pasa a Hermes");
        }
        Err(e) => log::warn!("investigación: DeepSeek no respondió ({e}); la síntesis pasa a Hermes"),
    }
    match correr_hermes(st, prompt, 300).await {
        Ok(texto) => {
            let (resumen, principio, descartar) = leer_sintesis(&texto);
            if resumen.is_empty() {
                return Err("los dos motores respondieron, pero ninguno devolvió un resumen usable".into());
            }
            Ok((resumen, principio, descartar))
        }
        Err(e) => Err(format!("DeepSeek no respondió y Hermes tampoco: {e}")),
    }
}

/// Corre la investigación completa. Es lo que arranca `POST /api/ai/investigar`.
pub async fn correr(st: &AppState, pedido: String) {
    let dir = st.data_dir.clone();
    let nodo_central = format!("Investigación: {}", recorta(&pedido, 60));

    // ── 🌱 Semilla: nace el nodo, al instante ────────────────────────────────────────────────
    anotar(
        &dir,
        "semilla",
        &format!("Nació el nodo «{nodo_central}» y arrancó la búsqueda."),
        vec![json!({
            "accion": "crear",
            "titulo": nodo_central,
            "descripcion": format!("Investigando: {}. Fase Semilla.", recorta(&pedido, 160)),
            "categoria": "INVESTIGACIÓN",
        })],
    );

    // ── ⚔️ Fricción: el mundo (Hermes busca; es el único con herramientas) ────────────────────
    let prompt_fuentes = format!(
        "Buscá en la web 3 a 5 fuentes REALES sobre: {pedido}\n\n\
         Devolvé SOLO un objeto JSON, sin explicaciones ni markdown:\n\
         {{\"fuentes\":[{{\"titulo\":\"…\",\"url\":\"https://…\",\"por_que\":\"…por qué sirve…\"}}]}}\n\
         Las URL tienen que existir de verdad: son la evidencia de esta investigación."
    );
    let fuentes: Vec<Value> = match buscar_con_tavily(st, &pedido).await {
        Ok(f) => {
            log::info!("investigación: {} fuentes de Tavily", f.len());
            f
        }
        Err(motivo) => {
            log::info!("investigación: Tavily no está disponible ({motivo}); salgo con Hermes");
            match correr_hermes(st, &prompt_fuentes, 300).await {
                Ok(texto) => primer_json(&texto).map(|v| fuentes_validas(&v)).unwrap_or_default(),
                Err(e) => {
                    terminar(&dir, &format!("No pude salir a buscar: {e}"), false);
                    return;
                }
            }
        }
    };
    if fuentes.is_empty() {
        terminar(&dir, "La búsqueda no devolvió fuentes usables (títulos y URL válidas).", false);
        return;
    }
    anotar(
        &dir,
        "friccion",
        // Las fuentes no se vuelven nodos: viajan como datos de la fase y la app las mete en la
        // nota del nodo central (ver `proponer` en App.tsx). El lienzo no se llena de bibliografía.
        &format!("Encontró {} fuentes: van dentro de la nota del nodo.", fuentes.len()),
        comandos_de_fuentes(&pedido, &fuentes),
    );

    // ── 🧪 Cápsula: razona la síntesis (DeepSeek, que mide 5/5) ──────────────────────────────
    let listado = fuentes
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let cabeza = format!(
                "[{}] {} ({})\n{}",
                i + 1,
                f["titulo"].as_str().unwrap_or(""),
                f["url"].as_str().unwrap_or(""),
                f["por_que"].as_str().unwrap_or("")
            );
            match f["contenido"].as_str() {
                Some(c) if !c.trim().is_empty() => format!("{cabeza}\nExtracto real: {}", c.chars().take(1200).collect::<String>()),
                _ => cabeza,
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let prompt_sintesis = format!(
        "Investigación: {pedido}\n\nFuentes encontradas:\n{listado}\n\n\
         Con eso, devolvé SOLO un JSON. Apoyate en los extractos reales cuando estén, y si algo no está en\n\
         las fuentes, no lo afirmes.\n\
         {{\"resumen\":\"3 o 4 frases con el hallazgo concreto\",\"principio\":\"una oración: el principio sólido que queda\",\
         \"descartar\":[\"títulos de fuentes que no aportan, si hay\"]}}\n\
         Si una fuente no aporta al hallazgo, decila en 'descartar'."
    );
    let (resumen, principio, descartar) = match sintetizar(st, &prompt_sintesis).await {
        Ok(t) => t,
        Err(e) => {
            terminar(&dir, &format!("Se hallaron las fuentes, pero la síntesis no salió: {e}"), false);
            return;
        }
    };
    anotar(
        &dir,
        "capsula",
        &format!("Sintetizó el hallazgo{}", if descartar.len() > 1 { format!(" y descartó {} fuentes", descartar.len()) } else { String::new() }),
        comandos_de_sintesis(&nodo_central, &resumen, &principio, &descartar),
    );

    // ── 🚀 Hexágono: cristaliza (la bóveda guarda la nota del nodo sola) ─────────────────────
    anotar(
        &dir,
        "hexagono",
        "Cristalizó: el nodo queda en fase Hexágono, listo para la bóveda.",
        vec![json!({
            "accion": "actualizar",
            "titulo": nodo_central,
            "maturity": 5,
            "tags": ["investigación", "cristalizado"],
        })],
    );
    terminar(&dir, &resumen, true);
}

/// La clave de Tavily: resolución central (`claves.rs`: entorno → `.env` → **llavero** → config).
/// Si además la pegaron a mano dentro del campo `clave_config` de un proveedor, también se acepta.
pub fn clave_tavily(st: &AppState) -> Option<String> {
    if let Some(v) = crate::claves::obtener("tavily_api_key", &st.data_dir) {
        return Some(v);
    }
    let txt = std::fs::read_to_string(st.data_dir.join("nodeflow.config.json")).ok()?;
    let cfg: Value = serde_json::from_str(&txt).ok()?;
    cfg["proveedores"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["clave_config"].as_str())
        .find(|k| k.starts_with("tvly-"))
        .map(String::from)
}

/// **Tavily** — búsqueda estructurada para LLMs: trae fuentes limpias **y su contenido**, que es lo que
/// permite sintetizar citando en vez de recordar. Si no hay clave, la investigación sigue con Hermes.
pub async fn buscar_con_tavily(st: &AppState, consulta: &str) -> Result<Vec<Value>, String> {
    let clave = clave_tavily(st).ok_or("sin clave de Tavily")?;
    let r = st
        .http
        .post("https://api.tavily.com/search")
        .bearer_auth(clave)
        .json(&json!({
            "query": consulta,
            "search_depth": "advanced",
            "max_results": 5,
            "include_raw_content": "markdown",
        }))
        .timeout(std::time::Duration::from_secs(90))
        .send()
        .await
        .map_err(|e| format!("Tavily no respondió: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("Tavily respondió {}", r.status()));
    }
    let v: Value = r.json().await.map_err(|e| format!("respuesta ilegible: {e}"))?;
    let fuentes: Vec<Value> = v["results"]
        .as_array()
        .into_iter()
        .flatten()
        .take(5)
        .filter_map(|x| {
            let url = x["url"].as_str()?.trim();
            if !url.starts_with("http") {
                return None;
            }
            let titulo = x["title"].as_str().unwrap_or(url).trim();
            let contenido = x["raw_content"].as_str().unwrap_or("").trim();
            Some(json!({
                "titulo": titulo.chars().take(120).collect::<String>(),
                "url": url.chars().take(300).collect::<String>(),
                "por_que": x["content"].as_str().unwrap_or("").chars().take(240).collect::<String>(),
                "contenido": contenido.chars().take(2000).collect::<String>(),
            }))
        })
        .collect();
    if fuentes.is_empty() {
        return Err("Tavily no devolvió resultados usables".into());
    }
    Ok(fuentes)
}

/// Una pasada de Hermes (tiene las herramientas: web, archivos, terminal).
async fn correr_hermes(st: &AppState, prompt: &str, tope_s: u64) -> Result<String, String> {
    let exe = crate::voz::hermes_exe();
    // Fase 2: la investigación corre en la **misma sesión nombrada** que los turnos del panel. Antes era
    // un `-z` suelto: investigaba, olvidaba todo y el usuario quedaba con un cerebro amnésico.
    let args = crate::cerebro::argv(prompt, &st.cerebro);
    tokio::task::spawn_blocking(move || {
        crate::cerebro::correr(&exe, &args, std::time::Duration::from_secs(tope_s))
    })
    .await
    .map_err(|e| format!("{e}"))?
}

/// Una pasada de **DeepSeek** (el motor que la planilla midió 5/5), por el mismo camino que la app
/// usa para sus acciones: con clave, con costo medido y **sin caché** (es una investigación nueva).
async fn correr_deepseek(st: &AppState, prompt: &str, tope_s: u64) -> Result<String, String> {
    // El motor de síntesis: el primer DeepSeek del catálogo de proveedores (el chat, no el razonador:
    // para sintetizar alcanza, es más rápido y más barato).
    let motor = catalogo_deepseek(st).ok_or("no hay un motor DeepSeek configurado (falta la clave)")?;
    let clave = clave_del_motor(st, &motor).ok_or("falta la clave de DeepSeek")?;
    let url = format!("{}/chat/completions", motor.base_url.clone().unwrap_or_default().trim_end_matches('/'));
    let cuerpo = json!({
        "model": motor.modelo,
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": 0.4,
    });
    let r = st
        .http
        .post(&url)
        .bearer_auth(clave)
        .json(&cuerpo)
        .timeout(std::time::Duration::from_secs(tope_s))
        .send()
        .await
        .map_err(|e| format!("DeepSeek no respondió: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("DeepSeek respondió {}", r.status()));
    }
    let v: Value = r.json().await.map_err(|e| format!("respuesta ilegible: {e}"))?;
    Ok(v["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string())
}

fn catalogo_deepseek(st: &AppState) -> Option<crate::motores::Motor> {
    let txt = std::fs::read_to_string(st.data_dir.join("nodeflow.config.json")).ok()?;
    let cfg: Value = serde_json::from_str(&txt).ok()?;
    cfg["proveedores"].as_array().into_iter().flatten().find_map(|p| {
        let modelo = p["modelo"].as_str()?;
        if !modelo.contains("deepseek") || modelo.contains("reasoner") {
            return None; // para sintetizar alcanza el chat: es más rápido y más barato
        }
        let base = p["base_url"].as_str()?;
        let mut m = crate::motores::Motor::nuevo("openai", modelo, crate::motores::NUBE_PAGA, None, Some(base.to_string()));
        m.id = format!("openai:{}", p["id"].as_str().unwrap_or("deepseek"));
        m.clave_ref = p["clave_config"].as_str().map(String::from);
        Some(m)
    })
}

fn clave_del_motor(st: &AppState, m: &crate::motores::Motor) -> Option<String> {
    let nombre = m.clave_ref.clone()?;
    if let Ok(v) = std::env::var(&nombre) {
        if !v.trim().is_empty() {
            return Some(v);
        }
    }
    let txt = std::fs::read_to_string(st.data_dir.join("nodeflow.config.json")).ok()?;
    let cfg: Value = serde_json::from_str(&txt).ok()?;
    cfg[&nombre]
        .as_str()
        .map(String::from)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| (nombre.len() > 20 && !nombre.contains(char::is_whitespace)).then(|| nombre.clone()))
}

/// Arranca la investigación (endpoint `POST /api/ai/investigar`).
pub async fn iniciar(st: &AppState, pedido: String) -> Result<(), String> {
    if en_curso(&st.data_dir) {
        return Err("ya hay una investigación corriendo".into());
    }
    let inicial = json!({ "pedido": pedido, "fase": "semilla", "pasos": [], "terminado": false });
    std::fs::write(
        archivo(&st.data_dir),
        serde_json::to_string_pretty(&inicial).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    // El instante de arranque, no un "1": así la bandera puede vencer (ver `en_curso`).
    let _ = std::fs::write(st.data_dir.join("investigacion.corriendo"), ahora_s().to_string());
    let st2 = st.clone();
    tokio::spawn(async move { correr(&st2, pedido).await });
    Ok(())
}

#[cfg(test)]
mod tests_investigacion {
    use super::*;

    #[test]
    fn saca_el_json_de_entre_la_prosa() {
        let texto = "Claro, acá va:\n```json\n{\"fuentes\":[{\"titulo\":\"Sensirion\",\"url\":\"https://sensirion.com\"}]}\n```\nEspero que sirva.";
        let v = primer_json(texto).expect("tiene que encontrar el objeto");
        assert_eq!(v["fuentes"][0]["titulo"], "Sensirion");
        assert!(primer_json("no hay json acá").is_none());
    }

    #[test]
    fn solo_pasan_fuentes_con_titulo_y_url_real() {
        let crudo = json!({"fuentes": [
            {"titulo": "Sensirion SHT31", "url": "https://sensirion.com/sht31", "por_que": "fabricante"},
            {"titulo": "sin url", "url": "no-es-una-url"},
            {"titulo": "x", "url": "https://corta.com"},
            {"titulo": "Otra buena", "url": "https://ejemplo.com/doc"}
        ]});
        let ok = fuentes_validas(&crudo);
        assert_eq!(ok.len(), 2, "una sin url y una con título de 1 letra se descartan");
        assert_eq!(ok[0]["titulo"], "Sensirion SHT31");
    }

    #[test]
    fn las_fuentes_arman_nodos_y_enlaces() {
        let fuentes = vec![json!({"titulo": "Fuente A", "url": "https://a.com", "por_que": "x"})];
        let comandos = comandos_de_fuentes("sensores de humedad", &fuentes);
        // (1 crear + 1 enlazar) por fuente — el nodo central NO se repite: lo hizo la Semilla.
        assert_eq!(comandos.len(), 2, "no se duplica el nodo central");
        assert_eq!(comandos[0]["accion"], "crear");
        assert_eq!(comandos[0]["categoria"], "FUENTE");
        assert_eq!(comandos[1]["accion"], "enlazar");
        assert_eq!(comandos[1]["hasta"], "Fuente A", "el enlace apunta al título, que el ejecutor resuelve");
        assert!(comandos[1]["desde"].as_str().unwrap().starts_with("Investigación: "));
    }

    #[test]
    fn la_sintesis_muta_el_nodo_y_poda_si_sobra() {
        let con_poda = comandos_de_sintesis("Investigación: x", "resumen largo del hallazgo", "el principio", &["A".into(), "B".into()]);
        assert_eq!(con_poda[0]["accion"], "actualizar");
        assert_eq!(con_poda[0]["maturity"], 3, "Cápsula es la fase 3");
        assert_eq!(con_poda[1]["accion"], "condensar", "con 2 o más sobrantes se poda");
        let sin_poda = comandos_de_sintesis("Investigación: x", "resumen", "principio", &[]);
        assert_eq!(sin_poda.len(), 1, "sin sobrantes no se poda nada");
    }

    #[test]
    fn una_bandera_vencida_no_deja_la_app_creyendo_que_investiga() {
        let dir = std::env::temp_dir().join(format!("nf-inv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bandera = dir.join("investigacion.corriendo");
        // bandera fresca: hay investigación en curso
        std::fs::write(&bandera, ahora_s().to_string()).unwrap();
        assert!(en_curso(&dir));
        // bandera vieja (proceso muerto a mitad): se descarta y se limpia sola
        std::fs::write(&bandera, (ahora_s() - TOPE_CORRIENDO_S - 5).to_string()).unwrap();
        assert!(!en_curso(&dir));
        assert!(!bandera.exists(), "la bandera vencida tiene que quedar borrada");
        // bandera ilegible (formato viejo "1"): también se descarta
        std::fs::write(&bandera, "1").unwrap();
        assert!(!en_curso(&dir));
        assert!(!bandera.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn las_cuatro_fases_estan_en_orden() {
        assert_eq!(FASES.len(), 4);
        assert_eq!(FASES[0].0, "semilla");
        assert_eq!(FASES[3].0, "hexagono");
        assert_eq!(FASES[3].2, "🚀");
    }
    #[test]
    fn lee_la_sintesis_del_json_limpio() {
        let (r, p, d) = super::leer_sintesis(
            "{\"resumen\":\"Q4_K_M de 7B entra en 12 GB\",\"principio\":\"mejor calidad por GB\",\"descartar\":[\"uno\"]}",
        );
        assert_eq!(r, "Q4_K_M de 7B entra en 12 GB");
        assert_eq!(p, "mejor calidad por GB");
        assert_eq!(d, vec!["uno".to_string()]);
    }

    #[test]
    fn lee_la_sintesis_envuelta_en_prosa() {
        let (r, _, _) = super::leer_sintesis("Claro, acá va:\n```json\n{\"resumen\":\"ok\"}\n```\nSaludos.");
        assert_eq!(r, "ok", "el motor a veces envuelve el JSON: hay que encontrarlo igual");
    }

    #[test]
    fn sin_resumen_es_fallo_no_exito() {
        let (r, _, _) = super::leer_sintesis("no tengo idea");
        assert!(r.is_empty(), "vacío = el llamador reintenta con el otro motor");
    }



}

//! Fase 1 del plan del cerebro residente (ver `docs/PLAN-CEREBRO-RESIDENTE.md`).
//!
//! El motor profundo deja de ser un one-shot sin memoria: cada delegación corre en una **sesión
//! nombrada** de Hermes (`hermes chat -c <nombre> --create-if-missing`), así el turno siguiente
//! recuerda lo que se habló — sin que la app tenga que orquestar ids — y deja una **nota episódica**
//! en la bóveda. La memoria del proyecto vive en el lienzo; Hermes es el ejecutor.
//!
//! Verificado el 15/09 sobre la máquina: dos procesos distintos, mismo `-c nf-cerebro`, el segundo
//! recordó el dato del primero. `--create-if-missing` **sólo existe en `hermes chat`**, no en la forma
//! `hermes -z`, y el prompt va por `-q` (posicional falla).

use serde_json::Value;

/// La carpeta del cerebro dentro de la bóveda: una nota por turno. La usan el que nombra y el que cuenta.
pub const CARPETA: &str = "cerebro";
/// Planes del cerebro: una nota por tema, dentro de `cerebro/planes/`.
pub const PLANES: &str = "planes";
/// La bitácora: el diario de decisiones del cerebro (append-only).
pub const BITACORA: &str = "bitacora.md";

/// Parámetros del cerebro residente. Prioridad: variable de entorno → `nodeflow.config.json`
/// (`"cerebro": {...}`) → default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Nombre de la sesión de Hermes. Es la memoria del proyecto: cambiarlo arranca una mente nueva.
    pub sesion: String,
    /// ¿Deja nota episódica en la bóveda al terminar cada turno?
    pub notas: bool,
    /// Presupuesto de la corrida que se le pasa a Hermes (`--run-budget`, segundos de trabajo del agente).
    pub run_budget_s: u64,
    /// Tope duro del proceso en la app (mata el hijo si se pasa): red de seguridad sobre el anterior.
    pub tope_s: u64,
    /// Desfase horario del usuario en horas (±14) para nombrar y fechar la nota episódica en **hora
    /// local**. `0` = UTC.
    pub offset_h: i32,
    /// Fase 4 — puerto del gateway propio (`hermes serve`). 9119 es el del escritorio de Hermes: acá se
    /// usa 9121 para no pisarlo.
    pub gateway_puerto: u16,
    /// Fase 5.4 — raíz del repo de NodeFlow: de ahí se inventaría la arquitectura. Si está vacío se busca
    /// desde la ubicación del ejecutable (o desde `NODEFLOW_REPO`).
    pub repo: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            sesion: "nf-cerebro".into(),
            notas: true,
            run_budget_s: 900,
            tope_s: 1200,
            offset_h: -3,
            repo: std::env::var("NODEFLOW_REPO").unwrap_or_default(),
            gateway_puerto: crate::cerebro_gateway::PUERTO_POR_DEFECTO,
        }
    }
}

impl Config {
    pub fn desde(cfg: Option<&Value>) -> Config {
        let mut c = Config::default();
        if let Some(s) = cfg {
            if let Some(v) = s
                .get("sesion")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
            {
                c.sesion = v.trim().to_string();
            }
            if let Some(v) = s.get("notas").and_then(|v| v.as_bool()) {
                c.notas = v;
            }
            if let Some(v) = s.get("run_budget_s").and_then(|v| v.as_u64()) {
                c.run_budget_s = v.clamp(60, 7200);
            }
            if let Some(v) = s.get("tope_s").and_then(|v| v.as_u64()) {
                c.tope_s = v.clamp(120, 14_400);
            }
            if let Some(v) = s.get("offset_h").and_then(|v| v.as_i64()) {
                c.offset_h = (v as i32).clamp(-14, 14);
            }
            if let Some(v) = s.get("gateway_puerto").and_then(|v| v.as_u64()) {
                c.gateway_puerto = (v as u16).clamp(1024, 65_535);
            }
            if let Some(v) = s
                .get("repo")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
            {
                c.repo = v.trim().to_string();
            }
        }
        if let Ok(v) = std::env::var("NODEFLOW_CEREBRO_SESION") {
            if !v.trim().is_empty() {
                c.sesion = v.trim().to_string();
            }
        }
        if let Ok(v) = std::env::var("NODEFLOW_REPO") {
            if !v.trim().is_empty() {
                c.repo = v.trim().to_string();
            }
        }
        if let Ok(v) = std::env::var("NODEFLOW_CEREBRO_OFFSET_H") {
            if let Ok(n) = v.trim().parse::<i32>() {
                c.offset_h = n.clamp(-14, 14);
            }
        }
        if let Ok(v) = std::env::var("NODEFLOW_CEREBRO_NOTAS") {
            let v = v.trim().to_ascii_lowercase();
            c.notas = !matches!(v.as_str(), "0" | "false" | "no" | "off");
        }
        // El tope duro nunca puede quedar por debajo del presupuesto: mataría la corrida antes de que
        // Hermes sepa que se quedó sin presupuesto.
        if c.tope_s <= c.run_budget_s {
            c.tope_s = c.run_budget_s + 120;
        }
        c
    }
}

/// Corre `hermes` con argumentos explícitos, **sin consola** y con tope de tiempo (mata el hijo si se
/// pasa). Vive acá para que **todos** los caminos de la app que despiertan a Hermes compartan sesión y
/// forma de invocarlo: antes había una copia en `server.rs` (one-shot) y otra en `investigacion.rs`
/// (también one-shot), y ninguna tenía memoria.
pub fn correr(exe: &str, args: &[String], tope: std::time::Duration) -> Result<String, String> {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut hijo = cmd
        .spawn()
        .map_err(|e| format!("No pude iniciar el motor profundo ({exe}): {e}"))?;
    let inicio = std::time::Instant::now();
    loop {
        match hijo.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if inicio.elapsed() > tope {
                    let _ = hijo.kill();
                    return Err(format!(
                        "El motor profundo tardó más de {} s y lo detuve.",
                        tope.as_secs()
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Err(e) => return Err(format!("no pude seguir el proceso del motor profundo: {e}")),
        }
    }
    let salida = hijo.wait_with_output().map_err(|e| e.to_string())?;
    let texto = String::from_utf8_lossy(&salida.stdout).trim().to_string();
    if texto.is_empty() {
        let err = String::from_utf8_lossy(&salida.stderr);
        return Err(format!(
            "el motor profundo no devolvió texto ({})",
            err.trim().chars().take(200).collect::<String>()
        ));
    }
    Ok(texto)
}

/// Lo que la app le manda al agente como contexto del turno. **Nada de esto crece con el tamaño del
/// lienzo**: son la visión, un puñado de recuerdos dirigidos y el puntero a las herramientas.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contexto {
    /// El nodo del Norte Estratégico: la visión del proyecto, siempre presente.
    pub norte: Option<String>,
    /// Notas de la bóveda que la memoria (BM25) consideró relevantes para **este** pedido: (título, ruta).
    pub memoria: Vec<(String, String)>,
    /// Foco del hilo de diálogo (títulos). Da continuidad sin arrastrar toda la conversación.
    pub foco: Vec<String>,
    pub nodos: usize,
    pub aristas: usize,
    /// Propuestas esperando aprobación humana en la cola.
    pub pendientes: usize,
    /// El camino del proyecto: los nodos de EJECUCIÓN (fases), en orden.
    pub camino: Vec<String>,
    /// Lo abierto: nodos PENDIENTE del lienzo.
    pub abiertos: Vec<String>,
    /// Los últimos hitos (HITO): lo que ya está hecho, para no reproponerlo.
    pub hitos: Vec<String>,
    /// Las notas propias del cerebro (planes, bitácora): lo que pensé antes, en la bóveda.
    pub mis_notas: Vec<String>,
    /// Fase 5.4 — el bloque acotado de la arquitectura de la app (inventariado del árbol real). Va acá
    /// para que el turno hable del código **sin** leer el repo: es contexto fijo, de una vez por turno.
    pub arquitectura: String,
}

/// Extrae el bloque acotado de la arquitectura (`<!-- mapa --> … <!-- /mapa -->`) de la nota generada.
/// Devuelve `None` si no está: el turno simplemente no lleva esa sección.
pub fn mapa_de_la_nota(texto: &str) -> Option<String> {
    let ini = texto.find(crate::cerebro_arquitectura::MAPA_INICIO)?;
    let desde = ini + crate::cerebro_arquitectura::MAPA_INICIO.len();
    let fin = texto[desde..].find(crate::cerebro_arquitectura::MAPA_FIN)? + desde;
    let dentro = texto[desde..fin].trim();
    if dentro.is_empty() {
        None
    } else {
        Some(dentro.to_string())
    }
}

/// Cuántas notas de turno hay en la carpeta del cerebro. El panel lo muestra como señal de que la
/// memoria crece en la bóveda (y no en el chat): una nota por turno, con su fecha local.
pub fn contar_notas(raiz_boveda: &std::path::Path) -> usize {
    std::fs::read_dir(raiz_boveda.join(CARPETA))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .and_then(|x| x.to_str())
                        .map(|x| x.eq_ignore_ascii_case("md"))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

/// Una entrada de bitácora: encabezado con fecha y quién la escribió, y el texto. Append-only.
pub fn entrada_bitacora(fecha_local: &str, quien: &str, texto: &str) -> String {
    format!("\n## {fecha_local} · {quien}\n\n{}\n", texto.trim())
}

/// Encabezado de la bitácora cuando todavía no existe.
pub fn encabezado_bitacora() -> String {
    "---\ntitulo: \"Bitácora del cerebro\"\ntipo: nota\ngenerado_por: cerebro\n---\n\n\
     # Bitácora del cerebro\n\n\
     Decisiones y criterios del cerebro residente, en orden. Lo escribe el agente, y el humano cuando\n\
     quiere dejarle algo. Cada entrada lleva fecha y quién la escribió.\n"
        .to_string()
}

/// El nombre de un plan es un slug: es un nombre de archivo y viaja en rutas.
pub fn slug_plan(nombre: &str) -> Result<String, String> {
    let limpio: String = nombre
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c if c.is_ascii_alphanumeric() => c,
            c if c.is_whitespace() || c == '-' || c == '_' => '-',
            _ => '-',
        })
        .collect();
    let slug: String = limpio
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.chars().count() < 3 || slug.chars().count() > 60 {
        return Err(format!("nombre de plan inválido: «{nombre}» (3-60 chars)"));
    }
    Ok(slug)
}

/// Prompt del turno del cerebro. El cambio de fondo respecto del viejo `prompt_delegar`: la app **no**
/// le pasa el lienzo masticado (hasta 40 títulos, que crecían con el mapa) — le da la visión, el
/// recuerdo dirigido y el puntero a sus herramientas MCP, y el agente consulta lo que necesita.
pub fn prompt_turno(pedido: &str, c: &Contexto) -> String {
    let mut p = String::new();
    p.push_str("Sos el motor profundo de NodeFlow, un lienzo visual de ideas que vive en la bóveda del usuario.\n");
    p.push_str(&format!("Pedido del usuario: {pedido}\n\n"));
    p.push_str(&briefing_texto(c));
    // El estado de ahora (nodos, aristas, pendientes) va **acá**, no dentro del briefing: el briefing es
    // el bloque estable que el agente manda en el `system` (el prefijo que el proveedor cachea) y un
    // contador que se mueve en cada turno tira esa caché. Este prompt es un mensaje de usuario, así que
    // puede llevarlo sin costo.
    p.push_str(&estado_volatil(c));
    p.push('\n');
    if !c.memoria.is_empty() {
        p.push_str(
            "\nRecuerdo dirigido (notas de la bóveda que la memoria consideró relevantes):\n",
        );
        for (titulo, ruta) in c.memoria.iter().take(6) {
            p.push_str(&format!("- {titulo} ({ruta})\n"));
        }
    }
    if !c.foco.is_empty() {
        let foco: Vec<&str> = c.foco.iter().take(8).map(|s| s.as_str()).collect();
        p.push_str(&format!(
            "\nFoco del hilo de diálogo: {}\n",
            foco.join(" · ")
        ));
    }
    p.push_str(
        "\nTenés las herramientas MCP del lienzo (canvas_summary, canvas_stats, search_nodes, search_vault,\n\
         leer_nota, garden_scan, capture_knowledge, export_document, ...): usalas para mirar lo que necesites\n\
         antes de responder, en vez de suponer. Toda escritura que propongas queda en la cola de aprobación\n\
         del humano: no toca el lienzo por sí sola.\n\n\
         Respondé en 2 a 4 frases, en español, concreto y sin adornos: es lo que se va a mostrar en el panel y\n\
         puede convertirse en un nodo nuevo del lienzo.",
    );
    p
}

/// El **briefing**: el estado del proyecto en un bloque acotado que se antepone al pedido.
///
/// Es la versión aplicada de lo que pide el usuario en sus notas: en vez de subir todo el lienzo en cada
/// turno, va sólo lo que hace falta para saber dónde estamos parados — visión, camino, abierto, hitos,
/// notas propias y los recuerdos que el BM25 consideró relevantes para **este** pedido. No crece con el
/// tamaño del lienzo: todo va recortado.
pub fn briefing_texto(c: &Contexto) -> String {
    let mut p = String::new();
    if let Some(n) = c.norte.as_deref().filter(|n| !n.trim().is_empty()) {
        p.push_str(&format!("Visión del proyecto (Norte Estratégico): {n}\n"));
    }
    // OJO: acá **no** va nada que cambie entre turnos. El briefing viaja en el `system`, que es el
    // prefijo que el proveedor cachea: un contador que se mueve (nodos, aristas, pendientes) invalida
    // la caché de todo lo que viene detrás. El estado de ahora lo arma `estado_volatil()` y va al
    // final del mensaje del usuario.
    let linea = |p: &mut String, rotulo: &str, xs: &[String], tope: usize| {
        if xs.is_empty() {
            return;
        }
        let cortos: Vec<&str> = xs.iter().take(tope).map(|s| s.as_str()).collect();
        p.push_str(&format!("{rotulo}: {}\n", cortos.join(" · ")));
    };
    linea(&mut p, "Camino del proyecto", &c.camino, 8);
    linea(&mut p, "Abierto", &c.abiertos, 10);
    linea(&mut p, "Ya hecho", &c.hitos, 5);
    linea(&mut p, "Mis notas del cerebro", &c.mis_notas, 6);
    if !c.arquitectura.trim().is_empty() {
        p.push_str("\nArquitectura de la app (inventariada del árbol real: NO hace falta leer el repo para esto):\n");
        p.push_str(c.arquitectura.trim());
        p.push('\n');
    }
    if !c.memoria.is_empty() {
        p.push_str(
            "\nRecuerdo dirigido (notas de la bóveda que la memoria consideró relevantes):\n",
        );
        for (titulo, ruta) in c.memoria.iter().take(6) {
            p.push_str(&format!("- {titulo} ({ruta})\n"));
        }
    }
    if !c.foco.is_empty() {
        let foco: Vec<&str> = c.foco.iter().take(8).map(|s| s.as_str()).collect();
        p.push_str(&format!(
            "\nFoco del hilo de diálogo: {}\n",
            foco.join(" · ")
        ));
    }
    p
}

/// El **estado de ahora**, que sí cambia entre turnos: tamaño del lienzo y propuestas en cola.
///
/// Va al **final del mensaje del usuario**, nunca en el `system`: el system es el prefijo que el
/// proveedor cachea y todo lo que cambie dentro de él tira la caché de lo que venga después (medido el
/// 17/09: 80,8 % de la entrada venía de caché, a ~1/50 del precio).
pub fn estado_volatil(c: &Contexto) -> String {
    format!(
        "Lienzo ahora: {} nodos · {} aristas · {} propuesta(s) esperando aprobación del humano.",
        c.nodos, c.aristas, c.pendientes
    )
}

/// Resumen del contexto usado, para guardarlo en `delegacion.json` y que el panel pueda mostrar **qué**
/// se le mandó** (transparencia: el prompt no es una caja negra).
pub fn resumen_contexto(c: &Contexto) -> String {
    format!(
        "lienzo={}/{} · pendientes={} · norte={} · memoria=[{}] · foco=[{}]",
        c.nodos,
        c.aristas,
        c.pendientes,
        c.norte.as_deref().unwrap_or("-"),
        c.memoria
            .iter()
            .map(|(t, _)| t.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        c.foco.join(", ")
    )
}

/// Argumentos para `hermes chat`: sesión nombrada (creada si falta), salida limpia, una sola respuesta.
pub fn argv(prompt: &str, c: &Config) -> Vec<String> {
    vec![
        "chat".into(),
        "-q".into(),
        prompt.to_string(),
        "-c".into(),
        c.sesion.clone(),
        "--create-if-missing".into(),
        "-Q".into(),
        "--oneshot".into(),
        "--run-budget".into(),
        c.run_budget_s.to_string(),
    ]
}

/// Quita el ruido de la salida de Hermes: el `session_id:` que imprime al arrancar no es parte de la
/// respuesta, y el panel muestra esto tal cual. Devuelve también el id visto (para el log).
pub fn limpiar_salida(bruto: &str) -> (String, Option<String>) {
    let mut id = None;
    let mut lineas: Vec<&str> = Vec::new();
    for l in bruto.lines() {
        let t = l.trim();
        if let Some(resto) = t.strip_prefix("session_id:") {
            id = Some(resto.trim().to_string());
            continue;
        }
        if t.starts_with("Session ") && t.ends_with("Starting fresh.") {
            continue;
        }
        lineas.push(l);
    }
    (lineas.join("\n").trim().to_string(), id)
}

/// Nombre de la nota episódica, relativo a la bóveda. Se arma con el sello que le pasa el llamador
/// (fecha/hora) para que sea testeable sin reloj: `cerebro/2026-09-15-2110-turno.md`.
pub fn nombre_nota(sello: &str) -> String {
    let limpio: String = sello
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect();
    let limpio = limpio.trim_matches('-').to_string();
    format!("{CARPETA}/{limpio}-turno.md")
}

/// La nota que deja cada turno. Es la memoria episódica: qué se pidió, qué volvió y en qué sesión.
/// Frontmatter en el formato de la bóveda (mismo estilo que las notas de nodo, sin `id` de nodo).
pub fn nota_markdown(
    pedido: &str,
    resultado: &str,
    sesion: &str,
    ok: bool,
    ms: u64,
    fecha_local: &str,
    fecha_utc: &str,
) -> String {
    let esc = |s: &str| s.replace('"', "'").replace('\n', " ");
    // El **título es el pedido**, no «Turno del cerebro»: la memoria (BM25) saca el título del primer
    // `# ` del archivo, así que con un título genérico las cuatro notas de turno se pisaban en el
    // recuerdo y no decían de qué hablaban (medido: `memoria=[Turno del cerebro, Turno del cerebro, …]`).
    let titulo: String = pedido
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(90)
        .collect();
    format!(
        "---\ntipo: turno\ntitulo: \"{}\"\nfecha: \"{}\"\nfecha_utc: \"{}\"\nsesion: \"{}\"\nestado: {}\nms: {}\npedido: \"{}\"\n---\n\n# Turno: {}\n\n## Pedido\n{}\n\n## Respuesta\n{}\n",
        esc(&titulo),
        esc(fecha_local),
        esc(fecha_utc),
        esc(sesion),
        if ok { "ok" } else { "error" },
        ms,
        esc(pedido),
        titulo,
        pedido,
        if resultado.trim().is_empty() { "(sin respuesta)" } else { resultado }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuenta_las_notas_del_cerebro_y_ignora_lo_que_no_es_md() {
        let base = std::env::temp_dir().join(format!("nf-cerebro-test-{}", std::process::id()));
        let dir = base.join("cerebro");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("2026-09-15T21-16-turno.md"), "—").unwrap();
        std::fs::write(dir.join("2026-09-15T21-30-turno.md"), "—").unwrap();
        std::fs::write(dir.join("borrame.txt"), "—").unwrap();
        assert_eq!(
            super::contar_notas(&base),
            2,
            "sólo las notas .md del cerebro"
        );
        assert_eq!(
            super::contar_notas(&base.join("no-existe")),
            0,
            "sin carpeta no hay notas, no hay error"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn la_config_prioriza_entorno_config_y_default() {
        let cfg = serde_json::json!({ "sesion": "  cerebro-tomas  ", "notas": false, "run_budget_s": 10, "tope_s": 1 });
        let c = Config::desde(Some(&cfg));
        assert_eq!(c.sesion, "cerebro-tomas", "el nombre se limpia");
        assert!(!c.notas);
        assert_eq!(c.run_budget_s, 60, "el presupuesto se acota al mínimo");
        assert_eq!(c.tope_s, 120, "el tope duro se acota al mínimo razonable");
        assert!(c.tope_s > c.run_budget_s, "y siempre queda por encima del presupuesto: si no, la app mataría al agente antes de que sepa que se quedó sin presupuesto");
        let d = Config::desde(None);
        assert_eq!(d.sesion, "nf-cerebro");
        assert!(d.notas);
        assert_eq!(d.offset_h, -3, "por defecto, hora de Buenos Aires");
        let e = Config::desde(Some(&serde_json::json!({"offset_h": 99})));
        assert_eq!(e.offset_h, 14, "el desfase se acota");
    }

    #[test]
    fn el_briefing_no_lleva_nada_que_cambie_entre_turnos() {
        // El briefing viaja en el `system`: es el prefijo que el proveedor cachea. Un contador que se
        // mueve entre turnos (nodos, aristas, pendientes) invalidaba la caché de todo lo que venía
        // después —y la caché era el 80,8 % de la entrada medida, a ~1/50 del precio—.
        let a = Contexto {
            norte: Some("Norte Estratégico · NodeFlow".into()),
            nodos: 10,
            aristas: 12,
            pendientes: 0,
            ..Contexto::default()
        };
        let b = Contexto {
            nodos: 99,
            aristas: 120,
            pendientes: 7,
            ..a.clone()
        };
        assert_eq!(
            briefing_texto(&a),
            briefing_texto(&b),
            "el system tiene que ser byte a byte igual aunque el lienzo cambie"
        );
        assert!(
            estado_volatil(&a).contains("10 nodos") && estado_volatil(&b).contains("99 nodos"),
            "el estado de ahora sí dice el tamaño del lienzo: {} / {}",
            estado_volatil(&a),
            estado_volatil(&b)
        );
    }

    #[test]
    fn el_prompt_lleva_vision_recuerdo_y_el_puntero_a_las_herramientas() {
        let c = Contexto {
            norte: Some("Norte Estratégico · NodeFlow".into()),
            memoria: vec![
                ("Motor dual por tarea".into(), "nodos/motor-dual.md".into()),
                ("Caché semántica".into(), "nodos/cache.md".into()),
            ],
            foco: vec!["Motor dual".into()],
            nodos: 89,
            aristas: 119,
            pendientes: 2,
            ..Contexto::default()
        };
        let p = prompt_turno("¿por dónde sigo?", &c);
        assert!(p.contains("¿por dónde sigo?"));
        assert!(
            p.contains("Norte Estratégico · NodeFlow"),
            "la visión viaja siempre"
        );
        assert!(p.contains("89 nodos · 119 aristas · 2 propuesta(s)"));
        assert!(
            p.contains("- Motor dual por tarea (nodos/motor-dual.md)"),
            "recuerdo con su ruta"
        );
        assert!(p.contains("Foco del hilo de diálogo: Motor dual"));
        assert!(
            p.contains("herramientas MCP del lienzo"),
            "el agente consulta, no recibe el mapa masticado"
        );
    }

    #[test]
    fn el_mapa_sale_del_bloque_marcado() {
        let doc = "---\ntitulo: x\n---\n\n# Arquitectura\n\n<!-- mapa -->\n## Mapa\n- Rust: 22 módulos\n<!-- /mapa -->\n\n## Detalle\n";
        let m = mapa_de_la_nota(doc).unwrap();
        assert!(m.contains("Rust: 22 módulos"));
        assert!(
            !m.contains("## Detalle"),
            "no se lleva el detalle: sólo el mapa"
        );
        assert!(mapa_de_la_nota("# sin marcas").is_none());
    }

    #[test]
    fn la_entrada_de_bitacora_lleva_fecha_y_quien() {
        let e = entrada_bitacora(
            "2026-09-16T11:20",
            "cerebro",
            "  Probé el registro de herramientas.  ",
        );
        assert!(
            e.starts_with("\n## 2026-09-16T11:20 · cerebro\n"),
            "encabezado con fecha y autor"
        );
        assert!(
            e.trim_end().ends_with("Probé el registro de herramientas."),
            "el texto va recortado"
        );
    }

    #[test]
    fn el_slug_del_plan_es_un_nombre_de_archivo_y_no_una_ruta() {
        assert_eq!(
            slug_plan("Cerebro local en NodeFlow").unwrap(),
            "cerebro-local-en-nodeflow"
        );
        assert_eq!(
            slug_plan("  Visión  2026  ").unwrap(),
            "vision-2026",
            "acentos y espacios de más"
        );
        assert!(slug_plan("a").is_err(), "muy corto para ser un nombre");
        let s = slug_plan("../escape y ruta/C:\\x").unwrap();
        assert!(
            !s.contains('/') && !s.contains('.') && !s.contains('\\'),
            "el slug no puede traer rutas: {s}"
        );
    }

    #[test]
    fn el_briefing_lleva_el_estado_del_proyecto_y_no_secciones_vacias() {
        let c = Contexto {
            norte: Some("Norte Estratégico · NodeFlow".into()),
            camino: vec!["Fase 3b".into(), "Fase 4".into()],
            abiertos: vec!["Partir el skill".into()],
            hitos: vec!["v0.3.0 publicada".into()],
            mis_notas: vec!["plan-cerebro.md".into()],
            memoria: vec![("Caché semántica".into(), "nodos/cache.md".into())],
            nodos: 42,
            aristas: 72,
            pendientes: 2,
            foco: vec!["Fase 4 [n-ag-fase-4]".into()],
            arquitectura: "- **Rust**: 22 módulos · 21.000 líneas.".into(),
        };
        let b = briefing_texto(&c);
        assert!(b.contains("Visión del proyecto (Norte Estratégico): Norte Estratégico · NodeFlow"));
        assert!(b.contains("Camino del proyecto: Fase 3b · Fase 4"));
        assert!(b.contains("Abierto: Partir el skill"));
        assert!(b.contains("Ya hecho: v0.3.0 publicada"));
        assert!(b.contains("Mis notas del cerebro: plan-cerebro.md"));
        assert!(
            b.contains("Arquitectura de la app") && b.contains("22 módulos"),
            "el mapa de la arquitectura viaja en el turno: {b}"
        );
        assert!(
            estado_volatil(&c).contains("42 nodos · 72 aristas · 2 propuesta(s)"),
            "el estado de ahora viaja aparte: {}",
            estado_volatil(&c)
        );
        assert!(
            !b.contains("nodos ·"),
            "el briefing tiene que quedar estable (sin contadores): {b}"
        );
        assert!(b.contains("- Caché semántica (nodos/cache.md)"));
        assert!(b.contains("Foco del hilo de diálogo: Fase 4 [n-ag-fase-4]"));

        let vacio = briefing_texto(&Contexto::default());
        for rotulo in [
            "Camino del proyecto",
            "Abierto",
            "Ya hecho",
            "Mis notas",
            "Recuerdo dirigido",
            "Foco del hilo",
        ] {
            assert!(
                !vacio.contains(rotulo),
                "sin datos no se imprime «{rotulo}»"
            );
        }
    }

    #[test]
    fn el_prompt_sin_contexto_no_deja_secciones_vacias() {
        let p = prompt_turno("hola", &Contexto::default());
        assert!(!p.contains("Recuerdo dirigido"));
        assert!(!p.contains("Foco del hilo"));
        assert!(!p.contains("Visión del proyecto"));
        assert!(p.contains("0 nodos · 0 aristas"));
    }

    #[test]
    fn el_resumen_del_contexto_es_legible_por_el_panel() {
        let c = Contexto {
            norte: Some("N".into()),
            memoria: vec![("A".into(), "a.md".into()), ("B".into(), "b.md".into())],
            foco: vec!["F".into()],
            nodos: 3,
            aristas: 4,
            pendientes: 1,
            ..Contexto::default()
        };
        let r = resumen_contexto(&c);
        assert!(r.contains("lienzo=3/4"));
        assert!(r.contains("memoria=[A, B]"));
        assert!(r.contains("foco=[F]"));
    }

    #[test]
    fn el_argv_lleva_sesion_nombrada_y_create_if_missing() {
        let a = argv("hola", &Config::default());
        assert_eq!(a[0], "chat");
        assert_eq!(a[1], "-q");
        assert_eq!(a[2], "hola");
        assert!(a.iter().any(|x| x == "-c"));
        assert!(
            a.iter().any(|x| x == "--create-if-missing"),
            "sin esto arranca una mente nueva cada turno"
        );
        assert!(a.iter().any(|x| x == "--oneshot"));
        assert!(
            a.iter().any(|x| x == "-Q"),
            "salida limpia: la respuesta se muestra tal cual"
        );
        let i = a.iter().position(|x| x == "--run-budget").unwrap();
        assert_eq!(a[i + 1], "900");
    }

    #[test]
    fn la_salida_pierde_el_session_id_pero_lo_devuelve() {
        let bruto = "session_id: 20260915_210513_7bdd2f\nRespuesta real\nmás texto";
        let (limpio, id) = limpiar_salida(bruto);
        assert_eq!(id.as_deref(), Some("20260915_210513_7bdd2f"));
        assert!(!limpio.contains("session_id"));
        assert!(limpio.starts_with("Respuesta real"));
    }

    #[test]
    fn el_nombre_de_nota_es_una_ruta_relativa_segura() {
        let n = nombre_nota("2026-09-15T21:10");
        assert_eq!(n, "cerebro/2026-09-15T21-10-turno.md");
        assert!(!n.contains("..") && !n.starts_with('/'));
    }

    #[test]
    fn la_nota_lleva_frontmatter_de_turno() {
        let nota = nota_markdown(
            "¿qué hacemos?",
            "seguimos con el plan",
            "nf-cerebro",
            true,
            1234,
            "2026-09-15T21:10",
            "2026-09-16T00:10:33.000Z",
        );
        assert!(nota.starts_with(
            "---\ntipo: turno\ntitulo: \"¿qué hacemos?\"\nfecha: \"2026-09-15T21:10\"\n"
        ));
        assert!(
            nota.contains("\n# Turno: ¿qué hacemos?\n"),
            "el título de la nota es el pedido: es lo que hace útil el recuerdo"
        );
        assert!(nota.contains("fecha_utc: \"2026-09-16T00:10:33.000Z\""));
        assert!(nota.contains("sesion: \"nf-cerebro\""));
        assert!(nota.contains("estado: ok"));
        assert!(nota.contains("## Pedido\n¿qué hacemos?"));
        assert!(nota.contains("## Respuesta\nseguimos con el plan"));
        // Comillas y saltos en el pedido no rompen el frontmatter
        let raro = nota_markdown("dijo \"esto\"\ny más", "", "s", false, 0, "t", "u");
        assert!(raro.contains("pedido: \"dijo 'esto' y más\""));
        assert!(raro.contains("(sin respuesta)"));
    }
}

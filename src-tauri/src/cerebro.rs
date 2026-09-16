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
}

impl Default for Config {
    fn default() -> Self {
        Config { sesion: "nf-cerebro".into(), notas: true, run_budget_s: 900, tope_s: 1200 }
    }
}

impl Config {
    pub fn desde(cfg: Option<&Value>) -> Config {
        let mut c = Config::default();
        if let Some(s) = cfg {
            if let Some(v) = s.get("sesion").and_then(|v| v.as_str()).filter(|v| !v.trim().is_empty()) {
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
        }
        if let Ok(v) = std::env::var("NODEFLOW_CEREBRO_SESION") {
            if !v.trim().is_empty() {
                c.sesion = v.trim().to_string();
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
    format!("cerebro/{limpio}-turno.md")
}

/// La nota que deja cada turno. Es la memoria episódica: qué se pidió, qué volvió y en qué sesión.
/// Frontmatter en el formato de la bóveda (mismo estilo que las notas de nodo, sin `id` de nodo).
pub fn nota_markdown(pedido: &str, resultado: &str, sesion: &str, ok: bool, ms: u64, fecha: &str) -> String {
    let esc = |s: &str| s.replace('"', "'").replace('\n', " ");
    format!(
        "---\ntipo: turno\nfecha: \"{}\"\nsesion: \"{}\"\nestado: {}\nms: {}\npedido: \"{}\"\n---\n\n# Turno del cerebro\n\n## Pedido\n{}\n\n## Respuesta\n{}\n",
        esc(fecha),
        esc(sesion),
        if ok { "ok" } else { "error" },
        ms,
        esc(pedido),
        pedido,
        if resultado.trim().is_empty() { "(sin respuesta)" } else { resultado }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn el_argv_lleva_sesion_nombrada_y_create_if_missing() {
        let a = argv("hola", &Config::default());
        assert_eq!(a[0], "chat");
        assert_eq!(a[1], "-q");
        assert_eq!(a[2], "hola");
        assert!(a.iter().any(|x| x == "-c"));
        assert!(a.iter().any(|x| x == "--create-if-missing"), "sin esto arranca una mente nueva cada turno");
        assert!(a.iter().any(|x| x == "--oneshot"));
        assert!(a.iter().any(|x| x == "-Q"), "salida limpia: la respuesta se muestra tal cual");
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
            "2026-09-15T21:10:33.000Z",
        );
        assert!(nota.starts_with("---\ntipo: turno\nfecha: \"2026-09-15T21:10:33.000Z\"\n"));
        assert!(nota.contains("sesion: \"nf-cerebro\""));
        assert!(nota.contains("estado: ok"));
        assert!(nota.contains("## Pedido\n¿qué hacemos?"));
        assert!(nota.contains("## Respuesta\nseguimos con el plan"));
        // Comillas y saltos en el pedido no rompen el frontmatter
        let raro = nota_markdown("dijo \"esto\"\ny más", "", "s", false, 0, "t");
        assert!(raro.contains("pedido: \"dijo 'esto' y más\""));
        assert!(raro.contains("(sin respuesta)"));
    }
}

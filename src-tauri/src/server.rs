//! Servidor local de NodeFlow — reemplazo 1:1 del `server.ts` de Express.
//!
//! Rutas (mismas que el original):
//!   GET  /api/health                  POST /api/ai/action
//!   GET  /api/hitl/preferences        POST /api/hitl/feedback
//!   POST /api/hitl/profile            POST /api/hitl/recalibrate
//!   POST /api/hitl/reset
//!
//! Los prompts y schemas de las 9 acciones viven en `specs/actions.json`, extraídos
//! mecánicamente desde `server.ts` para que no haya drift de comportamiento.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::memoria::Memoria;
use crate::vault::Vault;

pub const API_PORT: u16 = 37371;

const SPECS: &str = include_str!("../specs/actions.json");

/// Cascada resiliente contra 503/429 (igual orden que el original).
const CANDIDATE_MODELS: [&str; 4] = [
    "gemini-3.6-flash",
    "gemini-3.1-flash-lite",
    "gemini-flash-latest",
    "gemini-3.8-flash",
];

#[derive(Clone)]
pub struct AppState {
    pub data_dir: PathBuf,
    pub http: reqwest::Client,
    pub env_key: Option<String>,
    /// Fase 3 — vault en disco (fuente de verdad + notas editables desde Obsidian).
    pub vault: Arc<Vault>,
    /// Fase 5b — memoria semántica de la bóveda (BM25 sobre todas las notas).
    pub memoria: Arc<Memoria>,
    /// Fase 9 — caché de respuestas de IA (`.nodeflow/ai-cache.json`): repetir no cuesta tokens.
    pub cache: Arc<crate::costo::Cache>,
    /// Fase 9 — tarifas declaradas por el usuario (USD por 1M tokens). Sin declarar: `None`, no 0.
    pub tarifas: crate::costo::Tarifas,
    /// Fase 10 — microservicio de borradores con el modelo local (modelo, keep_alive, tope).
    pub borrador: crate::borrador::Config,
    /// Fase 4 — el gateway propio (`hermes serve` en loopback): es lo que hace que el panel vea el
    /// turno en vivo y pueda resolver las aprobaciones sin salir de la app.
    pub gateway: Arc<crate::cerebro_gateway::Gateway>,
    /// Fase 12 — motores que fallaron por créditos o clave (id → motivo). Se aprende en la primera
    /// corrida: el catálogo los declara y los automáticos los saltean, en vez de elegir un motor muerto.
    pub motores_caidos: Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
    /// Sello del config (tamaño + fecha). Si cambia, se olvidan los motores caídos: agregar una clave
    /// es información nueva y el motivo viejo ("falta la clave", "requiere créditos") puede ya no
    /// aplicar. Sin esto, pegar la clave no tenía efecto hasta reiniciar la app.
    pub config_sello: Arc<std::sync::Mutex<u64>>,
    /// B — turnos de generación en curso (clave → instante de arranque). La app no encola prompts sin
    /// freno: una generación por nodo y acción, y una sola cuando corre en la placa.
    pub ia_en_curso: Arc<std::sync::Mutex<std::collections::HashMap<String, f64>>>,
    /// Fase 1 del cerebro residente: sesión nombrada de Hermes (memoria entre turnos) + notas episódicas.
    pub cerebro: crate::cerebro::Config,
}

impl AppState {
    /// Registra que un motor no está disponible (y por qué). Idempotente.
    pub fn marcar_motor_caido(&self, id: &str, motivo: &str) {
        if let Ok(mut m) = self.motores_caidos.lock() {
            m.insert(id.to_string(), motivo.to_string());
        }
    }

    pub fn motivos_de_motores(&self) -> std::collections::HashMap<String, String> {
        self.motores_caidos
            .lock()
            .map(|m| m.clone())
            .unwrap_or_default()
    }

    /// Toma el turno de generación para `claves`. `None` = ya hay una corrida con alguna de esas claves.
    pub fn ia_tomar(&self, claves: &[String]) -> Option<IaTurno> {
        ia_tomar_en(&self.ia_en_curso, claves, ahora_s()).then(|| IaTurno {
            mapa: self.ia_en_curso.clone(),
            claves: claves.to_vec(),
        })
    }
}

/// TTL del turno de IA. Una corrida que muere sin soltar (kill, crash, cliente que cancela) no puede
/// dejar la IA bloqueada para siempre: es la misma lección que `investigacion::en_curso`, que guardaba
/// "1" y rechazaba toda corrida nueva después de un apagón.
const IA_TURNO_TTL_S: f64 = 600.0;

/// Clave del turno que comparten **todos** los pedidos que corren en la placa: la GPU hace una por vez.
pub const IA_CLAVE_PLACA: &str = "ia:placa";

/// Turno de generación. Se suelta solo al salir de alcance (`Drop`), así ningún camino —incluido un
/// `return` temprano o un error— deja el turno tomado.
pub struct IaTurno {
    mapa: Arc<std::sync::Mutex<std::collections::HashMap<String, f64>>>,
    claves: Vec<String>,
}

impl Drop for IaTurno {
    fn drop(&mut self) {
        if let Ok(mut m) = self.mapa.lock() {
            for k in &self.claves {
                m.remove(k);
            }
        }
    }
}

/// Toma las claves si están libres, descartando antes las vencidas. `ahora` entra por parámetro para
/// poder testear el vencimiento sin depender del reloj.
fn ia_tomar_en(
    mapa: &std::sync::Mutex<std::collections::HashMap<String, f64>>,
    claves: &[String],
    ahora: f64,
) -> bool {
    let Ok(mut m) = mapa.lock() else {
        return false;
    };
    m.retain(|_, t| ahora - *t < IA_TURNO_TTL_S);
    if claves.iter().any(|k| m.contains_key(k)) {
        return false;
    }
    for k in claves {
        m.insert(k.clone(), ahora);
    }
    true
}

fn ahora_s() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Arranque
// ─────────────────────────────────────────────────────────────────────────────

pub fn spawn(data_dir: PathBuf, env_key: Option<String>, vault: Arc<Vault>, memoria: Arc<Memoria>) {
    tauri::async_runtime::spawn(async move {
        // Fase 9: tarifas declaradas (config del vault) + caché en disco junto al resto del estado.
        let cfg: Option<Value> = std::fs::read_to_string(data_dir.join("nodeflow.config.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok());
        let tarifas = cfg
            .as_ref()
            .map(|c| c.get("costo").unwrap_or(c))
            .map(|seccion| crate::costo::Tarifas::desde_config(Some(seccion)))
            .unwrap_or_default();
        // Fase 10: el borrador local se configura con env > `nodeflow.config.json` > default.
        let borrador = crate::borrador::Config::desde(cfg.as_ref().and_then(|c| c.get("borrador")));
        // Fase 1: el cerebro residente (sesión nombrada de Hermes + notas episódicas en la bóveda).
        let cerebro = crate::cerebro::Config::desde(cfg.as_ref().and_then(|c| c.get("cerebro")));
        // El gateway del cerebro se levanta **a pedido** (el panel lo pide): nada de un proceso de más
        // corriendo siempre. Puerto propio, override por `cerebro.gateway_puerto`.
        let gateway = std::sync::Arc::new(crate::cerebro_gateway::Gateway::nuevo(
            cerebro.gateway_puerto,
        ));
        let cache = Arc::new(crate::costo::Cache::cargar(
            vault.raiz().join(".nodeflow").join("ai-cache.json"),
        ));

        let state = AppState {
            data_dir,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            env_key,
            vault,
            memoria,
            cache,
            tarifas,
            borrador,
            gateway,
            cerebro,
            motores_caidos: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            config_sello: Arc::new(std::sync::Mutex::new(0)),
            ia_en_curso: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        };

        // Aprendizaje automático: revisa cada 2 minutos si juntó suficientes decisiones nuevas como
        // para recalibrar el perfil solo. La idea del motor de auto-mejora es que la app aprenda de
        // lo que aceptás, lo que descartás y lo que te interesa sin que aprietes nada.
        {
            let st_auto = state.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(120)).await;
                    let p = get_profile(&st_auto.data_dir);
                    let auto = p.get("autoAprendizaje").cloned().unwrap_or(Value::Null);
                    if !auto["activo"].as_bool().unwrap_or(false) {
                        continue;
                    }
                    let cada = auto["cada"].as_i64().unwrap_or(10).max(1);
                    let hechas = auto["decisionesEnLaUltima"].as_i64().unwrap_or(0);
                    let total = p["totalDecisions"].as_i64().unwrap_or(0);
                    if total - hechas < cada {
                        continue;
                    }
                    // La corrección usa el **motor elegido**: si es local, aprende gratis y sin red.
                    // Por eso no se exige clave de nube acá (cada motor resuelve la suya).
                    let key = st_auto.env_key.clone().unwrap_or_default();
                    log::info!("aprendizaje automático: {total} decisiones ({cada} nuevas desde la última) → recalibro");
                    if recalibrar_perfil_con_ia(&st_auto, &key).await.is_none() {
                        log::warn!("aprendizaje automático: el motor no devolvió perfil; reintento en el próximo ciclo");
                    }
                }
            });
        }

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = Router::new()
            .route("/api/health", get(health))
            .route("/api/hitl/preferences", get(hitl_preferences))
            .route("/api/hitl/feedback", post(hitl_feedback))
            .route("/api/hitl/auto", post(hitl_auto))
            .route("/api/hitl/profile", post(hitl_set_profile))
            .route("/api/hitl/recalibrate", post(hitl_recalibrate))
            .route("/api/hitl/reset", post(hitl_reset))
            .route("/api/ai/action", post(ai_action))
            .route("/api/ai/cache", get(ai_cache))
            .route("/api/voz/estado", get(voz_estado))
            .route("/api/voz/jwt", get(voz_jwt))
            .route("/api/voz/proveedores", get(voz_proveedores))
            .route("/api/voz/proveedor", post(voz_proveedor))
            .route("/api/claves/estado", get(claves_estado))
            .route("/api/claves/migrar", post(claves_migrar))
            .route("/api/claves", post(claves_guardar))
            .route("/api/idioma", get(idioma_leer).post(idioma_guardar))
            .route("/api/idioma/voz", post(idioma_voz_guardar))
            .route("/api/voz/decir", post(voz_decir))
            .route("/api/voz/dialogo", get(voz_dialogo))
            .route("/api/ai/delegar", post(delegar).get(delegar_estado))
            .route("/api/agente/turno", post(agente_turno))
            .route("/api/agente/estado", get(agente_estado))
            .route("/api/agente/parche", get(agente_parche))
            .route("/api/agente/parche/aprobar", post(agente_parche_aprobar))
            .route("/api/agente/parche/rechazar", post(agente_parche_rechazar))
            .route("/api/agente/parche/revertir", post(agente_parche_revertir))
            .route("/api/ai/evaluar", post(ai_evaluar).get(ai_evaluar_leer))
            .route(
                "/api/ai/investigar",
                post(ai_investigar).get(ai_investigar_estado),
            )
            .route("/api/ai/motores", get(ai_motores))
            .route("/api/ai/motor", post(ai_motor))
            .route("/api/ai/proveedor", post(ai_proveedor))
            // Fase 10 — el modelo local propone, el código valida
            .route("/api/knowledge/draft", post(knowledge_draft))
            // Fase 3 — vault en disco
            .route("/api/graph/state", get(graph_state).post(graph_save))
            .route("/api/vault/info", get(vault_info))
            // Fase 5b — memoria semántica del vault
            .route("/api/vault/search", get(vault_search))
            .route("/api/vault/reindex", post(vault_reindex))
            .route("/api/vault/memory", get(vault_memory))
            // Fase 7b — métrica de valor (T0 → T1)
            .route("/api/metrics", get(metrics))
            .route("/api/vault/note", get(vault_note))
            // Sesiones del lienzo en la bóveda: la mitad «volver» de guardar el progreso.
            // En disco (no en el WebView) viajan en el respaldo y las ve cualquier superficie.
            .route("/api/sesiones", get(sesiones_listar))
            .route("/api/sesiones/guardar", post(sesiones_guardar))
            .route("/api/sesiones/leer", get(sesiones_leer))
            .route("/api/sesiones/borrar", post(sesiones_borrar))
            // Fase 4 — superficie para el agente (leer y escribir el lienzo)
            .route("/api/graph/summary", get(graph_summary))
            .route("/api/graph/siguiente", get(graph_siguiente))
            .route("/api/graph/node", post(graph_node))
            .route("/api/graph/edge", post(graph_edge))
            .route("/api/graph/node/delete", post(graph_delete))
            .route("/api/graph/merge", post(graph_merge))
            .route("/api/graph/prune", post(graph_prune))
            // Fase 7a — agente jardín: diagnóstico, arreglo propuesto y reacomodo por niveles
            .route("/api/graph/garden", get(graph_garden))
            .route("/api/graph/garden/fix", post(graph_garden_fix))
            .route("/api/graph/tidy", post(graph_tidy))
            // Fase 8 — captura de conocimiento y exportación
            // Slice 1 — Expertos y Contrato de Artefactos
            .route("/api/expertos", get(expertos_listar))
            .route("/api/expertos/guardar", post(expertos_guardar))
            .route("/api/mcp/estado", get(mcp_estado))
            .route("/api/mcp/instalar", post(mcp_instalar))
            .route("/api/voz/motor/estado", get(voz_motor_estado))
            .route("/api/voz/motor/instalar", post(voz_motor_instalar))
            .route("/api/voz/motor/arrancar", post(voz_motor_arrancar))
            .route("/api/expert/run", post(experto_run))
            .route("/api/knowledge/preview", post(knowledge_preview))
            .route("/api/knowledge/capture", post(knowledge_capture))
            .route("/api/export/document", get(export_document))
            .route("/api/export/json", get(export_json))
            // Fase 5a — el agente propone, el humano aprueba
            .route("/api/agent/pending", get(agent_pending))
            // Fase 4 — el gateway propio del cerebro: el panel se conecta por WebSocket y ve el turno
            // en vivo. Se levanta a pedido y se baja a pedido (nada corriendo de más).
            .route("/api/cerebro/briefing", post(cerebro_briefing))
            // Fase 5.5 — el curador: propone fusiones y podas con motivo (no aplica nada: va a la cola).
            .route("/api/cerebro/curaduria", post(cerebro_curaduria))
            // Fase 5.4 — la arquitectura de la app, inventariada del árbol real (no escrita a mano).
            .route(
                "/api/cerebro/arquitectura/generar",
                post(cerebro_arquitectura_generar),
            )
            // Fase 5.2 — mi espacio: la bitácora y los planes del cerebro (notas de la bóveda, no nodos).
            .route("/api/cerebro/espacio", get(cerebro_espacio))
            .route("/api/cerebro/espacio/nota", post(cerebro_espacio_nota))
            // Fase 5.3 — el registro de herramientas del cerebro: listar, proponer (entra a la cola) y usar.
            .route("/api/cerebro/herramientas", get(cerebro_herramientas))
            .route(
                "/api/cerebro/herramienta/proponer",
                post(cerebro_herramienta_proponer),
            )
            .route("/api/cerebro/herramienta", post(cerebro_herramienta_usar))
            .route("/api/cerebro/gateway", get(cerebro_gateway_estado))
            .route(
                "/api/cerebro/gateway/arrancar",
                post(cerebro_gateway_arrancar),
            )
            .route("/api/cerebro/gateway/parar", post(cerebro_gateway_parar))
            .route("/api/agent/approve", post(agent_approve))
            .route("/api/agent/reject", post(agent_reject))
            .route("/api/azure/modelos", get(azure_modelos))
            .with_state(state)
            .layer(cors);

        match bind_con_reintentos().await {
            Ok(listener) => {
                log::info!("NodeFlow API escuchando en http://127.0.0.1:{API_PORT}");
                if let Err(e) = axum::serve(listener, app).await {
                    log::error!("Servidor API detenido: {e}");
                }
            }
            Err(e) => log::error!("No pude bindear el puerto {API_PORT} tras los reintentos: {e}"),
        }
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// HITL: perfil persistido en disco
// ─────────────────────────────────────────────────────────────────────────────

fn profile_path(data_dir: &Path) -> PathBuf {
    data_dir.join("user_preferences.json")
}

fn default_profile() -> Value {
    json!({
        "version": "2.0",
        "updatedAt": now_iso(),
        "autoAprendizaje": {
            "activo": false,
            "cada": 10,
            "decisionesEnLaUltima": 0,
            "ultimaMs": null
        },
        "totalDecisions": 4,
        "acceptanceRate": 85,
        "learnedProfile": "El usuario prefiere un enfoque técnico, conciso y estructurado. Suele descartar conexiones genéricas o superficiales y favorece patrones de arquitectura de sistemas, código en Python y filosofía pragmática. Adapta las respuestas a esta preferencia aprendida.",
        "categoriesAccepted": ["ARQUITECTURA", "SISTEMAS", "SEGURIDAD", "CRIPTOGRAFÍA"],
        "topicsRejected": ["Conexiones genéricas", "Slogans superficiales", "Filtro de tokens"],
        "recentFeedback": [{
            "id": "hitl-seed-1",
            "timestamp": now_iso(),
            "action": "NODE_EDIT",
            "prompt_original": "Sugerir 3 conexiones para el nodo 'Guardrails'",
            "ai_suggestion": ["Verificación de firma", "Base de datos vector", "Filtro de tokens"],
            "human_decision": {
                "accepted": ["Verificación de firma"],
                "rejected": ["Filtro de tokens"],
                "added_manually": ["Módulo de Auditoría Criptográfica"]
            },
            "contextSnippet": "Nodo Guardrails refinado hacia arquitectura criptográfica",
            "inferredPreference": "Alta prioridad a esquemas deterministas y seguridad"
        }]
    })
}

fn get_profile(data_dir: &Path) -> Value {
    let p = profile_path(data_dir);
    let defaults = default_profile();
    match std::fs::read_to_string(&p) {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(mut parsed) => {
                // merge superficial: los campos del archivo ganan, con saneo de arrays
                if let (Some(obj), Some(def)) = (parsed.as_object_mut(), defaults.as_object()) {
                    for (k, v) in def {
                        if !obj.contains_key(k) {
                            obj.insert(k.clone(), v.clone());
                        }
                    }
                    for key in ["categoriesAccepted", "topicsRejected", "recentFeedback"] {
                        let bad = obj.get(key).map(|v| !v.is_array()).unwrap_or(true);
                        if bad {
                            if let Some(d) = def.get(key) {
                                obj.insert(key.to_string(), d.clone());
                            }
                        }
                    }
                }
                parsed
            }
            Err(_) => defaults,
        },
        Err(_) => defaults,
    }
}

fn save_profile(data_dir: &Path, profile: &Value) {
    let _ = std::fs::create_dir_all(data_dir);
    let p = profile_path(data_dir);
    if let Ok(txt) = serde_json::to_string_pretty(profile) {
        if let Err(e) = crate::estado::escribir_atomico(&p, &txt) {
            log::error!("No pude escribir {}: {e}", p.display());
        }
    }
}

/// Limpia una señal aprendida: colapsa espacios, elimina caracteres de control, exige largo
/// 5..60 y al menos un alfanumérico. Devuelve `None` si no sirve para inyectar en un prompt.
fn sanitize_item(raw: &str) -> Option<String> {
    let cleaned = raw
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.len() < 5 || cleaned.len() > 60 || !cleaned.chars().any(|c| c.is_alphanumeric()) {
        return None;
    }
    Some(cleaned)
}

fn push_unique(out: &mut Vec<String>, raw: &str) {
    if let Some(item) = sanitize_item(raw) {
        if !out.iter().any(|x| x.eq_ignore_ascii_case(&item)) {
            out.push(item);
        }
    }
}

/// Cuántas decisiones humanas mencionan cada señal (comparación case-insensitive).
fn feedback_counts(profile: &Value) -> std::collections::HashMap<String, usize> {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    if let Some(events) = profile["recentFeedback"].as_array() {
        for ev in events {
            let d = &ev["human_decision"];
            for key in ["accepted", "added_manually", "rejected"] {
                if let Some(arr) = d[key].as_array() {
                    for raw in arr.iter().filter_map(|x| x.as_str()) {
                        if let Some(item) = sanitize_item(raw) {
                            *counts.entry(item.to_lowercase()).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
    }
    counts
}

/// Señales a inyectar: las semilla (siempre, son el piso de calidad) + las que tienen respaldo.
fn curated_items(
    seed: &Value,
    current: Option<&Vec<Value>>,
    counts: &std::collections::HashMap<String, usize>,
    min_count: usize,
    max: usize,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(arr) = seed.as_array() {
        for raw in arr.iter().filter_map(|x| x.as_str()) {
            push_unique(&mut out, raw);
        }
    }
    if let Some(arr) = current {
        for raw in arr.iter().filter_map(|x| x.as_str()) {
            // Si ya entró como semilla, no se re-evalúa (evita logs engañosos).
            let ya_incluida = sanitize_item(raw)
                .map(|item| out.iter().any(|x| x.eq_ignore_ascii_case(&item)))
                .unwrap_or(false);
            if ya_incluida {
                continue;
            }
            let respaldo = sanitize_item(raw)
                .map(|item| *counts.get(&item.to_lowercase()).unwrap_or(&0))
                .unwrap_or(0);
            if respaldo >= min_count {
                push_unique(&mut out, raw);
            } else {
                log::info!("HITL: señal descartada por falta de respaldo ({respaldo} decisión/es): {raw:?}");
            }
        }
    }
    let len = out.len();
    if len > max {
        out.split_off(len - max)
    } else {
        out
    }
}

fn build_hitl_system_instruction(profile: &Value, custom_override: Option<&str>) -> String {
    let base = custom_override
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| profile["learnedProfile"].as_str().unwrap_or("").trim());

    // Curaduría de señales: se inyectan las categorías semilla y, del resto, SOLO lo que tiene
    // respaldo (aparece en >= 2 decisiones humanas). Un token suelto —un typo, una prueba, un
    // disparo accidental— no debe convertirse en tema de generación.
    let counts = feedback_counts(profile);
    let seed = default_profile();
    let mut accepted = curated_items(
        &seed["categoriesAccepted"],
        profile["categoriesAccepted"].as_array(),
        &counts,
        2,
        8,
    );
    let mut rejected = curated_items(
        &seed["topicsRejected"],
        profile["topicsRejected"].as_array(),
        &counts,
        2,
        8,
    );
    if accepted.is_empty() {
        accepted = vec!["Arquitectura".into(), "Sistemas".into(), "Métricas".into()];
    }
    if rejected.is_empty() {
        rejected = vec!["Ideas superficiales".into(), "Slogans genéricos".into()];
    }
    let accepted = accepted.join(", ");
    let rejected = rejected.join(", ");

    format!(
        "Eres el motor cognitivo y analítico de NeuralMind con arquitectura HITL (Human-in-the-Loop Continuous Learning).\n\n\
PERFIL ADAPTATIVO DEL USUARIO (Aprendido por retroalimentación humana continua):\n\"{base}\"\n\n\
DIRECTRICES DE CURADURÍA APRENDIDAS:\n\
- Preferencias y temáticas aceptadas con frecuencia: {accepted}\n\
- Patrones o enfoques rechazados previamente por el usuario: {rejected}\n\n\
REGLAS DE GENERACIÓN ESTRICTAS:\n\
1. Aplica un nivel de abstracción técnico riguroso, conciso y accionable.\n\
2. Evita conceptos vagos, generalidades trilladas o contenido de relleno.\n\
3. Cada propuesta debe ser conceptualmente densa y complementar la red de ideas.\n\
4. Respeta rigurosamente el esquema JSON indicado."
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

async fn health(State(st): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "hasApiKey": st.env_key.is_some(),
        "timestamp": now_iso()
    }))
}

async fn hitl_preferences(State(st): State<AppState>) -> impl IntoResponse {
    Json(json!({ "success": true, "profile": get_profile(&st.data_dir) }))
}

/// `POST /api/hitl/auto` — enciende/apaga el aprendizaje automático y cada cuántas decisiones corre.
///
/// La idea del motor de auto-mejora: que la app aprenda sola de qué aceptás, qué descartás y qué te
/// interesa, sin que tengas que apretar un botón.
async fn hitl_auto(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let mut profile = get_profile(&st.data_dir);
    let auto = profile["autoAprendizaje"].clone();
    let activo = body["activo"]
        .as_bool()
        .unwrap_or_else(|| auto["activo"].as_bool().unwrap_or(false));
    let cada = body["cada"]
        .as_i64()
        .or_else(|| auto["cada"].as_i64())
        .unwrap_or(10)
        .clamp(1, 500);
    let total = profile["totalDecisions"].as_i64().unwrap_or(0);

    if activo && !auto["activo"].as_bool().unwrap_or(false) {
        // Al encenderlo, la cuenta arranca desde acá: no recalibra por lo viejo.
        profile["autoAprendizaje"]["decisionesEnLaUltima"] = json!(total);
    }
    profile["autoAprendizaje"]["activo"] = json!(activo);
    profile["autoAprendizaje"]["cada"] = json!(cada);
    save_profile(&st.data_dir, &profile);
    log::info!(
        "aprendizaje automático: {} (cada {cada} decisiones)",
        if activo { "encendido" } else { "apagado" }
    );
    Json(json!({ "success": true, "profile": profile }))
}

async fn hitl_feedback(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let action = body["action"].as_str().unwrap_or("").to_string();
    if action.is_empty()
        || body
            .get("human_decision")
            .map(|v| v.is_null())
            .unwrap_or(true)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Estructura de evento feedback inválida" })),
        );
    }

    let current = get_profile(&st.data_dir);
    let decision = &body["human_decision"];
    let arr = |v: &Value| -> Vec<String> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    };
    let accepted = arr(&decision["accepted"]);
    let rejected = arr(&decision["rejected"]);
    let added = arr(&decision["added_manually"]);

    let event = json!({
        "id": body["id"].as_str().map(|s| s.to_string()).unwrap_or_else(|| format!("hitl-{}-{}", now_ms(), counters::next())),
        "timestamp": body["timestamp"].as_str().map(|s| s.to_string()).unwrap_or_else(now_iso),
        "action": action,
        "prompt_original": body["prompt_original"].as_str().unwrap_or("Interacción conceptual en el lienzo"),
        "ai_suggestion": body["ai_suggestion"].as_array().cloned().unwrap_or_default(),
        "human_decision": { "accepted": accepted, "rejected": rejected, "added_manually": added },
        "contextSnippet": body["contextSnippet"].as_str().unwrap_or(""),
        "inferredPreference": body["inferredPreference"].as_str().unwrap_or("")
    });

    // categorías y tópicos
    let mut cats: Vec<String> = current["categoriesAccepted"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let mut topics: Vec<String> = current["topicsRejected"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    // Solo se persisten señales que pasan el saneo (largo 5..60, sin control chars, con alfanumérico).
    for s in accepted.iter().chain(added.iter()) {
        if let Some(item) = sanitize_item(s) {
            if !cats.iter().any(|x| x.eq_ignore_ascii_case(&item)) {
                cats.push(item);
            }
        } else {
            log::info!("HITL: feedback descartado por saneo: {s:?}");
        }
    }
    for s in rejected.iter() {
        if let Some(item) = sanitize_item(s) {
            if !topics.iter().any(|x| x.eq_ignore_ascii_case(&item)) {
                topics.push(item);
            }
        }
    }

    // historial (últimos 50)
    let mut history: Vec<Value> = vec![event.clone()];
    if let Some(prev) = current["recentFeedback"].as_array() {
        history.extend(prev.iter().cloned());
    }
    history.truncate(50);

    // acceptance rate global
    let (mut total_acc, mut total_rej) = (0usize, 0usize);
    for ev in &history {
        let d = &ev["human_decision"];
        for k in ["accepted", "added_manually"] {
            total_acc += d[k].as_array().map(|a| a.len()).unwrap_or(0);
        }
        total_rej += d["rejected"].as_array().map(|a| a.len()).unwrap_or(0);
    }
    let total_decisions = current["totalDecisions"].as_i64().unwrap_or(0) + 1;
    let acceptance_rate = if total_acc + total_rej > 0 {
        ((total_acc as f64 / (total_acc + total_rej) as f64) * 100.0).round() as i64
    } else {
        current["acceptanceRate"].as_i64().unwrap_or(0)
    };

    // refinamiento heurístico incremental
    let mut learned = current["learnedProfile"].as_str().unwrap_or("").to_string();
    if !added.is_empty() {
        let top = added.iter().take(2).cloned().collect::<Vec<_>>().join(", ");
        if !learned.contains(&top) {
            let trimmed = learned.trim_end_matches('.').to_string();
            learned = format!("{trimmed}. Incluye afinidad expresa por conceptos como: {top}.");
        }
    }

    let mut updated = json!({
        "version": "2.0",
        "updatedAt": now_iso(),
        "totalDecisions": total_decisions,
        "acceptanceRate": acceptance_rate,
        "learnedProfile": learned,
        "categoriesAccepted": cats.split_off(cats.len().saturating_sub(15)),
        "topicsRejected": topics.split_off(topics.len().saturating_sub(15)),
        "recentFeedback": history
    });
    // Registrar una decisión reescribe el perfil entero: hay que preservar la configuración del
    // aprendizaje automático. Medido: sin esto el interruptor se apagaba solo en la primera decisión.
    if let Some(auto) = current.get("autoAprendizaje") {
        updated["autoAprendizaje"] = auto.clone();
    }

    save_profile(&st.data_dir, &updated);
    (
        StatusCode::OK,
        Json(json!({ "success": true, "profile": updated })),
    )
}

async fn hitl_set_profile(
    State(st): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let Some(learned) = body["learnedProfile"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "El perfil aprendido debe ser un texto válido" })),
        );
    };
    let mut profile = get_profile(&st.data_dir);
    profile["learnedProfile"] = json!(learned);
    profile["updatedAt"] = json!(now_iso());
    save_profile(&st.data_dir, &profile);
    (
        StatusCode::OK,
        Json(json!({ "success": true, "profile": profile })),
    )
}

/// Recalibra el perfil con IA a partir de las últimas decisiones de curaduría.
///
/// Un solo lugar para las dos formas de dispararlo: el botón del panel y el aprendizaje automático.
/// Devuelve el perfil actualizado, o `None` si no hay nada que analizar o el motor no respondió.
async fn recalibrar_perfil_con_ia(st: &AppState, key: &str) -> Option<Value> {
    let mut profile = get_profile(&st.data_dir);
    let sample: Vec<Value> = profile["recentFeedback"]
        .as_array()
        .map(|a| {
            a.iter()
                .take(10)
                .map(|e| {
                    json!({
                        "prompt": e["prompt_original"],
                        "accepted": e["human_decision"]["accepted"],
                        "rejected": e["human_decision"]["rejected"],
                        "added_manually": e["human_decision"]["added_manually"]
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if sample.is_empty() {
        return None;
    }

    let prompt = format!(
        "Analiza estas decisiones recientes de curaduría de un usuario en un mapa mental (HITL Loop):\n{}\n\nSintetiza un perfil de estilo y preferencia cognitiva de 2 o 3 oraciones contundentes para inyectar en el system prompt.\nEjemplo: \"El usuario prefiere un enfoque técnico, conciso y estructurado. Suele descartar conexiones genéricas y favorece patrones de arquitectura, código y filosofía pragmática.\"\nResponde en formato JSON:\n{{\"profile\": \"El usuario prefiere...\"}}",
        serde_json::to_string_pretty(&sample).unwrap_or_default()
    );
    let schema = json!({
        "type": "OBJECT",
        "properties": { "profile": { "type": "STRING" } },
        "required": ["profile"]
    });
    let llamada = call_model(
        st, key, &prompt, &schema, None, "", None,
        "borrador", // rápido y gratis: es el bucle del lienzo
        false,      // los borradores sí usan caché
        None,       // el perfil HITL no es un pedido del usuario
    )
    .await?;
    let aprendido = llamada.valor["profile"].as_str()?.trim().to_string();
    if aprendido.is_empty() {
        return None;
    }

    profile["learnedProfile"] = json!(aprendido);
    profile["updatedAt"] = json!(now_iso());
    // Cualquier recalibración (manual o automática) reinicia la cuenta del automático.
    profile["autoAprendizaje"]["decisionesEnLaUltima"] = profile["totalDecisions"].clone();
    profile["autoAprendizaje"]["ultimaMs"] = json!(now_ms());
    save_profile(&st.data_dir, &profile);
    log::info!(
        "aprendizaje: perfil recalibrado con IA ({} decisiones acumuladas)",
        profile["totalDecisions"]
    );
    Some(profile)
}

async fn hitl_recalibrate(State(st): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(key) = resolve_key(&st, &headers) {
        if let Some(profile) = recalibrar_perfil_con_ia(&st, &key).await {
            return Json(json!({ "success": true, "profile": profile, "calibratedWithAi": true }));
        }
    }
    // Sin IA disponible: heurística local sobre lo aceptado y lo descartado.
    let mut profile = get_profile(&st.data_dir);
    let last3: Vec<String> = profile["categoriesAccepted"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .rev()
                .take(3)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let cats = if last3.is_empty() {
        "arquitectura y sistemas".to_string()
    } else {
        last3.join(", ")
    };
    profile["learnedProfile"] = json!(format!(
        "El usuario prefiere un enfoque técnico y conciso. Prioriza {cats}, descartando generalidades."
    ));
    profile["updatedAt"] = json!(now_iso());
    save_profile(&st.data_dir, &profile);
    Json(json!({ "success": true, "profile": profile, "calibratedWithAi": false }))
}

async fn hitl_reset(State(st): State<AppState>) -> impl IntoResponse {
    let profile = default_profile();
    save_profile(&st.data_dir, &profile);
    Json(json!({ "success": true, "profile": profile }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Motor de acciones IA (genérico, alimentado por specs/actions.json)
// ─────────────────────────────────────────────────────────────────────────────

async fn ai_action(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let action_type = body["type"].as_str().unwrap_or("").to_string();
    let specs: Value = serde_json::from_str(SPECS).unwrap_or(json!({}));

    // resolver alias (critique | devils_advocate)
    let mut spec = specs.get(&action_type).cloned();
    if spec.is_none() {
        for (_, s) in specs
            .as_object()
            .map(|o| {
                o.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
        {
            let is_alias = s["aliases"]
                .as_array()
                .map(|a| a.iter().any(|x| x.as_str() == Some(action_type.as_str())))
                .unwrap_or(false);
            if is_alias {
                spec = Some(s);
                break;
            }
        }
    }
    let Some(spec) = spec else {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "success": false, "error": format!("Tipo de acción desconocido: {action_type}") }),
            ),
        );
    };

    let profile = get_profile(&st.data_dir);
    let override_txt = body["hitlProfileOverride"].as_str();
    let system_instruction = build_hitl_system_instruction(&profile, override_txt);
    // Fase 6: la bóveda del usuario entra al prompt como contexto del nodo.
    let context = build_context(
        &action_type,
        &body,
        Some((st.vault.as_ref(), st.memoria.as_ref())),
    );
    let prompt = fill_template(spec["prompt"].as_str().unwrap_or(""), &context);
    let schema = spec["schema"].clone();
    let resp_key = spec["response"]["key"]
        .as_str()
        .unwrap_or("variations")
        .to_string();
    let nested = spec["response"]
        .get("nested")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Fase 9: la clave de caché necesita saber de qué nodo salió el prompt.
    let nodo_id = body["nodeData"]["id"]
        .as_str()
        .or_else(|| body["nodeId"].as_str())
        .unwrap_or("")
        .to_string();
    // Fase 11 — el modo viaja con la petición: local (edge) | nube | auto (cadena configurada).
    let modo = body["modo"]
        .as_str()
        .map(|m| m.trim().to_lowercase())
        .filter(|m| !m.is_empty());

    // B — una generación por vez. Dos claves: por (nodo, acción) siempre, y una global cuando el plan
    // corre en la placa. La nube sí puede ir en paralelo, así que ahí no se limita.
    // El turno se suelta al salir de la función (`Drop`): un error no lo deja tomado.
    let toca_placa = plan_toca_la_placa(&st, modo.as_deref(), &action_type).await;
    let mut claves = vec![format!("ia:{nodo_id}:{action_type}")];
    if toca_placa {
        claves.push(IA_CLAVE_PLACA.to_string());
    }
    let Some(_turno) = st.ia_tomar(&claves) else {
        let error = if toca_placa {
            format!("Ya hay una generación en curso y corre en tu placa: hace una por vez. Esperá a que termine y volvé a pedir «{action_type}».")
        } else {
            format!("Ya hay una generación en curso para este nodo («{action_type}»). Esperá a que termine.")
        };
        return (
            StatusCode::CONFLICT,
            Json(json!({ "success": false, "ocupado": true, "error": error })),
        );
    };

    let called = match resolve_key(&st, &headers) {
        Some(key) if !prompt.is_empty() && !schema.is_null() => {
            call_model(
                &st,
                &key,
                &prompt,
                &schema,
                Some(&system_instruction),
                &nodo_id,
                modo.as_deref(),
                &action_type,
                // La planilla de evaluación pide medir al modelo, no a la caché.
                body["sin_cache"].as_bool().unwrap_or(false),
                body["texto"].as_str().or_else(|| body["prompt"].as_str()), // el pedido, no el prompt entero
            )
            .await
        }
        _ => None,
    };

    let mut uso = Value::Null;
    let (payload, model_used, used_ai) = match called {
        Some(llamada) => {
            uso = llamada.uso_json(&st.tarifas);
            let parsed = llamada.valor;
            let model = llamada.modelo;
            let mut value = match &nested {
                Some(k) => parsed[k].clone(),
                None => parsed.clone(),
            };
            // El modelo propone, el código valida: se normaliza al contrato de la acción.
            if action_type == "condensar" {
                normalizar_condensado(&mut value);
            }
            // Voz: el plan se valida contra el lienzo REAL (sólo ids que existen, sólo acciones
            // permitidas, topes). Lo que no pasa, se descarta y se informa; nunca se ejecuta a ciegas.
            if action_type == "voz" {
                let (ids, titulos): (Vec<String>, Vec<String>) =
                    st.vault.read_state().unwrap_or(serde_json::json!({}))["nodes"]
                        .as_array()
                        .map(|ns| {
                            ns.iter()
                                .map(|n| {
                                    let id = n["id"].as_str().unwrap_or("").to_string();
                                    let titulo = n["data"]["title"].as_str().unwrap_or("");
                                    (id.clone(), format!("{id} · {titulo}"))
                                })
                                .unzip()
                        })
                        .unwrap_or_default();
                let mut limpio = crate::voz::validar(&value, &ids);
                // Escalada medida: la planilla mostró que hay pedidos que el motor local no resuelve
                // (devolvió 0 comandos). Antes de devolver un plan vacío, se le pide una vez al motor
                // de nube, en la misma corrida y sin que el usuario repita nada. Sólo se adopta si
                // trae algo: si tampoco, se respeta el resultado local y se informa.
                if limpio["comandos"]
                    .as_array()
                    .map(|c| c.is_empty())
                    .unwrap_or(true)
                {
                    let clave_escalada = resolve_key(&st, &headers).unwrap_or_default();
                    let escalada = call_model(
                        &st,
                        &clave_escalada,
                        &prompt,
                        &schema,
                        Some(&system_instruction),
                        &nodo_id,
                        Some("nube"),
                        "voz",
                        false,
                        None, // reintento/escalada: no hay un pedido nuevo del usuario
                    )
                    .await;
                    if escalada.is_none() {
                        log::warn!(
                            "voz: el plan local quedó vacío y no hay motor de nube disponible para escalar \
                             (revisá el catálogo: los «-cloud» del daemon responden 402 sin créditos)"
                        );
                    }
                    if let Some(llamada) = escalada {
                        let alterno = match &nested {
                            Some(k) => llamada.valor[k].clone(),
                            None => llamada.valor.clone(),
                        };
                        let alt = crate::voz::validar(&alterno, &ids);
                        let trajo = alt["comandos"]
                            .as_array()
                            .map(|c| !c.is_empty())
                            .unwrap_or(false);
                        if trajo {
                            log::info!(
                                "voz: el local no propuso nada → escaló a {} y trajo {} comandos",
                                llamada.modelo,
                                alt["comandos"].as_array().map(|a| a.len()).unwrap_or(0)
                            );
                            uso = llamada.uso_json(&st.tarifas);
                            limpio = alt;
                        } else {
                            log::info!(
                                "voz: tampoco la nube propuso nada; se devuelve el plan local"
                            );
                        }
                    }
                }
                // La voz selectiva se decide acá (regla testeada en `voz::debe_hablar`): el frontend
                // sólo obedece. Crear o enlazar es visible y va en silencio; enfocar, condensar,
                // criticar o haber descartado algo son hallazgos: eso se dice.
                let habla = crate::voz::debe_hablar(&limpio);
                limpio["hablar"] = json!(habla);
                log::info!(
                    "voz: {} comandos válidos · {} descartados · voz {}",
                    limpio["comandos"].as_array().map(|a| a.len()).unwrap_or(0),
                    limpio["descartados"].as_u64().unwrap_or(0),
                    if habla { "activa" } else { "en silencio" }
                );
                // El turno queda anotado (con el foco que dejó) para que el próximo pedido tenga
                // de dónde agarrarse: es lo que convierte comandos sueltos en conversación.
                if let Err(e) = crate::dialogo::registrar(
                    &st.vault.raiz().join(".nodeflow"),
                    body["texto"].as_str().unwrap_or(""),
                    &limpio,
                    &titulos,
                ) {
                    log::warn!("diálogo: no pude anotar el turno: {e}");
                }
                value = limpio;
            }
            let ok = match &value {
                Value::Array(a) => !a.is_empty(),
                Value::Object(o) => !o.is_empty(),
                _ => false,
            };
            if ok {
                (value, model, true)
            } else {
                (
                    fallback_for(&action_type, &spec, &context),
                    "fallback".to_string(),
                    false,
                )
            }
        }
        None => (
            fallback_for(&action_type, &spec, &context),
            "fallback".to_string(),
            false,
        ),
    };

    let mut out = json!({
        "success": true,
        resp_key.clone(): payload,
        "modelUsed": model_used,
        "hitlActive": true
    });
    if spec["response"].get("extra").is_none() {
        out["learnedProfile"] = json!(profile["learnedProfile"]);
    } else {
        out["source"] = json!("fallback");
    }
    if used_ai {
        // El proveedor real, no un "gemini" fijo (antes mentía cuando respondía Ollama).
        out["source"] = uso["proveedor"].clone();
    }
    // Fase 9 — costo visible: tokens medidos (o estimados y declarados), costo y caché.
    if !uso.is_null() {
        out["uso"] = uso.clone();
    }
    // Fase 11 — trazabilidad del ruteo: qué modo pidió el usuario y qué cadena se intentó.
    out["modo"] = json!(modo.clone().unwrap_or_else(|| "auto".to_string()));
    out["cadena"] = json!(cadena_por_modo(modo.as_deref(), cadena_de_proveedores(&st)));
    // Trazabilidad: qué notas de la bóveda alimentaron esta generación.
    if let Some(f) = context.get("memoria_fuentes") {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(f) {
            if v.as_array().map(|a| !a.is_empty()).unwrap_or(false) {
                out["memoria_fuentes"] = v;
            }
        }
    }
    (StatusCode::OK, Json(out))
}

/// Bloque de contexto con notas de la bóveda del usuario relacionadas con este nodo.
/// Devuelve (bloque_para_el_prompt, fuentes). Vacío si no hay nada relevante: así el prompt
/// queda idéntico al original cuando la bóveda no aporta nada.
///
/// Fase 7b — RAG espacial: las notas que son nodos VECINOS del nodo enfocado pesan más
/// (vecino directo ×1.5, a dos saltos ×1.2) porque son el contexto real de trabajo.
fn bloque_memoria(vault: &Vault, memoria: &Memoria, nodo: &Value) -> (String, Vec<Value>) {
    let titulo = nodo["title"].as_str().unwrap_or("");
    let desc = nodo["description"].as_str().unwrap_or("");
    let consulta = format!("{titulo} {}", desc.chars().take(300).collect::<String>());
    if consulta.trim().chars().count() < 6 {
        return (String::new(), Vec::new());
    }
    let res = memoria.buscar(&consulta, 10);
    if res["ok"].as_bool() != Some(true) {
        return (String::new(), Vec::new());
    }
    // Excluir la nota del propio nodo: no tiene sentido citarse a sí mismo.
    let propia = format!("nodos/{}.md", crate::vault::slug(titulo));

    // Sesgo espacial: qué nodos están cerca del enfocado en el grafo, y cuánto pesa cada uno.
    let mapa = vault.nombre_mapa();
    let (gnodos, garistas) = vault.grafo_actual();
    let foco_id = gnodos
        .iter()
        .find(|n| crate::grafo::titulo_de(n) == titulo)
        .or_else(|| {
            gnodos
                .iter()
                .find(|n| crate::grafo::titulo_de(n).eq_ignore_ascii_case(titulo))
        })
        .map(crate::grafo::id_de);
    // El boost se indexa por SLUG, no por ruta completa: las notas del lienzo se indexan como
    // `NodeFlow/nodos/<slug>.md`, así que comparar la ruta entera nunca coincidía.
    let slug_de = |t: &str| crate::vault::slug(t);
    let mut boost_por_slug: std::collections::HashMap<String, f32> =
        std::collections::HashMap::new();
    if let Some(fid) = &foco_id {
        for (nid, factor) in crate::grafo::cercania(&gnodos, &garistas, fid, 2) {
            if let Some(n) = gnodos.iter().find(|n| crate::grafo::id_de(n) == nid) {
                boost_por_slug.insert(slug_de(&crate::grafo::titulo_de(n)), factor);
            }
        }
    }
    let slug_de_ruta = |ruta: &str| -> String {
        ruta.rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md")
            .to_string()
    };

    // Re-puntuar y reordenar con el sesgo espacial aplicado.
    let mut candidatos: Vec<(f64, Value)> = Vec::new();
    for r in res["resultados"].as_array().cloned().unwrap_or_default() {
        let ruta = r["ruta"].as_str().unwrap_or("");
        let base = r["puntaje"].as_f64().unwrap_or(0.0);
        if ruta.ends_with(&propia) {
            continue;
        }
        // Los artefactos GENERADOS (`<mapa>.md` y `<mapa>.canvas`) duplican todo el lienzo: inyectarlos
        // como contexto es ruido puro. Se excluyen por NOMBRE de mapa, no por ruta: los artefactos se
        // llaman como el mapa, y las rutas del índice son relativas (sin barra inicial).
        if ruta.ends_with(&format!("{mapa}.md")) || ruta.ends_with(&format!("{mapa}.canvas")) {
            continue;
        }
        let factor = boost_por_slug
            .get(&slug_de_ruta(ruta))
            .copied()
            .unwrap_or(1.0);
        let mut rr = r.clone();
        rr["puntaje_espacial"] = json!(((base * factor as f64) * 100.0).round() / 100.0);
        rr["factor_cercania"] = json!(factor);
        candidatos.push((base * factor as f64, rr));
    }
    candidatos.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut lineas: Vec<String> = Vec::new();
    let mut fuentes: Vec<Value> = Vec::new();
    for (_puntaje_boost, r) in candidatos {
        let ruta = r["ruta"].as_str().unwrap_or("");
        let puntaje = r["puntaje"].as_f64().unwrap_or(0.0);
        if puntaje < 2.0 {
            continue;
        }
        lineas.push(format!(
            "- «{}» [{}]: {}",
            r["titulo"].as_str().unwrap_or(""),
            ruta,
            r["fragmento"].as_str().unwrap_or("")
        ));
        fuentes.push(json!({
            "titulo": r["titulo"],
            "ruta": ruta,
            "puntaje": puntaje,
            "puntaje_con_sesgo": r["puntaje_espacial"],
            "factor_cercania": r["factor_cercania"],
            "ya_en_el_lienzo": r["ya_en_el_lienzo"],
        }));
        if lineas.len() >= 3 {
            break;
        }
    }
    if lineas.is_empty() {
        return (String::new(), Vec::new());
    }
    let bloque = format!(
        "\n\nCONTEXTO DE LA BÓVEDA DEL USUARIO (notas que él ya escribió y se relacionan con este \
nodo; usalas como materia prima concreta y nombrá la fuente entre corchetes cuando la uses):\n{}\n",
        lineas.join("\n")
    );
    (bloque, fuentes)
}

/// Valores que el servidor original construía antes de armar cada prompt.
fn build_context(
    action: &str,
    body: &Value,
    ctx_memoria: Option<(&crate::vault::Vault, &Memoria)>,
) -> std::collections::HashMap<String, String> {
    let mut ctx = std::collections::HashMap::new();
    let node = &body["nodeData"];
    let s = |v: &Value, k: &str| v[k].as_str().unwrap_or("").to_string();

    ctx.insert(
        "title".into(),
        if node["title"].is_null() {
            "Idea Central".into()
        } else {
            s(node, "title")
        },
    );
    ctx.insert(
        "description".into(),
        if node["description"].is_null() {
            "Sin descripción".into()
        } else {
            s(node, "description")
        },
    );
    ctx.insert(
        "rawText".into(),
        body["rawText"].as_str().unwrap_or("").to_string(),
    );

    let selected = body["selectedNodes"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for (i, alias) in ["nodeA", "nodeB"].iter().enumerate() {
        let n = selected.get(i).cloned().unwrap_or(json!({}));
        let data = if n["data"].is_object() {
            n["data"].clone()
        } else {
            n.clone()
        };
        ctx.insert(format!("{alias}.title"), s(&data, "title"));
        ctx.insert(format!("{alias}.description"), s(&data, "description"));
    }

    // Condensación dirigida por objetivo (Fase A): la lista COMPLETA de seleccionados y el Norte
    // Estratégico. El resto de las acciones siguen usando nodeA/nodeB (dos nodos).
    let lista_nodos = selected
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let d = if n["data"].is_object() {
                n["data"].clone()
            } else {
                n.clone()
            };
            format!("{}. {} — {}", i + 1, s(&d, "title"), s(&d, "description"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    ctx.insert("listaNodos".into(), lista_nodos);
    ctx.insert("cantidad".into(), selected.len().to_string());
    ctx.insert(
        "objetivo".into(),
        body["objetivo"].as_str().unwrap_or("").trim().to_string(),
    );

    // Voz (Fase B): lo que dijo el usuario + el lienzo REAL con ids, para que el plan sólo pueda
    // referirse a nodos que existen. Se acota a 120 nodos para no inflar el prompt.
    if action == "voz" {
        ctx.insert(
            "texto".into(),
            body["texto"].as_str().unwrap_or("").trim().to_string(),
        );
        // Hilo de diálogo: sin esto cada frase es un plan aislado y "¿y si lo damos vuelta?" no tiene
        // referente. Se le pasan los últimos intercambios y en qué quedó enfocada la conversación.
        // El hilo vive en la bóveda (`.nodeflow/dialogo.json`), junto a la caché: es del usuario.
        let sesion_previa = ctx_memoria
            .map(|(v, _)| crate::dialogo::leer(&v.raiz().join(".nodeflow")))
            .unwrap_or_else(|| serde_json::json!({ "turnos": [], "foco": [] }));
        let turnos = sesion_previa["turnos"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        ctx.insert("dialogo".into(), crate::dialogo::como_texto(&turnos));
        let foco = sesion_previa["foco"]
            .as_array()
            .map(|f| {
                f.iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                "(todavía no hay un foco: se define con el primer pedido de dirección)".to_string()
            });
        ctx.insert("foco".into(), foco);
        if let Some((vault, _)) = ctx_memoria {
            let estado = vault.read_state().unwrap_or(serde_json::json!({}));
            let lienzo = estado["nodes"]
                .as_array()
                .map(|ns| {
                    ns.iter()
                        .take(120)
                        .filter_map(|n| {
                            let d = if n["data"].is_object() { &n["data"] } else { n };
                            let id = s(n, "id");
                            let titulo = s(d, "title");
                            let cat = s(d, "category");
                            if id.is_empty() || titulo.is_empty() {
                                None
                            } else {
                                Some(format!("{id} · {titulo} · {cat}"))
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            ctx.insert("lienzo".into(), lienzo);
        }
    }

    // nodos: soporta tanto {id,data:{...}} como {id,title,...}
    let nodes = body["nodes"].as_array().cloned().unwrap_or_default();
    let norm = |n: &Value| -> (String, String, String, Vec<String>) {
        let d = if n["data"].is_object() { &n["data"] } else { n };
        let cat = d["category"]
            .as_str()
            .or_else(|| d["label"].as_str())
            .unwrap_or("Concepto")
            .to_string();
        (
            d["title"].as_str().unwrap_or("Sin título").to_string(),
            d["description"].as_str().unwrap_or("").to_string(),
            cat,
            d["tags"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default(),
        )
    };

    match action {
        "synthesize" => {
            let summaries: Vec<String> = nodes
                .iter()
                .map(|n| {
                    let (t, d, c, tags) = norm(n);
                    format!("- [{c}] {t}: {d} ({})", tags.join(", "))
                })
                .collect();
            ctx.insert("nodeSummaries".into(), summaries.join("\n"));
        }
        "find_bridges" => {
            let edges = body["edges"].as_array().cloned().unwrap_or_default();
            let mut conns: Vec<String> = Vec::new();
            for e in &edges {
                let (src, tgt) = (
                    e["source"].as_str().unwrap_or(""),
                    e["target"].as_str().unwrap_or(""),
                );
                conns.push(format!("{src}->{tgt}"));
                conns.push(format!("{tgt}->{src}"));
            }
            conns.truncate(30);
            ctx.insert(
                "Array.from(existingConnections).slice(0, 30).join(\", \")".into(),
                conns.join(", "),
            );

            let summaries: Vec<Value> = nodes
                .iter()
                .map(|n| {
                    let (t, d, c, _) = norm(n);
                    json!({ "id": n["id"], "title": t, "category": c, "description": d })
                })
                .collect();
            ctx.insert(
                "nodeSummaries".into(),
                serde_json::to_string_pretty(&summaries).unwrap_or_default(),
            );
            ctx.insert(
                "JSON.stringify(nodeSummaries, null, 2)".into(),
                serde_json::to_string_pretty(&summaries).unwrap_or_default(),
            );
        }
        _ => {
            let summaries: Vec<String> = nodes
                .iter()
                .map(|n| {
                    let (t, d, c, tags) = norm(n);
                    format!("- [{c}] {t}: {d} ({})", tags.join(", "))
                })
                .collect();
            ctx.insert("nodeSummaries".into(), summaries.join("\n"));
            ctx.insert(
                "JSON.stringify(nodeSummaries, null, 2)".into(),
                summaries.join("\n"),
            );
        }
    }
    // Fase 6: la memoria de la bóveda entra al prompt como materia prima del nodo.
    if let Some((vault, m)) = ctx_memoria {
        let (bloque, fuentes) = bloque_memoria(vault, m, node);
        if !bloque.is_empty() {
            log::info!(
                "memoria: {} nota(s) de la bóveda inyectadas en el prompt de {action}",
                fuentes.len()
            );
            for f in &fuentes {
                log::info!(
                    "   · {} ({})",
                    f["ruta"].as_str().unwrap_or(""),
                    f["titulo"].as_str().unwrap_or("")
                );
            }
        }
        ctx.insert("memoria".into(), bloque);
        ctx.insert(
            "memoria_fuentes".into(),
            serde_json::to_string(&fuentes).unwrap_or_else(|_| "[]".into()),
        );
    }
    ctx
}

/// El motor devuelve a veces el mismo contenido con otro nombre de campo (medido con
/// `gpt-oss:120b-cloud`, que ignora la gramática: `macro_concept` en vez de `title`).
/// Se lo lleva al contrato de la acción antes de que el frontend lo vea.
/// Nota: se aplica SÓLO a `condensar`. El resto de las acciones conservan su contrato tal cual,
/// porque ya vienen respetándolo (medido en cada corrida).
fn normalizar_condensado(v: &mut serde_json::Value) {
    fn toma(v: &serde_json::Value, nombres: &[&str]) -> Option<String> {
        nombres
            .iter()
            .find_map(|n| {
                v.get(n)
                    .and_then(|x| x.as_str())
                    .map(|s| s.trim().to_string())
            })
            .filter(|s| !s.is_empty())
    }
    let texto_largo = toma(v, &["macro_concept", "macro", "sintesis", "summary"]);
    let faltantes: [(&str, [&str; 3]); 4] = [
        ("title", ["titulo", "nombre", "concepto"]),
        ("description", ["descripcion", "detalle", "explicacion"]),
        ("resumen", ["sintesis", "summary", "resumen_ejecutivo"]),
        ("principio", ["principle", "insight", "regla"]),
    ];
    for (destino, alias) in faltantes {
        let ya = v
            .get(destino)
            .and_then(|x| x.as_str())
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if ya {
            continue;
        }
        if let Some(s) = toma(v, &alias) {
            v[destino] = serde_json::Value::String(s);
        } else if let Some(largo) = texto_largo.as_ref() {
            v[destino] = serde_json::Value::String(if destino == "title" {
                // Un título legible: la primera cláusula, sin cortar palabras al medio.
                let corte = largo
                    .find([',', '.', ';', ':'])
                    .filter(|&i| i >= 12)
                    .unwrap_or_else(|| largo.len().min(70));
                let mut s: String = largo.chars().take(corte).collect();
                s = s.trim().trim_end_matches([',', '.', ';', ':']).to_string();
                s
            } else {
                largo.clone()
            });
        }
    }
    if let Some(m) = v.get("match").and_then(|x| x.as_f64()) {
        if m > 1.0 {
            v["match"] = serde_json::json!(m / 100.0);
        }
    }
    let sin_tags = v
        .get("tags")
        .and_then(|x| x.as_array())
        .map(|a| a.is_empty())
        .unwrap_or(true);
    if sin_tags {
        v["tags"] = serde_json::json!(["Condensado"]);
    }
}

#[cfg(test)]
mod tests_condensar {
    use super::normalizar_condensado;
    use serde_json::json;

    #[test]
    fn normaliza_lo_medido_con_gpt_oss() {
        let mut v = json!({"macro_concept": "Grafo íntegro + API estable", "match": 92,
                           "principio": "La integridad referencial es la base."});
        normalizar_condensado(&mut v);
        assert_eq!(v["title"], json!("Grafo íntegro + API estable"));
        assert_eq!(v["match"], json!(0.92));
        assert!(v["description"].as_str().unwrap().contains("Grafo"));
        assert_eq!(v["tags"], json!(["Condensado"]));
    }

    #[test]
    fn respuesta_conforme_no_se_toca() {
        let mut ok = json!({"title": "X", "description": "Y", "resumen": "Z",
                            "principio": "P", "match": 0.5, "tags": ["a"]});
        let antes = ok.clone();
        normalizar_condensado(&mut ok);
        assert_eq!(ok, antes);
    }

    #[test]
    fn vacio_no_paniquea() {
        let mut vacio = json!({});
        normalizar_condensado(&mut vacio);
        assert_eq!(vacio["tags"], json!(["Condensado"]));
    }
}

fn fill_template(tpl: &str, ctx: &std::collections::HashMap<String, String>) -> String {
    let mut out = String::with_capacity(tpl.len() + 512);
    let bytes: Vec<char> = tpl.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == '{' && bytes[i + 1] == '{' {
            if let Some(close) = (i + 2..bytes.len().saturating_sub(1))
                .find(|&j| bytes[j] == '}' && bytes[j + 1] == '}')
            {
                let key: String = bytes[i + 2..close].iter().collect();
                out.push_str(ctx.get(&key).map(|s| s.as_str()).unwrap_or(""));
                i = close + 2;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// Fallback cuando el modelo no responde o devuelve vacío.
fn fallback_for(
    action: &str,
    spec: &Value,
    ctx: &std::collections::HashMap<String, String>,
) -> Value {
    let fb = &spec["fallbacks"];
    let first = fb.as_object().and_then(|o| o.values().next()).cloned();
    if let Some(v) = first {
        if !v.is_null() && v.get("__error").is_none() {
            // los literales extraídos pueden llevar placeholders
            let txt = serde_json::to_string(&v).unwrap_or_default();
            let filled = fill_template(&txt, ctx);
            if let Ok(parsed) = serde_json::from_str::<Value>(&filled) {
                return parsed;
            }
        }
    }
    // braindump: estructura calculada (el original la computa en runtime)
    if action == "braindump" {
        let raw = ctx.get("rawText").cloned().unwrap_or_default();
        let clean: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        let title = if clean.is_empty() {
            "Idea nuclear".to_string()
        } else {
            clean.chars().take(60).collect::<String>()
        };
        let desc = if clean.len() > 80 {
            format!("{}...", clean.chars().take(160).collect::<String>())
        } else {
            "Idea nuclear sintetizada a partir del volcado de pensamiento.".to_string()
        };
        let segs: Vec<String> = clean
            .split(['.', '\n', ';'])
            .map(|s| s.trim().to_string())
            .filter(|s| s.len() > 3)
            .collect();
        let cats = [
            "ESTRATEGIA",
            "ARQUITECTURA",
            "EJECUCIÓN",
            "VALIDACIÓN",
            "MÉTRICAS",
        ];
        let childs: Vec<Value> = segs.iter().take(8).enumerate().map(|(i, seg)| {
            json!({
                "tempId": format!("node-{}", i + 1),
                "connectsTo": "root",
                "title": if seg.len() > 38 { seg.chars().take(38).collect::<String>() } else { seg.clone() },
                "description": if seg.len() > 38 { seg.clone() } else { "Derivación estructurada a partir de la descarga conceptual.".to_string() },
                "category": cats[i % cats.len()],
                "tags": ["Idea", "Estructura"]
            })
        }).collect();
        return json!({
            "root": { "title": title, "description": desc, "category": "NÚCLEO", "tags": ["BrainDump", "Visión"] },
            "nodes": childs
        });
    }
    // hybrid: literal inline del original
    if action == "hybrid" {
        let a = ctx.get("nodeA.title").cloned().unwrap_or_default();
        let b = ctx.get("nodeB.title").cloned().unwrap_or_default();
        return json!({
            "title": format!("Híbrido: {} + {}", a.chars().take(15).collect::<String>(), b.chars().take(15).collect::<String>()),
            "description": format!("Sinergia que combina la propuesta central de {a} con las fortalezas operativas de {b}."),
            "tags": ["Híbrido IA", "Sinergia", "Fusión"],
            "rationale": "Unificación de conceptos complementarios para maximizar impacto."
        });
    }
    json!([])
}

// ─────────────────────────────────────────────────────────────────────────────
// Proveedores
// ─────────────────────────────────────────────────────────────────────────────

fn resolve_key(st: &AppState, headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-gemini-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| st.env_key.clone())
        .or_else(|| crate::claves::obtener("gemini_api_key", &st.data_dir))
}

/// La cadena de proveedores también se puede fijar en `nodeflow.config.json` (`ai_chain`), para que
/// la app instalada no dependa de variables de entorno.
fn cadena_del_config(data_dir: &std::path::Path) -> Option<String> {
    let txt = std::fs::read_to_string(data_dir.join("nodeflow.config.json")).ok()?;
    let v: Value = serde_json::from_str(&txt).ok()?;
    v["ai_chain"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Llama al proveedor configurado siguiendo una cadena de intentos.
///
/// Cadena por defecto: Ollama Cloud (gratis, vía el daemon local) → Gemini (fallback).
/// Se puede cambiar con `NODEFLOW_AI_CHAIN=ollama,gemini` o `NODEFLOW_AI_CHAIN=gemini`.
/// Orden de proveedores a intentar (variable de entorno o `ai_chain` del config).
fn cadena_de_proveedores(st: &AppState) -> Vec<String> {
    std::env::var("NODEFLOW_AI_CHAIN")
        .or_else(|_| std::env::var("NODEFLOW_AI_PROVIDER"))
        .ok()
        .or_else(|| cadena_del_config(&st.data_dir))
        .unwrap_or_else(|| "ollama,gemini".to_string())
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Fase 11 — **modo de inferencia por tarea**, elegido por el usuario en la UI.
///
/// El ruteo es determinista a propósito (ver ADR 0005: un modelo chico clasificando acertó 1 de 5).
/// - `local`/`edge` → sólo el modelo local: sin cuota, sin red, costo cero.
/// - `nube`/`cloud` → sólo el proveedor en la nube: razonamiento profundo sobre varias ramas.
/// - ausente, vacío o desconocido → la cadena configurada (`auto`).
fn cadena_por_modo(modo: Option<&str>, base: Vec<String>) -> Vec<String> {
    match modo.map(|m| m.trim().to_lowercase()).as_deref() {
        Some("local") | Some("edge") => vec!["ollama".to_string()],
        Some("nube") | Some("cloud") => vec!["gemini".to_string()],
        _ => base,
    }
}

/// Clave del motor: resolución central (`claves.rs`) — entorno, `.env`, **llavero del sistema** y
/// config en texto plano, en ese orden. Nunca sale de acá.
fn clave_del_motor(st: &AppState, m: &crate::motores::Motor) -> Option<String> {
    match m.proveedor.as_str() {
        "gemini" => st
            .env_key
            .clone()
            .or_else(|| crate::claves::obtener("gemini_api_key", &st.data_dir)),
        "openai" | "ollama" => {
            let nombre = m.clave_ref.clone()?;
            if let Some(v) = crate::claves::obtener(&nombre, &st.data_dir) {
                return Some(v);
            }
            // `clave_config` debería tener el NOMBRE del campo del config, pero es fácil pegar la clave
            // ahí. Si el nombre no existe y lo que hay parece una clave, se usa tal cual: es la
            // diferencia entre "falta la clave" y que el motor funcione.
            let parece_clave = nombre.len() > 20 && !nombre.contains(char::is_whitespace);
            parece_clave.then(|| nombre.clone())
        }
        _ => None,
    }
}

/// Fase 9 — llama a un proveedor **con caché**: consulta `nodo + prompt + proveedor + esquema` antes
/// de gastar la llamada y guarda el resultado si respondió. Siempre devuelve una `Llamada` con
/// consumo, costo y origen (`cache: "hit" | "miss"`), que es lo que viaja en la traza.
async fn call_provider_cached(
    st: &AppState,
    key: &str,
    motor: &crate::motores::Motor,
    prompt: &str,
    schema: &Value,
    system: Option<&str>,
    nodo: &str,
    sin_cache: bool,
    semilla: Option<&str>,
) -> Option<crate::costo::Llamada> {
    // La caché se identifica por **motor** (proveedor@modelo): cambiar de motor no reusa nada.
    // `sin_cache` la saltea por completo (lectura y escritura): lo usa la planilla de evaluación,
    // que mide al modelo real y no a la caché — si no, una segunda corrida reporta 0 ms y 0 tokens.
    let etiqueta = format!("{}@{}", motor.proveedor, motor.modelo);
    let provider = etiqueta.as_str();
    let clave = crate::costo::clave_cache(nodo, prompt, provider, schema);
    if !sin_cache {
        if let Some(e) = st.cache.get(&clave) {
            log::info!(
            "ia: caché HIT para «{nodo}» con {provider} · {} tokens evitados (modelo {}) · clave {}",
            e.tokens,
            e.modelo,
            &clave[..12]
        );
            return Some(crate::costo::Llamada {
                valor: e.valor,
                proveedor: provider.to_string(),
                modelo: e.modelo,
                consumo: crate::costo::Consumo::default(),
                estimado: false,
                cache: true,
                tokens_evitados: e.tokens,
                ms: 0,
            });
        }
    }

    // ── Capa 2: caché SEMÁNTICA ─────────────────────────────────────────────
    // La clave exacta falló. Antes de pagar una generación, buscamos una respuesta ya generada para un
    // pedido **parecido** (mismo proveedor, misma acción). Se compara por el pedido del usuario y no por
    // el prompt entero: el prompt lleva el lienzo, que cambia en cada pedido.
    let mut vector_pedido: Option<Vec<f32>> = None;
    if !sin_cache {
        if let Some(seed) = semilla.map(str::trim).filter(|s| !s.is_empty()) {
            vector_pedido = crate::semantica::vector(st, seed).await;
            if let Some((clave_vieja, e, sim, como)) =
                st.cache
                    .buscar_parecido(nodo, provider, seed, vector_pedido.as_deref())
            {
                st.cache.registrar_semantico(e.tokens);
                log::info!(
                    "ia: caché SEMÁNTICA ({como} {:.3}) para «{nodo}» con {provider} · {} tokens evitados \
                     (respuesta de «{}») · pedido «{}»",
                    sim,
                    e.tokens,
                    &clave_vieja[..12.min(clave_vieja.len())],
                    &seed.chars().take(60).collect::<String>()
                );
                return Some(crate::costo::Llamada {
                    valor: e.valor,
                    proveedor: provider.to_string(),
                    modelo: e.modelo,
                    consumo: crate::costo::Consumo::default(),
                    estimado: false,
                    cache: true,
                    tokens_evitados: e.tokens,
                    ms: 0,
                });
            }
        }
    }

    let t = std::time::Instant::now();
    let r = call_motor(st, key, motor, prompt, schema, system).await?;
    // Medido si el proveedor reporta `usage`; estimado —y declarado como estimado— si no.
    let (consumo, estimado) = match r.consumo.clone() {
        Some(c) => (c, false),
        None => (
            crate::costo::consumo_estimado(prompt, &r.valor.to_string()),
            true,
        ),
    };
    let llamada = crate::costo::Llamada {
        valor: r.valor,
        proveedor: provider.to_string(),
        modelo: r.modelo,
        consumo,
        estimado,
        cache: false,
        tokens_evitados: 0,
        ms: t.elapsed().as_millis(),
    };
    if !sin_cache {
        st.cache.put(
            &clave,
            crate::costo::entrada_nueva(
                llamada.valor.clone(),
                provider,
                &llamada.modelo,
                llamada.consumo.total(),
                &crate::semantica::normalizar(semilla.unwrap_or("")),
                vector_pedido.clone(),
            ),
        );
    }
    log::info!(
        "ia: {provider} «{}» · {} tokens ({}) · {} ms · nodo «{nodo}» · clave {}",
        llamada.modelo,
        llamada.consumo.total(),
        if llamada.estimado {
            "estimado"
        } else {
            "medido"
        },
        llamada.ms,
        &clave[..12]
    );
    Some(llamada)
}

/// Llama al motor elegido (o al que pida la tarea con `modo`).
async fn call_motor(
    st: &AppState,
    key: &str,
    m: &crate::motores::Motor,
    prompt: &str,
    schema: &Value,
    system: Option<&str>,
) -> Option<crate::costo::Respuesta> {
    match m.proveedor.as_str() {
        "ollama" => {
            let base = m
                .base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:11434/v1".to_string());
            let raiz_url = base
                .trim_end_matches("/v1")
                .trim_end_matches('/')
                .to_string();
            if let Some(r) =
                call_ollama_nativo(st, &raiz_url, &m.modelo, prompt, system, schema).await
            {
                return Some(r);
            }
            log::warn!(
                "{}: sin respuesta por la API nativa; pruebo el camino compatible con OpenAI",
                m.id
            );
            call_ollama(st, Some((base, m.modelo.clone())), prompt, system).await
        }
        "gemini" => {
            // La clave del WebView (BYOK) tiene prioridad sobre la del entorno.
            let clave = if !key.trim().is_empty() {
                key.to_string()
            } else {
                clave_del_motor(st, m).unwrap_or_default()
            };
            if clave.trim().is_empty() {
                log::warn!("motor «{}» sin clave de API", m.id);
                return None;
            }
            call_gemini(st, &clave, Some(&m.modelo), prompt, schema, system).await
        }
        "openai" => {
            let Some(clave) = clave_del_motor(st, m) else {
                log::warn!("motor «{}» sin clave de API", m.id);
                return None;
            };
            let Some(base) = m.base_url.clone() else {
                log::warn!("motor «{}» sin base_url", m.id);
                return None;
            };
            call_openai(st, &base, &clave, &m.modelo, prompt, system).await
        }
        otro => {
            log::warn!("proveedor desconocido en el motor «{}»: {otro}", m.id);
            None
        }
    }
}

/// Catálogo real de motores: lo que el daemon local ofrece + la nube configurada.
async fn catalogo(st: &AppState) -> Vec<crate::motores::Motor> {
    use crate::motores::Motor;
    let ollama_url = std::env::var("NODEFLOW_OLLAMA_URL")
        .unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
    let raiz_ollama = ollama_url
        .trim_end_matches("/v1")
        .trim_end_matches('/')
        .to_string();

    let mut v: Vec<Motor> = Vec::new();
    match st.http.get(format!("{raiz_ollama}/api/tags")).send().await {
        Ok(r) if r.status().is_success() => {
            let tags: Vec<String> = r
                .json::<Value>()
                .await
                .ok()
                .and_then(|j| {
                    j["models"].as_array().map(|a| {
                        a.iter()
                            .filter_map(|m| m["name"].as_str().map(String::from))
                            .collect()
                    })
                })
                .unwrap_or_default();
            v.extend(crate::motores::motores_de_tags(&tags, &ollama_url));
        }
        Ok(r) => log::warn!("Ollama /api/tags respondió HTTP {}", r.status()),
        Err(e) => {
            v.push(Motor {
                id: "ollama:daemon".into(),
                etiqueta: "Ollama no responde".into(),
                proveedor: "ollama".into(),
                modelo: String::new(),
                donde: crate::motores::EN_TU_PLACA.into(),
                base_url: Some(ollama_url.clone()),
                disponible: false,
                nota: Some(format!("el daemon local no responde ({e})")),
                clave_ref: None,
            });
        }
    }

    // Nube: el proveedor de Gemini, con clave o sin ella (declarado, para que se vea por qué no está)
    let con_clave = st.env_key.is_some();
    v.push(Motor {
        disponible: con_clave,
        nota: (!con_clave).then(|| "falta la clave (config o .env)".to_string()),
        ..Motor::nuevo(
            "gemini",
            CANDIDATE_MODELS[0],
            crate::motores::NUBE_PAGA,
            None,
            None,
        )
    });

    // Proveedores compatibles con OpenAI declarados en el config (`proveedores`).
    if let Ok(txt) = std::fs::read_to_string(st.data_dir.join("nodeflow.config.json")) {
        if let Ok(cfg) = serde_json::from_str::<Value>(&txt) {
            for p in cfg["proveedores"].as_array().into_iter().flatten() {
                let (Some(modelo), Some(base)) = (p["modelo"].as_str(), p["base_url"].as_str())
                else {
                    continue;
                };
                let clave_ref = p["clave_env"].as_str().map(String::from);
                // La clave puede estar en el entorno, en el llavero o (legado) en el config: se pregunta
                // a la resolución central en vez de mirar el archivo a mano.
                let en_llave_o_config = p["clave_config"]
                    .as_str()
                    .map(|k| crate::claves::disponible(k, &st.data_dir))
                    .unwrap_or(false);
                let pegada = p["clave_config"]
                    .as_str()
                    .filter(|k| {
                        !en_llave_o_config && k.len() > 20 && !k.contains(char::is_whitespace)
                    })
                    .is_some();
                let propia = clave_ref
                    .clone()
                    .map(|n| std::env::var(&n).is_ok())
                    .unwrap_or(false)
                    || en_llave_o_config
                    || pegada;
                v.push(Motor {
                    id: format!("openai:{}", p["id"].as_str().unwrap_or(modelo)),
                    etiqueta: p["etiqueta"].as_str().map(String::from).unwrap_or_else(|| {
                        format!("{modelo} · {}", p["id"].as_str().unwrap_or("api"))
                    }),
                    proveedor: "openai".into(),
                    modelo: modelo.to_string(),
                    donde: p["donde"]
                        .as_str()
                        .unwrap_or(crate::motores::NUBE_PAGA)
                        .to_string(),
                    base_url: Some(base.to_string()),
                    disponible: propia,
                    nota: (!propia).then(|| "falta la clave".to_string()),
                    clave_ref: p["clave_config"].as_str().map(String::from).or(clave_ref),
                });
            }
        }
    }
    // Antes de aplicar la memoria de fallos: ¿cambió el config? Entonces se olvida (el usuario pudo
    // haber agregado la clave que faltaba). Es la diferencia entre "pegar la clave y que funcione" y
    // "pegar la clave y que la app siga diciendo que no está".
    if let Ok(meta) = std::fs::metadata(st.data_dir.join("nodeflow.config.json")) {
        let sello = meta.len()
            + meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
        if let Ok(mut previo) = st.config_sello.lock() {
            if *previo != 0 && *previo != sello {
                if let Ok(mut m) = st.motores_caidos.lock() {
                    let cuantos = m.len();
                    m.clear();
                    log::info!("catálogo: el config cambió → se olvidan {cuantos} motor(es) caído(s) y se reintentan");
                }
            }
            *previo = sello;
        }
    }
    // Motores que ya fallaron por créditos o clave: se declaran, no se ofrecen como si anduvieran.
    let caidos = st.motivos_de_motores();
    for m in v.iter_mut() {
        if let Some(motivo) = caidos.get(&m.id) {
            m.disponible = false;
            m.nota = Some(motivo.clone());
        }
    }
    v
}

/// Plan de esta llamada: catálogo real + elección guardada + `modo` opcional de la tarea.
/// Elección efectiva del motor: la guardada por el usuario, o el modelo configurado cuando todavía
/// no eligió (así el default es el de siempre, no uno nuevo que sorprenda).
fn seleccion_efectiva(st: &AppState, cat: &[crate::motores::Motor]) -> Option<String> {
    if let Some(elegido) = crate::motores::seleccionado(&st.data_dir) {
        return Some(elegido);
    }
    let preferido = std::env::var("NODEFLOW_OLLAMA_MODEL")
        .unwrap_or_else(|_| "nemotron-3-nano:30b-cloud".to_string());
    cat.iter()
        .find(|m| m.modelo == preferido && m.disponible)
        .map(|m| m.id.clone())
}

async fn plan_de_motores(
    st: &AppState,
    modo: Option<&str>,
    tarea: crate::motores::Tarea,
    accion: &str,
) -> Vec<crate::motores::Motor> {
    let cat = catalogo(st).await;
    let mut sel = crate::motores::seleccionado(&st.data_dir);
    if sel.is_none() && modo.is_none() {
        sel = seleccion_efectiva(st, &cat);
    }
    // `auto:tarea` arma la cadena según lo que se está pidiendo (el bucle del lienzo no espera a
    // nadie, lo profundo usa el local más grande, lo que necesita herramientas sube a la nube) y
    // **según lo que la planilla de evaluación ya midió**: el ganador de esa acción va primero.
    let planilla = crate::eval::leer(st);
    let plan =
        crate::motores::plan_tarea(&cat, sel.as_deref(), modo, tarea, accion, planilla.as_ref());
    if plan.is_empty() {
        log::warn!("no hay ningún motor disponible (Ollama apagado y sin clave de nube)");
    }
    plan
}

/// La tarea de ruteo de una acción. Con una **conversación en curso** el pedido ya no es una orden
/// suelta ("ahora enfocá eso"): ahí manda el perfil Diálogo.
fn tarea_de(st: &AppState, accion: &str) -> crate::motores::Tarea {
    if accion == "voz" && crate::dialogo::tiene_hilo(&st.vault.raiz().join(".nodeflow")) {
        log::info!("ruteo: conversación en curso → perfil Diálogo para «{accion}»");
        crate::motores::Tarea::Dialogo
    } else {
        crate::motores::Tarea::de_accion(accion)
    }
}

/// ¿El plan de esta acción toca la placa? Se pregunta **antes** de generar: el turno local es uno solo.
/// Cuesta un `/api/tags` extra (~40 ms) sobre una generación de segundos: se paga solo.
async fn plan_toca_la_placa(st: &AppState, modo: Option<&str>, accion: &str) -> bool {
    plan_de_motores(st, modo, tarea_de(st, accion), accion)
        .await
        .iter()
        .any(|m| m.donde == crate::motores::EN_TU_PLACA)
}

async fn call_model(
    st: &AppState,
    key: &str,
    prompt: &str,
    schema: &Value,
    system: Option<&str>,
    nodo: &str,
    modo: Option<&str>,
    accion: &str,
    sin_cache: bool,
    semilla: Option<&str>,
) -> Option<crate::costo::Llamada> {
    // El perfil lo decide la acción… salvo que haya una **conversación en curso**: a partir del segundo
    // turno el pedido ya no es una orden suelta ("ahora enfocá eso"), y el hilo sólo sirve si el modelo
    // lo entiende. Ahí manda el perfil Diálogo (nube primero, local como último recurso).
    let tarea = tarea_de(st, accion);
    for (i, m) in plan_de_motores(st, modo, tarea, accion)
        .await
        .into_iter()
        .enumerate()
    {
        if i > 0 {
            // Estamos en la red de seguridad: quedó registrado para poder medirlo después.
            log::info!("ruteo: {} no alcanzó, sigo con {}", tarea.etiqueta(), m.id);
        }
        if let Some(llamada) = call_provider_cached(
            st, key, &m, prompt, schema, system, nodo, sin_cache, semilla,
        )
        .await
        {
            return Some(llamada);
        }
        log::warn!("el motor «{}» no respondió; no hay otro en el plan", m.id);
    }
    None
}

/// `GET /api/ai/motores` — catálogo real de motores y cuál está elegido.
///
/// El catálogo se arma con lo que existe en la máquina (tags del daemon de Ollama, nube configurada,
/// proveedores compatibles con OpenAI declarados en el config) y **declara lo que falta** en vez de
/// esconderlo: un motor sin clave o con el daemon apagado aparece como no disponible y con el motivo.
async fn ai_motores(State(st): State<AppState>) -> impl IntoResponse {
    let cat = catalogo(&st).await;
    let elegido = crate::motores::seleccionado(&st.data_dir);
    let efectivo = crate::motores::plan(&cat, seleccion_efectiva(&st, &cat).as_deref(), None)
        .first()
        .map(|m| m.id.clone());
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "seleccionado": elegido,
            "efectivo": efectivo,
            "motores": cat,
        })),
    )
}

/// `POST /api/ai/proveedor` — agrega (o actualiza) un proveedor compatible con OpenAI desde la UI.
///
/// La clave se guarda **en el config local** (`<id>_api_key`) y nunca se devuelve al frontend: la
/// respuesta sólo confirma y devuelve el catálogo actualizado.
async fn ai_proveedor(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let id = body["id"].as_str().unwrap_or("").trim().to_lowercase();
    let base = body["base_url"].as_str().unwrap_or("").trim().to_string();
    let modelo = body["modelo"].as_str().unwrap_or("").trim().to_string();
    let etiqueta = body["etiqueta"].as_str().unwrap_or("").trim().to_string();
    let clave = body["api_key"].as_str().unwrap_or("").trim().to_string();
    let donde = body["donde"]
        .as_str()
        .unwrap_or(crate::motores::NUBE_PAGA)
        .to_string();
    if id.is_empty() || base.is_empty() || modelo.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": "id, base_url y modelo son obligatorios" })),
        );
    }
    if !base.starts_with("http://") && !base.starts_with("https://") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": "base_url debe empezar con http:// o https://" })),
        );
    }

    let ruta = st.data_dir.join("nodeflow.config.json");
    let mut cfg: Value = std::fs::read_to_string(&ruta)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}));
    let Some(obj) = cfg.as_object_mut() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": "config inválido" })),
        );
    };

    let mut provs: Vec<Value> = obj
        .get("proveedores")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();
    provs.retain(|p| p["id"].as_str() != Some(id.as_str()));
    let mut entrada = json!({
        "id": id,
        "etiqueta": if etiqueta.is_empty() { format!("{modelo} · {id}") } else { etiqueta },
        "base_url": base,
        "modelo": modelo,
        "donde": donde,
    });
    let clave_config = format!("{id}_api_key");
    if !clave.is_empty() {
        // La clave va al **llavero del sistema**, no al config: el archivo deja de tener secretos.
        // Nunca viaja al frontend.
        if let Err(e) =
            crate::claves::Store::escribir(&crate::claves::Llavero, &clave_config, &clave)
        {
            log::warn!("claves: no pude guardar {clave_config} en el llavero: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    json!({ "ok": false, "error": format!("no pude guardar la clave en el llavero: {e}") }),
                ),
            );
        }
        // Cualquier copia previa en texto plano del mismo campo se limpia.
        obj.remove(&clave_config);
        entrada["clave_config"] = json!(clave_config);
    }
    provs.push(entrada);
    obj.insert("proveedores".into(), Value::Array(provs));
    let txt = match serde_json::to_string_pretty(&cfg) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": e.to_string() })),
            )
        }
    };
    if let Err(e) = std::fs::write(&ruta, txt) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e.to_string() })),
        );
    }
    log::info!(
        "proveedor agregado: {id} ({modelo}){}",
        if clave.is_empty() {
            " sin clave"
        } else {
            " con clave"
        }
    );

    let cat = catalogo(&st).await;
    (
        StatusCode::OK,
        Json(
            json!({ "ok": true, "motores": cat, "seleccionado": crate::motores::seleccionado(&st.data_dir) }),
        ),
    )
}

/// `POST /api/ai/motor` — elige el motor de **toda la app** (o `auto` para volver a la cadena).
async fn ai_motor(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let id = body["id"].as_str();
    // Validación contra el catálogo real (hallazgo del QA de escritura): antes, un id inventado se
    // guardaba con `ok: true` y la app quedaba apuntando a un motor inexistente.
    if let Some(id) = id.map(str::trim).filter(|s| !s.is_empty() && *s != "auto") {
        let cat = catalogo(&st).await;
        let ids: Vec<String> = cat.iter().map(|m| m.id.clone()).collect();
        if !crate::motores::motor_valido(id, &ids) {
            let mut disponibles = ids.clone();
            disponibles.push("auto".into());
            for a in ["auto:local", "auto:nube", "auto:tarea"] {
                disponibles.push(a.into());
            }
            log::warn!(
                "motor rechazado: {id} no está en el catálogo ({} disponibles)",
                ids.len()
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(
                    json!({"ok": false, "error": format!("motor desconocido: {id}"), "disponibles": disponibles}),
                ),
            );
        }
    }
    match crate::motores::guardar_seleccion(&st.data_dir, id) {
        Ok(_) => {
            let elegido = crate::motores::seleccionado(&st.data_dir);
            log::info!(
                "motor de la app: {:?}",
                elegido.as_deref().unwrap_or("auto (cadena configurada)")
            );
            (
                StatusCode::OK,
                Json(json!({ "ok": true, "seleccionado": elegido })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e })),
        ),
    }
}

/// `GET /api/ai/cache` — el ahorro medido por la propia app: entradas, hits y tokens evitados.
async fn ai_cache(State(st): State<AppState>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "cache": st.cache.stats(),
            "tarifas_declaradas": st.tarifas.modelos_declarados(),
        })),
    )
}

async fn call_gemini(
    st: &AppState,
    key: &str,
    modelo: Option<&str>,
    prompt: &str,
    schema: &Value,
    system: Option<&str>,
) -> Option<crate::costo::Respuesta> {
    // El motor elegido va primero; los candidatos quedan como respaldo si ese modelo no existe.
    let mut modelos: Vec<String> = Vec::new();
    if let Some(m) = modelo.filter(|m| !m.trim().is_empty()) {
        modelos.push(m.to_string());
    }
    for m in CANDIDATE_MODELS {
        if !modelos.iter().any(|x| x == m) {
            modelos.push(m.to_string());
        }
    }
    for model in modelos {
        let model = model.as_str();
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
        );
        let mut body = json!({
            "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
            "generationConfig": { "responseMimeType": "application/json", "responseSchema": schema }
        });
        if let Some(sys) = system {
            body["systemInstruction"] = json!({ "parts": [{ "text": sys }] });
        }

        match st
            .http
            .post(&url)
            .header("x-goog-api-key", key)
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                let value: Value = match resp.json().await {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let text = value["candidates"][0]["content"]["parts"][0]["text"]
                    .as_str()
                    .unwrap_or("");
                if let Some(parsed) = parse_json_text(text) {
                    // `usageMetadata` es la medición del proveedor; si falta, se estima y se declara.
                    return Some(crate::costo::Respuesta {
                        valor: parsed,
                        modelo: model.to_string(),
                        consumo: crate::costo::consumo_gemini(&value),
                    });
                }
                log::warn!("Gemini {model}: respuesta no parseable");
            }
            Ok(resp) => log::warn!("Gemini {model} falló con HTTP {}", resp.status()),
            Err(e) => log::warn!("Gemini {model} error de red: {e}"),
        }
    }
    log::warn!("Todos los modelos candidatos fallaron; activo fallback inteligente");
    None
}

/// Llama a un modelo servido por el daemon de Ollama (o por su nube gratuita, vía el daemon).
async fn call_ollama(
    st: &AppState,
    base_y_modelo: Option<(String, String)>,
    prompt: &str,
    system: Option<&str>,
) -> Option<crate::costo::Respuesta> {
    let base = base_y_modelo
        .as_ref()
        .map(|(b, _)| b.clone())
        .unwrap_or_else(|| {
            std::env::var("NODEFLOW_OLLAMA_URL")
                .unwrap_or_else(|_| "http://localhost:11434/v1".to_string())
        });
    let model = base_y_modelo.map(|(_, m)| m).unwrap_or_else(|| {
        std::env::var("NODEFLOW_OLLAMA_MODEL")
            .unwrap_or_else(|_| "nemotron-3-nano:30b-cloud".to_string())
    });
    let mut messages = Vec::new();
    if let Some(s) = system {
        messages.push(json!({ "role": "system", "content": s }));
    }
    messages.push(json!({ "role": "user", "content": prompt }));

    let body = json!({
        "model": model,
        "messages": messages,
        "response_format": { "type": "json_object" },
        "temperature": 0.7,
        // El camino compatible con OpenAI no acepta `num_ctx` ni `keep_alive`; el tope de salida sí.
        "max_tokens": st.borrador.num_predict
    });

    match st
        .http
        .post(format!("{base}/chat/completions"))
        .json(&body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let v: Value = resp.json().await.ok()?;
            let text = v["choices"][0]["message"]["content"].as_str().unwrap_or("");
            parse_json_text(text).map(|p| crate::costo::Respuesta {
                valor: p,
                modelo: format!("{model} (ollama)"),
                consumo: crate::costo::consumo_openai(&v),
            })
        }
        Ok(resp) => {
            let codigo = resp.status().as_u16();
            log::warn!("Ollama HTTP {codigo}");
            if codigo == 402 || codigo == 401 {
                st.marcar_motor_caido(
                    &format!("ollama:{model}"),
                    if codigo == 402 {
                        "requiere créditos (HTTP 402)"
                    } else {
                        "clave rechazada (HTTP 401)"
                    },
                );
            }
            None
        }
        Err(e) => {
            log::warn!("Ollama error de red: {e}");
            None
        }
    }
}

/// API **nativa** de Ollama (`/api/chat`) con el esquema como **gramática**.
///
/// Es la diferencia entre "el modelo hizo lo que quiso" y "el modelo cumple el contrato": medido, sin
/// gramática los modelos que corren en la placa devolvían un objeto donde la acción pide una lista, y
/// la función parecía rota. La gramática garantiza la forma (no la verdad: eso lo sigue validando el
/// código, ver ADR 0003).
async fn call_ollama_nativo(
    st: &AppState,
    raiz_url: &str,
    modelo: &str,
    prompt: &str,
    system: Option<&str>,
    schema: &Value,
) -> Option<crate::costo::Respuesta> {
    let mut messages = Vec::new();
    if let Some(s) = system {
        messages.push(json!({ "role": "system", "content": s }));
    }
    messages.push(json!({ "role": "user", "content": prompt }));
    let body = json!({
        "model": modelo,
        "messages": messages,
        "stream": false,
        "format": crate::motores::esquema_para_ollama(schema),
        // C — topes explícitos. Sin `num_predict` el modelo se explaya, cruza el contexto y el pedido
        // muere (medido 15/09: 6.238 tokens, `slot context shift`, 500 a los 1m49s). `num_ctx` se fija
        // por latencia objetivo, no por el máximo del modelo: 16k con un 7B son 426 s de TTFT.
        // `keep_alive` por request: una variable global en -1 pinnea la VRAM para siempre.
        "options": {
            "temperature": 0.7,
            "num_ctx": st.borrador.num_ctx,
            "num_predict": st.borrador.num_predict
        },
        "keep_alive": st.borrador.keep_alive
    });
    let url = format!("{}/api/chat", raiz_url.trim_end_matches('/'));
    match st.http.post(&url).json(&body).send().await {
        Ok(r) if r.status().is_success() => {
            let v: Value = r.json().await.ok()?;
            let text = v["message"]["content"].as_str().unwrap_or("");
            match parse_json_text(text) {
                Some(p) => Some(crate::costo::Respuesta {
                    valor: p,
                    modelo: modelo.to_string(),
                    consumo: crate::costo::consumo_ollama_nativo(&v),
                }),
                None => {
                    log::warn!("{modelo}: la gramática devolvió JSON no parseable");
                    None
                }
            }
        }
        Ok(r) => {
            let codigo = r.status().as_u16();
            log::warn!("{modelo}: HTTP {codigo} en /api/chat");
            if codigo == 402 || codigo == 401 {
                st.marcar_motor_caido(
                    &format!("ollama:{modelo}"),
                    if codigo == 402 {
                        "requiere créditos (HTTP 402)"
                    } else {
                        "clave rechazada (HTTP 401)"
                    },
                );
            }
            None
        }
        Err(e) => {
            log::warn!("{modelo}: error de red en /api/chat — {e}");
            None
        }
    }
}

/// Proveedor **compatible con OpenAI**: el mismo formato para DeepSeek, vLLM, Fireworks, etc.
/// La clave la resuelve el motor (`clave_ref`), nunca viaja al frontend.
async fn call_openai(
    st: &AppState,
    base: &str,
    clave: &str,
    modelo: &str,
    prompt: &str,
    system: Option<&str>,
) -> Option<crate::costo::Respuesta> {
    let mut messages = Vec::new();
    if let Some(s) = system {
        messages.push(json!({ "role": "system", "content": s }));
    }
    messages.push(json!({ "role": "user", "content": prompt }));
    let body = json!({
        "model": modelo,
        "messages": messages,
        "response_format": { "type": "json_object" },
        "temperature": 0.7
    });
    let url = format!("{}/chat/completions", base.trim_end_matches('/'));
    match st
        .http
        .post(&url)
        .bearer_auth(clave)
        .json(&body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let v: Value = resp.json().await.ok()?;
            let text = v["choices"][0]["message"]["content"].as_str().unwrap_or("");
            parse_json_text(text).map(|p| crate::costo::Respuesta {
                valor: p,
                modelo: modelo.to_string(),
                consumo: crate::costo::consumo_openai(&v),
            })
        }
        Ok(resp) => {
            log::warn!("{modelo}: HTTP {} en {url}", resp.status());
            None
        }
        Err(e) => {
            log::warn!("{modelo}: error de red — {e}");
            None
        }
    }
}

fn parse_json_text(raw: &str) -> Option<Value> {
    let t = raw.trim();
    let t = t.strip_prefix("```json").unwrap_or(t);
    let t = t.strip_prefix("```").unwrap_or(t);
    let t = t.trim_end().strip_suffix("```").unwrap_or(t).trim();
    serde_json::from_str::<Value>(t).ok()
}

// ─────────────────────────────────────────────────────────────────────────────
// Utilidades
// ─────────────────────────────────────────────────────────────────────────────

mod counters {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    pub fn next() -> u64 {
        N.fetch_add(1, Ordering::Relaxed)
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Fecha del día (UTC, `YYYY-MM-DD`). Es lo que va al snapshot medido: un `now_iso()` completo
/// hacía único cada prompt y la caché no podía acertar nunca.
pub(crate) fn hoy() -> String {
    now_iso().chars().take(10).collect()
}

pub(crate) fn now_iso() -> String {
    // ISO-8601 UTC sin dependencias extra (formato suficiente para el cliente)
    let ms = now_ms();
    let secs = ms / 1000;
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!(
        "{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}.{:03}Z",
        ms % 1000
    )
}

/// Sello **local** del turno (`YYYY-MM-DDTHH:MM`) para nombrar la nota episódica. El desfase horario lo
/// declara el usuario (`cerebro.offset_h`, por defecto -3 = Buenos Aires): una nota de las 21:11 de acá
/// no puede llamarse con la fecha de mañana, que es lo que pasaba nombrando en UTC.
pub(crate) fn sello_local(epoch_s: i64, offset_h: i32) -> String {
    let s = epoch_s + (offset_h as i64) * 3600;
    let days = s.div_euclid(86_400);
    let rem = s.rem_euclid(86_400);
    let (y, mo, d) = civil_from_days(days);
    format!(
        "{y:04}-{mo:02}-{d:02}T{:02}:{:02}",
        rem / 3600,
        (rem % 3600) / 60
    )
}

/// Días desde epoch → (año, mes, día). Algoritmo de Howard Hinnant.
pub(crate) fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ─────────────────────────────────────────────────────────────────────────────
// Fase 3 — Vault en disco
// ─────────────────────────────────────────────────────────────────────────────

/// Estado del vault: ruta, revisión, notas y últimos cambios externos.
async fn vault_info(State(st): State<AppState>) -> impl IntoResponse {
    Json(st.vault.info())
}

// ─────────────────────────────────────────────────────────────────────────────
// Sesiones del lienzo (`.nodeflow/sesiones/` en la bóveda)

/// Listado liviano (sin nodos ni aristas): es lo que dibuja el panel de sesiones.
async fn sesiones_listar(State(st): State<AppState>) -> impl IntoResponse {
    let s = crate::sesiones::Sesiones::nueva(&st.vault.raiz());
    Json(json!({
        "ok": true,
        "carpeta": crate::sesiones::CARPETA,
        "tope": crate::sesiones::MAX_SESIONES,
        "sesiones": s.listar(),
    }))
}

async fn sesiones_guardar(State(st): State<AppState>, Json(p): Json<Value>) -> impl IntoResponse {
    match crate::sesiones::Sesiones::nueva(&st.vault.raiz()).guardar(&p) {
        Ok(v) => {
            log::info!(
                "sesiones: guardada «{}» ({} nodos · {} aristas)",
                v["sesion"]["nombre"],
                v["sesion"]["nodos"],
                v["sesion"]["aristas"]
            );
            (StatusCode::OK, Json(v))
        }
        Err(e) => {
            log::warn!("sesiones: no pude guardar: {e}");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "ok": false, "error": e })),
            )
        }
    }
}

async fn sesiones_leer(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let id = q.get("id").cloned().unwrap_or_default();
    match crate::sesiones::Sesiones::nueva(&st.vault.raiz()).leer(&id) {
        Ok(v) => (StatusCode::OK, Json(json!({ "ok": true, "sesion": v }))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": e })),
        ),
    }
}

async fn sesiones_borrar(State(st): State<AppState>, Json(p): Json<Value>) -> impl IntoResponse {
    let id = p["id"].as_str().unwrap_or_default();
    match crate::sesiones::Sesiones::nueva(&st.vault.raiz()).borrar(id) {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": e })),
        ),
    }
}

/// Lee el grafo canónico. Con `?since=<rev>` responde barato cuando nada cambió (polling).
/// Clave del motor de voz activo, delegada en `stt`: el catálogo declara dónde puede estar cada clave
/// (entorno → `.env` del proyecto en dev → `nodeflow.config.json`). Nunca sale del backend y nunca se
/// escribe en un log.
fn clave_voz(st: &AppState, prov: &'static crate::stt::Proveedor) -> Option<String> {
    crate::stt::clave_de(&st.data_dir, prov)
}

/// `GET /api/idioma` — el idioma guardado. Es la fuente de verdad: la interfaz, la transcripción y la
/// voz de salida leen lo mismo, salvo que la voz tenga el suyo propio (`voz.idioma`).
async fn idioma_leer(State(st): State<AppState>) -> impl IntoResponse {
    let voz = crate::idioma::voz_idioma(&st.data_dir);
    Json(json!({
        "ok": true,
        "idioma": crate::idioma::actual(&st.data_dir),
        "voz_idioma": voz,
        "voz_tts": crate::idioma::voz_tts(&voz),
        "voz_sigue_a_la_interfaz": crate::idioma::voz_sigue_a_la_interfaz(&st.data_dir),
        "idiomas": crate::idioma::IDIOMAS,
    }))
}

/// `POST /api/idioma/voz` `{ "idioma": "es" }` — el idioma con el que la app **escucha y habla**,
/// independiente del de la interfaz. Con `auto` vuelve a seguir al de la interfaz.
async fn idioma_voz_guardar(
    State(st): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let pedido = body["idioma"].as_str().unwrap_or("");
    match crate::idioma::guardar_voz_idioma(&st.data_dir, pedido) {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": e })),
        ),
    }
}

/// `POST /api/idioma` `{ "idioma": "en" }` — guarda el idioma. Un idioma desconocido se rechaza.
async fn idioma_guardar(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let pedido = body["idioma"].as_str().unwrap_or("");
    match crate::idioma::guardar(&st.data_dir, pedido) {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": e })),
        ),
    }
}

/// `GET /api/claves/estado` — dónde vive cada clave (entorno, llavero o texto plano). Nunca el valor:
/// sólo el origen y una huella no invertible.
async fn claves_estado(State(st): State<AppState>) -> impl IntoResponse {
    let e = crate::claves::estado(&st.data_dir, &crate::claves::Llavero);
    let en_texto = e["en_texto_plano"].as_u64().unwrap_or(0);
    if en_texto > 0 {
        log::warn!(
            "claves: {en_texto} siguen en texto plano en nodeflow.config.json — se migran con POST /api/claves/migrar"
        );
    }
    Json(e)
}

/// `POST /api/claves/migrar` — mueve las claves en texto plano al llavero del sistema y las borra del
/// config. Idempotente: lo que ya está en el llavero no se toca, y si el llavero falla la clave se
/// conserva donde estaba (nunca se pierde un secreto por una migración fallida).
async fn claves_migrar(State(st): State<AppState>) -> impl IntoResponse {
    match crate::claves::migrar(&st.data_dir, &crate::claves::Llavero) {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": e })),
        ),
    }
}

/// `POST /api/claves` — guarda o borra una clave en el llavero del sistema.
/// Cuerpo: `{ "campo": "gemini_api_key", "valor": "..." }` para guardar,
/// o `{ "campo": "gemini_api_key", "borrar": true }` para borrar.
/// Responde: `{ "ok": true, "campo": "...", "origen": "llavero", "huella": "...", "largo": N }`.
/// Nunca devuelve el valor. Rechaza con 400 si el campo no pasa `claves::es_campo_de_clave`.
async fn claves_guardar(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let campo = body.get("campo").and_then(|v| v.as_str()).unwrap_or("").trim();
    if campo.is_empty() || !crate::claves::es_campo_de_clave(campo) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": "campo inválido o no es un campo de clave" })),
        );
    }

    let borrar = body.get("borrar").and_then(|v| v.as_bool()).unwrap_or(false);

    if borrar {
        match crate::claves::Store::borrar(&crate::claves::Llavero, campo) {
            Ok(()) => (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "campo": campo,
                    "origen": "llavero",
                    "huella": "",
                    "largo": 0,
                })),
            ),
            Err(e) => (
                StatusCode::BAD_REQUEST,
                Json(json!({ "ok": false, "error": e })),
            ),
        }
    } else {
        let valor = body.get("valor").and_then(|v| v.as_str()).unwrap_or("").trim();
        if valor.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "ok": false, "error": "falta \"valor\" para guardar" })),
            );
        }
        match crate::claves::Store::escribir(&crate::claves::Llavero, campo, valor) {
            Ok(()) => {
                let h = crate::claves::huella(valor);
                (
                    StatusCode::OK,
                    Json(json!({
                        "ok": true,
                        "campo": campo,
                        "origen": "llavero",
                        "huella": h,
                        "largo": valor.len(),
                    })),
                )
            }
            Err(e) => (
                StatusCode::BAD_REQUEST,
                Json(json!({ "ok": false, "error": e })),
            ),
        }
    }
}

/// `GET /api/azure/modelos` — qué modelos hay **desplegados** en la cuenta de Azure AI (plano de
/// gestión ARM). Query: `suscripcion`, `grupo`, `cuenta` (si no vienen, se resuelven del entorno o de
/// `nodeflow.config.json`) y `refrescar` para descartar el token cacheado tras un `az login`.
///
/// El token nunca sale de acá: lo resuelve `azure` (caché → token pegado → CLI) y sólo se devuelve el
/// catálogo. Falla con `ok:false` y el motivo cuando falta config o credencial, en vez de devolver una
/// lista vacía que parezca «no hay modelos».
async fn azure_modelos(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let refrescar = q.contains_key("refrescar");
    match crate::azure::consultar(&st.http, &st.data_dir, &q, refrescar).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (
            StatusCode::from_u16(e.codigo()).unwrap_or(StatusCode::BAD_GATEWAY),
            Json(json!({ "ok": false, "error": e.to_string() })),
        ),
    }
}

/// `GET /api/voz/estado` — si la voz está lista, sin exponer nunca la clave.
/// Reporta el **motor elegido** y el catálogo completo: el frontend no necesita conocerlos de antemano.
async fn voz_estado(State(st): State<AppState>) -> impl IntoResponse {
    let id = crate::stt::seleccionado(&st.data_dir);
    let prov = crate::stt::por_id(&id).unwrap_or(&crate::stt::CATALOGO[0]);
    let configurada = clave_voz(&st, prov).is_some();
    // El idioma se pide como lo pide la app (el entorno manda); si no, el de la VOZ, que puede ir por
    // su lado (leer la app en inglés y hablarle en castellano).
    let (url, modelo, idioma_ajustes) = crate::voz::ajustes();
    // El panel tiene que mostrar el modelo del motor ELEGIDO: con AssemblyAI anunciaba «enhanced»
    // (el default de Speechmatics) mientras la sesión real iba con otro modelo.
    let modelo_mostrado = if prov.id == "speechmatics" {
        modelo.clone() // Speechmatics sí se ajusta por entorno (NODEFLOW_VOZ_MODELO)
    } else {
        crate::stt::modelo_por_defecto(prov.id).to_string()
    };
    let idioma_pedido = if std::env::var("NODEFLOW_VOZ_IDIOMA").is_ok() {
        idioma_ajustes
    } else {
        crate::idioma::voz_idioma(&st.data_dir)
    };
    let (idioma, aviso) = crate::stt::idioma_efectivo(prov, &idioma_pedido);
    // La voz de salida (Kokoro local) es opcional: si no responde, se dice sin romper nada.
    let tts_url = crate::voz::tts_url();
    let tts_disponible = st
        .http
        .get(format!("{tts_url}/estado"))
        .timeout(std::time::Duration::from_millis(1500))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    let clave_campo = prov.clave_env.first().copied().unwrap_or("");
    Json(json!({
        "success": true,
        "configurada": configurada,
        "proveedor": prov.id,
        "proveedor_etiqueta": prov.etiqueta,
        "protocolo": prov.protocolo,
        // Compatibilidad: el panel actual lee `url`/`modelo`/`idioma` de acá.
        "url": prov.url,
        "modelo": modelo_mostrado,
        "idioma": idioma,
        "codec": crate::stt::CODEC,
        "idiomas_soportados": prov.idiomas,
        "nota": prov.nota,
        "aviso": aviso,
        "proveedores": crate::stt::catalogo_json(&st.data_dir),
        // Ajuste por entorno: se declara SÓLO cuando existe de verdad. Antes mostraba siempre los
        // valores base de Speechmatics, así que con AssemblyAI elegido el mismo endpoint anunciaba
        // `proveedor: assemblyai` y un modelo `enhanced` con la URL de otro motor.
        "ajuste_entorno": if std::env::var("NODEFLOW_VOZ_URL").is_ok()
            || std::env::var("NODEFLOW_VOZ_MODELO").is_ok()
        {
            json!({ "url": url, "modelo": modelo, "por_entorno": true })
        } else {
            json!({ "url": prov.url, "modelo": modelo_mostrado, "por_entorno": false })
        },
        "tts": { "disponible": tts_disponible, "url": tts_url, "motor": "Kokoro (local)" },
        "pista": if configurada {
            "Clave presente. El token temporal se pide a /api/voz/jwt.".to_string()
        } else {
            format!("Falta la clave: {clave_campo} en el entorno, o \"{}\" en nodeflow.config.json.",
                prov.clave_config.first().copied().unwrap_or(""))
        }
    }))
}

/// `GET /api/voz/proveedores` — el catálogo de motores de voz y cuál está elegido.
async fn voz_proveedores(State(st): State<AppState>) -> impl IntoResponse {
    let elegido = crate::stt::seleccionado(&st.data_dir);
    Json(json!({
        "success": true,
        "elegido": elegido,
        "proveedores": crate::stt::catalogo_json(&st.data_dir),
    }))
}

/// `POST /api/voz/proveedor` `{ "id": "assemblyai" }` — cambia el motor y lo guarda en el config.
async fn voz_proveedor(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let id = body["id"].as_str().unwrap_or("").trim().to_string();
    if id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "success": false, "error": "Falta `id`. Disponibles: ".to_string() + &crate::stt::ids().join(", ") }),
            ),
        );
    }
    match crate::stt::guardar_seleccion(&st.data_dir, &id) {
        Ok(()) => {
            log::info!("voz: motor de reconocimiento → {id}");
            let prov = crate::stt::por_id(&id).unwrap();
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "elegido": prov.id,
                    "proveedor_etiqueta": prov.etiqueta,
                    "idiomas": prov.idiomas,
                    "nota": prov.nota,
                })),
            )
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": e })),
        ),
    }
}

/// `POST /api/ai/evaluar` — corre la planilla sobre los motores pedidos (por defecto, los locales).
///
/// Tarda minutos, así que **no bloquea**: arranca en segundo plano y el frontend consulta el estado.
/// `GET` devuelve la última planilla guardada y si hay una corrida en curso.
async fn ai_evaluar(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let pedidos: Vec<String> = body["motores"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let modelos = if pedidos.is_empty() {
        catalogo(&st)
            .await
            .iter()
            .filter(|m| {
                m.disponible
                    && m.donde == crate::motores::EN_TU_PLACA
                    && !crate::eval::es_multimodal(&m.modelo) // los de visión ocupan VRAM y no aportan acá
            })
            .map(|m| m.id.clone())
            .collect::<Vec<String>>()
    } else {
        pedidos
    };
    if modelos.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "No hay motores locales que evaluar." })),
        )
            .into_response();
    }
    if crate::eval::en_curso(&st.data_dir) {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "success": false, "error": "Ya hay una evaluación corriendo." })),
        )
            .into_response();
    }
    let st2 = st.clone();
    let cuantos = modelos.len();
    tokio::spawn(async move {
        crate::eval::correr(&st2, modelos).await;
    });
    log::info!("evaluación: arrancada sobre {cuantos} motor(es)");
    (
        StatusCode::OK,
        Json(json!({ "success": true, "corriendo": true, "motores": cuantos })),
    )
        .into_response()
}

async fn ai_evaluar_leer(State(st): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "success": true,
        "corriendo": crate::eval::en_curso(&st.data_dir),
        "tabla": crate::eval::leer(&st),
    }))
}

/// `GET /api/cerebro/herramientas` — el registro: lo que el cerebro puede usar más allá de las 22 del
/// lienzo. Lo consumen el panel y el servidor MCP (una sola fuente de verdad).
async fn cerebro_herramientas(State(st): State<AppState>) -> impl IntoResponse {
    let lista = crate::cerebro_tools::listar(&st.vault.raiz());
    Json(json!({
        "success": true,
        "herramientas": lista.iter().map(|h| json!({
            "nombre": h.nombre,
            "descripcion": h.descripcion,
            "parametros": h.parametros,
            "riesgo": h.riesgo,
        })).collect::<Vec<_>>(),
        "carpeta": crate::cerebro_tools::dir_registro(&st.vault.raiz()).to_string_lossy(),
    }))
}

/// `POST /api/cerebro/herramienta/proponer` — una herramienta nueva entra a la **cola de propuestas**: el
/// humano la ve (riesgo, descripción, qué archivos se escriben) y recién al aprobarla queda disponible.
async fn cerebro_herramienta_proponer(
    State(st): State<AppState>,
    Json(mut body): Json<Value>,
) -> impl IntoResponse {
    if body["origen"].is_null() {
        body["origen"] = json!("cerebro");
    }
    match st.vault.propose("herramienta", &body) {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": e })),
        ),
    }
}

/// `POST /api/cerebro/herramienta {nombre, parametros}` — la ejecuta. La jaula (nombre válido, carpeta
/// dentro del registro, tope de tiempo, salida acotada) vive en `cerebro_tools`.
async fn cerebro_herramienta_usar(
    State(st): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let nombre = body["nombre"].as_str().unwrap_or("").trim().to_string();
    if nombre.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "falta `nombre`" })),
        );
    }
    let parametros = body.get("parametros").cloned().unwrap_or_else(|| json!({}));
    let raiz = st.vault.raiz();
    // `ejecutar` es bloqueante (subproceso con tope): no puede frenar el runtime de axum.
    let para_correr = nombre.clone();
    let r = tokio::task::spawn_blocking(move || {
        crate::cerebro_tools::ejecutar(&raiz, &para_correr, &parametros)
    })
    .await;
    match r {
        Ok(Ok((salida, ms))) => {
            log::info!("herramientas: «{nombre}» ok en {ms} ms");
            (
                StatusCode::OK,
                Json(json!({ "success": true, "nombre": nombre, "salida": salida, "ms": ms })),
            )
        }
        Ok(Err(e)) => {
            log::warn!("herramientas: «{nombre}» falló: {e}");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "nombre": nombre, "error": e })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": format!("no pude ejecutar: {e}") })),
        ),
    }
}

/// `POST /api/cerebro/briefing {pedido}` — el estado del proyecto en un bloque corto, para anteponerlo al
/// turno del gateway. Sin esto, un turno en una sesión nueva arranca sin saber dónde estamos parados: el
/// contexto quedaba sólo en la memoria de Hermes. Acotado: no crece con el tamaño del lienzo.
async fn cerebro_briefing(
    State(st): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let pedido = body["pedido"].as_str().unwrap_or("").trim().to_string();
    let ctx = contexto_del_turno(&st, &pedido);
    let briefing = crate::cerebro::briefing_texto(&ctx);
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "briefing": briefing,
            "resumen": crate::cerebro::resumen_contexto(&ctx),
            "caracteres": briefing.chars().count(),
        })),
    )
}

/// `GET /api/cerebro/gateway` — estado del gateway propio. El panel usa `url` (ya trae el token) para
/// abrir el WebSocket.
async fn cerebro_gateway_estado(State(st): State<AppState>) -> impl IntoResponse {
    let g = st.gateway.estado();
    Json(json!({ "success": true, "gateway": g.clone(), "url": g["url"] }))
}

/// `POST /api/cerebro/gateway/arrancar` — levanta el `hermes serve` de la app. La espera del arranque va
/// en un hilo aparte: importa el agente, sus MCP y la config, y eso tarda. El panel consulta el estado
/// hasta ver `listo: true`.
async fn cerebro_gateway_arrancar(State(st): State<AppState>) -> impl IntoResponse {
    let exe = crate::voz::hermes_exe();
    // El directorio de trabajo del gateway es la bóveda: es donde vive el conocimiento del proyecto.
    let cwd = st.vault.raiz();
    match st.gateway.arrancar(&exe, &cwd) {
        Ok(()) => {
            let g = st.gateway.clone();
            std::thread::spawn(move || g.esperar_listo());
            log::info!(
                "cerebro: gateway propio levantándose en el puerto {}",
                st.gateway.puerto()
            );
            (
                StatusCode::OK,
                Json(
                    json!({ "success": true, "gateway": st.gateway.estado(), "url": st.gateway.url_ws() }),
                ),
            )
        }
        Err(e) => {
            log::warn!("cerebro: no pude levantar el gateway: {e}");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": e, "gateway": st.gateway.estado() })),
            )
        }
    }
}

/// `POST /api/cerebro/gateway/parar` — lo baja. El panel lo pide al cerrarse; el `Drop` del `Gateway`
/// cubre el caso de que la app muera antes.
async fn cerebro_gateway_parar(State(st): State<AppState>) -> impl IntoResponse {
    st.gateway.parar();
    log::info!("cerebro: gateway propio detenido");
    Json(json!({ "success": true, "gateway": st.gateway.estado() }))
}

/// Arma el contexto del turno **desde la bóveda**: la visión (nodo del Norte), el recuerdo dirigido
/// (BM25 sobre el pedido) y el foco del hilo. Todo acotado: **no crece con el tamaño del lienzo**.
fn contexto_del_turno(st: &AppState, pedido: &str) -> crate::cerebro::Contexto {
    let estado = st.vault.read_state().unwrap_or(json!({}));
    let nodos_arr = estado["nodes"].as_array();
    let titulo_de = |id: &str| {
        nodos_arr
            .and_then(|ns| ns.iter().find(|n| n["id"].as_str() == Some(id)))
            .and_then(|n| n["data"]["title"].as_str().map(String::from))
    };
    let norte = nodos_arr
        .and_then(|ns| {
            ns.iter()
                .find(|n| n["data"]["category"].as_str() == Some("NORTE"))
                .or_else(|| {
                    ns.iter()
                        .find(|n| n["data"]["es_nucleo"].as_bool() == Some(true))
                })
        })
        .and_then(|n| n["data"]["title"].as_str().map(String::from));
    let memoria = st
        .memoria
        .buscar(pedido, 6)
        .get("resultados")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    Some((
                        r["titulo"].as_str()?.to_string(),
                        r["ruta"].as_str()?.to_string(),
                    ))
                })
                .collect::<Vec<(String, String)>>()
        })
        .unwrap_or_default();
    // El foco guarda ids: se traducen a títulos para que el prompt se lea (el id queda igual, al lado).
    let foco = crate::dialogo::leer(&st.data_dir)["foco"]
        .as_array()
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_str())
                .map(|id| match titulo_de(id) {
                    Some(t) => format!("{t} [{id}]"),
                    None => id.to_string(),
                })
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();
    // El estado del proyecto, tal cual está en el lienzo: es lo que evita que un turno nuevo arranque
    // sin saber dónde estamos parados (y lo que el usuario pidió como «briefing»).
    let titulos_de = |categoria: &str, tope: usize| -> Vec<String> {
        nodos_arr
            .map(|ns| {
                ns.iter()
                    .filter(|n| n["data"]["category"].as_str() == Some(categoria))
                    .filter_map(|n| n["data"]["title"].as_str().map(String::from))
                    .take(tope)
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default()
    };
    let hitos_todos = titulos_de("HITO", usize::MAX);
    let hitos = hitos_todos
        .iter()
        .rev()
        .take(4)
        .cloned()
        .collect::<Vec<String>>();
    crate::cerebro::Contexto {
        norte,
        memoria,
        foco,
        nodos: nodos_arr.map(|n| n.len()).unwrap_or(0),
        aristas: estado["edges"].as_array().map(|e| e.len()).unwrap_or(0),
        pendientes: st.vault.count_pending(),
        camino: titulos_de("EJECUCIÓN", 8),
        abiertos: titulos_de("PENDIENTE", 12),
        hitos,
        mis_notas: mis_notas_del_cerebro(&st),
        arquitectura: arquitectura_para_el_turno(&st),
    }
}

/// El bloque **mapa** de la arquitectura generada (si existe): viaja en cada turno para que el cerebro
/// hable del código sin grepear el repo. Es la nota que deja `POST /api/cerebro/arquitectura/generar`.
fn arquitectura_para_el_turno(st: &AppState) -> String {
    let ruta = st
        .vault
        .raiz()
        .join(crate::cerebro::CARPETA)
        .join(crate::cerebro_arquitectura::NOTA);
    std::fs::read_to_string(ruta)
        .ok()
        .and_then(|t| crate::cerebro::mapa_de_la_nota(&t))
        .unwrap_or_default()
}

/// Las notas propias del cerebro (bitácora y planes) que ya existen en la bóveda: lo que pensé antes
/// queda a mano en el turno siguiente sin que nadie lo arrastre a mano. Los planes viven en
/// `cerebro/planes/`, así que se listan aparte (con su prefijo) para que el briefing los vea.
fn mis_notas_del_cerebro(st: &AppState) -> Vec<String> {
    let raiz = st.vault.raiz().join(crate::cerebro::CARPETA);
    let md = |p: &std::path::Path| p.extension().and_then(|x| x.to_str()) == Some("md");
    let mut nombres: Vec<String> = std::fs::read_dir(&raiz)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| md(&e.path()))
                .filter_map(|e| e.file_name().to_str().map(String::from))
                .filter(|n| n.starts_with("bitacora"))
                .collect()
        })
        .unwrap_or_default();
    let planes = raiz.join(crate::cerebro::PLANES);
    let mut de_planes: Vec<String> = std::fs::read_dir(&planes)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| md(&e.path()))
                .filter_map(|e| e.file_name().to_str().map(|n| format!("planes/{n}")))
                .collect()
        })
        .unwrap_or_default();
    de_planes.sort();
    de_planes.truncate(4);
    nombres.extend(de_planes);
    nombres.sort();
    nombres.truncate(6);
    nombres
}

/// `POST /api/cerebro/curaduria` — el **curador del lienzo** (Fase 5.5): corre las reglas mecánicas y
/// **propone** fusiones y podas, cada una con su motivo y su evidencia. No aplica nada: todo entra a la
/// cola de propuestas y se aprueba (en bloque, si se quiere) en «Cambios del agente».
/// Es idempotente: repetirla no duplica propuestas (el mismo resumen → ya propuesto).
async fn cerebro_curaduria(
    State(st): State<AppState>,
    Json(_body): Json<Value>,
) -> impl IntoResponse {
    let Some(estado) = st.vault.read_state() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "todavía no hay estado del lienzo en disco" })),
        );
    };
    let nodos: Vec<Value> = estado["nodes"].as_array().cloned().unwrap_or_default();
    let aristas: Vec<Value> = estado["edges"].as_array().cloned().unwrap_or_default();
    let hallazgos = crate::curador::curar(&nodos, &aristas);
    let mut creadas: Vec<Value> = Vec::new();
    let mut declarados: Vec<Value> = Vec::new();
    let mut errores: Vec<Value> = Vec::new();
    for h in &hallazgos {
        if h.propuesta["declarado"].as_bool().unwrap_or(false) {
            declarados.push(json!({ "clase": h.clase, "titulo": h.titulo, "motivo": h.motivo }));
            continue;
        }
        let tipo = h.propuesta["tipo"].as_str().unwrap_or("");
        let mut payload = h.propuesta["payload"].clone();
        // El motivo, la clase y la evidencia viajan con la propuesta: en la cola se lee *por qué*.
        payload["motivo"] = json!(h.motivo);
        payload["clase"] = json!(h.clase);
        payload["evidencia"] = json!(h.evidencia);
        payload["confianza"] = json!(h.confianza);
        payload["origen"] = json!("curador");
        match st.vault.propose(tipo, &payload) {
            Ok(v) => creadas.push(json!({
                "clase": h.clase,
                "clase_legible": h.clase_legible(),
                "titulo": h.titulo,
                "motivo": h.motivo,
                "confianza": h.confianza,
                "id_pendiente": v["id_pendiente"],
                "ya_estaba": v["accion"] == "ya_propuesto",
            })),
            Err(e) => errores.push(json!({ "titulo": h.titulo, "error": e })),
        }
    }
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "resumen": crate::curador::resumen(&hallazgos),
            "propuestas": creadas,
            "declarados": declarados,
            "errores": errores,
            "lienzo": { "nodos": nodos.len(), "aristas": aristas.len() },
            "pendientes": st.vault.count_pending(),
        })),
    )
}

/// `POST /api/cerebro/arquitectura/generar` — inventaría el árbol real del proyecto y deja la nota
/// `cerebro/arquitectura.md` en la bóveda. Desde ahí, el bloque *mapa* viaja en cada turno (Fase 5.4).
///
/// La raíz del repo se resuelve así: el body (`{"repo": "..."}`) → `cerebro.repo` de la config →
/// `NODEFLOW_REPO` → la ubicación del ejecutable (subiendo por los ancestros hasta `src-tauri/src`).
async fn cerebro_arquitectura_generar(
    State(st): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let pista = body["repo"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| st.cerebro.repo.clone());
    let repo = if pista.is_empty() {
        let exe = std::env::current_exe().unwrap_or_default();
        match crate::cerebro_arquitectura::buscar_repo(&exe) {
            Some(r) => r,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "success": false,
                        "error": "no encuentro el repo de NodeFlow: poné \"cerebro\": {\"repo\": \"C:/ruta/al/repo\"} en nodeflow.config.json (o NODEFLOW_REPO)"
                    })),
                )
            }
        }
    } else {
        crate::cerebro_arquitectura::buscar_repo(std::path::Path::new(&pista))
            .unwrap_or_else(|| std::path::PathBuf::from(&pista))
    };
    let inv = crate::cerebro_arquitectura::inventariar(&repo);
    let sello = sello_local(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
        st.cerebro.offset_h,
    );
    let boveda = st.vault.raiz().to_string_lossy().to_string();
    let doc = crate::cerebro_arquitectura::redactar(
        &inv,
        &sello,
        &boveda,
        &repo.to_string_lossy(),
        env!("CARGO_PKG_VERSION"),
    );
    let rel = format!(
        "{}/{}",
        crate::cerebro::CARPETA,
        crate::cerebro_arquitectura::NOTA
    );
    match st.vault.escribir_nota(&rel, &doc) {
        Ok(p) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "ruta": p.to_string_lossy(),
                "sello": sello,
                "repo": repo.to_string_lossy(),
                "modulos_rust": inv.rust.len(),
                "lineas_rust": inv.lineas_rust(),
                "archivos_front": inv.front.len(),
                "rutas_http": inv.rutas.len(),
                "tools_mcp": inv.tools_mcp.len(),
                "chars": doc.chars().count(),
                "faltantes": inv.faltantes,
            })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": e })),
        ),
    }
}

/// `GET /api/cerebro/espacio` — mi espacio: la bitácora y los planes, más el contexto de la bóveda.
/// Es lo que el panel muestra como «Mi espacio» (y lo que el agente lee antes de tocar un plan).
async fn cerebro_espacio(State(st): State<AppState>) -> impl IntoResponse {
    let raiz = st.vault.raiz().join(crate::cerebro::CARPETA);
    let bitacora = std::fs::read_to_string(raiz.join(crate::cerebro::BITACORA)).unwrap_or_default();
    let planes_dir = raiz.join(crate::cerebro::PLANES);
    let mut planes: Vec<Value> = std::fs::read_dir(&planes_dir)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
                .filter_map(|e| {
                    let p = e.path();
                    let nombre = p.file_name()?.to_str()?.to_string();
                    let texto = std::fs::read_to_string(&p).ok()?;
                    let modificado = e
                        .metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    Some(json!({
                        "nombre": nombre,
                        "titulo": texto.lines().find(|l| l.starts_with("# ")).map(|l| l[2..].to_string()).unwrap_or_else(|| nombre.clone()),
                        "chars": texto.chars().count(),
                        // El texto se manda acotado: el panel lo muestra desplegable y el turno no se infla.
                        "texto": texto.chars().take(3000).collect::<String>(),
                        "modificado_ms": modificado,
                    }))
                })
                .collect()
        })
        .unwrap_or_default();
    planes.sort_by(|a, b| {
        b["modificado_ms"]
            .as_u64()
            .cmp(&a["modificado_ms"].as_u64())
    });
    // La arquitectura generada (Fase 5.4): el panel muestra cuándo se inventarió, sin abrir el archivo.
    let arq =
        std::fs::read_to_string(raiz.join(crate::cerebro_arquitectura::NOTA)).unwrap_or_default();
    let sello_arq = arq
        .lines()
        .find(|l| l.starts_with("generado:"))
        .map(|l| {
            l.trim_start_matches("generado:")
                .trim()
                .trim_matches('"')
                .to_string()
        })
        .unwrap_or_default();
    Json(json!({
        "success": true,
        "arquitectura": {
            "existe": !arq.is_empty(),
            "chars": arq.chars().count(),
            "sello": sello_arq,
        },
        "bitacora": {
            "texto": bitacora,
            "chars": bitacora.chars().count(),
            "existe": !bitacora.is_empty(),
        },
        "planes": planes,
        "turnos": crate::cerebro::contar_notas(&st.vault.raiz()),
        "carpeta": raiz.to_string_lossy(),
    }))
}

/// `POST /api/cerebro/espacio/nota` — escribe en mi espacio: una entrada de bitácora (append) o un plan
/// (reemplaza). Son **notas de la bóveda**, no nodos: no pasan por la cola de propuestas (lo acordado:
/// mis notas se escriben libres; el lienzo y las herramientas sí van a la cola).
async fn cerebro_espacio_nota(
    State(st): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let tipo = body["tipo"].as_str().unwrap_or("bitacora");
    let texto = body["contenido"].as_str().unwrap_or("").trim().to_string();
    if texto.chars().count() < 2 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "falta el contenido de la nota" })),
        );
    }
    let quien = body["quien"]
        .as_str()
        .unwrap_or("cerebro")
        .trim()
        .to_string();
    let sello = sello_local(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
        st.cerebro.offset_h,
    );
    let raiz = st.vault.raiz();
    match tipo {
        "plan" => {
            let nombre = body["nombre"].as_str().unwrap_or("").trim();
            let slug = match crate::cerebro::slug_plan(nombre) {
                Ok(s) => s,
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "success": false, "error": e })),
                    );
                }
            };
            let estado = body["estado"].as_str().unwrap_or("borrador").trim();
            let titulo = nombre.replace('"', "'");
            let contenido = format!(
                "---\ntitulo: \"{titulo}\"\ntipo: plan\nestado: {estado}\nactualizado: \"{sello}\"\n---\n\n# {nombre}\n\n{texto}\n"
            );
            let rel = format!(
                "{}/{}/{}.md",
                crate::cerebro::CARPETA,
                crate::cerebro::PLANES,
                slug
            );
            match st.vault.escribir_nota(&rel, &contenido) {
                Ok(p) => (
                    StatusCode::OK,
                    Json(
                        json!({ "success": true, "accion": "plan_escrito", "ruta": p.to_string_lossy(), "slug": slug }),
                    ),
                ),
                Err(e) => (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "success": false, "error": e })),
                ),
            }
        }
        _ => {
            let rel = format!("{}/{}", crate::cerebro::CARPETA, crate::cerebro::BITACORA);
            let previo = std::fs::read_to_string(raiz.join(&rel)).unwrap_or_default();
            let base = if previo.trim().is_empty() {
                crate::cerebro::encabezado_bitacora()
            } else {
                previo
            };
            let nuevo = format!(
                "{base}{}",
                crate::cerebro::entrada_bitacora(&sello, &quien, &texto)
            );
            match st.vault.escribir_nota(&rel, &nuevo) {
                Ok(p) => (
                    StatusCode::OK,
                    Json(
                        json!({ "success": true, "accion": "bitacora_anotada", "ruta": p.to_string_lossy() }),
                    ),
                ),
                Err(e) => (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "success": false, "error": e })),
                ),
            }
        }
    }
}
/// `POST /api/ai/delegar` — el **motor profundo** de NodeFlow.
///
/// Cuando el pedido necesita lo que el modelo local no tiene (buscar en la web, leer un repo,
/// razonar largo), el backend corre una pasada completa de Hermes con SUS herramientas y devuelve
/// el texto. Es la operación cara: el plan de voz la limita a una por pedido.
async fn delegar(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    // `POST` **arranca** la investigación y vuelve enseguida; `GET` informa si sigue y devuelve el
    // resultado. Así el panel no queda esperando los minutos que tarda el motor profundo.
    let pedido = body["pedido"].as_str().unwrap_or("").trim().to_string();
    if pedido.chars().count() < 4 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "Falta el pedido para delegar." })),
        )
            .into_response();
    }
    if crate::voz::delegacion_en_curso(&st.data_dir) {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "success": false, "error": "Ya hay una investigación en curso." })),
        )
            .into_response();
    }
    // Bandera de "en curso": evita dos investigaciones a la vez y le dice al panel que espere.
    let _ = std::fs::write(st.data_dir.join("delegacion.corriendo"), "1");
    let st2 = st.clone();
    let pedido2 = pedido.clone();
    tokio::spawn(async move {
        let t0 = std::time::Instant::now();
        let salida = tokio::task::spawn_blocking({
            let exe = crate::voz::hermes_exe();
            let cfg = st2.cerebro.clone();
            let st3 = st2.clone();
            let pedido3 = pedido2.clone();
            move || {
                // Fase 2: el contexto lo arma la app **desde la bóveda** (visión + recuerdo dirigido +
                // puntero a las herramientas). Ya no se le manda el lienzo masticado, que crecía con el mapa.
                let ctx = contexto_del_turno(&st3, &pedido3);
                let prompt = crate::cerebro::prompt_turno(&pedido3, &ctx);
                let args = crate::cerebro::argv(&prompt, &cfg);
                let r =
                    crate::cerebro::correr(&exe, &args, std::time::Duration::from_secs(cfg.tope_s));
                (r, crate::cerebro::resumen_contexto(&ctx))
            }
        })
        .await;
        let (salida, contexto_usado) = match salida {
            Ok((r, resumen)) => (r, resumen),
            Err(e) => (
                Err(format!("no pude correr el motor profundo: {e}")),
                String::new(),
            ),
        };
        // El turno corre en una **sesión nombrada** de Hermes: además de la respuesta queda la memoria
        // (el turno siguiente recuerda éste) y, si está activado, la nota episódica en la bóveda.
        let (ok, texto, sesion_vista) = match salida {
            Ok(bruto) => {
                let (limpio, id) = crate::cerebro::limpiar_salida(&bruto);
                (true, limpio, id)
            }
            Err(e) => (false, e, None),
        };
        let ms = t0.elapsed().as_millis() as u64;
        let _ = crate::voz::guardar_delegacion(
            &st2.data_dir,
            &pedido2,
            ok,
            &texto,
            ms,
            &contexto_usado,
        );
        if st2.cerebro.notas {
            let utc = now_iso();
            let local = sello_local((now_ms() / 1000) as i64, st2.cerebro.offset_h);
            let nota = crate::cerebro::nota_markdown(
                &pedido2,
                &texto,
                &st2.cerebro.sesion,
                ok,
                ms,
                &local,
                &utc,
            );
            match st2
                .vault
                .escribir_nota(&crate::cerebro::nombre_nota(&local), &nota)
            {
                Ok(p) => log::info!("cerebro: turno guardado en {}", p.display()),
                Err(e) => log::warn!("cerebro: no pude escribir la nota del turno: {e}"),
            }
        }
        log::info!(
            "motor profundo: {} · {} caracteres · sesión {}",
            if ok { "listo" } else { "falló" },
            texto.chars().count(),
            sesion_vista
                .as_deref()
                .unwrap_or(st2.cerebro.sesion.as_str())
        );
    });
    (
        StatusCode::OK,
        Json(json!({ "success": true, "corriendo": true, "pedido": pedido })),
    )
        .into_response()
}

/// `POST /api/ai/investigar` — arranca la **investigación por fases** (🌱⚔️🧪🚀) y vuelve enseguida.
async fn ai_investigar(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let pedido = body["pedido"].as_str().unwrap_or("").trim().to_string();
    if pedido.chars().count() < 4 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "Falta el tema a investigar." })),
        )
            .into_response();
    }
    match crate::investigacion::iniciar(&st, pedido).await {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "success": true, "corriendo": true })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(json!({ "success": false, "error": e })),
        )
            .into_response(),
    }
}

/// `GET /api/ai/investigar` — las fases, paso por paso (el panel lo consulta mientras crece).
async fn ai_investigar_estado(State(st): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "success": true,
        "corriendo": crate::investigacion::en_curso(&st.data_dir),
        "investigacion": crate::investigacion::leer(&st.data_dir),
        "fases": crate::investigacion::FASES
            .iter()
            .map(|(id, titulo, emoji)| json!({ "id": id, "titulo": titulo, "emoji": emoji }))
            .collect::<Vec<_>>(),
    }))
}

/// `GET /api/ai/delegar` — ¿sigue investigando? ¿qué respondió?
async fn delegar_estado(State(st): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "success": true,
        "corriendo": crate::voz::delegacion_en_curso(&st.data_dir),
        // El resultado trae también `contexto`: qué visión, recuerdos y foco se le mandaron al agente.
        // El panel lo muestra — el prompt del cerebro no es una caja negra.
        "resultado": crate::voz::leer_delegacion(&st.data_dir),
        "cerebro": {
            "sesion": st.cerebro.sesion,
            "notas": crate::cerebro::contar_notas(&st.vault.raiz()),
        },
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// El agente propio (etapas 1+3 del plan «sin Hermes»): bucle con herramientas de repo
// ─────────────────────────────────────────────────────────────────────────────

/// Estado del turno en disco, y su bandera. La bandera guarda el **instante**: una corrida muerta
/// (kill, apagón) no puede dejar al agente bloqueado para siempre — misma lección que
/// `investigacion::en_curso` y `IA_TURNO_TTL_S`.
const AGENTE_JSON: &str = "agente.json";
/// El hilo del agente: los últimos turnos, para que un pedido no arranque en frío. Es la lección medida
/// del 17/09 —el camino de voz ya tenía hilo (`dialogo.rs`) y el del agente no, así que cada pedido volvía
/// a explorar lo mismo—. Vive con el conocimiento del usuario (`<bóveda>/.nodeflow/`) y **expira solo**:
/// un hilo de ayer no debería condicionar el de hoy.
const AGENTE_HILO: &str = "agente-hilo.json";
const HILO_TURNOS: usize = 6;
const HILO_HORAS: i64 = 3;
const AGENTE_CORRIENDO: &str = "agente.corriendo";
const AGENTE_TTL_S: u64 = 900;

fn agente_en_curso(data_dir: &std::path::Path) -> bool {
    let f = data_dir.join(AGENTE_CORRIENDO);
    let Ok(txt) = std::fs::read_to_string(&f) else {
        return false;
    };
    let sello: u64 = txt.trim().parse().unwrap_or(0);
    let ahora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if sello > 0 && ahora.saturating_sub(sello) > AGENTE_TTL_S {
        log::warn!("agente: bandera vencida ({sello}), la limpio");
        let _ = std::fs::remove_file(&f);
        return false;
    }
    true
}

/// Baja la bandera y guarda el resultado del turno (con **todos** los pasos: es la auditoría).
fn guardar_agente(data_dir: &std::path::Path, v: Value) -> Result<(), String> {
    std::fs::write(
        data_dir.join(AGENTE_JSON),
        serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(data_dir.join(AGENTE_CORRIENDO));
    Ok(())
}

/// Llama al motor **con herramientas** (formato OpenAI) y devuelve el JSON crudo: el bucle necesita
/// `choices[0].message.tool_calls`, no un texto aplanado. El `temperature` se omite a propósito:
/// los razonadores rechazan cualquier valor que no sea el default.
async fn chat_con_tools(
    st: &AppState,
    base: &str,
    clave: &str,
    modelo: &str,
    messages: Vec<Value>,
    tools: Vec<Value>,
) -> Result<Value, String> {
    let mut body = json!({ "model": modelo, "messages": messages, "tool_choice": "auto" });
    if !tools.is_empty() {
        body["tools"] = json!(tools);
    }
    let url = format!("{}/chat/completions", base.trim_end_matches('/'));
    let resp = st
        .http
        .post(&url)
        .bearer_auth(clave)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("error de red hacia {url}: {e}"))?;
    if !resp.status().is_success() {
        let code = resp.status();
        let cuerpo = resp.text().await.unwrap_or_default();
        return Err(format!(
            "{modelo}: HTTP {code} — {}",
            cuerpo.chars().take(300).collect::<String>()
        ));
    }
    resp.json::<Value>()
        .await
        .map_err(|e| format!("respuesta ilegible de {modelo}: {e}"))
}

/// Motor del bucle: **tiene** que ser compatible con OpenAI (se le mandan `tools`) y tener clave. Si el
/// elegido no sirve, cae al primer proveedor compatible declarado antes que fallar.
async fn motor_para_agente(
    st: &AppState,
    pedido: Option<&str>,
) -> Result<crate::motores::Motor, String> {
    let cat = catalogo(st).await;
    let sel = pedido
        .map(|s| s.to_string())
        .or_else(|| crate::motores::seleccionado(&st.data_dir));
    for m in crate::motores::plan(&cat, sel.as_deref(), None) {
        if !matches!(m.proveedor.as_str(), "openai" | "ollama") || m.base_url.is_none() {
            continue;
        }
        if clave_del_motor(st, &m).is_none() {
            continue;
        }
        return Ok(m);
    }
    Err("ningún motor compatible con OpenAI tiene clave (agregá una, o usá openai:deepseek)".into())
}

/// Lee el hilo del agente, descartando lo que ya venció. Tolerante a propósito: un archivo corrupto o
/// ausente no puede impedir un turno (el hilo es contexto, no un requisito).
fn leer_hilo(dir: &std::path::Path) -> Vec<Value> {
    let Ok(t) = std::fs::read_to_string(dir.join(AGENTE_HILO)) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&t) else {
        return Vec::new();
    };
    let ahora = (now_ms() / 1000) as i64;
    v["turnos"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|t| ahora - t["cuando"].as_i64().unwrap_or(0) <= HILO_HORAS * 3600)
        .collect()
}

/// El bloque que se le antepone al sistema: qué ya se averiguó, para no volver a mirarlo de cero.
fn bloque_hilo(entradas: &[Value]) -> String {
    if entradas.is_empty() {
        return String::new();
    }
    let ahora = (now_ms() / 1000) as i64;
    let mut p = String::from(
        "\n\nTurnos anteriores de este agente (lo que YA averiguó: no lo vuelvas a mirar de cero):\n",
    );
    for e in entradas {
        let mins = ((ahora - e["cuando"].as_i64().unwrap_or(0)) / 60).max(0);
        let rec =
            |k: &str, n: usize| -> String { e[k].as_str().unwrap_or("").chars().take(n).collect() };
        p.push_str(&format!(
            "- hace {mins} min · «{}» → {}, {} pasos, {} recortes · {}\n",
            rec("pedido", 160),
            if e["ok"].as_bool().unwrap_or(false) {
                "cerró"
            } else {
                "no cerró"
            },
            e["pasos"].as_u64().unwrap_or(0),
            e["podas"].as_u64().unwrap_or(0),
            rec("plan", 500)
        ));
    }
    p
}

/// Guarda el turno en el hilo: pedido, desenlace, el plan con el que cerró y si dejó algo propuesto.
fn guardar_hilo(dir: &std::path::Path, entrada: Value) {
    let mut turnos = leer_hilo(dir);
    turnos.push(entrada);
    let sobra = turnos.len().saturating_sub(HILO_TURNOS);
    if sobra > 0 {
        turnos.drain(0..sobra);
    }
    if let Err(e) = std::fs::create_dir_all(dir) {
        log::warn!("agente: hilo: {e}");
        return;
    }
    let cuerpo = json!({ "ver": 1, "turnos": turnos });
    if let Err(e) = std::fs::write(dir.join(AGENTE_HILO), cuerpo.to_string()) {
        log::warn!("agente: no pude guardar el hilo: {e}");
    }
}

/// `POST /api/agente/turno` — arranca **el bucle de agente propio** (sin Hermes) y vuelve enseguida:
/// el panel consulta `GET /api/agente/estado` mientras el turno crece. Cuerpo: `{pedido, motor?, repo?}`.
async fn agente_turno(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let pedido = body["pedido"].as_str().unwrap_or("").trim().to_string();
    if pedido.chars().count() < 4 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "Falta el pedido del turno." })),
        )
            .into_response();
    }
    if agente_en_curso(&st.data_dir) {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "success": false, "error": "Ya hay un turno del agente en curso." })),
        )
            .into_response();
    }
    // La raíz del repo: el cuerpo (útil para probar) → `cerebro.repo` del config → la ubicación del exe.
    let repo = body["repo"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| Some(st.cerebro.repo.clone()).filter(|s| !s.trim().is_empty()))
        .map(std::path::PathBuf::from);
    let Some(repo) = repo.filter(|p| p.is_dir()) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "No sé dónde está el repo. Poné `cerebro.repo` en nodeflow.config.json (o mandá `repo`)."
            })),
        )
            .into_response();
    };

    let motor = match motor_para_agente(&st, body["motor"].as_str()).await {
        Ok(m) => m,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": e })),
            )
                .into_response()
        }
    };
    let clave = clave_del_motor(&st, &motor).unwrap_or_default();
    let base = motor
        .base_url
        .clone()
        .unwrap_or_else(|| "http://localhost:11434/v1".to_string());

    let sello = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = std::fs::write(st.data_dir.join(AGENTE_CORRIENDO), sello.to_string());

    let cfg_crudo: Option<Value> =
        std::fs::read_to_string(st.data_dir.join("nodeflow.config.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok());
    let cfg = crate::agente::Config::desde(
        cfg_crudo.as_ref().and_then(|c| c.get("agente")),
        repo.clone(),
        st.data_dir.clone(),
    );
    // Tarifas declaradas por el usuario (config) — incluye, si está, el precio de la entrada cacheada.
    let tarifas = crate::costo::Tarifas::desde_config(cfg_crudo.as_ref());
    // El hilo de turnos anteriores: se lee antes de arrancar y viaja en el sistema del turno.
    let hilo_dir = st.vault.raiz().join(".nodeflow");
    let hilo_previo = leer_hilo(&hilo_dir);
    let n_hilo = hilo_previo.len();
    let hilo = bloque_hilo(&hilo_previo);
    // Lo que la respuesta necesita, capturado **antes** de que el `spawn` se lleve `cfg` y `motor`.
    let (res_motor, res_modelo, res_repo, res_tope) = (
        motor.id.clone(),
        motor.modelo.clone(),
        cfg.repo.clone(),
        cfg.tope_pasos,
    );
    let st2 = st.clone();
    let pedido2 = pedido.clone();
    let hilo2 = hilo.clone();
    let hilo_dir2 = hilo_dir.clone();
    tokio::spawn(async move {
        let t0 = std::time::Instant::now();
        // El briefing se arma **desde la bóveda** (visión + camino + recuerdo dirigido + el mapa de la
        // arquitectura): es lo mismo que recibe el motor profundo, así el agente no arranca a ciegas.
        let ctx = {
            let st3 = st2.clone();
            let p = pedido2.clone();
            tokio::task::spawn_blocking(move || contexto_del_turno(&st3, &p))
                .await
                .unwrap_or_default()
        };
        let sistema = format!(
            "{}\n\n{}{}",
            crate::agente::sistema(&cfg.repo),
            crate::cerebro::briefing_texto(&ctx),
            hilo2
        );
        // El consumo del turno se acumula desde afuera: el bucle no sabe de costos, el backend sí.
        // Se guardan las tres cifras que reporta el proveedor —prompt, completion y **cache_hit**—:
        // sin la tercera, un turno sólo se puede reportar en tokens brutos y el descuento por caché de
        // prefijo (que es como DeepSeek cobra barato el contexto repetido) queda invisible.
        // `llamadas` guarda el desglose por vuelta: el prompt crece en cada vuelta y ahí está el costo.
        let consumo = std::sync::Arc::new(std::sync::Mutex::new(crate::costo::Consumo::default()));
        let llamadas = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
        let consumo2 = consumo.clone();
        let llamadas2 = llamadas.clone();
        let (st4, base2, clave2, modelo2) = (
            st2.clone(),
            base.clone(),
            clave.clone(),
            motor.modelo.clone(),
        );
        let llamar = move |msgs: Vec<Value>, tools: Vec<Value>| {
            let (st5, base, clave, modelo, acc, reg) = (
                st4.clone(),
                base2.clone(),
                clave2.clone(),
                modelo2.clone(),
                consumo2.clone(),
                llamadas2.clone(),
            );
            async move {
                let t0v = std::time::Instant::now();
                let r = chat_con_tools(&st5, &base, &clave, &modelo, msgs, tools).await?;
                let ms_v = t0v.elapsed().as_millis() as u64;
                let mut c = crate::costo::consumo_openai(&r).unwrap_or_default();
                if c.total() == 0 {
                    // El proveedor no separó prompt/salida: lo que reportó entra entero, sin inventar.
                    if let Some(t) = r["usage"]["total_tokens"].as_u64() {
                        c.prompt = t;
                    }
                }
                if let Ok(mut g) = acc.lock() {
                    g.sumar(&c);
                }
                if let Ok(mut v) = reg.lock() {
                    if v.len() < 128 {
                        v.push(json!({
                            "prompt": c.prompt,
                            "completion": c.completion,
                            "cache_hit": c.cache_hit,
                            "ms": ms_v,
                        }));
                    }
                }
                Ok(r)
            }
        };
        let turno = crate::agente::correr(&pedido2, &sistema, &cfg, llamar).await;
        let ms = t0.elapsed().as_millis() as u64;
        // El consumo del turno, con la parte que el proveedor sirvió desde su caché de prefijo.
        let consumo_final = consumo.lock().map(|g| g.clone()).unwrap_or_default();
        let llamadas_final: Vec<Value> = llamadas.lock().map(|g| g.clone()).unwrap_or_default();
        let (costo, costo_sin_cache) = tarifas
            .costo_con_cache(&motor.modelo, &motor.id, &consumo_final)
            .map(|(con, sin)| (Some(con), Some(sin)))
            .unwrap_or((None, None));
        let ahorro_cache = match (costo, costo_sin_cache) {
            (Some(a), Some(b)) => Some(((b - a) * 1_000_000.0).round() / 1_000_000.0),
            _ => None,
        };
        // El desenlace trae los pasos en todos los casos: un turno cortado sigue siendo auditable.
        let (ok, texto, error_turno) = (turno.ok, turno.texto.clone(), turno.error.clone());
        let pasos: Vec<Value> = turno
            .pasos
            .iter()
            .map(|p| {
                json!({
                    "herramienta": p.herramienta,
                    "argumentos": p.argumentos,
                    "ok": p.ok,
                    "ms": p.ms,
                    "salida": p.salida.chars().take(1_200).collect::<String>(),
                })
            })
            .collect();
        let resultado = json!({
            "pedido": pedido2,
            "ok": ok,
            "respuesta": texto,
            "pasos": pasos,
            "herramientas_usadas": pasos.len(),
            "ms": ms,
            "motor": motor.id,
            "modelo": motor.modelo,
            "tokens": consumo_final.total(),
            "consumo": consumo_final.json(),
            "presupuesto_entrada": cfg.presupuesto_entrada,
            "entrada_estimada": turno.entrada_estimada,
            "podas": turno.podas,
            "llamadas": llamadas_final,
            "costo_usd": costo,
            "costo_sin_cache_usd": costo_sin_cache,
            "cache_ahorro_usd": ahorro_cache,
            "error": error_turno,
            "contexto": crate::cerebro::resumen_contexto(&ctx),
            "cuando": sello,
        });
        if let Err(e) = guardar_agente(&st2.data_dir, resultado.clone()) {
            log::warn!("agente: no pude guardar el resultado: {e}");
            let _ = std::fs::remove_file(st2.data_dir.join(AGENTE_CORRIENDO));
        }
        // El turno entra al hilo: el próximo arranca sabiendo qué se averiguó en éste.
        guardar_hilo(
            &hilo_dir2,
            json!({
                "cuando": sello,
                "pedido": pedido2,
                "ok": ok,
                "pasos": pasos.len(),
                "podas": turno.podas,
                "propuso": pasos.iter().any(|p| p["herramienta"] == "proponer_parche"),
                "plan": if ok { texto.clone() } else { String::new() },
            }),
        );
        // Nota episódica: la memoria del proyecto vive en la bóveda, no en el proceso.
        if st2.cerebro.notas {
            let utc = now_iso();
            let local = sello_local((now_ms() / 1000) as i64, st2.cerebro.offset_h);
            let nota = crate::cerebro::nota_markdown(
                &resultado["pedido"].as_str().unwrap_or(""),
                &texto,
                &format!("agente:{}", resultado["motor"].as_str().unwrap_or("")),
                ok,
                ms,
                &local,
                &utc,
            );
            if let Err(e) = st2
                .vault
                .escribir_nota(&crate::cerebro::nombre_nota(&local), &nota)
            {
                log::warn!("agente: no pude escribir la nota del turno: {e}");
            }
        }
        log::info!(
            "agente: {} · {} pasos ({} recortes) · {} ms · {} tokens ({} in / {} out · {} de caché) · {} llamadas · costo {} · motor {}",
            if ok { "listo" } else { "falló" },
            resultado["herramientas_usadas"],
            turno.podas,
            ms,
            consumo_final.total(),
            consumo_final.prompt,
            consumo_final.completion,
            consumo_final.cache_hit,
            llamadas_final.len(),
            match costo {
                Some(c) => format!("US$ {c:.6}"),
                None => "sin tarifa declarada".to_string(),
            },
            resultado["motor"]
        );
    });

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "corriendo": true,
            "pedido": pedido,
            "motor": res_motor,
            "modelo": res_modelo,
            "repo": res_repo,
            "tope_pasos": res_tope,
            "hilo_turnos": n_hilo
        })),
    )
        .into_response()
}

/// `GET /api/agente/estado` — ¿sigue el turno? ¿qué herramientas usó y qué contestó?
async fn agente_estado(State(st): State<AppState>) -> impl IntoResponse {
    let resultado = std::fs::read_to_string(st.data_dir.join(AGENTE_JSON))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok());
    Json(json!({
        "success": true,
        "corriendo": agente_en_curso(&st.data_dir),
        "resultado": resultado,
        "repo": st.cerebro.repo,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Etapa 4 — el parche: se propone, el humano aprueba, se aplica y se verifica
// ─────────────────────────────────────────────────────────────────────────────

/// El repo para los parches: el mismo que usa el agente (`cerebro.repo` del config).
fn repo_de_parches(st: &AppState) -> Result<std::path::PathBuf, String> {
    let p = std::path::PathBuf::from(st.cerebro.repo.trim());
    if p.is_dir() {
        Ok(p)
    } else {
        Err("no sé dónde está el repo: poné `cerebro.repo` en nodeflow.config.json".into())
    }
}

/// `GET /api/agente/parche` — la propuesta pendiente (o la última aplicada) con su verificación.
async fn agente_parche(State(st): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "success": true,
        "propuesta": crate::parche::leer(&st.data_dir),
        "repo": st.cerebro.repo,
        "topes": { "archivos": crate::parche::MAX_ARCHIVOS, "ediciones": crate::parche::MAX_EDICIONES },
    }))
}

/// `POST /api/agente/parche/aprobar` — **aplica** lo que estaba en la cola y lanza los *gates* en
/// segundo plano (aplicar es instantáneo; verificar tarda). La respuesta dice qué gates van a correr.
async fn agente_parche_aprobar(State(st): State<AppState>) -> impl IntoResponse {
    let Some(mut v) = crate::parche::leer(&st.data_dir) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "No hay ninguna propuesta de parche." })),
        )
            .into_response();
    };
    if v["estado"].as_str() != Some("pendiente") {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "error": format!("La propuesta está en estado «{}»: no hay nada que aprobar.", v["estado"].as_str().unwrap_or("?")),
            })),
        )
            .into_response();
    }
    let eds = match crate::parche::eds_de_json(&v["ediciones"]) {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": e })),
            )
                .into_response()
        }
    };
    let repo = match repo_de_parches(&st) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": e })),
            )
                .into_response()
        }
    };
    match crate::parche::aplicar(&repo, &eds) {
        Ok(apl) => {
            let gates = crate::parche::gates(&apl.archivos);
            v["estado"] = json!("aplicado");
            v["aplicado"] = json!({
                "archivos": apl.archivos,
                "ediciones": apl.ediciones,
                "mas": apl.mas,
                "menos": apl.menos,
                "cuando": now_ms(),
            });
            v["verificacion"] = json!([{ "comando": "(corriendo)", "ok": null, "salida": "" }]);
            if let Err(e) = crate::parche::escribir(&st.data_dir, &v) {
                log::warn!("parche: no pude guardar el estado tras aplicar: {e}");
            }
            // La verificación no se sostiene en la petición: se corre aparte y el panel la consulta.
            let st2 = st.clone();
            let archivos = apl.archivos.clone();
            let repo2 = repo.clone();
            tokio::spawn(async move {
                let tope = crate::agente::TOPE_CMD_S;
                let verificacion = tokio::task::spawn_blocking(move || {
                    crate::parche::verificar(&repo2, &archivos, tope)
                })
                .await
                .unwrap_or_else(|e| {
                    vec![json!({ "comando": "verificación", "ok": false, "salida": e.to_string() })]
                });
                if let Some(mut actual) = crate::parche::leer(&st2.data_dir) {
                    let ok = verificacion
                        .iter()
                        .all(|g| g["ok"].as_bool().unwrap_or(false));
                    actual["verificacion"] = json!(verificacion);
                    actual["verificado_ok"] = json!(ok);
                    if let Err(e) = crate::parche::escribir(&st2.data_dir, &actual) {
                        log::warn!("parche: no pude guardar la verificación: {e}");
                    }
                    log::info!(
                        "parche: verificación {} — {} gate(s)",
                        if ok { "verde" } else { "roja" },
                        verificacion.len()
                    );
                }
            });
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "aplicado": { "archivos": apl.archivos, "ediciones": apl.ediciones, "mas": apl.mas, "menos": apl.menos },
                    "gates": gates,
                })),
            )
                .into_response()
        }
        Err(e) => {
            // No se escribió nada (o se restauró): el estado queda con el motivo.
            v["estado"] = json!("error");
            v["error"] = json!(e);
            let _ = crate::parche::escribir(&st.data_dir, &v);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": e })),
            )
                .into_response()
        }
    }
}

/// `POST /api/agente/parche/rechazar` — se descarta sin tocar el disco.
async fn agente_parche_rechazar(State(st): State<AppState>) -> impl IntoResponse {
    let Some(mut v) = crate::parche::leer(&st.data_dir) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "No hay propuesta." })),
        )
            .into_response();
    };
    v["estado"] = json!("rechazado");
    v["cuando_rechazo"] = json!(now_ms());
    match crate::parche::escribir(&st.data_dir, &v) {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "success": true, "estado": "rechazado" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": e })),
        )
            .into_response(),
    }
}

/// `POST /api/agente/parche/revertir` — aplica la **inversa** del parche aplicado (el repo vuelve solo).
async fn agente_parche_revertir(State(st): State<AppState>) -> impl IntoResponse {
    let Some(v) = crate::parche::leer(&st.data_dir) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "No hay propuesta." })),
        )
            .into_response();
    };
    if !matches!(v["estado"].as_str(), Some("aplicado") | Some("error")) {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "success": false, "error": "Sólo se revierte lo que se aplicó." })),
        )
            .into_response();
    }
    let eds = match crate::parche::eds_de_json(&v["ediciones"]) {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": e })),
            )
                .into_response()
        }
    };
    let repo = match repo_de_parches(&st) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": e })),
            )
                .into_response()
        }
    };
    match crate::parche::aplicar(&repo, &crate::parche::invertir(&eds)) {
        Ok(apl) => {
            let mut v2 = v.clone();
            v2["estado"] = json!("revertido");
            v2["revertido"] = json!({ "archivos": apl.archivos, "cuando": now_ms() });
            let _ = crate::parche::escribir(&st.data_dir, &v2);
            (
                StatusCode::OK,
                Json(json!({ "success": true, "estado": "revertido", "archivos": apl.archivos })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": e })),
        )
            .into_response(),
    }
}

/// `GET /api/voz/dialogo` — el hilo de la conversación en curso (turnos y foco).
async fn voz_dialogo(State(st): State<AppState>) -> impl IntoResponse {
    let sesion = crate::dialogo::leer(&st.vault.raiz().join(".nodeflow"));
    Json(json!({
        "success": true,
        "turnos": sesion["turnos"],
        "foco": sesion["foco"],
        "minutos_de_vida": crate::dialogo::MINUTOS_DE_VIDA,
    }))
}

/// `POST /api/voz/decir` — sintetiza una frase con la voz LOCAL (Kokoro) y devuelve el WAV.
///
/// La voz vive en la máquina del usuario (`tools/tts/servidor.py`, puerto 8125): sin cuotas, sin
/// mandar el texto a ningún servicio. Si no está corriendo, la app sigue andando y lo dice claro.
async fn voz_decir(
    State(st): State<AppState>,
    Json(body): Json<Value>,
) -> axum::response::Response {
    use axum::body::Body;
    use axum::response::Response;

    let responder_json = |codigo: StatusCode, mensaje: String| -> Response {
        Response::builder()
            .status(codigo)
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({ "success": false, "error": mensaje }).to_string(),
            ))
            .unwrap()
    };

    let texto = body["texto"].as_str().unwrap_or("").trim().to_string();
    if texto.is_empty() {
        return responder_json(StatusCode::BAD_REQUEST, "Falta el texto a decir.".into());
    }
    let url = format!("{}/decir", crate::voz::tts_url());
    // La voz sigue al idioma **de la voz** (no al de la interfaz): si el llamador no pide una voz
    // puntual, se usa la nativa de ese idioma. Así el switch ES/EN también **se escucha**, y la voz
    // puede ir por su lado cuando la interfaz está en el otro idioma.
    let idioma = crate::idioma::voz_idioma(&st.data_dir);
    let voz = body["voz"]
        .as_str()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| crate::idioma::voz_tts(&idioma))
        .to_string();
    match st
        .http
        .post(&url)
        .json(&json!({ "texto": texto, "voz": voz, "idioma": idioma }))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => match r.bytes().await {
            Ok(audio) => {
                log::info!(
                    "voz: {} caracteres sintetizados ({} KB)",
                    texto.chars().count(),
                    audio.len() / 1024
                );
                Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "audio/wav")
                    .body(Body::from(audio))
                    .unwrap()
            }
            Err(e) => responder_json(
                StatusCode::BAD_GATEWAY,
                format!("Respuesta de voz inválida: {e}"),
            ),
        },
        Ok(r) => responder_json(
            StatusCode::BAD_GATEWAY,
            format!("El servidor de voz respondió {}.", r.status()),
        ),
        Err(_) => responder_json(
            StatusCode::BAD_GATEWAY,
            format!("La voz local no responde en {url}. Arrancala con tools/tts/servidor.py."),
        ),
    }
}

/// `GET /api/voz/jwt` — token temporal de realtime para el WebSocket del navegador.
/// La clave de cuenta se queda en el backend: el frontend sólo ve un token que expira.
/// **Agnóstico del motor**: el proveedor elegido decide cómo se emite (ver `stt::abrir_sesion`).
async fn voz_jwt(State(st): State<AppState>) -> impl IntoResponse {
    let id = crate::stt::seleccionado(&st.data_dir);
    let Some(prov) = crate::stt::por_id(&id) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                json!({ "success": false, "error": format!("Motor de voz «{id}» no está en el catálogo.") }),
            ),
        );
    };
    let Some(clave) = clave_voz(&st, prov) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "proveedor": prov.id,
                "error": format!(
                    "Falta la clave de {}: {} en el entorno, o \"{}\" en nodeflow.config.json.",
                    prov.etiqueta,
                    prov.clave_env.first().copied().unwrap_or(""),
                    prov.clave_config.first().copied().unwrap_or("")
                )
            })),
        );
    };
    let (_, _, idioma_ajustes) = crate::voz::ajustes();
    let idioma_pedido = if std::env::var("NODEFLOW_VOZ_IDIOMA").is_ok() {
        idioma_ajustes
    } else {
        crate::idioma::voz_idioma(&st.data_dir)
    };
    let modelo_pedido = std::env::var("NODEFLOW_VOZ_MODELO").unwrap_or_default();
    match crate::stt::abrir_sesion(&st.http, prov, &clave, &idioma_pedido, &modelo_pedido).await {
        Ok(sesion) => {
            log::info!(
                "voz: sesión de {} emitida ({} s, idioma {})",
                sesion.proveedor,
                sesion.expira_en_s,
                sesion.idioma
            );
            (StatusCode::OK, Json(sesion.a_json()))
        }
        Err(e) => {
            log::warn!("voz: no pude abrir sesión con {}: {e}", prov.id);
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "success": false, "proveedor": prov.id, "error": e })),
            )
        }
    }
}

async fn graph_state(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let info = st.vault.info();
    let revision = info["revision"].as_u64().unwrap_or(0);
    if let Some(since) = q.get("since").and_then(|s| s.parse::<u64>().ok()) {
        if since == revision {
            return Json(json!({ "changed": false, "revision": revision }));
        }
    }
    // `info` ya incluye `ultimos_cambios_externos` (se limpia en el próximo guardado propio),
    // así el frontend distingue una edición de Obsidian de su propio autoguardado.
    Json(json!({
        "changed": true,
        "revision": revision,
        "info": info,
        "cambios_externos": st.vault.info()["ultimos_cambios_externos"].clone(),
        "pendientes": st.vault.count_pending(),
        "metricas": st.vault.metricas(),
        "state": st.vault.read_state(),
    }))
}

/// Guarda el grafo: escribe el canónico + `.canvas` + nota índice + una nota por nodo.
async fn graph_save(State(st): State<AppState>, Json(payload): Json<Value>) -> impl IntoResponse {
    match st.vault.save(&payload) {
        Ok(res) => {
            log::info!(
                "vault: guardado rev={} · {} nodos · {} aristas",
                res["revision"],
                res["nodos"],
                res["aristas"]
            );
            (StatusCode::OK, Json(res))
        }
        Err(e) => {
            log::warn!("vault: guardado rechazado: {e}");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "ok": false, "error": e })),
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fase 4 — Superficie del agente (lee y escribe el lienzo)
// ─────────────────────────────────────────────────────────────────────────────

/// Vista compacta del grafo: stats + nodos con sus conexiones. Leer antes de escribir.
async fn graph_summary(State(st): State<AppState>) -> impl IntoResponse {
    Json(st.vault.summary())
}

/// Crea (o actualiza, si el título/id coincide) un nodo. Con `parent` lo conecta.
async fn graph_node(State(st): State<AppState>, Json(p): Json<Value>) -> impl IntoResponse {
    if modo_apply(&p) {
        responder(st.vault.upsert_node(&p), "nodo")
    } else {
        responder(st.vault.propose("nodo", &p), "propuesta")
    }
}

/// Por defecto las escrituras del agente son PROPUESTAS: se aplican solo si el llamador manda
/// `mode: "apply"` (y el humano igual ve el resultado en el panel).
fn modo_apply(p: &Value) -> bool {
    matches!(p["mode"].as_str(), Some("apply") | Some("aplicar"))
        || matches!(p["modo"].as_str(), Some("apply") | Some("aplicar"))
}

/// Conecta dos nodos por id o título.
async fn graph_edge(State(st): State<AppState>, Json(p): Json<Value>) -> impl IntoResponse {
    if modo_apply(&p) {
        responder(st.vault.add_edge(&p), "arista")
    } else {
        responder(st.vault.propose("conectar", &p), "propuesta")
    }
}

/// Borra un nodo y sus aristas (nunca el núcleo).
/// `POST /api/graph/merge` — fusionar dos nodos: `origen` se absorbe en `destino` (Fase 5.5).
/// Como el resto: propone, salvo `{"apply": true}`.
async fn graph_merge(State(st): State<AppState>, Json(p): Json<Value>) -> impl IntoResponse {
    if modo_apply(&p) {
        responder(st.vault.merge_node(&p), "fusionado")
    } else {
        responder(st.vault.propose("fusionar", &p), "propuesta")
    }
}

async fn graph_delete(State(st): State<AppState>, Json(p): Json<Value>) -> impl IntoResponse {
    if modo_apply(&p) {
        responder(st.vault.delete_node(&p), "borrado")
    } else {
        responder(st.vault.propose("borrar", &p), "propuesta")
    }
}

fn responder(res: Result<Value, String>, que: &str) -> (StatusCode, Json<Value>) {
    match res {
        Ok(v) => {
            log::info!(
                "agente: {que} {} · rev={} · {}",
                v["accion"].as_str().unwrap_or("ok"),
                v["revision"],
                v["id"].as_str().unwrap_or("-")
            );
            (StatusCode::OK, Json(v))
        }
        Err(e) => {
            log::warn!("agente: {que} rechazado: {e}");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "ok": false, "error": e })),
            )
        }
    }
}

/// Saca aristas colgadas (integridad del grafo).
async fn graph_prune(State(st): State<AppState>, Json(p): Json<Value>) -> impl IntoResponse {
    if modo_apply(&p) {
        responder(st.vault.prune(&p), "saneo")
    } else {
        responder(st.vault.propose("sanear", &p), "propuesta")
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fase 5a — Propuestas del agente (aprobar / rechazar)
// ─────────────────────────────────────────────────────────────────────────────

/// Lista de propuestas pendientes. `?since=<rev>` responde barato cuando nada cambió.
async fn agent_pending(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let since = q.get("since").and_then(|s| s.parse::<u64>().ok());
    Json(st.vault.pending_list(since))
}

async fn agent_approve(State(st): State<AppState>, Json(p): Json<Value>) -> impl IntoResponse {
    let (ids, todos) = ids_y_todos(&p);
    responder(st.vault.resolve_pending(&ids, todos, true), "aprobación")
}

async fn agent_reject(State(st): State<AppState>, Json(p): Json<Value>) -> impl IntoResponse {
    let (ids, todos) = ids_y_todos(&p);
    responder(st.vault.resolve_pending(&ids, todos, false), "rechazo")
}

/// Acepta `{id}`, `{ids: [...]}` o `{todos: true}` / `{all: true}`.
fn ids_y_todos(p: &Value) -> (Vec<String>, bool) {
    let mut ids: Vec<String> = p["ids"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    if let Some(uno) = p["id"].as_str() {
        ids.push(uno.to_string());
    }
    let todos = p["todos"].as_bool().unwrap_or(false) || p["all"].as_bool().unwrap_or(false);
    (ids, todos)
}

// ─────────────────────────────────────────────────────────────────────────────
// Fase 5b — Memoria semántica del vault
// ─────────────────────────────────────────────────────────────────────────────

/// Busca en toda la bóveda: `?q=...&limit=8`.
async fn vault_search(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let consulta = q.get("q").cloned().unwrap_or_default();
    let limite = q
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(8);
    Json(st.memoria.buscar(&consulta, limite))
}

/// Reconstruye el índice a pedido (el automático corre si pasaron 30 s).
async fn vault_reindex(State(st): State<AppState>) -> impl IntoResponse {
    let indexadas = st.memoria.indexar();
    Json(json!({
        "ok": true,
        "indexadas": indexadas,
        "estado": st.memoria.estado(),
    }))
}

/// Estado del índice: cuántas notas, cuándo se construyó, ruta.
async fn vault_memory(State(st): State<AppState>) -> impl IntoResponse {
    Json(st.memoria.estado())
}

/// Lee una nota de la bóveda: `?ruta=01_Identidad/foo.md`.
async fn vault_note(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let ruta = q.get("ruta").cloned().unwrap_or_default();
    match st.memoria.leer_nota(&ruta) {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"ok": false, "error": e})),
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fase 7a — Agente jardín
// ─────────────────────────────────────────────────────────────────────────────

/// Diagnóstico del grafo: invariantes + hallazgos (solo lectura, no toca nada).
async fn graph_garden(State(st): State<AppState>) -> impl IntoResponse {
    Json(st.vault.jardin_scan())
}

/// `GET /api/graph/siguiente` — el camino crítico del mapa: qué frena, qué falta y qué conviene hacer.
/// Existe para que "¿qué sigue?" se pueda **consultar** en vez de intuir: es el propósito del mapa.
async fn graph_siguiente(State(st): State<AppState>) -> impl IntoResponse {
    Json(st.vault.siguiente())
}

/// Convierte los hallazgos accionables en PROPUESTAS para aprobar en el panel.
async fn graph_garden_fix(State(st): State<AppState>, Json(p): Json<Value>) -> impl IntoResponse {
    responder(st.vault.jardin_proponer(&p), "jardín")
}

/// Propone reacomodar el lienzo en niveles (layout determinista sin solapamientos).
async fn graph_tidy(State(st): State<AppState>, Json(p): Json<Value>) -> impl IntoResponse {
    responder(st.vault.tidy(&p), "reacomodo")
}

/// Métrica de valor: minutos entre el brain dump (T0) y el primer artefacto aprobado (T1).
async fn metrics(State(st): State<AppState>) -> impl IntoResponse {
    Json(st.vault.metricas())
}

// ─────────────────────────────────────────────────────────────────────────────
// Fase 8 — Captura de conocimiento y exportación
// ─────────────────────────────────────────────────────────────────────────────

/// Vista previa: texto crudo → candidatos a nodo (no propone nada).
/// `POST /api/knowledge/draft` — Fase 10: el **modelo local propone** (título, categoría, madurez,
/// tags) y el **código valida** campo por campo; lo rechazado cae al valor determinista del sistema.
/// Con `proponer: true`, lo validado entra a la cola de aprobación — nunca al lienzo.
/// `simular` es un hook de verificación: alimenta el validador con un JSON dado sin cargar el modelo.
async fn knowledge_draft(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let nodos: Vec<Value> = match body["nodos"].as_array() {
        Some(a) => a.clone(),
        None => crate::conocimiento::segmentar(body["texto"].as_str().unwrap_or(""), None, None),
    };
    if nodos.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "ok": false, "error": "el texto no produjo candidatos (cada bloque necesita densidad mínima)" }),
            ),
        );
    }
    let categoria_fallback = body["categoria"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "CONOCIMIENTO".to_string());
    let limite = body["limite"]
        .as_u64()
        .map(|v| (v as usize).clamp(1, 15))
        .unwrap_or(st.borrador.tope_por_pedido);
    let simulado = body.get("simular").cloned();
    // Vocabulario medido: las categorías que el lienzo YA usa. Es verdad medida y evita que el
    // validador rechace una categoría legítima solo porque no estaba en una lista escrita a mano.
    let vocabulario: Vec<String> = st
        .vault
        .grafo_actual()
        .0
        .iter()
        .filter_map(|n| n["data"]["category"].as_str().map(|s| s.to_string()))
        .collect();
    let inicio = std::time::Instant::now();

    let mut borradores: Vec<Value> = Vec::new();
    let mut enviados: Vec<Value> = Vec::new();
    for n in nodos.iter().take(limite) {
        let titulo_h = n["titulo"]
            .as_str()
            .or_else(|| n["title"].as_str())
            .unwrap_or("")
            .to_string();
        let cuerpo = n["descripcion"]
            .as_str()
            .or_else(|| n["description"].as_str())
            .unwrap_or("")
            .to_string();
        let (crudo, autor_del_crudo, ms) = match &simulado {
            Some(v) => (
                Some(v.clone()),
                "simulado (hook de verificación)".to_string(),
                0u128,
            ),
            None => match call_borrador_local(&st, &titulo_h, &cuerpo).await {
                Some((v, modelo, ms)) => (Some(v), modelo, ms),
                None => (None, "sin respuesta del daemon local".to_string(), 0u128),
            },
        };
        let b = crate::borrador::validar_con(
            crudo.as_ref(),
            &titulo_h,
            &categoria_fallback,
            &vocabulario,
        );
        borradores.push(json!({
            "titulo_heuristica": titulo_h,
            "borrador": b.json(),
            // Lo que devolvió el modelo, tal cual: sin esto un rechazo no se puede auditar.
            "crudo": crudo,
            "autor_crudo": autor_del_crudo,
            "ms": ms,
        }));
        enviados.push(json!({
            "titulo": b.titulo,
            "descripcion": cuerpo,
            "categoria": b.categoria,
            "madurez": b.madurez,
            "tags": b.tags,
        }));
    }

    let proponer = body["proponer"].as_bool().unwrap_or(false);
    let mut propuestas = Value::Null;
    if proponer {
        let req = json!({
            "nodos": enviados,
            "categoria": categoria_fallback,
            "parent": body["parent"].as_str().unwrap_or(""),
            "motivo": "Borrador local (modelo chico) validado por el código",
        });
        propuestas = match st.vault.conocimiento_capturar(&req) {
            Ok(v) => v,
            Err(e) => json!({ "ok": false, "error": e }),
        };
    }

    let cubiertos = nodos.len().min(limite);
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "modelo": st.borrador.modelo,
            "keep_alive": st.borrador.keep_alive,
            "candidatos": nodos.len(),
            "con_borrador": cubiertos,
            "borradores": borradores,
            "propuestas": propuestas,
            "nota": if cubiertos < nodos.len() {
                format!("{cubiertos} de {} candidatos fueron al modelo; el resto usa el camino determinista (subí `limite`)", nodos.len())
            } else {
                "el modelo propone, el código valida: cada rechazo queda en `problemas`".to_string()
            },
            "ms": inicio.elapsed().as_millis(),
        })),
    )
}

/// Llama al daemon local por su API **nativa** (`/api/chat`): es la que acepta un JSON Schema como
/// gramática y `keep_alive` **por request** (una variable global en 0 s recarga el modelo cada vez y
/// convierte 12 ms en 13 s). No confundir con `call_ollama`, que usa la ruta `/v1` compatible con
/// OpenAI de los modelos de la cuota gratuita.
async fn call_borrador_local(
    st: &AppState,
    titulo: &str,
    cuerpo: &str,
) -> Option<(Value, String, u128)> {
    let cfg = &st.borrador;
    let url = format!("{}/api/chat", cfg.url.trim_end_matches('/'));
    let body = json!({
        "model": cfg.modelo,
        "messages": [{ "role": "user", "content": crate::borrador::prompt(titulo, cuerpo) }],
        "stream": false,
        "format": crate::borrador::esquema(),
        "keep_alive": cfg.keep_alive,
        "options": { "temperature": 0 }
    });
    let t = std::time::Instant::now();
    match st.http.post(&url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            let v: Value = resp.json().await.ok()?;
            let ms = t.elapsed().as_millis();
            match parse_json_text(v["message"]["content"].as_str().unwrap_or("")) {
                Some(p) => Some((p, cfg.modelo.clone(), ms)),
                None => {
                    log::warn!("borrador: el modelo local respondió algo que no es JSON");
                    None
                }
            }
        }
        Ok(resp) => {
            log::warn!("borrador: el daemon local respondió HTTP {}", resp.status());
            None
        }
        Err(e) => {
            log::warn!("borrador: el daemon local no responde ({e})");
            None
        }
    }
}

async fn knowledge_preview(State(st): State<AppState>, Json(p): Json<Value>) -> impl IntoResponse {
    match st.vault.conocimiento_preview(&p) {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"ok": false, "error": e})),
        ),
    }
}

/// Captura: convierte los candidatos en propuestas para aprobar.
async fn knowledge_capture(State(st): State<AppState>, Json(p): Json<Value>) -> impl IntoResponse {
    responder(st.vault.conocimiento_capturar(&p), "captura")
}

/// Entregable: el mapa como documento Markdown.
async fn export_document(State(st): State<AppState>) -> impl IntoResponse {
    match st.vault.exportar_documento() {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"ok": false, "error": e})),
        ),
    }
}

/// Exportación portable: el estado canónico tal cual (re-importable).
async fn export_json(State(st): State<AppState>) -> impl IntoResponse {
    match st.vault.read_state() {
        Some(s) => (StatusCode::OK, Json(json!({"ok": true, "estado": s}))),
        None => (
            StatusCode::BAD_REQUEST,
            Json(json!({"ok": false, "error": "todavía no hay estado en disco"})),
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Slice 1 — Expertos y Contrato de Artefactos
// ─────────────────────────────────────────────────────────────────────────────

/// `GET /api/voz/motor/estado` — la voz local (Kokoro): si está instalada, si está corriendo y
/// cuánto lleva la descarga. Arranca el servidor si hace falta (es idempotente y barato).
async fn voz_motor_estado(State(st): State<AppState>) -> impl IntoResponse {
    if crate::voz_local::instalada(&st.data_dir) && !crate::voz_local::corriendo() {
        let _ = crate::voz_local::arrancar(&st.data_dir);
    }
    Json(json!({ "success": true, "motor": crate::voz_local::estado(&st.data_dir) }))
}

/// `POST /api/voz/motor/instalar` — baja el runtime (80 MB) y el modelo (325 MB), verifica el
/// hash, despliega y arranca. **No bloquea**: la descarga corre en segundo plano y el panel sigue
/// el progreso con `voz.instalacion.json` (una petición abierta cinco minutos no es una opción).
async fn voz_motor_instalar(State(st): State<AppState>) -> impl IntoResponse {
    if crate::voz_local::corriendo() && crate::voz_local::instalada(&st.data_dir) {
        return (
            StatusCode::OK,
            Json(
                json!({ "success": true, "ya_estaba": true, "motor": crate::voz_local::estado(&st.data_dir) }),
            ),
        );
    }
    if crate::voz_local::en_curso(&st.data_dir) {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "ocupado": true,
                "error": "ya hay una descarga de la voz local en curso"
            })),
        );
    }
    let (zip_url, modelo_url) = crate::voz_local::urls(&st.data_dir);
    let data_dir = st.data_dir.clone();
    tokio::spawn(async move {
        match crate::voz_local::instalar(data_dir, zip_url, modelo_url).await {
            Ok(m) => log::info!("voz local: instalada · {m}"),
            Err(e) => log::warn!("voz local: la instalación falló · {e}"),
        }
    });
    (
        StatusCode::OK,
        Json(
            json!({ "success": true, "iniciada": true, "motor": crate::voz_local::estado(&st.data_dir) }),
        ),
    )
}

/// `POST /api/voz/motor/arrancar` — arranca el servidor de voz ya instalado (sin ventana).
async fn voz_motor_arrancar(State(st): State<AppState>) -> impl IntoResponse {
    match crate::voz_local::arrancar(&st.data_dir) {
        Ok(m) => (
            StatusCode::OK,
            Json(json!({ "success": true, "mensaje": m })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": e })),
        ),
    }
}

/// Dónde quedó el servidor MCP. El instalador lo copia junto al ejecutable (Tauri respeta la
/// estructura relativa del proyecto); en desarrollo vive en el repo, un nivel arriba de `src-tauri`.
fn mcp_server_recurso() -> Option<std::path::PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let candidatos = [
        exe_dir.join("mcp-server").join("nodeflow_mcp.py"),
        exe_dir
            .join("resources")
            .join("mcp-server")
            .join("nodeflow_mcp.py"),
        exe_dir
            .join("_up_")
            .join("mcp-server")
            .join("nodeflow_mcp.py"),
        exe_dir
            .join("..")
            .join("..")
            .join("mcp-server")
            .join("nodeflow_mcp.py"),
    ];
    if let Some(p) = candidatos.into_iter().find(|p| p.exists()) {
        return Some(p);
    }
    // El instalador puede haberlo dejado en otra profundidad (depende de la versión de Tauri):
    // se busca el archivo hasta 3 niveles abajo del ejecutable y se declara dónde estaba.
    fn buscar(dir: &std::path::Path, prof: u8) -> Option<std::path::PathBuf> {
        if prof == 0 {
            return None;
        }
        let entradas = std::fs::read_dir(dir).ok()?;
        let mut subdirs = Vec::new();
        for e in entradas.flatten() {
            let p = e.path();
            if p.is_dir() {
                subdirs.push(p);
            } else if p.file_name().and_then(|n| n.to_str()) == Some("nodeflow_mcp.py") {
                return Some(p);
            }
        }
        subdirs.iter().find_map(|d| buscar(d, prof - 1))
    }
    buscar(&exe_dir, 3)
}

/// Qué intérprete de Python hay para correr el servidor MCP: el runtime de la voz local (si se
/// descargó), el Python del sistema, o el lanzador `py`. Sin intérprete no hay MCP: se declara.
fn mcp_interprete(data_dir: &std::path::Path) -> Option<String> {
    let portatil = data_dir.join("voz").join("python.exe");
    if portatil.exists() {
        return Some(portatil.to_string_lossy().to_string());
    }
    for cand in ["python", "python3", "py"] {
        if std::process::Command::new(cand)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Some(cand.to_string());
        }
    }
    None
}

/// El servidor MCP como dato: dónde está, dónde se instala y con qué comando se conecta.
///
/// El bloque `mcp_servers` es el formato del config de Hermes (`~/.hermes/config.yaml`), el mismo
/// que entienden los demás clientes MCP por stdio.
async fn mcp_estado(State(st): State<AppState>) -> impl IntoResponse {
    let recurso = mcp_server_recurso();
    let destino = st.data_dir.join("mcp").join("nodeflow_mcp.py");
    let interprete = mcp_interprete(&st.data_dir);
    let listo = destino.exists() && interprete.is_some();
    let cmd = interprete.clone().unwrap_or_else(|| "python".into());
    Json(json!({
        "success": true,
        "recurso": recurso.as_ref().map(|p| p.to_string_lossy().to_string()),
        "instalado": destino.exists(),
        "ruta": destino.to_string_lossy(),
        "interprete": interprete,
        "listo": listo,
        "snippet": format!(
            "mcp_servers:\n  nodeflow:\n    command: \"{cmd}\"\n    args: [\"{}\"]",
            destino.to_string_lossy().replace('\\', "/")
        ),
        "registro": format!(
            "printf 'Y\\n' | hermes mcp add nodeflow --command '{cmd}' --args '{}'",
            destino.to_string_lossy().replace('\\', "/")
        ),
        "nota": if listo {
            "Reiniciá el agente después de registrarlo: el descubrimiento corre una vez por proceso."
        } else if !destino.exists() {
            "El servidor MCP todavía no está copiado: usá «Instalar el servidor MCP»."
        } else {
            "Falta un intérprete de Python. Descargá la voz local (trae uno) o instalá Python 3."
        },
    }))
}

/// Copia el servidor MCP a una carpeta estable del usuario y devuelve cómo conectarlo.
async fn mcp_instalar(State(st): State<AppState>) -> impl IntoResponse {
    let Some(recurso) = mcp_server_recurso() else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": "no encontré el servidor MCP junto a la app (¿binario de desarrollo?)"
            })),
        );
    };
    let dir = st.data_dir.join("mcp");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": format!("no pude crear {dir:?}: {e}") })),
        );
    }
    let destino = dir.join("nodeflow_mcp.py");
    match std::fs::copy(&recurso, &destino) {
        Ok(bytes) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "ruta": destino.to_string_lossy(),
                "bytes": bytes,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": format!("no pude copiarlo: {e}") })),
        ),
    }
}

/// Guarda el **system prompt de un experto** como nota en `<vault>/expertos/<slug>.md`.
///
/// El prompt de un experto es **identidad del usuario, no del producto**: la app no trae ninguno
/// embebido y esto escribe únicamente en su bóveda. Si el slug ya existe, el archivo se reemplaza
/// (el cuerpo entero es el prompt, así que pegarlo de nuevo es la forma de editarlo).
async fn expertos_guardar(
    State(st): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let nombre = body["nombre"].as_str().unwrap_or("").trim().to_string();
    let tipo = body["tipo_artefacto"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    let system = body["system"].as_str().unwrap_or("").trim().to_string();
    if nombre.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "falta el nombre del experto" })),
        );
    }
    if crate::artefactos::schema(&tipo).is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "success": false, "error": format!("tipo de artefacto desconocido: «{tipo}»") }),
            ),
        );
    }
    if system.chars().count() < 40 {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "success": false, "error": "el prompt es muy corto: pegá el system prompt completo" }),
            ),
        );
    }
    // El nombre del archivo sale del slug: minúsculas/números/guiones, 3-60, sin rutas (mismo
    // camino jaula que los planes del cerebro). Si no lo mandan, se deriva del nombre.
    let slug = match body["slug"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(s) => match crate::cerebro::slug_plan(s) {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "success": false, "error": e })),
                );
            }
        },
        None => match crate::cerebro::slug_plan(&nombre) {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(
                        json!({ "success": false, "error": format!("no pude formar el nombre de archivo: {e}") }),
                    ),
                );
            }
        },
    };
    let rol = body["rol"].as_str().unwrap_or("").to_string();
    let descripcion = body["descripcion"].as_str().unwrap_or("").to_string();
    let proveedor = body["proveedor"].as_str().unwrap_or("").to_string();
    let modelo = body["modelo"].as_str().unwrap_or("").to_string();
    let txt = crate::expertos::md_desde(
        &nombre,
        &tipo,
        &[
            ("rol", rol.as_str()),
            ("descripcion", descripcion.as_str()),
            ("proveedor", proveedor.as_str()),
            ("modelo", modelo.as_str()),
        ],
        &system,
    );
    let rel = format!("{}/{}.md", crate::expertos::CARPETA, slug);
    match st.vault.escribir_nota(&rel, &txt) {
        Ok(p) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "slug": slug,
                "ruta": p.to_string_lossy(),
                "caracteres_system": system.chars().count(),
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e })),
        ),
    }
}

/// Lista los expertos disponibles (notas en `<vault>/expertos/*.md`) y los tipos de artefacto.
async fn expertos_listar(State(st): State<AppState>) -> impl IntoResponse {
    let dir = st.vault.raiz().join(crate::expertos::CARPETA);
    Json(json!({
        "ok": true,
        "expertos": crate::expertos::cargar(&dir),
        "tipos": crate::artefactos::tipos(),
        "carpeta": dir.to_string_lossy(),
    }))
}

/// Ejecuta un Experto sobre un nodo del lienzo.
///
/// Arma el contexto (el nodo + sus vecinos en el grafo + las notas de la bóveda que recupera BM25),
/// llama al modelo con el system prompt del experto y devuelve un artefacto **validado contra su
/// destino**. Si el validador lo rechaza, hace UN reintento de reparación explicándole al modelo qué
/// falló: es más barato que devolver basura y que el usuario la pegue en Flow.
async fn experto_run(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let inicio = std::time::Instant::now();
    let nodo_ref = body["nodo"].as_str().unwrap_or("").trim().to_string();
    let experto_ref = body["experto"].as_str().unwrap_or("").trim().to_string();
    if nodo_ref.is_empty() || experto_ref.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"ok": false, "error": "faltan `nodo` y `experto`"})),
        );
    }

    let Some(nodo) = st.vault.buscar_nodo(&nodo_ref) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"ok": false, "error": format!("no encontré el nodo «{nodo_ref}»")})),
        );
    };

    let dir = st.vault.raiz().join(crate::expertos::CARPETA);
    let expertos = crate::expertos::cargar(&dir);
    let encontrado = expertos.iter().find(|e| {
        let n = e["nombre"].as_str().unwrap_or("");
        let s = e["slug"].as_str().unwrap_or("");
        n.eq_ignore_ascii_case(&experto_ref) || s.eq_ignore_ascii_case(&experto_ref)
    });
    let Some(exp) = encontrado else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"ok": false, "error": format!(
                "no encontré el experto «{experto_ref}» (la carpeta es {})", dir.to_string_lossy()
            )})),
        );
    };
    if exp["valido"].as_bool() != Some(true) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"ok": false, "error": format!(
                "el experto «{}» declara un tipo de artefacto desconocido: «{}»",
                exp["nombre"], exp["tipo_artefacto"]
            )})),
        );
    }

    let tipo = exp["tipo_artefacto"].as_str().unwrap_or("").to_string();
    let system = exp["system"].as_str().unwrap_or("").to_string();
    let Some(schema) = crate::artefactos::schema(&tipo) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "error": "tipo de artefacto sin schema"})),
        );
    };

    // ── Contexto: el nodo + vecinos del grafo + memoria de la bóveda ──────────
    let titulo = crate::grafo::titulo_de(&nodo);
    let desc = crate::grafo::descripcion_de(&nodo);
    let categoria = nodo["data"]["category"].as_str().unwrap_or("").to_string();
    let id = crate::grafo::id_de(&nodo);
    let (nodes, edges) = st.vault.grafo_actual();
    let mut vecinos: Vec<String> = Vec::new();
    for e in &edges {
        let s = e["source"].as_str().unwrap_or("");
        let t = e["target"].as_str().unwrap_or("");
        let otro = if s == id {
            t
        } else if t == id {
            s
        } else {
            continue;
        };
        if let Some(n) = nodes.iter().find(|x| crate::grafo::id_de(x) == otro) {
            let d: String = crate::grafo::descripcion_de(n).chars().take(220).collect();
            let ct = n["data"]["category"].as_str().unwrap_or("");
            // La madurez es la señal de confianza del propio sistema: 1-2 = idea o semilla,
            // 4-5 = validado en la práctica. Sin esto el modelo no puede pesar el contexto.
            let mad = n["data"]["maturity"]
                .as_i64()
                .map(|m| format!(" · madurez {m}/5"))
                .unwrap_or_default();
            vecinos.push(format!(
                "- [{ct}{mad}] {}: {}",
                crate::grafo::titulo_de(n),
                d.trim()
            ));
        }
    }
    vecinos.truncate(8);
    let (bloque, fuentes) = bloque_memoria(
        &st.vault,
        &st.memoria,
        &json!({"title": titulo, "description": desc}),
    );

    // ── Snapshot de estado MEDIDO ─────────────────────────────────────────────
    // Sin esto, un experto puede afirmar que una capacidad «no existe» porque las notas que recuperó
    // de la bóveda son viejas. Pasó dos veces el mismo día («la métrica no está medida» cuando ya
    // estaba instrumentada). Los hechos medidos ganan sobre cualquier nota.
    let diag = crate::grafo::diagnostico(&nodes, &edges);
    let st_stats = &diag["stats"];
    let mut hechos = String::new();
    hechos += &format!(
        "- lienzo: {} nodos · {} aristas · {} aristas colgadas · {} nodos sin conexiones\n",
        st_stats["nodos"].as_i64().unwrap_or(0),
        st_stats["aristas"].as_i64().unwrap_or(0),
        diag["bloqueantes"].as_i64().unwrap_or(0),
        st_stats["huerfanos"].as_i64().unwrap_or(0),
    );
    hechos += &format!(
        "- propuestas esperando aprobación humana: {}\n",
        st.vault.count_pending()
    );
    let met = st.vault.metricas();
    if met["conversiones"].as_i64().unwrap_or(0) > 0 {
        hechos += &format!(
            "- métrica de valor T0→T1: INSTRUMENTADA, {} conversión(es) medida(s) · promedio {} min · objetivo {} min\n",
            met["conversiones"].as_i64().unwrap_or(0),
            met["promedio_min"].as_f64().unwrap_or(0.0),
            met["objetivo_min"].as_f64().unwrap_or(3.0),
        );
    } else {
        hechos += "- métrica de valor T0→T1: instrumentada, todavía sin lecturas\n";
    }
    hechos += "- el sistema ya tiene: cola de aprobación persistente, memoria BM25 de la bóveda, \
                jardín de invariantes, panel de conocimiento, orquestador de expertos con contrato de \
                artefactos, puente MCP (22 herramientas) y app instalable\n";
    hechos += "- el cliente MCP (el agente) SÍ re-registra herramientas en caliente al recibir la \
                notificación tools/list_changed: no hace falta reiniciar nada para eso\n";
    // Cabecera: fecha del día + huella del contenido medido. Dos corridas con el mismo estado
    // producen el mismo prompt (y la caché acierta); si algo cambió, la huella cambia sola.
    let huella = crate::costo::hash_estable(&hechos);
    hechos = format!(
        "- fecha: {}\n- huella del estado medido: {huella}\n{hechos}",
        hoy()
    );

    let extra = body["extra"].as_str().unwrap_or("").trim().to_string();
    let mut prompt = format!("CONCEPTO (nodo del lienzo)\nTítulo: {titulo}\nDescripción: {desc}\n");
    if !categoria.is_empty() {
        prompt += &format!("Categoría: {categoria}\n");
    }
    if !vecinos.is_empty() {
        prompt += &format!(
            "\nNODOS CONECTADOS (contexto del mismo grafo)\n{}\n",
            vecinos.join("\n")
        );
    }
    prompt += &format!("\nESTADO MEDIDO DEL SISTEMA\n{hechos}");
    prompt += "\nJERARQUÍA DE CONFIANZA (regla del sistema, no negociable)\n\
1. ESTADO MEDIDO (arriba): son hechos que el sistema verifica en este momento. Es la verdad operativa:\n\
   si algo de lo que sabés los contradice, ganan ellos y lo decís.\n\
2. NOTAS DE LA BÓVEDA: son REFERENCIA, nunca verdad. Ahí hay ideas, teorías, borradores y datos sin\n\
   corroborar. Usalas como material; si afirmás algo que sale de una nota, atribuilo («según la nota\n\
   X»); si plantean algo no verificado, presentalo como hipótesis y no como hecho.\n\
3. TU CONOCIMIENTO PREVIO: puede estar desactualizado. No lo presentes con más seguridad que los\n\
   datos de arriba.\n\
Todo artefacto tiene que distinguir lo establecido de lo propuesto, y lo medido de lo supuesto.\n";
    if !bloque.is_empty() {
        prompt += &format!("\nNOTAS DE LA BÓVEDA (las más relevantes por BM25; pueden estar desactualizadas)\n{bloque}\n");
    }
    if !extra.is_empty() {
        prompt += &format!("\nINDICACIONES EXTRA DEL USUARIO\n{extra}\n");
    }
    prompt += &format!(
        "\nTAREA\n{}\n\n{}",
        crate::artefactos::instruccion(&tipo),
        crate::artefactos::regla_epistemica()
    );

    // Escalado por contrato: se prueba proveedor por proveedor y se avanza cuando el artefacto NO
    // cumple. Ollama es gratis y rápido pero no aplica el schema (solo pide "JSON"); Gemini sí lo
    // aplica. Así el barato va primero y el estricto corrige, y no se gasta un llamado de más cuando
    // el primero ya cumple el contrato.
    let key = resolve_key(&st, &headers).unwrap_or_default();
    let mut artefacto = json!({});
    let mut problemas: Vec<String> = vec!["todavía sin respuesta".into()];
    let mut proveedor = String::new();
    let mut intentos = 0;
    let mut traza: Vec<Value> = Vec::new();
    let mut respondio_alguno = false;
    // Fase 9 — el costo del artefacto: cada intento aporta consumo, costo, caché y tokens evitados.
    let mut consumo_total = crate::costo::Consumo::default();
    let mut estimado_total = false;
    let mut cache_hits: u64 = 0;
    let mut tokens_evitados: u64 = 0;
    let mut modelo_final = String::new();
    let mut prov_final = String::new();

    // Si el experto declara su proveedor, va primero (y el resto de la cadena queda de respaldo).
    // Motivo medido: Ollama es gratis pero no aplica el schema, así que un artefacto estricto gasta
    // 50-60 s hasta que el validador lo rechaza y recién ahí escala. Lo decide el experto.
    // El plan sale del catálogo de motores. Si el experto declara su proveedor, sus motores van
    // primero; después el motor elegido por el usuario y, al final, el resto disponible. Con tope de
    // 3 intentos: cada escalada cuesta tokens y segundos.
    let cat = catalogo(&st).await;
    let mut plan: Vec<crate::motores::Motor> = Vec::new();
    let preferido = exp["proveedor"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if !preferido.is_empty() {
        plan.extend(
            cat.iter()
                .filter(|m| m.disponible && m.proveedor == preferido)
                .cloned(),
        );
    }
    if let Some(sel) = seleccion_efectiva(&st, &cat) {
        if let Some(m) = cat.iter().find(|m| m.id == sel && m.disponible) {
            plan.push(m.clone());
        }
    }
    for m in cat.iter().filter(|m| m.disponible) {
        if !plan.iter().any(|x| x.id == m.id) {
            plan.push(m.clone());
        }
    }
    plan.truncate(3);
    for m in plan {
        let t = std::time::Instant::now();
        let Some(llamada) = call_provider_cached(
            &st,
            &key,
            &m,
            &prompt,
            &schema,
            Some(&system),
            &id,
            false,
            body["texto"].as_str().or_else(|| body["prompt"].as_str()),
        )
        .await
        else {
            traza.push(json!({"motor": m.id, "proveedor": m.proveedor, "resultado": "sin respuesta", "ms": t.elapsed().as_millis()}));
            continue;
        };
        respondio_alguno = true;
        intentos += 1;
        let mut v = llamada.valor.clone();
        let mut consumo = llamada.consumo.clone();
        let mut estimado = llamada.estimado;
        let mut evitados = llamada.tokens_evitados;
        if llamada.cache {
            cache_hits += 1;
        }
        let mut p = crate::artefactos::validar(&tipo, &v);
        if !p.is_empty() {
            // Un reintento de reparación en el mismo proveedor: sale más barato que cambiar de modelo.
            let reintento = format!(
                "{prompt}\n\nTU RESPUESTA ANTERIOR FUE RECHAZADA POR EL VALIDADOR:\n{}\n\nCorregí exactamente eso y devolvé el JSON completo con TODOS los campos.",
                p.join("\n")
            );
            if let Some(segunda) = call_provider_cached(
                &st,
                &key,
                &m,
                &reintento,
                &schema,
                Some(&system),
                &id,
                false,
                None,
            )
            .await
            {
                let p2 = crate::artefactos::validar(&tipo, &segunda.valor);
                intentos += 1;
                consumo.sumar(&segunda.consumo);
                estimado = estimado || segunda.estimado;
                evitados += segunda.tokens_evitados;
                if segunda.cache {
                    cache_hits += 1;
                }
                if p2.len() < p.len() {
                    v = segunda.valor;
                    p = p2;
                }
            }
        }
        let valido = p.is_empty();
        consumo_total.sumar(&consumo);
        estimado_total = estimado_total || estimado;
        tokens_evitados += evitados;
        traza.push(json!({
            "motor": m.id, "proveedor": m.proveedor, "modelo": llamada.modelo, "valido": valido,
            "problemas": p.clone(), "ms": t.elapsed().as_millis(),
            "tokens": consumo.json(), "estimado": estimado,
            "costo_usd": st.tarifas.costo(&llamada.modelo, &m.proveedor, &consumo),
            "cache": if llamada.cache { "hit" } else { "miss" },
            "tokens_evitados": evitados,
        }));
        if proveedor.is_empty() || p.len() < problemas.len() {
            artefacto = v;
            problemas = p.clone();
            proveedor = format!("{} ({})", llamada.modelo, m.proveedor);
            modelo_final = llamada.modelo.clone();
            prov_final = m.proveedor.clone();
        }
        if valido {
            break; // cumple el contrato: no gastamos un llamado más
        }
        log::warn!(
            "experto: «{}» no cumplió el contrato ({p:?}); escalo al siguiente proveedor",
            llamada.modelo
        );
    }

    if !respondio_alguno {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({"ok": false, "error": "ningún proveedor de IA respondió"})),
        );
    }
    let valido = problemas.is_empty();
    // Fase 9 — costo del artefacto, medido o estimado (y declarado), nunca inventado.
    let costo_total = st.tarifas.costo(&modelo_final, &prov_final, &consumo_total);
    let costo_texto = crate::costo::texto_costo(
        &consumo_total,
        costo_total,
        estimado_total,
        cache_hits,
        tokens_evitados,
    );
    log::info!(
        "experto: «{}» sobre «{titulo}» → {tipo} · {} · {intentos} intento(s) · {} ms · {costo_texto}",
        exp["nombre"].as_str().unwrap_or(""),
        if valido { "válido" } else { "con problemas" },
        inicio.elapsed().as_millis()
    );

    (
        StatusCode::OK,
        Json(json!({
            "ok": valido,
            "tipo": tipo,
            "experto": exp["nombre"],
            "nodo": { "id": id, "titulo": titulo },
            "artefacto": artefacto,
            "texto": crate::artefactos::como_texto(&tipo, &artefacto),
            "problemas": problemas,
            "intentos": intentos,
            "proveedor": proveedor,
            "traza": traza,
            "ms": inicio.elapsed().as_millis(),
            "fuentes": fuentes,
            "contexto_chars": prompt.chars().count(),
            "uso": {
                "tokens": consumo_total.json(),
                "estimado": estimado_total,
                "costo_usd": costo_total,
                "cache": cache_hits,
                "tokens_evitados": tokens_evitados,
                // Identifica el estado medido con el que se generó: si el lienzo cambia, cambia.
                "huella_estado": huella,
            },
            "costo": costo_texto,
        })),
    )
}

#[cfg(test)]
mod tests_sello_local {
    use super::sello_local;

    #[test]
    fn el_sello_usa_la_hora_local_y_no_la_de_manana() {
        // 21:11 de Buenos Aires (UTC-3) es 00:11 del día siguiente en UTC: la nota tiene que decir 15.
        let epoch_utc = 1789517482; // 2026-09-16T00:11:22Z
        assert_eq!(sello_local(epoch_utc, -3), "2026-09-15T21:11");
        assert_eq!(
            sello_local(epoch_utc, 0),
            "2026-09-16T00:11",
            "con 0 el sello es UTC"
        );
        assert_eq!(sello_local(epoch_utc, 2), "2026-09-16T02:11");
    }

    #[test]
    fn el_sello_cruza_bien_el_cambio_de_mes() {
        let epoch_utc = 1788220800; // 2026-09-01T00:00:00Z
        assert_eq!(
            sello_local(epoch_utc, -3),
            "2026-08-31T21:00",
            "un turno de las 21 cae el mes anterior"
        );
    }
}

#[cfg(test)]
mod tests_modo {
    use super::cadena_por_modo;

    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn el_modo_local_fuerza_el_modelo_local() {
        assert_eq!(
            cadena_por_modo(Some("local"), v(&["gemini"])),
            v(&["ollama"])
        );
    }

    #[test]
    fn el_modo_nube_fuerza_el_proveedor_en_la_nube() {
        assert_eq!(
            cadena_por_modo(Some("nube"), v(&["ollama"])),
            v(&["gemini"])
        );
    }

    #[test]
    fn sin_modo_o_desconocido_se_usa_la_cadena_configurada() {
        let base = v(&["ollama", "gemini"]);
        assert_eq!(cadena_por_modo(None, base.clone()), base);
        assert_eq!(cadena_por_modo(Some("   "), base.clone()), base);
        assert_eq!(cadena_por_modo(Some("otro"), base.clone()), base);
    }

    #[test]
    fn los_alias_edge_y_cloud_no_distinguen_mayusculas_ni_espacios() {
        assert_eq!(cadena_por_modo(Some(" LOCAL "), vec![]), v(&["ollama"]));
        assert_eq!(cadena_por_modo(Some("Cloud"), vec![]), v(&["gemini"]));
    }
}

/// Intentos y espera del bind del puerto de la API.
const INTENTOS_BIND: u32 = 20;
const ESPERA_BIND_SEG: u64 = 6;

/// Bindeo con reintentos.
///
/// Al cerrar la app y reabrirla enseguida, Windows deja el puerto en `TIME_WAIT` (medido: 110 s
/// hasta quedar libre) y el bind falla con `os error 10048`. Con un solo intento la ventana
/// quedaba abierta pero **sin API**: parecía un cuelgue de la UI. Ahora reintenta hasta 2 minutos,
/// que cubre la espera medida, y deja el motivo en el log si igual no puede.
async fn bind_con_reintentos() -> std::io::Result<tokio::net::TcpListener> {
    let mut ultimo: Option<std::io::Error> = None;
    for intento in 1..=INTENTOS_BIND {
        match tokio::net::TcpListener::bind(("127.0.0.1", API_PORT)).await {
            Ok(listener) => {
                if intento > 1 {
                    log::info!(
                        "API: puerto {API_PORT} liberado en el intento {intento} ({}s de espera)",
                        ((intento - 1) as u64) * ESPERA_BIND_SEG
                    );
                }
                return Ok(listener);
            }
            Err(e) => {
                if intento == 1 {
                    log::warn!(
                        "puerto {API_PORT} ocupado ({e}); reintento cada {ESPERA_BIND_SEG}s hasta {INTENTOS_BIND} veces (TIME_WAIT tras cerrar la app)"
                    );
                }
                ultimo = Some(e);
                tokio::time::sleep(std::time::Duration::from_secs(ESPERA_BIND_SEG)).await;
            }
        }
    }
    Err(ultimo.unwrap_or_else(|| std::io::Error::other("bind: sin intentos ejecutados")))
}

#[cfg(test)]
mod tests_turno_ia {
    use super::{ia_tomar_en, IaTurno, IA_TURNO_TTL_S};
    use std::sync::{Arc, Mutex};

    fn claves(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    fn mapa() -> Arc<Mutex<std::collections::HashMap<String, f64>>> {
        Arc::new(Mutex::new(std::collections::HashMap::new()))
    }

    #[test]
    fn el_mismo_nodo_y_accion_no_se_pisa_y_otro_nodo_si_entra() {
        let m = mapa();
        assert!(ia_tomar_en(&m, &claves(&["ia:n1:explore"]), 100.0));
        assert!(
            !ia_tomar_en(&m, &claves(&["ia:n1:explore"]), 101.0),
            "el mismo nodo y acción no puede correr dos veces"
        );
        assert!(
            ia_tomar_en(&m, &claves(&["ia:n2:explore"]), 101.0),
            "otro nodo sí puede (salvo que comparta la clave de placa)"
        );
    }

    #[test]
    fn la_clave_de_placa_es_exclusiva() {
        let m = mapa();
        assert!(ia_tomar_en(
            &m,
            &claves(&["ia:placa", "ia:n1:explore"]),
            10.0
        ));
        assert!(
            !ia_tomar_en(&m, &claves(&["ia:placa", "ia:n2:critique"]), 11.0),
            "con la placa ocupada, ningún otro pedido local entra"
        );
    }

    #[test]
    fn un_turno_vencido_no_bloquea_para_siempre() {
        let m = mapa();
        assert!(ia_tomar_en(&m, &claves(&["ia:placa"]), 1000.0));
        assert!(!ia_tomar_en(
            &m,
            &claves(&["ia:placa"]),
            1000.0 + IA_TURNO_TTL_S - 1.0
        ));
        assert!(
            ia_tomar_en(&m, &claves(&["ia:placa"]), 1000.0 + IA_TURNO_TTL_S + 1.0),
            "una corrida muerta (kill, crash) no puede dejar la IA bloqueada"
        );
    }

    #[test]
    fn el_turno_se_suelta_al_salir_de_alcance() {
        // El `Drop` es lo que hace que un `return` temprano o un error no deje el turno tomado:
        // se toma el turno de verdad (inserta la clave) y se suelta al salir del bloque.
        let m = mapa();
        assert!(
            ia_tomar_en(&m, &claves(&["ia:placa"]), 200.0),
            "el turno se toma"
        );
        {
            let turno = IaTurno {
                mapa: m.clone(),
                claves: claves(&["ia:placa"]),
            };
            drop(turno);
        }
        assert!(
            ia_tomar_en(&m, &claves(&["ia:placa"]), 201.0),
            "tras soltar, la placa vuelve a estar libre"
        );
    }
}

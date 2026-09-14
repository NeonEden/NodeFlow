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
    /// Fase 12 — motores que fallaron por créditos o clave (id → motivo). Se aprende en la primera
    /// corrida: el catálogo los declara y los automáticos los saltean, en vez de elegir un motor muerto.
    pub motores_caidos: Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
}

impl AppState {
    /// Registra que un motor no está disponible (y por qué). Idempotente.
    pub fn marcar_motor_caido(&self, id: &str, motivo: &str) {
        if let Ok(mut m) = self.motores_caidos.lock() {
            m.insert(id.to_string(), motivo.to_string());
        }
    }

    pub fn motivos_de_motores(&self) -> std::collections::HashMap<String, String> {
        self.motores_caidos.lock().map(|m| m.clone()).unwrap_or_default()
    }
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
            motores_caidos: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
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
            .route("/api/voz/decir", post(voz_decir))
            .route("/api/ai/delegar", post(delegar).get(delegar_estado))
            .route("/api/ai/evaluar", post(ai_evaluar).get(ai_evaluar_leer))
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
            // Fase 4 — superficie para el agente (leer y escribir el lienzo)
            .route("/api/graph/summary", get(graph_summary))
            .route("/api/graph/node", post(graph_node))
            .route("/api/graph/edge", post(graph_edge))
            .route("/api/graph/node/delete", post(graph_delete))
            .route("/api/graph/prune", post(graph_prune))
            // Fase 7a — agente jardín: diagnóstico, arreglo propuesto y reacomodo por niveles
            .route("/api/graph/garden", get(graph_garden))
            .route("/api/graph/garden/fix", post(graph_garden_fix))
            .route("/api/graph/tidy", post(graph_tidy))
            // Fase 8 — captura de conocimiento y exportación
            // Slice 1 — Expertos y Contrato de Artefactos
            .route("/api/expertos", get(expertos_listar))
            .route("/api/expert/run", post(experto_run))
            .route("/api/knowledge/preview", post(knowledge_preview))
            .route("/api/knowledge/capture", post(knowledge_capture))
            .route("/api/export/document", get(export_document))
            .route("/api/export/json", get(export_json))
            // Fase 5a — el agente propone, el humano aprueba
            .route("/api/agent/pending", get(agent_pending))
            .route("/api/agent/approve", post(agent_approve))
            .route("/api/agent/reject", post(agent_reject))
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
        if let Err(e) = std::fs::write(&p, txt) {
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
    let activo = body["activo"].as_bool().unwrap_or_else(|| auto["activo"].as_bool().unwrap_or(false));
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
    log::info!("aprendizaje automático: {} (cada {cada} decisiones)", if activo { "encendido" } else { "apagado" });
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
        st,
        key,
        &prompt,
        &schema,
        None,
        "",
        None,
        "borrador", // rápido y gratis: es el bucle del lienzo
        false,      // los borradores sí usan caché
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
    log::info!("aprendizaje: perfil recalibrado con IA ({} decisiones acumuladas)", profile["totalDecisions"]);
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
                let ids: Vec<String> = st
                    .vault
                    .read_state()
                    .unwrap_or(serde_json::json!({}))["nodes"]
                    .as_array()
                    .map(|ns| {
                        ns.iter()
                            .filter_map(|n| n["id"].as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let mut limpio = crate::voz::validar(&value, &ids);
                // Escalada medida: la planilla mostró que hay pedidos que el motor local no resuelve
                // (devolvió 0 comandos). Antes de devolver un plan vacío, se le pide una vez al motor
                // de nube, en la misma corrida y sin que el usuario repita nada. Sólo se adopta si
                // trae algo: si tampoco, se respeta el resultado local y se informa.
                if limpio["comandos"].as_array().map(|c| c.is_empty()).unwrap_or(true) {
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
                        let trajo = alt["comandos"].as_array().map(|c| !c.is_empty()).unwrap_or(false);
                        if trajo {
                            log::info!(
                                "voz: el local no propuso nada → escaló a {} y trajo {} comandos",
                                llamada.modelo,
                                alt["comandos"].as_array().map(|a| a.len()).unwrap_or(0)
                            );
                            uso = llamada.uso_json(&st.tarifas);
                            limpio = alt;
                        } else {
                            log::info!("voz: tampoco la nube propuso nada; se devuelve el plan local");
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
            let d = if n["data"].is_object() { n["data"].clone() } else { n.clone() };
            format!("{}. {} — {}", i + 1, s(&d, "title"), s(&d, "description"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    ctx.insert("listaNodos".into(), lista_nodos);
    ctx.insert("cantidad".into(), selected.len().to_string());
    ctx.insert(
        "objetivo".into(),
        body["objetivo"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string(),
    );

    // Voz (Fase B): lo que dijo el usuario + el lienzo REAL con ids, para que el plan sólo pueda
    // referirse a nodos que existen. Se acota a 120 nodos para no inflar el prompt.
    if action == "voz" {
        ctx.insert("texto".into(), body["texto"].as_str().unwrap_or("").trim().to_string());
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
            .find_map(|n| v.get(n).and_then(|x| x.as_str()).map(|s| s.trim().to_string()))
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

/// Clave del motor: la propia del proveedor (variable de entorno o config). Nunca sale de acá.
fn clave_del_motor(st: &AppState, m: &crate::motores::Motor) -> Option<String> {
    match m.proveedor.as_str() {
        "gemini" => st.env_key.clone(),
        "openai" | "ollama" => {
            let nombre = m.clave_ref.clone()?;
            if let Ok(v) = std::env::var(&nombre) {
                if !v.trim().is_empty() {
                    return Some(v);
                }
            }
            let txt = std::fs::read_to_string(st.data_dir.join("nodeflow.config.json")).ok()?;
            let cfg: Value = serde_json::from_str(&txt).ok()?;
            cfg[&nombre].as_str().map(|s| s.to_string()).filter(|s| !s.trim().is_empty())
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
            ),
        );
    }
    log::info!(
        "ia: {provider} «{}» · {} tokens ({}) · {} ms · nodo «{nodo}» · clave {}",
        llamada.modelo,
        llamada.consumo.total(),
        if llamada.estimado { "estimado" } else { "medido" },
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
            let base = m.base_url.clone().unwrap_or_else(|| "http://localhost:11434/v1".to_string());
            let raiz_url = base.trim_end_matches("/v1").trim_end_matches('/').to_string();
            if let Some(r) = call_ollama_nativo(st, &raiz_url, &m.modelo, prompt, system, schema).await {
                return Some(r);
            }
            log::warn!("{}: sin respuesta por la API nativa; pruebo el camino compatible con OpenAI", m.id);
            call_ollama(st, Some((base, m.modelo.clone())), prompt, system).await
        }
        "gemini" => {
            // La clave del WebView (BYOK) tiene prioridad sobre la del entorno.
            let clave = if !key.trim().is_empty() { key.to_string() } else { clave_del_motor(st, m).unwrap_or_default() };
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
    let raiz_ollama = ollama_url.trim_end_matches("/v1").trim_end_matches('/').to_string();

    let mut v: Vec<Motor> = Vec::new();
    match st.http.get(format!("{raiz_ollama}/api/tags")).send().await {
        Ok(r) if r.status().is_success() => {
            let tags: Vec<String> = r
                .json::<Value>()
                .await
                .ok()
                .and_then(|j| j["models"].as_array().map(|a| a.iter().filter_map(|m| m["name"].as_str().map(String::from)).collect()))
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
        ..Motor::nuevo("gemini", CANDIDATE_MODELS[0], crate::motores::NUBE_PAGA, None, None)
    });

    // Proveedores compatibles con OpenAI declarados en el config (`proveedores`).
    if let Ok(txt) = std::fs::read_to_string(st.data_dir.join("nodeflow.config.json")) {
        if let Ok(cfg) = serde_json::from_str::<Value>(&txt) {
            for p in cfg["proveedores"].as_array().into_iter().flatten() {
                let (Some(modelo), Some(base)) = (p["modelo"].as_str(), p["base_url"].as_str()) else { continue };
                let clave_ref = p["clave_env"].as_str().map(String::from);
                let propia = clave_ref.clone().map(|n| std::env::var(&n).is_ok()).unwrap_or(false)
                    || p["clave_config"].as_str().and_then(|k| cfg[k].as_str()).is_some();
                v.push(Motor {
                    id: format!("openai:{}", p["id"].as_str().unwrap_or(modelo)),
                    etiqueta: p["etiqueta"].as_str().map(String::from)
                        .unwrap_or_else(|| format!("{modelo} · {}", p["id"].as_str().unwrap_or("api"))),
                    proveedor: "openai".into(),
                    modelo: modelo.to_string(),
                    donde: p["donde"].as_str().unwrap_or(crate::motores::NUBE_PAGA).to_string(),
                    base_url: Some(base.to_string()),
                    disponible: propia,
                    nota: (!propia).then(|| "falta la clave".to_string()),
                    clave_ref: p["clave_config"].as_str().map(String::from).or(clave_ref),
                });
            }
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
    let plan = crate::motores::plan_tarea(&cat, sel.as_deref(), modo, tarea, accion, planilla.as_ref());
    if plan.is_empty() {
        log::warn!("no hay ningún motor disponible (Ollama apagado y sin clave de nube)");
    }
    plan
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
) -> Option<crate::costo::Llamada> {
    let tarea = crate::motores::Tarea::de_accion(accion);
    for (i, m) in plan_de_motores(st, modo, tarea, accion)
        .await
        .into_iter()
        .enumerate()
    {
        if i > 0 {
            // Estamos en la red de seguridad: quedó registrado para poder medirlo después.
            log::info!("ruteo: {} no alcanzó, sigo con {}", tarea.etiqueta(), m.id);
        }
        if let Some(llamada) =
            call_provider_cached(st, key, &m, prompt, schema, system, nodo, sin_cache).await
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
    let donde = body["donde"].as_str().unwrap_or(crate::motores::NUBE_PAGA).to_string();
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
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "ok": false, "error": "config inválido" })));
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
        // La clave vive en el config local, junto al resto. Nunca viaja al frontend.
        obj.insert(clave_config.clone(), json!(clave));
        entrada["clave_config"] = json!(clave_config);
    }
    provs.push(entrada);
    obj.insert("proveedores".into(), Value::Array(provs));
    let txt = match serde_json::to_string_pretty(&cfg) {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "ok": false, "error": e.to_string() }))),
    };
    if let Err(e) = std::fs::write(&ruta, txt) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "ok": false, "error": e.to_string() })));
    }
    log::info!("proveedor agregado: {id} ({modelo}){}", if clave.is_empty() { " sin clave" } else { " con clave" });

    let cat = catalogo(&st).await;
    (
        StatusCode::OK,
        Json(json!({ "ok": true, "motores": cat, "seleccionado": crate::motores::seleccionado(&st.data_dir) })),
    )
}

/// `POST /api/ai/motor` — elige el motor de **toda la app** (o `auto` para volver a la cadena).
async fn ai_motor(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let id = body["id"].as_str();
    match crate::motores::guardar_seleccion(&st.data_dir, id) {
        Ok(_) => {
            let elegido = crate::motores::seleccionado(&st.data_dir);
            log::info!("motor de la app: {:?}", elegido.as_deref().unwrap_or("auto (cadena configurada)"));
            (StatusCode::OK, Json(json!({ "ok": true, "seleccionado": elegido })))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "ok": false, "error": e }))),
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
            std::env::var("NODEFLOW_OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434/v1".to_string())
        });
    let model = base_y_modelo
        .map(|(_, m)| m)
        .unwrap_or_else(|| {
            std::env::var("NODEFLOW_OLLAMA_MODEL").unwrap_or_else(|_| "nemotron-3-nano:30b-cloud".to_string())
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
        "temperature": 0.7
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
                    if codigo == 402 { "requiere créditos (HTTP 402)" } else { "clave rechazada (HTTP 401)" },
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
        "options": { "temperature": 0.7 }
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
                    if codigo == 402 { "requiere créditos (HTTP 402)" } else { "clave rechazada (HTTP 401)" },
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
    match st.http.post(&url).bearer_auth(clave).json(&body).send().await {
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

/// Días desde epoch → (año, mes, día). Algoritmo de Howard Hinnant.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
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

/// Lee el grafo canónico. Con `?since=<rev>` responde barato cuando nada cambió (polling).
/// Clave de Speechmatics: variable de entorno → `.env` del proyecto (dev) → `nodeflow.config.json`.
/// Nunca sale del backend y nunca se escribe en un log.
fn clave_speechmatics(st: &AppState) -> Option<String> {
    for var in ["SPEECHMATICS_API_KEY", "SPEECHMATICS_KEY"] {
        if let Ok(v) = std::env::var(var) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    for candidate in ["../.env", ".env"] {
        if let Ok(txt) = std::fs::read_to_string(candidate) {
            for line in txt.lines() {
                let line = line.trim();
                for campo in ["SPEECHMATICS_API_KEY=", "SPEECHMATICS_KEY="] {
                    if let Some(rest) = line.strip_prefix(campo) {
                        let v = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                        if !v.is_empty() {
                            log::info!("Speechmatics: clave leída desde {candidate}");
                            return Some(v);
                        }
                    }
                }
            }
        }
    }
    if let Ok(txt) = std::fs::read_to_string(st.data_dir.join("nodeflow.config.json")) {
        if let Ok(v) = serde_json::from_str::<Value>(&txt) {
            for campo in ["speechmatics_api_key", "SPEECHMATICS_API_KEY"] {
                if let Some(k) = v[campo].as_str() {
                    let k = k.trim().to_string();
                    if !k.is_empty() {
                        log::info!("Speechmatics: clave leída desde nodeflow.config.json");
                        return Some(k);
                    }
                }
            }
        }
    }
    None
}

/// `GET /api/voz/estado` — si la voz está lista, sin exponer nunca la clave.
async fn voz_estado(State(st): State<AppState>) -> impl IntoResponse {
    let configurada = clave_speechmatics(&st).is_some();
    let (url, modelo, idioma) = crate::voz::ajustes();
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
    Json(json!({
        "success": true,
        "configurada": configurada,
        "proveedor": "Speechmatics",
        "url": url,
        "modelo": modelo,
        "idioma": idioma,
        "codec": "pcm_s16le 16000 Hz",
        "tts": { "disponible": tts_disponible, "url": tts_url, "motor": "Kokoro (local)" },
        "pista": if configurada {
            "Clave presente. El token temporal se pide a /api/voz/jwt."
        } else {
            "Falta la clave: SPEECHMATICS_API_KEY en el entorno, o \"speechmatics_api_key\" en nodeflow.config.json."
        }
    }))
}

/// `POST /api/ai/evaluar` — corre la planilla sobre los motores pedidos (por defecto, los locales).
///
/// Tarda minutos, así que **no bloquea**: arranca en segundo plano y el frontend consulta el estado.
/// `GET` devuelve la última planilla guardada y si hay una corrida en curso.
async fn ai_evaluar(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let pedidos: Vec<String> = body["motores"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
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
            let titulos = st2
                .vault
                .read_state()
                .unwrap_or(json!({}))["nodes"]
                .as_array()
                .map(|ns| {
                    ns.iter()
                        .filter_map(|n| n["data"]["title"].as_str().map(String::from))
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();
            let prompt = crate::voz::prompt_delegar(&pedido2, &titulos);
            let exe = crate::voz::hermes_exe();
            move || correr_proceso(&exe, &prompt, std::time::Duration::from_secs(300))
        })
        .await;
        let (ok, texto) = match salida {
            Ok(Ok(t)) => (true, t),
            Ok(Err(e)) => (false, e),
            Err(e) => (false, format!("no pude correr el motor profundo: {e}")),
        };
        let ms = t0.elapsed().as_millis() as u64;
        let _ = crate::voz::guardar_delegacion(&st2.data_dir, &pedido2, ok, &texto, ms);
        log::info!("motor profundo: {} · {} caracteres", if ok { "listo" } else { "falló" }, texto.chars().count());
    });
    (
        StatusCode::OK,
        Json(json!({ "success": true, "corriendo": true, "pedido": pedido })),
    )
        .into_response()
}

/// `GET /api/ai/delegar` — ¿sigue investigando? ¿qué respondió?
async fn delegar_estado(State(st): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "success": true,
        "corriendo": crate::voz::delegacion_en_curso(&st.data_dir),
        "resultado": crate::voz::leer_delegacion(&st.data_dir),
    }))
}


/// Corre un proceso y devuelve su salida con tope de tiempo. En Windows se lanza **sin consola**:
/// nada de ventanas apareciendo mientras la app trabaja.
fn correr_proceso(exe: &str, arg: &str, tope: std::time::Duration) -> Result<String, String> {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(exe);
    cmd.arg("-z")
        .arg(arg)
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
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            Err(e) => return Err(format!("Error esperando al motor profundo: {e}")),
        }
    }
    let salida = hijo
        .wait_with_output()
        .map_err(|e| format!("No pude leer la respuesta: {e}"))?;
    let crudo = String::from_utf8_lossy(&salida.stdout);
    // Hermes puede avisar cosas al arrancar (gateway viejo, actualizaciones): eso no es la respuesta.
    let texto: String = crudo
        .lines()
        .skip_while(|l| {
            let l = l.trim();
            l.is_empty() || l.starts_with('⚠') || l.starts_with("Gateways") || l.starts_with("Run `hermes")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if texto.is_empty() {
        let err = String::from_utf8_lossy(&salida.stderr);
        return Err(format!(
            "El motor profundo no devolvió texto. {}",
            err.lines().last().unwrap_or("").trim()
        ));
    }
    Ok(texto)
}

/// `POST /api/voz/decir` — sintetiza una frase con la voz LOCAL (Kokoro) y devuelve el WAV.
///
/// La voz vive en la máquina del usuario (`tools/tts/servidor.py`, puerto 8125): sin cuotas, sin
/// mandar el texto a ningún servicio. Si no está corriendo, la app sigue andando y lo dice claro.
async fn voz_decir(State(st): State<AppState>, Json(body): Json<Value>) -> axum::response::Response {
    use axum::body::Body;
    use axum::response::Response;

    let responder_json = |codigo: StatusCode, mensaje: String| -> Response {
        Response::builder()
            .status(codigo)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({ "success": false, "error": mensaje }).to_string()))
            .unwrap()
    };

    let texto = body["texto"].as_str().unwrap_or("").trim().to_string();
    if texto.is_empty() {
        return responder_json(StatusCode::BAD_REQUEST, "Falta el texto a decir.".into());
    }
    let url = format!("{}/decir", crate::voz::tts_url());
    match st
        .http
        .post(&url)
        .json(&json!({ "texto": texto, "voz": body["voz"] }))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => match r.bytes().await {
            Ok(audio) => {
                log::info!("voz: {} caracteres sintetizados ({} KB)", texto.chars().count(), audio.len() / 1024);
                Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "audio/wav")
                    .body(Body::from(audio))
                    .unwrap()
            }
            Err(e) => responder_json(StatusCode::BAD_GATEWAY, format!("Respuesta de voz inválida: {e}")),
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
async fn voz_jwt(State(st): State<AppState>) -> impl IntoResponse {
    let Some(clave) = clave_speechmatics(&st) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "Falta la clave de Speechmatics (SPEECHMATICS_API_KEY o \"speechmatics_api_key\" en nodeflow.config.json)."
            })),
        );
    };
    let pedido = st
        .http
        .post("https://mp.speechmatics.com/v1/api_keys?type=rt")
        .header("Authorization", format!("Bearer {clave}"))
        .json(&json!({ "ttl": 300 }))
        .send()
        .await;
    match pedido {
        Ok(r) => {
            let code = r.status();
            let body: Value = r.json().await.unwrap_or(json!({}));
            if !code.is_success() {
                let detalle = body["error"].as_str().or(body["message"].as_str()).unwrap_or("sin detalle");
                log::warn!("voz: Speechmatics rechazó la petición de token ({code})");
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({ "success": false, "error": format!("Speechmatics rechazó la clave ({code}): {detalle}") })),
                );
            }
            let jwt = body["key_value"].as_str().unwrap_or("").to_string();
            if jwt.is_empty() {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({ "success": false, "error": "Speechmatics no devolvió token temporal." })),
                );
            }
            let (url, modelo, idioma) = crate::voz::ajustes();
            log::info!("voz: token temporal emitido (300 s)");
            (
                StatusCode::OK,
                Json(json!({ "success": true, "jwt": jwt, "url": url, "modelo": modelo, "idioma": idioma, "expira_en_s": 300 })),
            )
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "success": false, "error": format!("No pude hablar con Speechmatics: {e}") })),
        ),
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
            Json(json!({ "ok": false, "error": "el texto no produjo candidatos (cada bloque necesita densidad mínima)" })),
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
            Some(v) => (Some(v.clone()), "simulado (hook de verificación)".to_string(), 0u128),
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
async fn call_borrador_local(st: &AppState, titulo: &str, cuerpo: &str) -> Option<(Value, String, u128)> {
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
    hechos = format!("- fecha: {}\n- huella del estado medido: {huella}\n{hechos}", hoy());

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
    let preferido = exp["proveedor"].as_str().unwrap_or("").trim().to_lowercase();
    if !preferido.is_empty() {
        plan.extend(cat.iter().filter(|m| m.disponible && m.proveedor == preferido).cloned());
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
        let Some(llamada) =
            call_provider_cached(&st, &key, &m, &prompt, &schema, Some(&system), &id, false).await
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
            if let Some(segunda) =
                call_provider_cached(&st, &key, &m, &reintento, &schema, Some(&system), &id, false).await
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
mod tests_modo {
    use super::cadena_por_modo;

    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn el_modo_local_fuerza_el_modelo_local() {
        assert_eq!(cadena_por_modo(Some("local"), v(&["gemini"])), v(&["ollama"]));
    }

    #[test]
    fn el_modo_nube_fuerza_el_proveedor_en_la_nube() {
        assert_eq!(cadena_por_modo(Some("nube"), v(&["ollama"])), v(&["gemini"]));
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

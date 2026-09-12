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
}

// ─────────────────────────────────────────────────────────────────────────────
// Arranque
// ─────────────────────────────────────────────────────────────────────────────

pub fn spawn(
    data_dir: PathBuf,
    env_key: Option<String>,
    vault: Arc<Vault>,
    memoria: Arc<Memoria>,
) {
    tauri::async_runtime::spawn(async move {
        let state = AppState {
            data_dir,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            env_key,
            vault,
            memoria,
        };

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = Router::new()
            .route("/api/health", get(health))
            .route("/api/hitl/preferences", get(hitl_preferences))
            .route("/api/hitl/feedback", post(hitl_feedback))
            .route("/api/hitl/profile", post(hitl_set_profile))
            .route("/api/hitl/recalibrate", post(hitl_recalibrate))
            .route("/api/hitl/reset", post(hitl_reset))
            .route("/api/ai/action", post(ai_action))
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
            // Fase 5a — el agente propone, el humano aprueba
            .route("/api/agent/pending", get(agent_pending))
            .route("/api/agent/approve", post(agent_approve))
            .route("/api/agent/reject", post(agent_reject))
            .with_state(state)
            .layer(cors);

        match tokio::net::TcpListener::bind(("127.0.0.1", API_PORT)).await {
            Ok(listener) => {
                log::info!("NodeFlow API escuchando en http://127.0.0.1:{API_PORT}");
                if let Err(e) = axum::serve(listener, app).await {
                    log::error!("Servidor API detenido: {e}");
                }
            }
            Err(e) => log::error!("No pude bindear el puerto {API_PORT}: {e}"),
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

async fn hitl_feedback(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let action = body["action"].as_str().unwrap_or("").to_string();
    if action.is_empty() || body.get("human_decision").map(|v| v.is_null()).unwrap_or(true) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Estructura de evento feedback inválida" })),
        );
    }

    let current = get_profile(&st.data_dir);
    let decision = &body["human_decision"];
    let arr = |v: &Value| -> Vec<String> {
        v.as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
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
    let mut cats: Vec<String> = current["categoriesAccepted"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).map(String::from).collect()).unwrap_or_default();
    let mut topics: Vec<String> = current["topicsRejected"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).map(String::from).collect()).unwrap_or_default();
    // Solo se persisten señales que pasan el saneo (largo 5..60, sin control chars, con alfanumérico).
    for s in accepted.iter().chain(added.iter()) {
        if let Some(item) = sanitize_item(s) {
            if !cats.iter().any(|x| x.eq_ignore_ascii_case(&item)) { cats.push(item); }
        } else {
            log::info!("HITL: feedback descartado por saneo: {s:?}");
        }
    }
    for s in rejected.iter() {
        if let Some(item) = sanitize_item(s) {
            if !topics.iter().any(|x| x.eq_ignore_ascii_case(&item)) { topics.push(item); }
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

    let updated = json!({
        "version": "2.0",
        "updatedAt": now_iso(),
        "totalDecisions": total_decisions,
        "acceptanceRate": acceptance_rate,
        "learnedProfile": learned,
        "categoriesAccepted": cats.split_off(cats.len().saturating_sub(15)),
        "topicsRejected": topics.split_off(topics.len().saturating_sub(15)),
        "recentFeedback": history
    });

    save_profile(&st.data_dir, &updated);
    (StatusCode::OK, Json(json!({ "success": true, "profile": updated })))
}

async fn hitl_set_profile(State(st): State<AppState>, Json(body): Json<Value>) -> impl IntoResponse {
    let Some(learned) = body["learnedProfile"].as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "El perfil aprendido debe ser un texto válido" })));
    };
    let mut profile = get_profile(&st.data_dir);
    profile["learnedProfile"] = json!(learned);
    profile["updatedAt"] = json!(now_iso());
    save_profile(&st.data_dir, &profile);
    (StatusCode::OK, Json(json!({ "success": true, "profile": profile })))
}

async fn hitl_recalibrate(State(st): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
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

    if !sample.is_empty() {
        if let Some(key) = resolve_key(&st, &headers) {
            let prompt = format!(
                "Analiza estas decisiones recientes de curaduría de un usuario en un mapa mental (HITL Loop):\n{}\n\nSintetiza un perfil de estilo y preferencia cognitiva de 2 o 3 oraciones contundentes para inyectar en el system prompt.\nEjemplo: \"El usuario prefiere un enfoque técnico, conciso y estructurado. Suele descartar conexiones genéricas y favorece patrones de arquitectura, código y filosofía pragmática.\"\nResponde en formato JSON:\n{{\"profile\": \"El usuario prefiere...\"}}",
                serde_json::to_string_pretty(&sample).unwrap_or_default()
            );
            let schema = json!({
                "type": "OBJECT",
                "properties": { "profile": { "type": "STRING" } },
                "required": ["profile"]
            });
            if let Some((parsed, _)) = call_model(&st, &key, &prompt, &schema, None).await {
                if let Some(p) = parsed["profile"].as_str() {
                    profile["learnedProfile"] = json!(p);
                    profile["updatedAt"] = json!(now_iso());
                    save_profile(&st.data_dir, &profile);
                    return Json(json!({ "success": true, "profile": profile, "calibratedWithAi": true }));
                }
            }
        }
    }

    let last3: Vec<String> = profile["categoriesAccepted"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).rev().take(3).collect::<Vec<_>>().into_iter().rev().map(String::from).collect())
        .unwrap_or_default();
    let cats = if last3.is_empty() { "arquitectura y sistemas".to_string() } else { last3.join(", ") };
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

async fn ai_action(State(st): State<AppState>, headers: HeaderMap, Json(body): Json<Value>) -> impl IntoResponse {
    let action_type = body["type"].as_str().unwrap_or("").to_string();
    let specs: Value = serde_json::from_str(SPECS).unwrap_or(json!({}));

    // resolver alias (critique | devils_advocate)
    let mut spec = specs.get(&action_type).cloned();
    if spec.is_none() {
        for (_, s) in specs.as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>()).unwrap_or_default() {
            let is_alias = s["aliases"].as_array().map(|a| a.iter().any(|x| x.as_str() == Some(action_type.as_str()))).unwrap_or(false);
            if is_alias {
                spec = Some(s);
                break;
            }
        }
    }
    let Some(spec) = spec else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "success": false, "error": format!("Tipo de acción desconocido: {action_type}") })));
    };

    let profile = get_profile(&st.data_dir);
    let override_txt = body["hitlProfileOverride"].as_str();
    let system_instruction = build_hitl_system_instruction(&profile, override_txt);
    // Fase 6: la bóveda del usuario entra al prompt como contexto del nodo.
    let context = build_context(&action_type, &body, Some((st.vault.as_ref(), st.memoria.as_ref())));
    let prompt = fill_template(spec["prompt"].as_str().unwrap_or(""), &context);
    let schema = spec["schema"].clone();
    let resp_key = spec["response"]["key"].as_str().unwrap_or("variations").to_string();
    let nested = spec["response"].get("nested").and_then(|v| v.as_str()).map(String::from);

    let called = match resolve_key(&st, &headers) {
        Some(key) if !prompt.is_empty() && !schema.is_null() => {
            call_model(&st, &key, &prompt, &schema, Some(&system_instruction)).await
        }
        _ => None,
    };

    let (payload, model_used, used_ai) = match called {
        Some((parsed, model)) => {
            let value = match &nested {
                Some(k) => parsed[k].clone(),
                None => parsed.clone(),
            };
            let ok = match &value {
                Value::Array(a) => !a.is_empty(),
                Value::Object(o) => !o.is_empty(),
                _ => false,
            };
            if ok { (value, model, true) } else { (fallback_for(&action_type, &spec, &context), "fallback".to_string(), false) }
        }
        None => (fallback_for(&action_type, &spec, &context), "fallback".to_string(), false),
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
        out["source"] = json!("gemini");
    }
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
    let consulta = format!(
        "{titulo} {}",
        desc.chars().take(300).collect::<String>()
    );
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
    let mut boost_por_slug: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
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

    ctx.insert("title".into(), if node["title"].is_null() { "Idea Central".into() } else { s(node, "title") });
    ctx.insert("description".into(), if node["description"].is_null() { "Sin descripción".into() } else { s(node, "description") });
    ctx.insert("rawText".into(), body["rawText"].as_str().unwrap_or("").to_string());

    let selected = body["selectedNodes"].as_array().cloned().unwrap_or_default();
    for (i, alias) in ["nodeA", "nodeB"].iter().enumerate() {
        let n = selected.get(i).cloned().unwrap_or(json!({}));
        let data = if n["data"].is_object() { n["data"].clone() } else { n.clone() };
        ctx.insert(format!("{alias}.title"), s(&data, "title"));
        ctx.insert(format!("{alias}.description"), s(&data, "description"));
    }

    // nodos: soporta tanto {id,data:{...}} como {id,title,...}
    let nodes = body["nodes"].as_array().cloned().unwrap_or_default();
    let norm = |n: &Value| -> (String, String, String, Vec<String>) {
        let d = if n["data"].is_object() { &n["data"] } else { n };
        let cat = d["category"].as_str().or_else(|| d["label"].as_str()).unwrap_or("Concepto").to_string();
        (
            d["title"].as_str().unwrap_or("Sin título").to_string(),
            d["description"].as_str().unwrap_or("").to_string(),
            cat,
            d["tags"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).map(String::from).collect()).unwrap_or_default(),
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
                let (src, tgt) = (e["source"].as_str().unwrap_or(""), e["target"].as_str().unwrap_or(""));
                conns.push(format!("{src}->{tgt}"));
                conns.push(format!("{tgt}->{src}"));
            }
            conns.truncate(30);
            ctx.insert("Array.from(existingConnections).slice(0, 30).join(\", \")".into(), conns.join(", "));

            let summaries: Vec<Value> = nodes
                .iter()
                .map(|n| {
                    let (t, d, c, _) = norm(n);
                    json!({ "id": n["id"], "title": t, "category": c, "description": d })
                })
                .collect();
            ctx.insert("nodeSummaries".into(), serde_json::to_string_pretty(&summaries).unwrap_or_default());
            ctx.insert("JSON.stringify(nodeSummaries, null, 2)".into(), serde_json::to_string_pretty(&summaries).unwrap_or_default());
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
            ctx.insert("JSON.stringify(nodeSummaries, null, 2)".into(), summaries.join("\n"));
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
                log::info!("   · {} ({})", f["ruta"].as_str().unwrap_or(""), f["titulo"].as_str().unwrap_or(""));
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

fn fill_template(tpl: &str, ctx: &std::collections::HashMap<String, String>) -> String {
    let mut out = String::with_capacity(tpl.len() + 512);
    let bytes: Vec<char> = tpl.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == '{' && bytes[i + 1] == '{' {
            if let Some(close) = (i + 2..bytes.len().saturating_sub(1)).find(|&j| bytes[j] == '}' && bytes[j + 1] == '}') {
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
fn fallback_for(action: &str, spec: &Value, ctx: &std::collections::HashMap<String, String>) -> Value {
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
        let title = if clean.is_empty() { "Idea nuclear".to_string() } else { clean.chars().take(60).collect::<String>() };
        let desc = if clean.len() > 80 { format!("{}...", clean.chars().take(160).collect::<String>()) } else { "Idea nuclear sintetizada a partir del volcado de pensamiento.".to_string() };
        let segs: Vec<String> = clean
            .split(['.', '\n', ';'])
            .map(|s| s.trim().to_string())
            .filter(|s| s.len() > 3)
            .collect();
        let cats = ["ESTRATEGIA", "ARQUITECTURA", "EJECUCIÓN", "VALIDACIÓN", "MÉTRICAS"];
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

/// Llama al proveedor configurado siguiendo una cadena de intentos.
///
/// Cadena por defecto: Ollama Cloud (gratis, vía el daemon local) → Gemini (fallback).
/// Se puede cambiar con `NODEFLOW_AI_CHAIN=ollama,gemini` o `NODEFLOW_AI_CHAIN=gemini`.
async fn call_model(st: &AppState, key: &str, prompt: &str, schema: &Value, system: Option<&str>) -> Option<(Value, String)> {
    let chain = std::env::var("NODEFLOW_AI_CHAIN")
        .or_else(|_| std::env::var("NODEFLOW_AI_PROVIDER"))
        .unwrap_or_else(|_| "ollama,gemini".to_string());

    for provider in chain.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) {
        let attempt = match provider.as_str() {
            "ollama" => call_ollama(st, prompt, system).await,
            "gemini" => {
                if key.is_empty() {
                    None
                } else {
                    call_gemini(st, key, prompt, schema, system).await
                }
            }
            other => {
                log::warn!("proveedor desconocido en la cadena: {other}");
                None
            }
        };
        if attempt.is_some() {
            return attempt;
        }
        log::warn!("proveedor '{provider}' no respondió; paso al siguiente de la cadena");
    }
    None
}

async fn call_gemini(st: &AppState, key: &str, prompt: &str, schema: &Value, system: Option<&str>) -> Option<(Value, String)> {
    for model in CANDIDATE_MODELS {
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");
        let mut body = json!({
            "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
            "generationConfig": { "responseMimeType": "application/json", "responseSchema": schema }
        });
        if let Some(sys) = system {
            body["systemInstruction"] = json!({ "parts": [{ "text": sys }] });
        }

        match st.http.post(&url).header("x-goog-api-key", key).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                let value: Value = match resp.json().await {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let text = value["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("");
                if let Some(parsed) = parse_json_text(text) {
                    return Some((parsed, model.to_string()));
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

async fn call_ollama(st: &AppState, prompt: &str, system: Option<&str>) -> Option<(Value, String)> {
    let base = std::env::var("NODEFLOW_OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
    let model = std::env::var("NODEFLOW_OLLAMA_MODEL").unwrap_or_else(|_| "nemotron-3-nano:30b-cloud".to_string());
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

    match st.http.post(format!("{base}/chat/completions")).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            let v: Value = resp.json().await.ok()?;
            let text = v["choices"][0]["message"]["content"].as_str().unwrap_or("");
            parse_json_text(text).map(|p| (p, format!("{model} (ollama)")))
        }
        Ok(resp) => {
            log::warn!("Ollama HTTP {}", resp.status());
            None
        }
        Err(e) => {
            log::warn!("Ollama error de red: {e}");
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

fn now_iso() -> String {
    // ISO-8601 UTC sin dependencias extra (formato suficiente para el cliente)
    let ms = now_ms();
    let secs = ms / 1000;
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}.{:03}Z", ms % 1000)
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
            (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e })))
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
            (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e })))
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
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"ok": false, "error": e}))),
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

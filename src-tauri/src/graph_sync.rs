//! OneDrive / Microsoft Graph sync utilities – minimal stub for step 6
//!
//! The real implementation will use the Microsoft Graph REST API (HTTPS) with OAuth2
//! device‑code flow (or auth‑code) to obtain an access token that has the scopes
//! `Files.ReadWrite.All` and `offline_access`. The token is cached in the vault
//! (JSON file) so that the background sync can run without prompting the user.
//!
//! This stub provides the public API expected by the server routes:
//!
//! * `pub async fn start_sync(state: &mut AppState, folder: &str) -> Result<String, String>`
//! * `pub async fn stop_sync(state: &mut AppState) -> Result<(), String>`
//! * `pub fn sync_status(state: &AppState) -> SyncStatus`
//!
//! The heavy‑lifting (HTTP calls, delta queries, PDF → MD conversion, embedding)
//! will be implemented later. For now the functions only record the requested
//! folder in the vault and report success.

use crate::cerebro::AppState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncStatus {
    pub running: bool,
    pub folder: Option<String>,
    pub last_error: Option<String>,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            running: false,
            folder: None,
            last_error: None,
        }
    }
}

/// Starts a OneDrive sync for the given folder (relative to the vault root).
/// Returns a human‑readable message.
pub async fn start_sync(state: &mut AppState, folder: &str) -> Result<String, String> {
    // Store the requested folder in the vault under `cerebro.sync` (simple JSON).
    let mut sync = state.cerebro.sync.clone().unwrap_or_default();
    sync.running = true;
    sync.folder = Some(folder.to_string());
    sync.last_error = None;
    state.cerebro.sync = Some(sync);
    // Persist to disk (vault write). Errors are propagated.
    state
        .vault
        .escribir_nota("cerebro/sync.json", &serde_json::to_string_pretty(&state.cerebro.sync)?)
        .map_err(|e| format!("cannot persist sync config: {e}"))?;
    Ok(format!("OneDrive sync started for folder '{}'.", folder))
}

/// Stops the running sync.
pub async fn stop_sync(state: &mut AppState) -> Result<(), String> {
    if let Some(mut sync) = state.cerebro.sync.clone() {
        sync.running = false;
        sync.last_error = None;
        state.cerebro.sync = Some(sync);
        state
            .vault
            .escribir_nota("cerebro/sync.json", &serde_json::to_string_pretty(&state.cerebro.sync)?)
            .map_err(|e| format!("cannot persist sync config: {e}"))?;
        Ok(())
    } else {
        Err("no sync configured".into())
    }
}

/// Returns the current sync status.
pub fn sync_status(state: &AppState) -> SyncStatus {
    state.cerebro.sync.clone().unwrap_or_default()
}

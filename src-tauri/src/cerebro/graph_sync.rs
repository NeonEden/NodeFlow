use crate::cerebro::{AppState, SyncStatus};
use crate::cerebro::graph_client::TokenInfo;

use std::fs;

fn load_token(state: &AppState) -> Result<TokenInfo, String> {
    let path = state.vault.raiz().join("cerebro").join("graph_auth.json");
    let txt = fs::read_to_string(&path).map_err(|e| format!("cannot read token file: {e}"))?;
    let tok: TokenInfo = serde_json::from_str(&txt).map_err(|e| format!("token parse error: {e}"))?;
    if !tok.is_valid() {
        return Err("token expired – refresh not implemented".into());
    }
    Ok(tok)
}

/// Starts OneDrive sync for a given folder (relative to the drive root).
pub fn start_sync(state: &mut AppState, folder: &str) -> Result<String, String> {
    // Verify we have a valid token (loads and checks expiry).
    let _ = load_token(state)?;
    // Persist sync config.
    let mut sync = state.cerebro.sync.clone().unwrap_or_default();
    sync.running = true;
    sync.folder = Some(folder.to_string());
    sync.last_error = None;
    state.cerebro.sync = Some(sync.clone());
    let txt = serde_json::to_string_pretty(&sync).map_err(|e| e.to_string())?;
    state
        .vault
        .escribir_nota("cerebro/sync.json", &txt)
        .map_err(|e| format!("cannot persist sync config: {e}"))?;
    Ok(format!("OneDrive sync started for folder '{}'.", folder))
}

/// Stops the running sync.
pub fn stop_sync(state: &mut AppState) -> Result<(), String> {
    if let Some(mut sync) = state.cerebro.sync.clone() {
        sync.running = false;
        sync.last_error = None;
        state.cerebro.sync = Some(sync.clone());
        let txt = serde_json::to_string_pretty(&sync).map_err(|e| e.to_string())?;
        state
            .vault
            .escribir_nota("cerebro/sync.json", &txt)
            .map_err(|e| format!("cannot persist sync config: {e}"))?;
        Ok(())
    } else {
        Err("no sync configured".into())
    }
}

/// Returns current sync status.
pub fn sync_status(state: &AppState) -> SyncStatus {
    state.cerebro.sync.clone().unwrap_or_default()
}

mod grafo;
mod memoria;
mod server;
mod vault;

use std::path::PathBuf;
use tauri::Manager;

/// La API key puede venir del entorno o del `.env` del proyecto (dev) — nunca del WebView.
fn read_env_key() -> Option<String> {
    if let Ok(k) = std::env::var("GEMINI_API_KEY") {
        let k = k.trim().to_string();
        if !k.is_empty() {
            return Some(k);
        }
    }
    for candidate in ["../.env", ".env"] {
        if let Ok(txt) = std::fs::read_to_string(candidate) {
            for line in txt.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("GEMINI_API_KEY=") {
                    let v = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                    if !v.is_empty() {
                        log::info!("GEMINI_API_KEY leída desde {candidate}");
                        return Some(v);
                    }
                }
            }
        }
    }
    None
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Datos de la app: %APPDATA%\<identifier> (estable entre versiones)
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("./data"));
            let _ = std::fs::create_dir_all(&data_dir);

            let key = read_env_key();
            log::info!(
                "NodeFlow arrancando — data_dir={} · api_key={}",
                data_dir.display(),
                if key.is_some() { "presente" } else { "ausente" }
            );

            // Fase 3: el vault en disco es la fuente de verdad (y se observa para cambios externos)
            let vault = vault::Vault::new(&data_dir);
            vault::start_watcher(vault.clone());
            // Fase 5b: memoria semántica de la bóveda (se indexa en el primer uso)
            let memoria = memoria::Memoria::new(&data_dir);
            server::spawn(data_dir, key, vault, memoria);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

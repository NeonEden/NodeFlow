mod artefactos;
mod borrador;
mod conocimiento;
mod costo;
mod expertos;
mod grafo;
mod memoria;
mod motores;
mod eval;
mod semantica;
mod stt;
mod dialogo;
mod investigacion;
mod server;
mod vault;
mod voz;

use std::path::PathBuf;
use tauri::Manager;

/// La API key puede venir (en este orden): del entorno, del `.env` del proyecto (modo desarrollo), o
/// del `nodeflow.config.json` de la app instalada. Nunca del WebView.
fn read_env_key(data_dir: &std::path::Path) -> Option<String> {
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
    // App instalada: su propio config en %APPDATA%. Es el camino que hace que el .exe suelto tenga IA
    // sin depender de variables de entorno ni de la carpeta del proyecto.
    let cfg = data_dir.join("nodeflow.config.json");
    if let Ok(txt) = std::fs::read_to_string(&cfg) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
            for campo in ["gemini_api_key", "GEMINI_API_KEY"] {
                if let Some(k) = v[campo].as_str() {
                    let k = k.trim().to_string();
                    if !k.is_empty() {
                        log::info!("GEMINI_API_KEY leída desde nodeflow.config.json");
                        return Some(k);
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
        // Guardián de instancia única: si ya hay una app abierta, esta la trae al frente y termina.
        // Sin esto, un segundo clic en el acceso directo arrancaba una app SIN backend (el puerto
        // 37371 ya está tomado) y mostraba el lienzo vacío — peor que no abrir nada.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        // Updater: la app puede buscarse, descargar y aplicar una version nueva firmada.
        // La clave PUBLICA vive en tauri.conf.json; la privada, en el perfil del usuario.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Log SIEMPRE activo, también en la app instalada: es la única forma de diagnosticar un
            // .exe suelto (en dev además sale por stdout, que es lo que leo yo).
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .targets([
                        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                            file_name: Some("NodeFlow".into()),
                        }),
                        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    ])
                    .build(),
            )?;

            // Datos de la app: %APPDATA%\<identifier> (estable entre versiones)
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("./data"));
            let _ = std::fs::create_dir_all(&data_dir);

            let key = read_env_key(&data_dir);
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

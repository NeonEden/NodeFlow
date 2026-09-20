mod agente;
mod artefactos;
mod azure;
mod borrador;
mod cerebro;
mod cerebro_arquitectura;
mod cerebro_gateway;
mod cerebro_tools;
mod claves;
mod conocimiento;
mod costo;
mod curador;
mod dialogo;
pub mod estado;
mod eval;
mod expertos;
mod grafo;
mod idioma;
mod investigacion;
mod memoria;
mod motores;
mod parche;
mod semantica;
mod server;
mod sesiones;
mod stt;
mod vault;
mod voz;
mod voz_local;

use std::path::PathBuf;
use tauri::{Emitter, Manager};

/// La API key puede venir (en este orden): del entorno, del `.env` del proyecto (modo desarrollo), o
/// del `nodeflow.config.json` de la app instalada. Nunca del WebView.
/// La API key de Gemini puede venir del entorno, del `.env` del proyecto (modo desarrollo), del
/// **llavero del sistema** o del config (legado). La resolución vive en `claves.rs`: es una sola para
/// toda la app, y el llavero gana sobre el archivo en texto plano.
fn read_env_key(data_dir: &std::path::Path) -> Option<String> {
    crate::claves::obtener("gemini_api_key", data_dir)
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

            // Guardián del build (15/09). Un build de DESARROLLO no embebe la interfaz: la sirve Vite
            // en `devUrl`. Si ese binario queda como app instalada (o se levanta a mano) y Vite no
            // está corriendo, la ventana muestra ERR_CONNECTION_REFUSED y **el log no decía nada**:
            // parecía que la app "no andaba" cuando el código estaba bien. En una app GUI de Windows
            // el archivo de log es la única superficie de diagnóstico, así que el build se declara solo.
            if tauri::is_dev() {
                let url = app
                    .config()
                    .build
                    .dev_url
                    .as_ref()
                    .map(|u| u.to_string())
                    .unwrap_or_else(|| "http://localhost:5173".into());
                log::warn!(
                    "build de DESARROLLO: la interfaz NO está embebida — la sirve {url}. Sin ese servidor \
                     corriendo la ventana queda con ERR_CONNECTION_REFUSED (arrancá `npm run dev:web`, o \
                     instalá un build de producción: `scripts/release.sh`). Diagnóstico: scripts/verificar-app.sh"
                );
            } else {
                log::info!("build de producción: la interfaz va embebida en el binario (no necesita Vite)");
            }

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

            // La voz local (Kokoro), si está descargada, se arranca sola: es lo que reemplaza al
            // `.vbs` de la máquina de desarrollo, que tenía una ruta absoluta dentro y no servía en
            // ninguna otra PC. Si no está instalada, no pasa nada: el panel usa la voz del sistema.
            if voz_local::instalada(&data_dir) && !voz_local::corriendo() {
                match voz_local::arrancar(&data_dir) {
                    Ok(m) => log::info!("voz local: {m}"),
                    Err(e) => log::warn!("voz local: {e}"),
                }
            }

            // Fase 3: el vault en disco es la fuente de verdad (y se observa para cambios externos)
            let vault = vault::Vault::new(&data_dir);
            vault::start_watcher(vault.clone());
            // Fase 5b: memoria semántica de la bóveda (se indexa en el primer uso)
            let memoria = memoria::Memoria::new(&data_dir);
            server::spawn(data_dir, key, vault, memoria);

            // Atajo global de dictado (Fase B, 20/09/2026): «abrir la app para hablar» era la última
            // fricción que quedaba. Ctrl+Shift+Space dicta desde donde estés y, al soltar, el turno se
            // corta y se procesa (push-to-talk, como los dictados del sistema). El frontend escucha
            // `voz-atajo` y abre el panel solo. Un atajo tomado por otro programa NO rompe el arranque.
            #[cfg(desktop)]
            {
                use tauri_plugin_global_shortcut::{
                    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
                };
                // Cadena de candidatas, no una tecla fija: `Ctrl+Shift+Space` ya está tomado en este
                // Windows (medido 20/09: «HotKey already registered», lo usa el propio sistema para
                // cambiar de método de entrada). Se registran TODAS las que estén libres y el handler
                // responde a cualquiera, así que el atajo existe aunque la primera no se pueda tomar. Un
                // atajo que no se registra en silencio es peor que no tenerlo: por eso cada intento queda
                // en el log con su veredicto.
                let candidatas: [(&str, Shortcut); 3] = [
                    (
                        "Ctrl+Alt+Space",
                        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space),
                    ),
                    (
                        "Ctrl+Shift+F9",
                        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F9),
                    ),
                    (
                        "Ctrl+Alt+V",
                        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyV),
                    ),
                ];
                let validas: Vec<Shortcut> = candidatas.iter().map(|(_, s)| s.clone()).collect();
                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(move |app, s, evento| {
                            if validas.iter().any(|v| v == s) {
                                let cual = match evento.state() {
                                    ShortcutState::Pressed => "pressed",
                                    ShortcutState::Released => "released",
                                };
                                log::info!("atajo de voz: {cual}");
                                let _ = app.emit("voz-atajo", cual);
                            }
                        })
                        .build(),
                )?;
                let mut libres = 0;
                for (nombre, atajo) in candidatas.iter() {
                    match app.global_shortcut().register(atajo.clone()) {
                        Ok(()) => {
                            libres += 1;
                            log::info!("atajo de voz listo: {nombre} (push-to-talk)");
                        }
                        Err(e) => log::warn!("no pude registrar {nombre}: {e}"),
                    }
                }
                if libres == 0 {
                    log::warn!(
                        "sin atajo de dictado: las {} candidatas estaban tomadas. El panel de voz sigue andando desde la app.",
                        candidatas.len()
                    );
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

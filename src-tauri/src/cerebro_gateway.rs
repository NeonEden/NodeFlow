//! Fase 4 del plan del cerebro residente: pasar de *delegar* a **habitar**.
//!
//! Hasta acá la app despertaba a Hermes como subproceso (`hermes chat -q`, una sola respuesta, con tope
//! de 240 s y sin ver nada hasta el final). Acá la app levanta **su propio gateway** (`hermes serve`,
//! JSON-RPC sobre WebSocket en loopback) y le habla en vivo: el panel ve el turno token por token, los
//! usos de herramientas y las **aprobaciones**, que se resuelven ahí mismo (con «aplicar a todo» para no
//! frenar cambios grandes).
//!
//! Protocolo verificado a mano el 15/09 contra un `hermes serve` real:
//! - URL `ws://127.0.0.1:<puerto>/api/ws?token=<token>` (el token es `HERMES_DASHBOARD_SESSION_TOKEN`).
//! - Pedidos JSON-RPC 2.0: `session.create` → `{session_id}` · `prompt.submit {session_id, text}`.
//! - Eventos: sobre `event` con `params.type` (`message.delta`, `thinking.delta`, `tool.start`,
//!   `message.complete{usage}`, `session.info`, `session.title`).
//! - El server también **pregunta**: pedidos con `method: "approval"` que el cliente contesta
//!   `{result:{choice:"once|session|always|deny", all:bool}}` (`all` = aplicar a todo lo pendiente).
//!
//! Decisiones: puerto propio (9121) para no pelear con el gateway del escritorio de Hermes (9119), token
//! generado en cada arranque de la app y **nunca** escrito en logs, y un `Drop` que se lleva el proceso
//! cuando la app se cierra (si no, quedaría un `hermes serve` huérfano — el bug clásico de esta casa).

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// Puerto propio del gateway de la app. 9119 es el del escritorio de Hermes: no se toca.
pub const PUERTO_POR_DEFECTO: u16 = 9121;

/// Cuánto se espera a que el gateway conteste `/api/health` antes de darlo por fallado.
pub const ESPERA_LISTO_S: u64 = 25;

/// El gateway de la app: un `hermes serve` propio, con su token, del que el panel vive.
pub struct Gateway {
    puerto: u16,
    token: String,
    proceso: Mutex<Option<Child>>,
    listo: AtomicBool,
}

/// Puerto libre desde `desde`: un `hermes serve` huérfano de una corrida anterior **sobrevive a un cierre
/// forzado** (el `Drop` no corre si matan el proceso), sigue escuchando y contesta el health — y entonces
/// el panel recibe 403 al conectar porque el token del huérfano es otro (medido 16/09: dos `serve` en el
/// 9121, el nuevo sin poder bindear). Elegir un puerto realmente libre evita quedar colgados de un zombie.
fn puerto_libre(desde: u16) -> u16 {
    for p in desde..desde.saturating_add(20) {
        if std::net::TcpListener::bind(("127.0.0.1", p)).is_ok() {
            return p;
        }
    }
    desde
}

impl Gateway {
    pub fn nuevo(puerto: u16) -> Self {
        let puerto = if puerto == 0 {
            PUERTO_POR_DEFECTO
        } else {
            puerto
        };
        let puerto = puerto_libre(puerto);
        Self {
            puerto,
            token: token_nuevo(),
            proceso: Mutex::new(None),
            listo: AtomicBool::new(false),
        }
    }

    pub fn puerto(&self) -> u16 {
        self.puerto
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// URL del WebSocket. El token viaja en la query porque el navegador no puede mandar headers.
    pub fn url_ws(&self) -> String {
        format!("ws://127.0.0.1:{}/api/ws?token={}", self.puerto, self.token)
    }

    pub fn listo(&self) -> bool {
        self.listo.load(Ordering::Relaxed)
    }

    pub fn vivo(&self) -> bool {
        self.proceso
            .lock()
            .map(|mut p| {
                p.as_mut()
                    .and_then(|c| c.try_wait().ok().flatten())
                    .is_none()
                    && p.is_some()
            })
            .unwrap_or(false)
    }

    /// Estado para el panel. El token se entrega porque la app es local y el WebView es propio; nunca
    /// se loguea.
    pub fn estado(&self) -> Value {
        json!({
            "listo": self.listo(),
            "vivo": self.vivo(),
            "puerto": self.puerto,
            "url": self.url_ws(),
        })
    }

    /// Levanta el gateway si no está levantado. Devuelve enseguida: la espera de arranque va en un hilo.
    pub fn arrancar(&self, exe: &str, cwd: &Path) -> Result<(), String> {
        if self.vivo() {
            return Ok(());
        }
        let mut cmd = Command::new(exe);
        argv(&mut cmd, self.puerto);
        // **Nunca `piped()` para un proceso largo sin lector**: un `hermes serve` escribe su log de
        // arranque y, al llenarse el buffer del pipe (~64 KB), el hijo **se bloquea escribiendo** y
        // nunca llega a contestar `/api/health`. Síntoma medido: `vivo: true` y `listo: false` para
        // siempre, y el panel esperando el gateway que ya estaba trabado. La salida del gateway no se
        // usa: va a null. (En `cerebro::correr` sí hay pipe, pero ahí se lee al terminar.)
        cmd.current_dir(cwd)
            .env("HERMES_DASHBOARD_SESSION_TOKEN", &self.token)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: el gateway no abre consola
        }
        let hijo = cmd
            .spawn()
            .map_err(|e| format!("no pude levantar el gateway del cerebro ({exe}): {e}"))?;
        *self
            .proceso
            .lock()
            .map_err(|_| "gateway bloqueado".to_string())? = Some(hijo);
        Ok(())
    }

    /// Marca `listo` cuando `/api/health` contesta. Se llama en un hilo aparte (arrancar un gateway
    /// tarda: importa el agente, la config y los MCP).
    pub fn esperar_listo(&self) {
        let limite = Instant::now() + Duration::from_secs(ESPERA_LISTO_S);
        while Instant::now() < limite {
            if !self.vivo() {
                break;
            }
            // Ojo: no alcanza con que el health conteste 200. Un gateway ajeno (huérfano, otro token)
            // también lo contesta y nos deja creyendo que estamos listos; el panel entonces no puede
            // conectar. Se comprueba que **el nuestro** acepte el token.
            if propio_ok(self.puerto, &self.token) {
                self.listo.store(true, Ordering::Relaxed);
                return;
            }
            std::thread::sleep(Duration::from_millis(400));
        }
        self.listo.store(false, Ordering::Relaxed);
    }

    /// Baja el gateway. Idempotente: si no hay nada, no hace nada.
    pub fn parar(&self) {
        self.listo.store(false, Ordering::Relaxed);
        if let Ok(mut g) = self.proceso.lock() {
            if let Some(mut hijo) = g.take() {
                let _ = hijo.kill();
                let _ = hijo.wait();
            }
        }
    }
}

impl Drop for Gateway {
    fn drop(&mut self) {
        self.parar();
    }
}

/// Argumentos de `hermes serve`. Puro y testeado: un flag mal escrito deja el panel esperando para siempre.
pub fn argv(cmd: &mut Command, puerto: u16) {
    cmd.arg("serve")
        .arg("--port")
        .arg(puerto.to_string())
        // La interfaz web del dashboard no se usa: el panel propio es la superficie. Ahorra el build.
        .arg("--skip-build");
}

/// Token del gateway: aleatorio por arranque (nunca en disco, nunca en logs).
fn token_nuevo() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id() as u128;
    // Mezcla barata y suficiente para un secreto de loopback que vive lo que vive la app.
    let mut estado = nanos ^ (pid << 64) ^ 0x9E37_79B9_7F4A_7C15;
    let mut out = String::with_capacity(32);
    for _ in 0..32 {
        estado ^= estado << 13;
        estado ^= estado >> 7;
        estado ^= estado << 17;
        const ALFA: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
        out.push(ALFA[(estado % ALFA.len() as u128) as usize] as char);
    }
    out
}

/// Código de estado de un GET mínimo (sin dependencias HTTP).
fn http_status(url: &str) -> Option<u16> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    let resto = url.strip_prefix("http://")?;
    let (host_puerto, ruta) = resto.split_once('/')?;
    let (host, puerto) = host_puerto.split_once(':')?;
    let mut s = TcpStream::connect_timeout(
        &format!("{host}:{puerto}").parse().ok()?,
        Duration::from_millis(900),
    )
    .ok()?;
    let _ = s.set_read_timeout(Some(Duration::from_millis(1500)));
    s.write_all(
        format!("GET /{ruta} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes(),
    )
    .ok()?;
    let mut buf = [0u8; 256];
    let n = s.read(&mut buf).ok()?;
    let cabeza = String::from_utf8_lossy(&buf[..n]);
    cabeza.split_whitespace().nth(1)?.parse().ok()
}

/// ¿El gateway que escucha es **nuestro**? Se pide la ruta del WebSocket con nuestro token: con token
/// válido el server contesta 400/426 (falta el upgrade); con un token ajeno, 403.
fn propio_ok(puerto: u16, token: &str) -> bool {
    match http_status(&format!("http://127.0.0.1:{puerto}/api/ws?token={token}")) {
        Some(codigo) => codigo != 401 && codigo != 403,
        None => false,
    }
}

#[cfg(test)]
mod tests_gateway {
    use super::*;

    #[test]
    fn el_argv_levanta_serve_en_el_puerto_propio_sin_construir_la_web() {
        let mut cmd = Command::new("hermes");
        argv(&mut cmd, 9121);
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(args, vec!["serve", "--port", "9121", "--skip-build"]);
    }

    #[test]
    fn la_url_del_websocket_lleva_el_token_y_no_es_el_puerto_del_escritorio() {
        let g = Gateway::nuevo(0);
        assert!(
            g.puerto() >= PUERTO_POR_DEFECTO,
            "arranca en el puerto propio, o en el siguiente libre si el propio está ocupado"
        );
        assert_ne!(
            g.puerto(),
            9119,
            "9119 es del escritorio de Hermes: no se pisa"
        );
        let url = g.url_ws();
        assert!(url.starts_with(&format!("ws://127.0.0.1:{}/api/ws?token=", g.puerto())));
        assert!(url.ends_with(g.token()));
        assert!(g.token().len() >= 24, "token corto = adivinable");
    }

    #[test]
    fn dos_gateways_no_comparten_token() {
        let a = Gateway::nuevo(9121);
        let b = Gateway::nuevo(9121);
        assert_ne!(
            a.token(),
            b.token(),
            "el token es por arranque, no una constante"
        );
    }

    #[test]
    fn el_puerto_libre_salta_los_ocupados() {
        let ocupado = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let p = ocupado.local_addr().unwrap().port();
        assert_ne!(
            puerto_libre(p),
            p,
            "no puede elegir un puerto que ya está escuchando: sería el de un gateway zombie"
        );
    }

    #[test]
    fn sin_nada_escuchando_el_gateway_no_se_da_por_listo() {
        // Puerto libre de verdad: nadie contesta, así que `propio_ok` tiene que dar falso (nada de
        // creerle a un health ajeno).
        let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        assert!(!propio_ok(p, "token-de-prueba"));
    }

    #[test]
    fn el_estado_arranca_apagado_y_parar_sin_proceso_no_revienta() {
        let g = Gateway::nuevo(9121);
        let e = g.estado();
        assert_eq!(e["listo"], json!(false));
        assert_eq!(e["vivo"], json!(false));
        assert!(
            e["puerto"].as_u64().unwrap_or(0) >= PUERTO_POR_DEFECTO as u64,
            "informa el puerto propio (o el siguiente libre si estaba ocupado)"
        );
        g.parar();
        assert!(!g.listo());
        assert!(!g.vivo());
    }
}

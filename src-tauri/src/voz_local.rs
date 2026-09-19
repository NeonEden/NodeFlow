//! La voz local (Kokoro) como **paquete descargable**.
//!
//! El instalador de NodeFlow es chico a propósito: la voz de salida de calidad (Kokoro, 82M) pesa
//! ~390 MB entre runtime y modelo, y no tiene por qué viajar con todos. Hasta acá la voz local sólo
//! existía en la máquina de desarrollo (`tools/tts/`), así que en una PC recién instalada la app
//! hablaba con la voz del sistema y nada más.
//!
//! Acá vive el ciclo completo: **estado → descarga con progreso → verificación de hash → despliegue
//! → arranque**. Dos fuentes, las dos públicas y verificables:
//!   - el runtime (Python embebible + paquetes + servidor + voces) desde el release de NodeFlow;
//!   - el modelo, desde HuggingFace (325 MB, es el archivo original de Kokoro v1.0).
//!
//! La app **arranca el servidor sola** si está instalado: eso reemplaza al `.vbs` de la máquina de
//! desarrollo, que tenía una ruta absoluta dentro y no servía en ninguna otra PC.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Paquete del runtime (80,5 MB): Python embebible + `Lib/site-packages` + `servidor.py` + voces (**sin** el
/// modelo, que se baja aparte). Se arma con `scripts/empaquetar-voz.sh`.
pub const ZIP_URL_DEFECTO: &str =
    "https://github.com/NeonEden/NodeFlow/releases/download/v0.3.6/nodeflow-voz-local.zip";
/// SHA-256 del paquete. Si no coincide, no se despliega: un runtime a medias deja la app muda y
/// culpando a la red.
pub const ZIP_SHA256: &str = "77e6ca0af0a65338efc120e85543698a1d7a3d4d876b9cf80270f4035ce3187a";
/// El modelo original de Kokoro v1.0 (325 MB) servido por HuggingFace.
pub const MODELO_URL_DEFECTO: &str =
    "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/main/onnx/model.onnx";
pub const MODELO_BYTES: u64 = 325_532_232;
pub const PUERTO: u16 = 8125;

/// Carpeta donde vive el runtime descargado.
pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("voz")
}

/// ¿Está todo lo que hace falta para sintetizar?
pub fn instalada(data_dir: &Path) -> bool {
    let d = dir(data_dir);
    d.join("python.exe").exists()
        && d.join("servidor.py").exists()
        && d.join("kokoro-v1.0.onnx").exists()
        && d.join("voices-v1.0.bin").exists()
}

/// ¿El servidor de voz contesta? (conexión TCP corta: no se le pide sintetizar nada)
pub fn corriendo() -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], PUERTO)),
        std::time::Duration::from_millis(400),
    )
    .is_ok()
}

fn ruta_progreso(data_dir: &Path) -> PathBuf {
    data_dir.join("voz.instalacion.json")
}

pub fn escribir_progreso(data_dir: &Path, fase: &str, bajado: u64, total: u64, error: Option<&str>) {
    let v = json!({
        "fase": fase,
        "bajado": bajado,
        "total": total,
        "error": error,
        "cuando": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    });
    let v_txt = v.to_string();
    let _ = crate::estado::escribir_atomico(&ruta_progreso(data_dir), &v_txt);
}

pub fn leer_progreso(data_dir: &Path) -> Value {
    std::fs::read_to_string(ruta_progreso(data_dir))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null)
}

/// ¿Hay una instalación en curso? (una fase activa sin error, con sello de tiempo fresco)
pub fn en_curso(data_dir: &Path) -> bool {
    let p = leer_progreso(data_dir);
    let fase = p["fase"].as_str().unwrap_or("");
    if fase.is_empty() || fase == "listo" || p["error"].is_string() {
        return false;
    }
    let cuando = p["cuando"].as_u64().unwrap_or(0);
    let ahora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Vencimiento: una descarga muerta a mitad no puede dejar la app creyendo para siempre que
    // está instalando (misma lección que las banderas «corriendo» del resto del proyecto).
    ahora.saturating_sub(cuando) < 20 * 60
}

/// Arranca el servidor de voz descargado, sin ventana. Idempotente.
pub fn arrancar(data_dir: &Path) -> Result<String, String> {
    if corriendo() {
        return Ok("ya estaba escuchando".into());
    }
    if !instalada(data_dir) {
        return Err("la voz local no está instalada".into());
    }
    let d = dir(data_dir);
    let mut cmd = std::process::Command::new(d.join("python.exe"));
    cmd.arg("servidor.py")
        .current_dir(&d)
        .env("NF_TTS_PUERTO", PUERTO.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    match cmd.spawn() {
        Ok(p) => Ok(format!("arrancado (pid {})", p.id())),
        Err(e) => Err(format!("no pude arrancarlo: {e}")),
    }
}

pub fn sha256_de(p: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut f = std::fs::File::open(p).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = std::io::Read::read(&mut f, &mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// Descomprime el paquete. Se usa `Expand-Archive` (nativo): el `tar` de Windows es GNU tar y no
/// lee ZIP — medido.
fn expandir(zip: &Path, destino: &Path) -> Result<(), String> {
    let mut cmd = std::process::Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        &format!(
            "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
            zip.display(),
            destino.display()
        ),
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| format!("no pude descomprimir: {e}"))?;
    if !out.status.success() {
        let err: String = String::from_utf8_lossy(&out.stderr).chars().take(220).collect();
        return Err(format!("Expand-Archive falló: {err}"));
    }
    Ok(())
}

/// Descarga con progreso. El progreso se escribe en disco porque la descarga corre en segundo plano
/// (una petición HTTP sostenida durante 5 minutos no es una opción).
async fn descargar(url: &str, destino: &Path, data_dir: &Path, fase: &str) -> Result<u64, String> {
    use futures_util::StreamExt;
    let cli = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| format!("{fase}: {e}"))?;
    let r = cli
        .get(url)
        .send()
        .await
        .map_err(|e| format!("{fase}: no pude conectar ({e})"))?;
    if !r.status().is_success() {
        return Err(format!("{fase}: el servidor respondió {}", r.status()));
    }
    let total = r.content_length().unwrap_or(0);
    let mut archivo = std::fs::File::create(destino).map_err(|e| format!("{fase}: {e}"))?;
    let mut bajado: u64 = 0;
    let mut stream = r.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let c = chunk.map_err(|e| format!("{fase}: se cortó la descarga ({e})"))?;
        std::io::Write::write_all(&mut archivo, &c).map_err(|e| format!("{fase}: {e}"))?;
        bajado += c.len() as u64;
        escribir_progreso(data_dir, fase, bajado, total, None);
    }
    Ok(bajado)
}

/// Instala la voz local: runtime → verificación → despliegue → modelo → arranque.
pub async fn instalar(data_dir: PathBuf, zip_url: String, modelo_url: String) -> Result<String, String> {
    let d = dir(&data_dir);
    if let Err(e) = std::fs::create_dir_all(&d) {
        return Err(format!("no pude crear {d:?}: {e}"));
    }
    escribir_progreso(&data_dir, "runtime", 0, 0, None);
    let zip_local = d.join("runtime.zip");
    if let Err(e) = descargar(&zip_url, &zip_local, &data_dir, "runtime").await {
        escribir_progreso(&data_dir, "error", 0, 0, Some(&e));
        return Err(e);
    }
    let hash = sha256_de(&zip_local)?;
    if !hash.eq_ignore_ascii_case(ZIP_SHA256) {
        escribir_progreso(&data_dir, "error", 0, 0, Some("el paquete no coincide"));
        let _ = std::fs::remove_file(&zip_local);
        return Err(format!("el paquete descargado no coincide (sha256 {hash})"));
    }
    escribir_progreso(&data_dir, "desplegando", 1, 1, None);
    if let Err(e) = expandir(&zip_local, &d) {
        escribir_progreso(&data_dir, "error", 0, 0, Some(&e));
        return Err(e);
    }
    let _ = std::fs::remove_file(&zip_local);

    let modelo = d.join("kokoro-v1.0.onnx");
    if !modelo.exists() || std::fs::metadata(&modelo).map(|m| m.len()).unwrap_or(0) < MODELO_BYTES {
        escribir_progreso(&data_dir, "modelo", 0, MODELO_BYTES, None);
        if let Err(e) = descargar(&modelo_url, &modelo, &data_dir, "modelo").await {
            escribir_progreso(&data_dir, "error", 0, 0, Some(&e));
            return Err(e);
        }
    }
    let arranque = arrancar(&data_dir).unwrap_or_else(|e| format!("(no arrancó: {e})"));
    escribir_progreso(&data_dir, "listo", 1, 1, None);
    Ok(arranque)
}

/// De dónde se baja cada cosa. Configurable (`tts.paquete_url` / `tts.modelo_url`) para poder mover
/// el paquete de host sin recompilar.
pub fn urls(data_dir: &Path) -> (String, String) {
    let cfg = std::fs::read_to_string(data_dir.join("nodeflow.config.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let tts = cfg
        .as_ref()
        .and_then(|c| c.get("tts"))
        .cloned()
        .unwrap_or(Value::Null);
    let zip = tts["paquete_url"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or(ZIP_URL_DEFECTO)
        .to_string();
    let modelo = tts["modelo_url"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or(MODELO_URL_DEFECTO)
        .to_string();
    (zip, modelo)
}

/// Todo lo que el panel necesita saber, en un solo lugar.
pub fn estado(data_dir: &Path) -> Value {
    let d = dir(data_dir);
    let modelo = d.join("kokoro-v1.0.onnx");
    json!({
        "instalada": instalada(data_dir),
        "corriendo": corriendo(),
        "dir": d.to_string_lossy(),
        "modelo_bytes": std::fs::metadata(&modelo).map(|m| m.len()).unwrap_or(0),
        "modelo_esperado": MODELO_BYTES,
        "en_curso": en_curso(data_dir),
        "progreso": leer_progreso(data_dir),
        "tamano_descarga": "≈405 MB (runtime 80 MB + modelo 325 MB)",
        "url": ZIP_URL_DEFECTO,
    })
}

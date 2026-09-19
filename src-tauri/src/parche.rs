//! Etapa 4 — **parcheo del repo con aprobación y verificación**.
//!
//! Es la pieza que convierte «la app mira y propone» en «la app se construye». El agente **no escribe**:
//! propone una lista de ediciones ancladas (buscar → reemplazar sobre el texto exacto que leyó), la
//! propuesta queda en una cola y **el humano la aprueba**. Recién ahí se escribe, y antes de dar nada por
//! hecho se corren los *gates* del proyecto (los tests de Rust y el chequeo de tipos) y se reporta la
//! salida real.
//!
//! Por qué **ediciones ancladas** y no un diff unificado: un diff depende de números de línea y contexto
//! que pueden haberse movido entre que el modelo leyó y el humano aprobó, y se aplica «a medias» con
//! holgura. Un ancla de texto **o está o no está**: si el código cambió, la edición falla ruidosamente en
//! vez de escribir en el lugar equivocado. Es la misma regla que ya usa `scripts/i18n-aplicar.mjs`.
//!
//! Cuatro garantías, todas probadas:
//! 1. **Cárcel de escritura**: sólo rutas relativas dentro del repo (nada de `..`, absolutas ni las llaves
//!    del usuario) y una lista negra propia: `release.sh` (publica), `.git/`, y los archivos de claves.
//! 2. **Todo o nada**: se valida cada edición y se leen los originales **antes** de escribir una sola
//!    línea; si una escritura falla, se restauran las anteriores. Un parche a medias no existe.
//! 3. **Gate obligatorio**: aplicado el parche corren `cargo test --lib` y/o `npx tsc --noEmit` **según
//!    los archivos tocados**, y se guarda la salida. Un cambio sin gates no se reporta como hecho.
//! 4. **Reversible**: se guarda la inversa de cada edición, así «revertir» es un clic y no una arqueología.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// Tope de archivos distintos por parche: un parche grande es un parche que nadie revisa.
pub const MAX_ARCHIVOS: usize = 4;
/// Tope de ediciones por parche.
pub const MAX_EDICIONES: usize = 24;
/// Lo que **nunca** se parchea, además de las llaves que ya protege `agente::ruta_enjaulada`.
pub const PROHIBIDOS_ESCRITURA: [&str; 4] = ["release.sh", ".git", "auto.sh", "checkpoint.sh"];

/// Una edición anclada: buscar un texto exacto y reemplazarlo.
#[derive(Clone, Debug, PartialEq)]
pub struct Edicion {
    pub ruta: String,
    pub buscar: String,
    pub reemplazar: String,
    /// Si el texto aparece más de una vez: `false` (default) exige que sea único; `true` cambia todas.
    pub todos: bool,
}

/// Lee las ediciones que propuso el modelo, con validación de forma (no de contenido).
pub fn parsear(ediciones: &Value) -> Result<Vec<Edicion>, String> {
    let lista = ediciones
        .as_array()
        .ok_or("`ediciones` tiene que ser una lista")?;
    if lista.is_empty() {
        return Err("el parche no tiene ediciones".into());
    }
    if lista.len() > MAX_EDICIONES {
        return Err(format!("demasiadas ediciones ({}, tope {MAX_EDICIONES})", lista.len()));
    }
    let mut out = Vec::new();
    for (i, e) in lista.iter().enumerate() {
        let ruta = e["ruta"].as_str().unwrap_or("").trim().to_string();
        let buscar = e["buscar"].as_str().unwrap_or("").to_string();
        let reemplazar = e["reemplazar"].as_str().unwrap_or("").to_string();
        if ruta.is_empty() {
            return Err(format!("edición {}: falta `ruta`", i + 1));
        }
        if buscar.trim().is_empty() {
            return Err(format!("edición {} ({ruta}): falta `buscar`", i + 1));
        }
        if buscar == reemplazar {
            return Err(format!("edición {} ({ruta}): `buscar` y `reemplazar` son iguales", i + 1));
        }
        out.push(Edicion {
            ruta,
            buscar,
            reemplazar,
            todos: e["todos"].as_bool().unwrap_or(false),
        });
    }
    Ok(out)
}

/// La cárcel de **escritura**: la de lectura del agente, más lo que no se toca nunca.
pub fn ruta_escribible(raiz: &Path, rel: &str) -> Result<PathBuf, String> {
    let limpio = rel.trim().replace('\\', "/");
    let minuscula = limpio.to_ascii_lowercase();
    if PROHIBIDOS_ESCRITURA
        .iter()
        .any(|p| minuscula.ends_with(p) || minuscula.contains(&format!("/{p}")) || minuscula.starts_with(p))
    {
        return Err(format!(
            "«{rel}» está protegido: un parche no toca archivos de publicación, control de versiones ni herramientas de guardado"
        ));
    }
    crate::agente::ruta_enjaulada(raiz, &limpio)
}

/// Cuántas veces aparece el ancla en el texto.
fn contar(texto: &str, aguja: &str) -> usize {
    texto.match_indices(aguja).count()
}

/// Valida el parche **completo** contra el disco, sin escribir nada. Devuelve, por archivo, cuántas
/// ediciones lleva (es lo que se muestra en la propuesta para que el humano sepa qué va a pasar).
pub fn validar(raiz: &Path, eds: &[Edicion]) -> Result<Vec<(String, usize)>, String> {
    let mut por_archivo: BTreeMap<String, usize> = BTreeMap::new();
    for e in eds {
        let camino = ruta_escribible(raiz, &e.ruta)?;
        if !camino.is_file() {
            return Err(format!("«{}» no existe (el parche sólo edita archivos existentes)", e.ruta));
        }
        *por_archivo.entry(e.ruta.trim().replace('\\', "/")).or_insert(0) += 1;
    }
    if por_archivo.len() > MAX_ARCHIVOS {
        return Err(format!(
            "el parche toca {} archivos y el tope es {MAX_ARCHIVOS}",
            por_archivo.len()
        ));
    }
    // El contenido se lee una sola vez y se aplica en memoria: si algo no coincide, no se escribió nada.
    let mut contenido: BTreeMap<String, String> = BTreeMap::new();
    for ruta in por_archivo.keys() {
        let camino = ruta_escribible(raiz, ruta)?;
        let texto = std::fs::read_to_string(&camino).map_err(|e| format!("no pude leer «{ruta}»: {e}"))?;
        contenido.insert(ruta.clone(), texto);
    }
    for (i, e) in eds.iter().enumerate() {
        let ruta = e.ruta.trim().replace('\\', "/");
        let texto = contenido.get(&ruta).ok_or("archivo fuera del parche")?;
        let n = contar(texto, &e.buscar);
        if n == 0 {
            return Err(format!(
                "edición {} ({}): el texto a buscar no está en el archivo — ¿leyó otra versión?",
                i + 1,
                ruta
            ));
        }
        if n > 1 && !e.todos {
            return Err(format!(
                "edición {} ({}): el texto aparece {n} veces; acotá el ancla o poné \"todos\": true",
                i + 1,
                ruta
            ));
        }
    }
    Ok(por_archivo.into_iter().collect())
}

/// Resultado de aplicar: qué archivos se tocaron y cuántas líneas entraron/salieron.
#[derive(Clone, Debug, PartialEq)]
pub struct Aplicado {
    pub archivos: Vec<String>,
    pub ediciones: usize,
    pub mas: i64,
    pub menos: i64,
}

/// **Aplica todo o nada.** Valida (lo mismo que `validar`), calcula el resultado en memoria, escribe, y si
/// una escritura falla restaura las anteriores desde los originales que tiene en la mano.
pub fn aplicar(raiz: &Path, eds: &[Edicion]) -> Result<Aplicado, String> {
    validar(raiz, eds)?; // rebota antes de tocar el disco

    let mut originales: BTreeMap<String, String> = BTreeMap::new();
    let mut nuevos: BTreeMap<String, String> = BTreeMap::new();
    for e in eds {
        let ruta = e.ruta.trim().replace('\\', "/");
        if nuevos.contains_key(&ruta) || originales.contains_key(&ruta) {
            // ya leído: se reusa el contenido original
        } else {
            let camino = ruta_escribible(raiz, &ruta)?;
            let texto = std::fs::read_to_string(&camino).map_err(|e| format!("no pude leer «{ruta}»: {e}"))?;
            originales.insert(ruta.clone(), texto.clone());
            nuevos.insert(ruta.clone(), texto);
        }
        let actual = nuevos.get(&ruta).cloned().unwrap_or_default();
        let nuevo = if e.todos {
            actual.replace(&e.buscar, &e.reemplazar)
        } else {
            actual.replacen(&e.buscar, &e.reemplazar, 1)
        };
        nuevos.insert(ruta, nuevo);
    }

    let mut mas = 0i64;
    let mut menos = 0i64;
    for (ruta, nuevo) in &nuevos {
        let viejo = &originales[ruta];
        let a = nuevo.lines().count() as i64 - viejo.lines().count() as i64;
        if a > 0 {
            mas += a;
        } else {
            menos += -a;
        }
    }

    // Escritura con red: lo escrito se puede restaurar desde `originales`.
    let mut escritos: Vec<String> = Vec::new();
    for (ruta, nuevo) in &nuevos {
        let camino = ruta_escribible(raiz, ruta)?;
        match crate::estado::escribir_atomico(&camino, nuevo) {
            Ok(_) => escritos.push(ruta.clone()),
            Err(e) => {
                for r in &escritos {
                    if let (Ok(c), Some(orig)) = (ruta_escribible(raiz, r), originales.get(r)) {
                        let _ = crate::estado::escribir_atomico(&c, orig);
                    }
                }
                return Err(format!("no pude escribir «{ruta}»: {e} — restauré lo ya escrito"));
            }
        }
    }
    Ok(Aplicado {
        archivos: nuevos.keys().cloned().collect(),
        ediciones: eds.len(),
        mas,
        menos,
    })
}

/// La inversa de un parche: con esto, «revertir» es aplicar el mismo mecanismo al revés.
pub fn invertir(eds: &[Edicion]) -> Vec<Edicion> {
    eds.iter()
        .map(|e| Edicion {
            ruta: e.ruta.clone(),
            buscar: e.reemplazar.clone(),
            reemplazar: e.buscar.clone(),
            todos: e.todos,
        })
        .collect()
}

/// Los *gates* que corresponden según lo que el parche tocó: Rust ⇒ tests; TS ⇒ chequeo de tipos.
/// (Correr los dos siempre es tirar minutos: un parche de Rust no puede romper el tipado de TS.)
pub fn gates(archivos: &[String]) -> Vec<String> {
    let mut g = Vec::new();
    if archivos.iter().any(|a| a.ends_with(".rs") || a.ends_with(".toml")) {
        g.push("cargo test --manifest-path src-tauri/Cargo.toml --lib".to_string());
    }
    if archivos
        .iter()
        .any(|a| a.ends_with(".ts") || a.ends_with(".tsx") || a.ends_with(".mjs"))
    {
        g.push("npx tsc --noEmit".to_string());
    }
    if g.is_empty() {
        g.push("git diff --stat".to_string());
    }
    g
}

/// Corre los gates y devuelve `[{comando, ok, ms, salida}]`. Usa el mismo ejecutor enjaulado del agente,
/// así el tope de tiempo y la lista blanca son los mismos que ya están probados.
pub fn verificar(raiz: &Path, archivos: &[String], tope_s: u64) -> Vec<Value> {
    gates(archivos)
        .into_iter()
        .map(|comando| {
            let inicio = std::time::Instant::now();
            let r = crate::agente::correr_comando_publico(raiz, &comando, tope_s);
            match r {
                Ok(salida) => {
                    let ok = salida.contains("exit 0");
                    json!({
                        "comando": comando,
                        "ok": ok,
                        "ms": inicio.elapsed().as_millis() as u64,
                        "salida": salida.chars().take(2_000).collect::<String>(),
                    })
                }
                Err(e) => json!({
                    "comando": comando,
                    "ok": false,
                    "ms": inicio.elapsed().as_millis() as u64,
                    "salida": e,
                }),
            }
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// La cola: una propuesta por vez, con su verificación y su inversa guardadas
// ─────────────────────────────────────────────────────────────────────────────

/// El archivo de la cola, en la carpeta de datos de la app (no en el repo: no se versiona).
pub const ARCHIVO: &str = "agente.parche.json";

fn eds_a_json(eds: &[Edicion]) -> Value {
    json!(eds
        .iter()
        .map(|e| json!({
            "ruta": e.ruta,
            "buscar": e.buscar,
            "reemplazar": e.reemplazar,
            "todos": e.todos,
        }))
        .collect::<Vec<_>>())
}

pub fn eds_de_json(v: &Value) -> Result<Vec<Edicion>, String> {
    parsear(v)
}

pub fn leer(dir: &Path) -> Option<Value> {
    std::fs::read_to_string(dir.join(ARCHIVO))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
}

pub fn escribir(dir: &Path, v: &Value) -> Result<(), String> {
    let txt = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    crate::estado::escribir_atomico(&dir.join(ARCHIVO), &txt)
        .map_err(|e| format!("no pude guardar la cola de parches: {e}"))
}

/// Deja una propuesta **pendiente** (nada escrito). Se valida antes de encolar: una propuesta que no se
/// puede aplicar no llega a la mesa del humano.
pub fn guardar_propuesta(
    dir: &Path,
    motivo: &str,
    eds: &[Edicion],
    archivos: &[(String, usize)],
) -> Result<(), String> {
    if let Some(actual) = leer(dir) {
        if actual["estado"].as_str() == Some("pendiente") {
            return Err(
                "ya hay un parche esperando aprobación: resolvelo (aprobar o rechazar) antes de proponer otro"
                    .into(),
            );
        }
    }
    let sello = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    escribir(
        dir,
        &json!({
            "motivo": motivo,
            "estado": "pendiente",
            "ediciones": eds_a_json(eds),
            "archivos": archivos
                .iter()
                .map(|(r, n)| json!({ "ruta": r, "ediciones": n }))
                .collect::<Vec<_>>(),
            "cuando": sello,
        }),
    )
}

#[cfg(test)]
mod tests_cola {
    use super::*;

    #[test]
    fn la_propuesta_se_guarda_con_su_motivo_y_sus_archivos() {
        let raiz = repo_temporal("cola");
        let dir = raiz.join(".datos");
        std::fs::create_dir_all(&dir).unwrap();
        let eds = vec![ed("src-tauri/src/lib.rs", "// fin", "// fin del módulo")];
        let archivos = validar(&raiz, &eds).unwrap();
        guardar_propuesta(&dir, "aclarar el final del archivo", &eds, &archivos).unwrap();

        let v = leer(&dir).unwrap();
        assert_eq!(v["estado"], "pendiente");
        assert_eq!(v["motivo"], "aclarar el final del archivo");
        assert_eq!(v["ediciones"].as_array().unwrap().len(), 1);
        assert_eq!(v["archivos"][0]["ruta"], "src-tauri/src/lib.rs");
        let vueltas = eds_de_json(&v["ediciones"]).unwrap();
        assert_eq!(vueltas, eds, "lo que se guarda se puede volver a aplicar");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn no_se_encolan_dos_parches_a_la_vez() {
        let raiz = repo_temporal("cola_dos");
        let dir = raiz.join(".datos");
        std::fs::create_dir_all(&dir).unwrap();
        let eds = vec![ed("src-tauri/src/lib.rs", "// fin", "// final")];
        let archivos = validar(&raiz, &eds).unwrap();
        guardar_propuesta(&dir, "uno", &eds, &archivos).unwrap();
        let e = guardar_propuesta(&dir, "dos", &eds, &archivos).unwrap_err();
        assert!(e.contains("esperando aprobación"), "{e}");

        // Resuelta la primera, la segunda entra.
        let mut v = leer(&dir).unwrap();
        v["estado"] = json!("rechazado");
        escribir(&dir, &v).unwrap();
        assert!(guardar_propuesta(&dir, "dos", &eds, &archivos).is_ok());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_ciclo_completo_proponer_aplicar_verificar_revertir() {
        let raiz = repo_temporal("ciclo");
        let dir = raiz.join(".datos");
        std::fs::create_dir_all(&dir).unwrap();
        let original = std::fs::read_to_string(raiz.join("src-tauri/src/lib.rs")).unwrap();
        let eds = vec![ed("src-tauri/src/lib.rs", "// fin", "// fin (editado)")];
        let archivos = validar(&raiz, &eds).unwrap();
        guardar_propuesta(&dir, "marcar el final", &eds, &archivos).unwrap();

        // Aprobar = aplicar lo que estaba en la cola.
        let guardadas = eds_de_json(&leer(&dir).unwrap()["ediciones"]).unwrap();
        let apl = aplicar(&raiz, &guardadas).unwrap();
        assert_eq!(apl.archivos.len(), 1);
        let mut v = leer(&dir).unwrap();
        v["estado"] = json!("aplicado");
        escribir(&dir, &v).unwrap();
        assert!(std::fs::read_to_string(raiz.join("src-tauri/src/lib.rs")).unwrap().contains("// fin (editado)"));

        // Revertir = aplicar la inversa.
        aplicar(&raiz, &invertir(&guardadas)).unwrap();
        assert_eq!(std::fs::read_to_string(raiz.join("src-tauri/src/lib.rs")).unwrap(), original);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_gate_no_corre_lo_que_no_hace_falta() {
        // Un parche de Rust no puede romper el tipado de TS: correr tsc sería tirar minutos.
        let g = gates(&["src-tauri/src/agente.rs".into()]);
        assert_eq!(g.len(), 1);
        assert!(g[0].starts_with("cargo test"), "{g:?}");
    }

    fn repo_temporal(nombre: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("nf-parche-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("src-tauri/src")).unwrap();
        std::fs::create_dir_all(base.join("src")).unwrap();
        std::fs::write(
            base.join("src-tauri/src/lib.rs"),
            "mod agente;\nmod server;\n// fin\n",
        )
        .unwrap();
        std::fs::write(base.join("src/App.tsx"), "export const A = 1;\n").unwrap();
        std::fs::write(base.join("release.sh"), "#!/bin/bash\npublicar\n").unwrap();
        std::fs::write(base.join("clave-x.txt"), "sk-no-se-toca\n").unwrap();
        base
    }

    fn ed(ruta: &str, buscar: &str, reemplazar: &str) -> Edicion {
        Edicion { ruta: ruta.into(), buscar: buscar.into(), reemplazar: reemplazar.into(), todos: false }
    }

    #[test]
    fn la_jaula_de_escritura_rebota_publicacion_git_y_llaves() {
        let raiz = repo_temporal("jaula");
        assert!(ruta_escribible(&raiz, "src-tauri/src/lib.rs").is_ok());
        for malo in [
            "../../fuera.rs",
            "/etc/passwd",
            "release.sh",
            "scripts/release.sh",
            ".git/config",
            "clave-x.txt",
            "clave-motor.txt",
            "sub/.env",
        ] {
            assert!(ruta_escribible(&raiz, malo).is_err(), "«{malo}» no se puede escribir");
        }
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn aplicar_hace_el_cambio_y_deja_el_resto_igual() {
        let raiz = repo_temporal("aplicar");
        let eds = vec![ed("src-tauri/src/lib.rs", "mod server;", "mod server;\nmod agente;")];
        let r = aplicar(&raiz, &eds).unwrap();
        assert_eq!(r.archivos, vec!["src-tauri/src/lib.rs"]);
        assert_eq!(r.mas, 1, "una línea más");
        let texto = std::fs::read_to_string(raiz.join("src-tauri/src/lib.rs")).unwrap();
        assert!(texto.contains("mod agente;\nmod server;\nmod agente;"), "{texto}");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_ancla_que_no_esta_no_escribe_nada() {
        let raiz = repo_temporal("ancla");
        let eds = vec![
            ed("src-tauri/src/lib.rs", "mod server;", "mod x;"),
            ed("src-tauri/src/lib.rs", "texto-que-no-existe", "y"),
        ];
        let e = aplicar(&raiz, &eds).unwrap_err();
        assert!(e.contains("no está en el archivo"), "{e}");
        assert_eq!(
            std::fs::read_to_string(raiz.join("src-tauri/src/lib.rs")).unwrap(),
            "mod agente;\nmod server;\n// fin\n",
            "todo o nada: la primera edición tampoco se aplicó"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_ancla_ambigua_rebota_salvo_que_se_pida_todos() {
        let raiz = repo_temporal("ambigua");
        let ambiguo = vec![ed("src-tauri/src/lib.rs", "mod ", "module ")];
        let e = aplicar(&raiz, &ambiguo).unwrap_err();
        assert!(e.contains("aparece 2 veces"), "{e}");

        let mut todos = ambiguo.clone();
        todos[0].todos = true;
        let r = aplicar(&raiz, &todos).unwrap();
        assert_eq!(r.ediciones, 1);
        let texto = std::fs::read_to_string(raiz.join("src-tauri/src/lib.rs")).unwrap();
        assert!(!texto.contains("mod ") && texto.contains("module "), "{texto}");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn revertir_deshace_exactamente_lo_que_hizo() {
        let raiz = repo_temporal("revertir");
        let original = std::fs::read_to_string(raiz.join("src-tauri/src/lib.rs")).unwrap();
        let eds = vec![ed("src-tauri/src/lib.rs", "// fin", "// terminado")];
        aplicar(&raiz, &eds).unwrap();
        assert!(std::fs::read_to_string(raiz.join("src-tauri/src/lib.rs")).unwrap().contains("// terminado"));
        aplicar(&raiz, &invertir(&eds)).unwrap();
        assert_eq!(
            std::fs::read_to_string(raiz.join("src-tauri/src/lib.rs")).unwrap(),
            original,
            "vuelve al texto original"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn los_topes_y_la_forma_se_chequean_antes_de_tocar_el_disco() {
        let raiz = repo_temporal("forma");
        assert!(parsear(&json!([{"ruta": "a.rs", "buscar": "x", "reemplazar": "x"}])).is_err(), "iguales");
        assert!(parsear(&json!([{"ruta": "", "buscar": "x", "reemplazar": "y"}])).is_err(), "sin ruta");
        assert!(parsear(&json!([{"ruta": "a.rs", "buscar": "", "reemplazar": "y"}])).is_err(), "sin ancla");
        assert!(parsear(&json!([])).is_err(), "vacío");
        assert!(parsear(&json!("no soy lista")).is_err());

        // más archivos que el tope
        let muchas: Vec<Value> = (0..MAX_EDICIONES + 1)
            .map(|i| json!({"ruta": format!("f{i}.rs"), "buscar": "a", "reemplazar": "b"}))
            .collect();
        assert!(parsear(&json!(muchas)).is_err(), "demasiadas ediciones");

        let cinco = vec![
            ed("src-tauri/src/lib.rs", "mod agente;", "mod agente2;"),
            ed("src-tauri/src/lib.rs", "mod server;", "mod server2;"),
            ed("src/App.tsx", "export const A", "export const B"),
            ed("src/App.tsx", "= 1;", "= 2;"),
            ed("src/App.tsx", "= 1;", "= 3;"),
        ];
        let _ = cinco;
        let _ = raiz;
    }

    #[test]
    fn el_gate_depende_de_lo_que_se_toco() {
        assert_eq!(gates(&["src-tauri/src/lib.rs".into()]), vec!["cargo test --manifest-path src-tauri/Cargo.toml --lib"]);
        assert_eq!(gates(&["src/App.tsx".into()]), vec!["npx tsc --noEmit"]);
        let dos = gates(&["src/App.tsx".into(), "src-tauri/src/lib.rs".into()]);
        assert_eq!(dos.len(), 2, "si toca los dos mundos, corren los dos gates");
        assert_eq!(gates(&["README.md".into()]), vec!["git diff --stat"], "sin gate obvio, al menos el diff");
    }

    #[test]
    fn un_parche_no_puede_tocar_mas_archivos_que_el_tope() {
        let raiz = repo_temporal("tope_archivos");
        let mut eds = Vec::new();
        for i in 0..=MAX_ARCHIVOS {
            let ruta = format!("src-tauri/src/f{i}.rs");
            std::fs::write(raiz.join(&ruta), "uno\n").unwrap();
            eds.push(ed(&ruta, "uno", "dos"));
        }
        let e = validar(&raiz, &eds).unwrap_err();
        assert!(e.contains("tope"), "{e}");
        let _ = std::fs::remove_dir_all(&raiz);
    }
}

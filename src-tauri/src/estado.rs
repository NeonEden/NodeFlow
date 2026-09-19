//! Persistencia atómica del estado de la aplicación en disco.
//!
//! Por qué atómica (tmp + rename) y no `fs::write` directo: si la máquina se apaga o la app se cierra
//! a mitad de una escritura, `fs::write` deja el archivo truncado a 0 bytes o corrupto. En el siguiente
//! arranque, el deserializador falla y la app pierde la contabilidad de costos, la investigación o la
//! configuración en silencio. Escribiendo primero a un archivo temporal en el mismo directorio (mismo
//! sistema de archivos para garantizar que el `rename` sea atómico) y renombrando al destino, el archivo
//! anterior sigue intacto hasta que el nuevo está completamente en disco.

use std::path::Path;

/// Escribe un archivo de estado de forma atómica: primero un `tmp` en el MISMO directorio y después
/// `rename` (en Windows `rename` reemplaza el destino que ya existe). Si algo falla, borra el `tmp` y
/// devuelve el error: el archivo bueno anterior nunca queda a medio escribir.
/// Regla de la casa: todo el estado de la app pasa por acá, nadie escribe con `fs::write` directo.
pub fn escribir_atomico(ruta: &Path, contenido: &str) -> Result<(), String> {
    if let Some(padre) = ruta.parent() {
        if !padre.as_os_str().is_empty() {
            std::fs::create_dir_all(padre).map_err(|e| e.to_string())?;
        }
    }
    // El temporal vive en la misma carpeta para que rename sea una operación atómica dentro
    // del mismo sistema de archivos (sin copias entre volúmenes). El sufijo `tmp-nf` es la
    // convención de la casa y está en `.gitignore`: si la app muere entre el write y el rename,
    // el residuo no se commitea (el checkpoint automático no se lo lleva).
    let tmp = ruta.with_extension("tmp-nf");
    if let Err(e) = std::fs::write(&tmp, contenido.as_bytes()) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    if let Err(e) = std::fs::rename(&tmp, ruta) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reemplaza_contenido_sin_dejar_temporales() {
        let dir = std::env::temp_dir().join(format!("nodeflow-test-estado-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let archivo = dir.join("estado.json");

        // 1. Escribir contenido inicial
        escribir_atomico(&archivo, "contenido viejo").unwrap();
        assert_eq!(
            std::fs::read_to_string(&archivo).unwrap(),
            "contenido viejo"
        );

        // 2. Escribir nuevo contenido sobre el archivo existente (debe reemplazar el contenido anterior)
        let nuevo_contenido = "{\"version\": 2, \"estado\": \"nuevo\"}";
        escribir_atomico(&archivo, nuevo_contenido).unwrap();

        // 3. El contenido escrito es exactamente el que se pidió
        let leido = std::fs::read_to_string(&archivo).unwrap();
        assert_eq!(leido, nuevo_contenido);

        // 4. Después de escribir no queda ningún .tmp en el directorio
        let mut cuenta_tmp = 0;
        let entradas = std::fs::read_dir(&dir).unwrap();
        for entrada in entradas {
            let path = entrada.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) == Some("tmp")
                || path.to_string_lossy().ends_with(".tmp")
            {
                cuenta_tmp += 1;
            }
        }
        assert_eq!(
            cuenta_tmp, 0,
            "No debe quedar ningún archivo .tmp en el directorio tras la escritura"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

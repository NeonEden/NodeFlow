//! Slice 1 — Contrato de Artefactos.
//!
//! Un artefacto es lo que sale de un Experto y entra a una herramienta: un prompt visual para Flow,
//! un brief para Copilot, una spec para TouchDesigner. Sin contrato, el pipeline se vuelve una cadena
//! de texto suelto y se rompe en el tercer paso.
//!
//! Criterio de aceptación (verificable por una persona): si podés pegarlo en su destino y obtenés un
//! buen resultado sin agregar nada, el artefacto está bien formado.
//!
//! Los schemas usan la convención que exige Gemini (`responseSchema`: tipos en MAYÚSCULA, sin
//! `oneOf`/`allOf`), porque es el proveedor que los aplica de verdad.

use serde_json::{json, Value};

/// Tipos de artefacto que el sistema conoce hoy.
pub fn tipos() -> Vec<Value> {
    vec![
        json!({
            "tipo": "prompt_visual",
            "destino": "Flow / ComfyUI / TouchDesigner",
            "descripcion": "Prompt de imagen listo para pegar, con estructura de arte.",
        }),
        json!({
            "tipo": "brief_documento",
            "destino": "Copilot / Word / Excel",
            "descripcion": "Brief para producir un entregable de documento o planilla.",
        }),
        json!({
            "tipo": "critica",
            "destino": "El propio lienzo / decisión humana",
            "descripcion": "Auditoría honesta: fortalezas, debilidades, riesgos y próximo paso.",
        }),
        json!({
            "tipo": "spec_td",
            "destino": "TouchDesigner",
            "descripcion": "Especificación visual + script Python para una red de TD.",
        }),
    ]
}

/// Schema de salida del artefacto (formato Gemini `responseSchema`).
pub fn schema(tipo: &str) -> Option<Value> {
    match tipo {
        "prompt_visual" => Some(json!({
            "type": "OBJECT",
            "properties": {
                "titulo": { "type": "STRING" },
                "prompt_final": { "type": "STRING" },
                "sujeto": { "type": "STRING" },
                "ambiente": { "type": "STRING" },
                "estilo": { "type": "STRING" },
                "iluminacion": { "type": "STRING" },
                "camara": { "type": "STRING" },
                "aspect_ratio": { "type": "STRING", "enum": ["16:9", "9:16", "1:1", "4:3", "3:2", "21:9"] },
                "negativo": { "type": "STRING" },
                "direccion_artistica": { "type": "STRING" },
                "variaciones": {
                    "type": "ARRAY",
                    "items": {
                        "type": "OBJECT",
                        "properties": {
                            "enfoque": { "type": "STRING" },
                            "prompt": { "type": "STRING" }
                        },
                        "required": ["enfoque", "prompt"]
                    }
                }
            },
            "required": ["titulo", "prompt_final", "sujeto", "ambiente", "estilo",
                          "iluminacion", "camara", "aspect_ratio", "negativo",
                          "direccion_artistica", "variaciones"]
        })),
        "brief_documento" => Some(json!({
            "type": "OBJECT",
            "properties": {
                "titulo": { "type": "STRING" },
                "objetivo": { "type": "STRING" },
                "audiencia": { "type": "STRING" },
                "formato": { "type": "STRING", "enum": ["docx", "xlsx", "pptx", "md"] },
                "secciones": {
                    "type": "ARRAY",
                    "items": {
                        "type": "OBJECT",
                        "properties": {
                            "titulo": { "type": "STRING" },
                            "contenido": { "type": "STRING" },
                            "puntos": { "type": "ARRAY", "items": { "type": "STRING" } }
                        },
                        "required": ["titulo", "contenido"]
                    }
                },
                "datos_clave": { "type": "ARRAY", "items": { "type": "STRING" } },
                "prompt_para_copilot": { "type": "STRING" }
            },
            "required": ["titulo", "objetivo", "audiencia", "formato", "secciones",
                          "datos_clave", "prompt_para_copilot"]
        })),
        "critica" => Some(json!({
            "type": "OBJECT",
            "properties": {
                "titulo": { "type": "STRING" },
                "veredicto": { "type": "STRING", "enum": ["solido", "a_medias", "flojo"] },
                "fortalezas": { "type": "ARRAY", "items": { "type": "STRING" } },
                "debilidades": { "type": "ARRAY", "items": { "type": "STRING" } },
                "riesgos": { "type": "ARRAY", "items": { "type": "STRING" } },
                "preguntas_abiertas": { "type": "ARRAY", "items": { "type": "STRING" } },
                "siguiente_paso": { "type": "STRING" }
            },
            "required": ["titulo", "veredicto", "fortalezas", "debilidades", "riesgos",
                          "preguntas_abiertas", "siguiente_paso"]
        })),
        "spec_td" => Some(json!({
            "type": "OBJECT",
            "properties": {
                "titulo": { "type": "STRING" },
                "objetivo_visual": { "type": "STRING" },
                "parametros": {
                    "type": "ARRAY",
                    "items": {
                        "type": "OBJECT",
                        "properties": {
                            "nombre": { "type": "STRING" },
                            "valor": { "type": "STRING" },
                            "por_que": { "type": "STRING" }
                        },
                        "required": ["nombre", "valor", "por_que"]
                    }
                },
                "red_sugerida": { "type": "ARRAY", "items": { "type": "STRING" } },
                "script_py": { "type": "STRING" }
            },
            "required": ["titulo", "objetivo_visual", "parametros", "red_sugerida", "script_py"]
        })),
        _ => None,
    }
}

/// Texto que se le explica al modelo para que el artefacto sirva en su destino.
pub fn instruccion(tipo: &str) -> &'static str {
    match tipo {
        "prompt_visual" => {
            "Proceso obligatorio, en este orden:\n\
             1) DESGLOSE: identificá la esencia temática, emocional y simbólica del concepto.\n\
             2) RUPTURA: llevá la idea a vanguardia con yuxtaposición disruptiva y estética \
             surrealista/futurista; evitá clichés visuales y estilos saturados.\n\
             3) PROMPT TÉCNICO: `prompt_final` en inglés, autosuficiente (≥120 caracteres), con esta \
             estructura interna: sujeto y metáfora central + ambiente y atmósfera + estilo y texturas \
             + iluminación y paleta + cámara y detalle técnico.\n\
             4) VARIACIONES: `variaciones` con EXACTAMENTE 2 direcciones distintas — una más \
             abstracta/minimalista y otra más hiper-detallada o compleja — cada una con su `enfoque` \
             (una línea que explica la intención) y su `prompt` completo en inglés (≥100 caracteres).\n\
             `direccion_artistica` (≥80 caracteres) explica la decisión: qué esencia elegiste, qué \
             cliché rompiste y por qué esa paleta y esa luz.\n\
             `negativo` SIEMPRE nombra el riesgo de manos y anatomía (dedos fusionados, manos \
             deformes, miembros extra) y el de cabeza sin rasgos. Respondé solo el JSON."
        }
        "brief_documento" => {
            "Devolvé un brief ejecutable para producir un documento o planilla. `secciones` necesita \
             al menos 3 entradas con contenido útil (no títulos vacíos). `datos_clave` son los datos \
             y cifras que el documento debe respetar. `prompt_para_copilot` es el texto que se pega \
             en Copilot: tiene que ser autosuficiente, estar en español, explicar el objetivo, la \
             audiencia, el formato y las secciones, y pedir el entregable completo. Respondé solo el JSON."
        }
        "spec_td" => {
            "Devolvé una especificación para una red de TouchDesigner más un script Python que la \
             construya usando `op()`/`td` desde el módulo de texto DAT. El script tiene que ser \
             ejecutable tal cual en la Textport, sin imports exóticos. `parametros` explica cada \
             decisión técnica. Respondé solo el JSON."
        }
        "critica" => {
            "Sos un auditor hostil pero justo. Una crítica sin debilidades concretas es un fracaso:              `debilidades` necesita al menos 2 puntos específicos y verificables, y `riesgos` al menos              1 consecuencia real si esto se construye así. No elogies por cortesía y no inventes              problemas que no existen: si algo está bien, va en `fortalezas` y el veredicto lo refleja.              `preguntas_abiertas` son las que hay que responder ANTES de seguir. Respondé solo el JSON."
        }
        _ => "Respondé solo el JSON pedido.",
    }
}

/// Regla de honestidad epistémica que acompaña a TODOS los tipos.
pub fn regla_epistemica() -> &'static str {
    "Distinguí en el artefacto lo que está establecido de lo que es una propuesta, y lo que está \
     medido de lo que es un supuesto. Si un dato proviene de una nota de la bóveda no corroborada, \
     marcalo como tal (o como supuesto a completar) en vez de presentarlo como un hecho. Una \
     invención segura de sí misma vale menos que un hueco declarado."
}

fn arr_len(v: &Value, campo: &str) -> usize {
    v[campo].as_array().map(|a| a.len()).unwrap_or(0)
}

/// Valida un artefacto contra su destino. Devuelve la lista de problemas (vacía = está bien formado).
pub fn validar(tipo: &str, v: &Value) -> Vec<String> {
    let mut p: Vec<String> = Vec::new();
    if !v.is_object() {
        return vec!["el artefacto no es un objeto JSON".into()];
    }
    let txt = |campo: &str| v[campo].as_str().unwrap_or("").trim().to_string();
    let exige = |campo: &str, minimo: usize, p: &mut Vec<String>| {
        let t = v[campo].as_str().unwrap_or("").trim();
        if t.is_empty() {
            p.push(format!("falta `{campo}`"));
        } else if t.chars().count() < minimo {
            p.push(format!(
                "`{campo}` es muy corto ({} caracteres, mínimo {minimo})",
                t.chars().count()
            ));
        }
    };

    match tipo {
        "prompt_visual" => {
            exige("titulo", 3, &mut p);
            exige("prompt_final", 120, &mut p);
            exige("sujeto", 3, &mut p);
            exige("ambiente", 3, &mut p);
            exige("estilo", 3, &mut p);
            exige("iluminacion", 3, &mut p);
            exige("camara", 3, &mut p);
            exige("negativo", 10, &mut p);
            exige("direccion_artistica", 80, &mut p);
            let neg = txt("negativo").to_lowercase();
            if !["mano", "fingers", "dedos", "hand", "anatom"]
                .iter()
                .any(|x| neg.contains(x))
            {
                p.push("`negativo` no menciona el riesgo de manos/anatomía (el fallo nº1 de estos modelos)".into());
            }
            let vars = v["variaciones"].as_array().cloned().unwrap_or_default();
            if vars.len() < 2 {
                p.push(format!(
                    "`variaciones` necesita 2 direcciones (hay {})",
                    vars.len()
                ));
            }
            for (i, var) in vars.iter().enumerate() {
                if var["enfoque"].as_str().unwrap_or("").trim().len() < 5 {
                    p.push(format!("la variación {} no tiene `enfoque`", i + 1));
                }
                let pr = var["prompt"].as_str().unwrap_or("").trim();
                if pr.chars().count() < 100 {
                    p.push(format!(
                        "el prompt de la variación {} es muy corto ({} caracteres)",
                        i + 1,
                        pr.chars().count()
                    ));
                }
            }
            let ar = txt("aspect_ratio");
            let validos = ["16:9", "9:16", "1:1", "4:3", "3:2", "21:9"];
            if !validos.contains(&ar.as_str()) {
                p.push(format!("`aspect_ratio` inválido: «{ar}»"));
            }
        }
        "brief_documento" => {
            exige("titulo", 3, &mut p);
            exige("objetivo", 20, &mut p);
            exige("audiencia", 3, &mut p);
            exige("prompt_para_copilot", 200, &mut p);
            let fmt = txt("formato");
            let validos = ["docx", "xlsx", "pptx", "md"];
            if !validos.contains(&fmt.as_str()) {
                p.push(format!("`formato` inválido: «{fmt}»"));
            }
            let secciones = v["secciones"].as_array().cloned().unwrap_or_default();
            if secciones.len() < 3 {
                p.push(format!(
                    "`secciones` necesita al menos 3 (hay {})",
                    secciones.len()
                ));
            }
            for (i, s) in secciones.iter().enumerate() {
                let t = s["titulo"].as_str().unwrap_or("").trim();
                let c = s["contenido"].as_str().unwrap_or("").trim();
                if t.is_empty() {
                    p.push(format!("la sección {} no tiene título", i + 1));
                }
                if c.chars().count() < 40 {
                    p.push(format!(
                        "la sección «{t}» tiene contenido muy corto ({} caracteres)",
                        c.chars().count()
                    ));
                }
            }
            if v["datos_clave"]
                .as_array()
                .map(|a| a.is_empty())
                .unwrap_or(true)
            {
                p.push("`datos_clave` está vacío".into());
            }
        }
        "critica" => {
            exige("titulo", 3, &mut p);
            exige("siguiente_paso", 20, &mut p);
            let ver = txt("veredicto");
            let validos = ["solido", "a_medias", "flojo"];
            if !validos.contains(&ver.as_str()) {
                p.push(format!("`veredicto` inválido: «{ver}»"));
            }
            let deb = arr_len(v, "debilidades");
            if deb < 2 {
                p.push(format!("`debilidades` necesita al menos 2 (hay {deb})"));
            }
            if arr_len(v, "riesgos") < 1 {
                p.push("`riesgos` está vacío".into());
            }
            if arr_len(v, "preguntas_abiertas") < 1 {
                p.push("`preguntas_abiertas` está vacío".into());
            }
            if arr_len(v, "fortalezas") < 1 {
                p.push("`fortalezas` está vacío".into());
            }
        }
        "spec_td" => {
            exige("titulo", 3, &mut p);
            exige("objetivo_visual", 20, &mut p);
            let script = txt("script_py");
            if script.chars().count() < 80 {
                p.push(format!(
                    "`script_py` es muy corto ({} caracteres)",
                    script.chars().count()
                ));
            } else if !script.contains("op(") && !script.contains("td.") {
                p.push("`script_py` no parece código de TouchDesigner (sin `op(` ni `td.`)".into());
            }
            if v["parametros"]
                .as_array()
                .map(|a| a.is_empty())
                .unwrap_or(true)
            {
                p.push("`parametros` está vacío".into());
            }
            if v["red_sugerida"]
                .as_array()
                .map(|a| a.is_empty())
                .unwrap_or(true)
            {
                p.push("`red_sugerida` está vacío".into());
            }
        }
        otro => p.push(format!("tipo de artefacto desconocido: «{otro}»")),
    }
    p
}

/// El artefacto como texto pegable en su destino (lo que usa el botón «Copiar»).
pub fn como_texto(tipo: &str, v: &Value) -> String {
    match tipo {
        "prompt_visual" => format!(
            "{}\n\n--- desglose ---\nSujeto: {}\nAmbiente: {}\nEstilo: {}\nIluminación: {}\nCámara: {}\nAspect ratio: {}\nNegativo: {}\n\n--- dirección artística ---\n{}\n\n--- 2 variaciones ---\n{}",
            v["prompt_final"].as_str().unwrap_or(""),
            v["sujeto"].as_str().unwrap_or(""),
            v["ambiente"].as_str().unwrap_or(""),
            v["estilo"].as_str().unwrap_or(""),
            v["iluminacion"].as_str().unwrap_or(""),
            v["camara"].as_str().unwrap_or(""),
            v["aspect_ratio"].as_str().unwrap_or(""),
            v["negativo"].as_str().unwrap_or(""),
            v["direccion_artistica"].as_str().unwrap_or(""),
            v["variaciones"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|x| {
                            format!(
                                "  [{}] {}",
                                x["enfoque"].as_str().unwrap_or(""),
                                x["prompt"].as_str().unwrap_or("")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default(),
        ),
        "brief_documento" => {
            let mut s = String::new();
            s += &format!("BRIEF: {}\n\n", v["titulo"].as_str().unwrap_or(""));
            s += &format!(
                "Objetivo: {}\nAudiencia: {}\nFormato pedido: {}\n\n",
                v["objetivo"].as_str().unwrap_or(""),
                v["audiencia"].as_str().unwrap_or(""),
                v["formato"].as_str().unwrap_or("")
            );
            s += "Secciones:\n";
            for sec in v["secciones"].as_array().cloned().unwrap_or_default() {
                s += &format!("  · {}\n", sec["titulo"].as_str().unwrap_or(""));
            }
            let datos = v["datos_clave"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str())
                        .map(|x| format!("  - {x}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            if !datos.is_empty() {
                s += &format!("\nDatos clave:\n{datos}\n");
            }
            s += &format!(
                "\n──── texto para pegar en Copilot ────\n{}",
                v["prompt_para_copilot"].as_str().unwrap_or("")
            );
            s
        }
        "critica" => {
            let lista = |campo: &str, v: &Value| -> String {
                v[campo]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str())
                            .map(|x| format!("  - {x}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default()
            };
            format!(
                "CRÍTICA: {}\nVeredicto: {}\n\nFortalezas:\n{}\n\nDebilidades:\n{}\n\nRiesgos:\n{}\n\nPreguntas abiertas:\n{}\n\nSiguiente paso: {}",
                v["titulo"].as_str().unwrap_or(""),
                v["veredicto"].as_str().unwrap_or(""),
                lista("fortalezas", v),
                lista("debilidades", v),
                lista("riesgos", v),
                lista("preguntas_abiertas", v),
                v["siguiente_paso"].as_str().unwrap_or("")
            )
        }
        "spec_td" => format!(
            "SPEC TD: {}\n\n{}\n\nParámetros:\n{}\n\nRed sugerida: {}\n\n── script ──\n{}",
            v["titulo"].as_str().unwrap_or(""),
            v["objetivo_visual"].as_str().unwrap_or(""),
            v["parametros"]
                .as_array()
                .map(|a| a
                    .iter()
                    .map(|x| format!(
                        "  · {} = {}  ({})",
                        x["nombre"].as_str().unwrap_or(""),
                        x["valor"].as_str().unwrap_or(""),
                        x["por_que"].as_str().unwrap_or("")
                    ))
                    .collect::<Vec<_>>()
                    .join("\n"))
                .unwrap_or_default(),
            v["red_sugerida"]
                .as_array()
                .map(|a| a
                    .iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join(" → "))
                .unwrap_or_default(),
            v["script_py"].as_str().unwrap_or("")
        ),
        _ => serde_json::to_string_pretty(v).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visual_ok() -> Value {
        json!({
            "titulo": "Tango ciberpunk",
            "prompt_final": "A rain-soaked Buenos Aires alley at night, a lone dancer in a mirrored suit, \
                              anamorphic 35mm lens, volumetric neon fog, high contrast, cinematic grade, \
                              film grain, cyan and magenta palette, shallow depth of field",
            "sujeto": "bailarín solitario con traje espejado",
            "ambiente": "callejón porteño bajo la lluvia",
            "estilo": "ciberpunk neo-barroco, 35mm anamórfico",
            "iluminacion": "neón volumétrico, contraluz alto contraste",
            "camara": "anamórfica 35mm, f/1.8, plano medio",
            "aspect_ratio": "16:9",
            "negativo": "texto, marca de agua, manos deformes, dedos fusionados, miembros extra",
            "direccion_artistica": "Se elige el cliché del bailarín romántico y se lo rompe con un traje espejado que refleja la ciudad en lugar del cuerpo, para que la metáfora sea el entorno y no el sujeto.",
            "variaciones": [
                {"enfoque": "abstracta y minimalista", "prompt": "Single mirrored figure dissolved into a flat plane of cyan neon, negative space dominant, ultra minimal composition, one continuous line of light, 35mm grain"},
                {"enfoque": "hiper-detallada", "prompt": "Extreme macro detail on thousands of mirror shards forming a dancer mid-spin, each shard reflecting a different neon sign, volumetric fog, anamorphic bokeh, hyper detailed 8k texture"}
            ]
        })
    }

    #[test]
    fn un_prompt_visual_completo_valida() {
        assert!(validar("prompt_visual", &visual_ok()).is_empty());
    }

    #[test]
    fn el_prompt_visual_detecta_lo_que_no_sirve() {
        let mut v = visual_ok();
        v["prompt_final"] = json!("un perro");
        v["aspect_ratio"] = json!("panoramico");
        v["negativo"] = json!("");
        let p = validar("prompt_visual", &v);
        assert!(
            p.iter()
                .any(|x| x.contains("prompt_final") && x.contains("corto")),
            "{p:?}"
        );
        assert!(p.iter().any(|x| x.contains("aspect_ratio")), "{p:?}");
        assert!(p.iter().any(|x| x.contains("negativo")), "{p:?}");
    }

    #[test]
    fn el_brief_exige_secciones_con_contenido() {
        let v = json!({
            "titulo": "Propuesta de servicio",
            "objetivo": "Presentar el servicio de síntesis de notas a un cliente nuevo",
            "audiencia": "dueño de una agencia chica",
            "formato": "docx",
            "secciones": [
                {"titulo": "Contexto", "contenido": "El cliente graba reuniones y nunca las vuelve a leer."},
                {"titulo": "Propuesta", "contenido": "x"},
                {"titulo": "Inversión", "contenido": "El costo se calcula por hora de audio procesada."}
            ],
            "datos_clave": ["15-30 min por audio", "entrega en 24h"],
            "prompt_para_copilot": "Necesito un documento Word para presentar un servicio de síntesis de notas. \
                                    El destinatario es el dueño de una agencia chica. Incluí contexto, propuesta, \
                                    alcance, inversión y próximos pasos, con un tono profesional y directo."
        });
        let p = validar("brief_documento", &v);
        assert_eq!(p.len(), 1, "solo la sección vacía debe fallar: {p:?}");
        assert!(p[0].contains("Propuesta"), "{p:?}");
    }

    #[test]
    fn la_spec_td_necesita_codigo_de_td() {
        let mut v = json!({
            "titulo": "Grid reactivo",
            "objetivo_visual": "Una grilla que responde al audio con desplazamiento de hue",
            "parametros": [{"nombre": "frequency", "valor": "0.4", "por_que": "pulso lento legible"}],
            "red_sugerida": ["audioin", "analyze", "chopto", "glsl"],
            "script_py": "raiz = op('/project1')\nruido = raiz.create('noiseTOP', 'ruido')\nruido.par.amp = 0.3\nruido.par.monochrome = 1\nnull1 = raiz.create('nullTOP', 'salida')\nnull1.inputConnectors[0].connect(ruido)\n"
        });
        assert!(validar("spec_td", &v).is_empty());
        v["script_py"] = json!("print('hola mundo')".repeat(6));
        assert!(validar("spec_td", &v)
            .iter()
            .any(|x| x.contains("TouchDesigner")));
    }

    #[test]
    fn una_critica_sin_debilidades_es_un_fracaso() {
        let mut v = json!({
            "titulo": "Servicio de síntesis de notas",
            "veredicto": "a_medias",
            "fortalezas": ["Ataca un dolor real: nadie relee sus reuniones"],
            "debilidades": ["El precio por hora de audio no está definido"],
            "riesgos": ["Sin límite de uso, el costo de API puede comerse el margen"],
            "preguntas_abiertas": ["¿Quién paga la transcripción?"],
            "siguiente_paso": "Definir el precio por hora de audio antes de mostrarlo"
        });
        let p = validar("critica", &v);
        assert_eq!(p.len(), 1, "solo la falta de una segunda debilidad: {p:?}");
        assert!(p[0].contains("debilidades"), "{p:?}");
        v["debilidades"] = json!(["una", "otra"]);
        v["veredicto"] = json!("buenisimo");
        let p2 = validar("critica", &v);
        assert_eq!(p2.len(), 1);
        assert!(p2[0].contains("veredicto"), "{p2:?}");
        assert!(como_texto("critica", &v).contains("Siguiente paso:"));
    }

    #[test]
    fn un_tipo_desconocido_avisa() {
        assert_eq!(validar("magia", &json!({})).len(), 1);
        assert!(schema("magia").is_none());
    }

    #[test]
    fn el_texto_pegable_incluye_lo_que_el_destino_necesita() {
        let t = como_texto("prompt_visual", &visual_ok());
        assert!(t.contains("A rain-soaked Buenos Aires"));
        assert!(t.contains("Aspect ratio: 16:9"));
        assert!(t.contains("desglose"));
    }
}

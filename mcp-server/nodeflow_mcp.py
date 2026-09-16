#!/usr/bin/env python3
"""Servidor MCP de NodeFlow — le da al agente (Hermes) ojos y manos sobre el lienzo.

Transporte: stdio, JSON-RPC 2.0 delimitado por saltos de línea (una línea = un mensaje).
Sin dependencias: solo stdlib, así no se rompe si cambia el entorno.

Habla con la API local de la app (Rust) en http://127.0.0.1:37371 — la misma que usa el
frontend, así que todo lo que escribe acá aparece en el lienzo en <= 3 s (polling de revisión)
y en el vault de Obsidian.

Herramientas: canvas_summary, canvas_stats, search_nodes, create_node, update_node,
connect_nodes, delete_node, vault_status.
"""

import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request

API = os.environ.get("NODEFLOW_API", "http://127.0.0.1:37371").rstrip("/")
PROTOCOL = "2024-11-05"
SERVER_INFO = {"name": "nodeflow", "version": "1.0.0"}


# ─────────────────────────── transporte HTTP local ───────────────────────────

def api(path, payload=None, method="GET", timeout=30):
    """Llama a la API local. Devuelve (ok, data_o_error_texto)."""
    url = API + path
    body = json.dumps(payload).encode("utf-8") if payload is not None else None
    req = urllib.request.Request(
        url, data=body, method=method, headers={"Content-Type": "application/json"}
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return True, json.loads(r.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        try:
            detalle = json.loads(e.read().decode("utf-8"))
        except Exception:
            detalle = {"error": f"HTTP {e.code}"}
        return False, detalle
    except Exception as e:  # conexión rechazada, timeout, etc.
        return False, {
            "error": f"no pude hablar con NodeFlow en {API} ({type(e).__name__}). "
            "¿Está abierta la app de escritorio?"
        }


# ──────────────────────────────── herramientas ───────────────────────────────

def _nodos(data):
    return data.get("nodos") or []


def _encabezado(data):
    st = data.get("stats") or {}
    return (
        f"MAPA: {data.get('mapa')} · {st.get('nodos')} nodos · {st.get('aristas')} aristas · "
        f"rev {data.get('revision')} · madurez promedio {st.get('madurez_promedio')}"
    )


def render_arbol(data, con_descripcion=True, max_nodos=200):
    """Dibuja el grafo como árbol desde el núcleo + lista lo que quedó fuera."""
    nodos = _nodos(data)[:max_nodos]
    por_id = {n["id"]: n for n in nodos}
    hijos, madres = {}, {}
    for n in nodos:
        for c in n.get("conexiones") or []:
            if c.get("dir") != "→":
                continue
            destino = next((x for x in nodos if x["titulo"] == c.get("titulo")), None)
            if not destino:
                continue
            hijos.setdefault(n["id"], []).append((destino["id"], c.get("label")))
            madres[destino["id"]] = n["id"]

    raiz = next((n["id"] for n in nodos if n.get("nucleo")), None)
    lineas, vistos = [], set()

    def etiqueta(n):
        partes = [n["titulo"]]
        meta = []
        if n.get("categoria"):
            meta.append(n["categoria"])
        if n.get("madurez") is not None:
            meta.append(f"madurez {n['madurez']}/5")
        if n.get("creado_por"):
            meta.append(f"por {n['creado_por']}")
        if meta:
            partes.append("[" + " · ".join(meta) + "]")
        return " ".join(partes)

    def recorrer(nid, prof, via):
        if nid in vistos or prof > 12:
            return
        vistos.add(nid)
        n = por_id[nid]
        prefijo = "  " * prof + ("↳ " if prof else "NÚCLEO ")
        flecha = f"—({via})→ " if via else ""
        lineas.append(f"{prefijo}{flecha}{etiqueta(n)}  (#{nid})")
        if con_descripcion and n.get("descripcion"):
            desc = " ".join(str(n["descripcion"]).split())
            lineas.append("  " * prof + f"    · {desc[:220]}")
        for cid, lab in hijos.get(nid, []):
            recorrer(cid, prof + 1, lab)

    if raiz:
        recorrer(raiz, 0, None)
    for n in nodos:  # sueltos / en ciclos
        if n["id"] not in vistos:
            recorrer(n["id"], 0, None)

    colgados = [n for n in nodos if not n.get("conexiones")]
    texto = "\n".join(lineas)
    if colgados:
        texto += "\n\nSIN CONEXIONES: " + ", ".join(f"{n['titulo']} (#{n['id']})" for n in colgados)
    return texto


def t_summary(args):
    ok, data = api("/api/graph/summary")
    if not ok:
        return texto_error(data)
    if not data.get("ok"):
        return f"El lienzo todavía no tiene estado en disco. {data.get('mensaje', '')}"
    con_desc = bool(args.get("include_descriptions", True))
    return f"{_encabezado(data)}\n\n{render_arbol(data, con_desc)}"


def t_stats(args):
    ok, data = api("/api/graph/summary")
    if not ok:
        return texto_error(data)
    if not data.get("ok"):
        return f"Sin estado en disco. {data.get('mensaje', '')}"
    st = data.get("stats") or {}
    return (
        f"MAPA: {data.get('mapa')}\n"
        f"nodos: {st.get('nodos')} · aristas: {st.get('aristas')} · "
        f"aristas colgadas: {st.get('aristas_colgadas')} · sin conexiones: {st.get('nodos_sin_conexiones')}\n"
        f"madurez promedio: {st.get('madurez_promedio')} · notas en disco: {st.get('notas_en_disco')}\n"
        f"vault: {data.get('vault')}\nrevision: {data.get('revision')}"
    )


def t_search(args):
    q = str(args.get("query") or "").strip().lower()
    if not q:
        return "Falta `query`."
    ok, data = api("/api/graph/summary")
    if not ok:
        return texto_error(data)
    limite = int(args.get("limit") or 12)
    hits = [
        n
        for n in _nodos(data)
        if q in (n.get("titulo") or "").lower()
        or q in str(n.get("descripcion") or "").lower()
        or q in str(n.get("categoria") or "").lower()
        or any(q in str(t).lower() for t in (n.get("tags") or []))
    ][:limite]
    if not hits:
        return f"Sin coincidencias para «{q}»."
    out = [f"{len(hits)} coincidencia(s) para «{q}»:"]
    for n in hits:
        out.append(f"\n• {n['titulo']}  (#{n['id']}) [{n.get('categoria')} · madurez {n.get('madurez')}]")
        if n.get("descripcion"):
            out.append(f"  {' '.join(str(n['descripcion']).split())[:200]}")
        if n.get("conexiones"):
            out.append(
                "  conexiones: "
                + "; ".join(f"{c.get('dir')} {c.get('titulo')}" for c in n["conexiones"][:6])
            )
    return "\n".join(out)


def t_create(args):
    if not str(args.get("title") or "").strip():
        return "Falta `title`."
    payload = {
        k: args[k]
        for k in ("title", "description", "category", "maturity", "parent", "link_label",
                  "tags", "x", "y", "colorAccent")
        if k in args
    }
    payload["prompt_original"] = str(args.get("prompt_original") or "creado por Hermes")
    ok, data = api("/api/graph/node", payload, "POST")
    if not ok:
        return texto_error(data)
    accion = data.get("accion")
    if accion in ("propuesto", "ya_propuesto"):
        return texto_propuesta(data)
    if accion == "creado":
        extra = f" conectado desde #{data.get('padre')}" if data.get("padre") else " (sin conexión)"
        return (
            f"NODO CREADO: «{payload['title']}»  id={data.get('id')}{extra}\n"
            f"lienzo: {data.get('nodos')} nodos · {data.get('aristas')} aristas · rev {data.get('revision')}\n"
            "Aparece en el lienzo y en el vault en ~3 s."
        )
    if accion == "actualizado":
        return f"NODO ACTUALIZADO: {data.get('id')} · campos: {', '.join(data.get('campos') or [])} · rev {data.get('revision')}"
    return f"Sin cambios ({data.get('id')})."


def t_update(args):
    if not args.get("id"):
        return "Falta `id` (usá `canvas_summary` o `search_nodes` para obtenerlo)."
    args = dict(args)
    # El backend exige `title` en el POST aunque sea una actualización: si no vino,
    # lo resolvemos desde el lienzo antes de escribir.
    if not str(args.get("title") or "").strip():
        ok, data = api("/api/graph/summary")
        if ok:
            buscado = str(args["id"]).lstrip("#")
            actual = next(
                (
                    n
                    for n in _nodos(data)
                    if n.get("id") == buscado
                    or (n.get("titulo") or "").lower() == buscado.lower()
                ),
                None,
            )
            if actual:
                args["title"] = actual["titulo"]
    ok, data = api("/api/graph/node", {**args, "prompt_original": "actualizado por Hermes"}, "POST")
    if not ok:
        return texto_error(data)
    if data.get("accion") in ("propuesto", "ya_propuesto"):
        return texto_propuesta(data)
    if data.get("accion") == "actualizado":
        return f"ACTUALIZADO {data.get('id')} · {', '.join(data.get('campos') or [])} · rev {data.get('revision')}"
    if data.get("accion") == "sin_cambios":
        return f"Sin cambios en {data.get('id')} (los valores ya eran esos)."
    return f"Resultado: {data.get('accion')} ({data.get('id')})"


def t_connect(args):
    if not args.get("source") or not args.get("target"):
        return "Faltan `source` y `target` (id o título de cada nodo)."
    ok, data = api("/api/graph/edge", args, "POST")
    if not ok:
        return texto_error(data)
    if data.get("accion") in ("propuesto", "ya_propuesto"):
        return texto_propuesta(data)
    if data.get("accion") == "ya_existia":
        return "Esa conexión ya existía; no dupliqué nada."
    return (
        f"CONECTADOS: #{data.get('origen')} → #{data.get('destino')} (arista {data.get('id')}, "
        f"rev {data.get('revision')})"
    )


def t_delete(args):
    if not args.get("id"):
        return "Falta `id`."
    ok, data = api("/api/graph/node/delete", args, "POST")
    if not ok:
        return texto_error(data)
    if data.get("accion") in ("propuesto", "ya_propuesto"):
        return texto_propuesta(data)
    return (
        f"BORRADO: «{data.get('titulo')}» ({data.get('id')}) y {data.get('aristas_borradas')} arista(s) · "
        f"quedan {data.get('nodos')} nodos · rev {data.get('revision')}"
    )


def t_pending(args):
    ok, data = api("/api/agent/pending")
    if not ok:
        return texto_error(data)
    if not data.get("total"):
        return "No hay propuestas pendientes: el panel está vacío."
    out = [f"{data['total']} propuesta(s) esperando aprobación (rev {data.get('revision')}):"]
    for p in data.get("pendientes") or []:
        v = p.get("vista") or {}
        out.append(
            f"\n• [{p['id']}] {v.get('accion_legible')} · peligro {v.get('peligro')}\n"
            f"  {v.get('resumen')}"
        )
        if p.get("motivo"):
            out.append(f"  motivo: {p['motivo']}")
    out.append("\nAprobar: approve_changes {id o todos:true} · Rechazar: reject_changes")
    return "\n".join(out)


def t_approve(args):
    ok, data = api("/api/agent/approve", args, "POST")
    if not ok:
        return texto_error(data)
    if data.get("accion") == "nada_pendiente":
        return "No había nada pendiente para aprobar."
    out = f"APROBADAS: {data.get('cantidad')} · rev lienzo {data.get('revision')} · quedan {data.get('pendientes')}"
    for d in data.get("detalle") or []:
        out += f"\n  · {d.get('id')} → {d.get('resultado')} ({d.get('detalle')})"
    for e in data.get("errores") or []:
        out += f"\n  ! {e.get('id')} sigue pendiente: {e.get('error')}"
    return out


def t_reject(args):
    ok, data = api("/api/agent/reject", args, "POST")
    if not ok:
        return texto_error(data)
    if data.get("accion") == "nada_pendiente":
        return "No había nada pendiente para rechazar."
    return f"RECHAZADAS: {data.get('cantidad')} · quedan {data.get('pendientes')} en la cola"


def t_repair(args):
    ok, data = api("/api/graph/prune", {}, "POST")
    if not ok:
        return texto_error(data)
    if data.get("accion") in ("propuesto", "ya_propuesto"):
        return texto_propuesta(data)
    if data.get("accion") == "nada_que_limpiar":
        return f"El grafo ya está sano: {data.get('aristas')} aristas, ninguna colgada."
    return (
        f"GRAFO SANEADO: saqué {data.get('aristas_quitadas')} arista(s) colgada(s) "
        f"({data.get('aristas_antes')} → {data.get('aristas')}) · rev {data.get('revision')}"
    )


def t_search_vault(args):
    q = str(args.get("query") or "").strip()
    if not q:
        return "Falta `query`."
    limite = int(args.get("limit") or 6)
    ok, data = api(f"/api/vault/search?q={urllib.parse.quote(q)}&limit={limite}")
    if not ok:
        return texto_error(data)
    if not data.get("ok"):
        return texto_error(data)
    res = data.get("resultados") or []
    if not res:
        return f"Sin coincidencias en la bóveda para «{q}» ({data.get('docs_indexados')} notas indexadas)."
    out = [
        f"MEMORIA DE LA BÓVEDA · {data.get('docs_indexados')} notas · {data.get('coincidencias')} "
        f"coincidencia(s) para «{q}» ({data.get('raiz')})"
    ]
    for r in res:
        marca = "[nodo del lienzo]" if r.get("ya_en_el_lienzo") else ""
        out.append(f"\n• {r['puntaje']} · {r['titulo']} {marca}\n  ruta: {r['ruta']}\n  {r['fragmento']}")
    out.append("\nPara usar una: leer_nota {ruta} y después create_node (queda como propuesta a aprobar).")
    return "\n".join(out)


def t_leer_nota(args):
    ruta = str(args.get("ruta") or "").strip()
    if not ruta:
        return "Falta `ruta` (relativa a la bóveda, ej. 05_Proyectos/nota.md)."
    ok, data = api(f"/api/vault/note?ruta={urllib.parse.quote(ruta)}")
    if not ok:
        return texto_error(data)
    texto = data.get("texto") or ""
    tope = int(args.get("max_chars") or 6000)
    recorte = "" if len(texto) <= tope else f"\n\n[…recortado: {len(texto)} caracteres en total]"
    return (
        f"NOTA: {data.get('titulo')} · {data.get('ruta')} ({data.get('caracteres')} caracteres)\n"
        f"{'-' * 60}\n{texto[:tope]}{recorte}"
    )


def t_garden_scan(args):
    ok, d = api("/api/graph/garden")
    if not ok:
        return texto_error(d)
    if d.get("error"):
        return f"No pude leer el lienzo: {d['error']}"
    st = d.get("stats") or {}
    out = [
        f"JARDÍN DEL LIENZO · {'SANO' if d.get('sano') else 'REQUIERE ATENCIÓN'} "
        f"· {len(d.get('problemas') or [])} hallazgo(s), {d.get('bloqueantes', 0)} bloqueante(s)",
        f"mapa: {d.get('mapa')} · {st.get('nodos')} nodos · {st.get('aristas')} aristas · "
        f"huérfanos: {st.get('huerfanos')} · sin descripción: {st.get('sin_descripcion')} · "
        f"sin madurez: {st.get('sin_madurez')}",
    ]
    orden = {"alta": 0, "media": 1, "baja": 2}
    for p in sorted(d.get("problemas") or [], key=lambda x: orden.get(x.get("gravedad"), 3)):
        out.append(f"\n[{p.get('gravedad').upper()}] {p.get('tipo')} → acción: {p.get('accion')}")
        out.append(f"  {p.get('detalle')}")
    pad = d.get("padrinos") or []
    if pad:
        out.append("\nPADRINOS SUGERIDOS (afinidad de contenido):")
        for p in pad:
            out.append(f"  · «{p.get('titulo')}» → colgar de «{p.get('padre_titulo')}» (similitud {p.get('similitud')})")
    out.append("\nUsá garden_fix para proponer los arreglos (el humano los aprueba en el panel) o tidy_canvas para el layout.")
    return "\n".join(out)


def t_garden_fix(args):
    ok, d = api("/api/graph/garden/fix", args or {}, "POST")
    if not ok:
        return texto_error(d)
    if not d.get("cantidad"):
        return "El jardín no encontró nada accionable: el grafo está limpio."
    out = [f"PROPUESTAS DEL JARDÍN: {d.get('cantidad')} (total en cola: {d.get('pendientes_totales')})"]
    for c in d.get("creadas") or []:
        extra = f" → #{c.get('id')}" if c.get("id") else ""
        out.append(f"  · {c.get('tipo')}{extra}: {c.get('resultado')} — {str(c.get('resumen'))[:110]}")
    if d.get("motivos"):
        out.append(f"motivos: {', '.join(d['motivos'])}")
    out.append("\n" + str(d.get("nota", "")))
    return "\n".join(out)


def t_tidy(args):
    ok, d = api("/api/graph/tidy", args or {}, "POST")
    if not ok:
        return texto_error(d)
    if d.get("accion") == "ya_ordenado":
        return d.get("mensaje", "El lienzo ya está en niveles.")
    v = d.get("vista") or {}
    return (
        f"REACOMODO PROPUESTO ({d.get('accion')}):\n"
        f"  {v.get('resumen')}\n"
        f"  id={d.get('id_pendiente')} · peligro {v.get('peligro')}\n"
        "El lienzo no se movió: aprobalo en «Cambios del agente»."
    )


def t_valor_medido(args):
    ok, d = api("/api/metrics")
    if not ok:
        return texto_error(d)
    objetivo = d.get("objetivo_min", 3.0)
    prom = d.get("promedio_min")
    out = [f"MÉTRICA DE VALOR · objetivo: menos de {objetivo} min entre el brain dump y el primer artefacto aprobado"]
    if prom is None:
        out.append("Todavía no hay ninguna conversión completa (T0→T1) registrada.")
    else:
        out.append(f"promedio: {prom} min · conversiones completas: {d.get('conversiones')} · última: {d.get('ultima_min')} min")
    if d.get("sesion_activa"):
        estado = "ya con artefacto aprobado" if d.get("t1_ms") else "esperando el primer artefacto aprobado"
        out.append(f"sesión en curso: {d.get('minutos_desde_t0')} min desde T0 · {estado}")
    else:
        out.append("no hay sesión activa (se abre con la primera escritura en el lienzo)")
    out.append("T0 = primera escritura humana de la sesión · T1 = primera propuesta de IA que aprobás.")
    return "\n".join(out)


def t_capture_knowledge(args):
    texto = str(args.get("texto") or "").strip()
    if len(texto) < 40:
        return "Falta `texto` (o es demasiado corto para extraer conocimiento)."
    ok, prev = api("/api/knowledge/preview", {"texto": texto}, "POST")
    if not ok:
        return texto_error(prev)
    nodos = prev.get("candidatos") or []
    if not nodos:
        return "El texto no produjo candidatos: los bloques son muy cortos o no tienen densidad suficiente."
    payload = {
        "nodos": nodos,
        "parent": args.get("parent") or "",
        "categoria": args.get("categoria") or "CONOCIMIENTO",
        "madurez": args.get("madurez") or 2,
        "motivo": args.get("motivo") or "Capturado desde el chat",
    }
    ok, d = api("/api/knowledge/capture", payload, "POST")
    if not ok:
        return texto_error(d)
    out = [
        f"CAPTURA: {d.get('propuestos')} nodo(s) propuesto(s) · cola total: {d.get('pendientes_totales')}",
        f"(se extrajeron de {prev.get('caracteres')} caracteres)",
    ]
    for c in nodos:
        marca = "  [ya está en el lienzo]" if c.get("ya_en_el_lienzo") else ""
        out.append(f"  · «{c.get('titulo')}» ({c.get('caracteres')} car.){marca}")
    out.append("\nNada entró al lienzo: el humano aprueba en «Cambios del agente».")
    return "\n".join(out)


def t_export_document(args):
    ok, d = api("/api/export/document")
    if not ok:
        return texto_error(d)
    contenido = d.get("contenido") or ""
    if args.get("completo"):
        return contenido
    return (
        f"DOCUMENTO LISTO: {d.get('nombre')} · {d.get('nodos')} nodos · {d.get('caracteres')} caracteres\n"
        "Descargable desde el panel Conocimiento → Exportar. Vista previa:\n\n"
        + contenido[:1500]
        + "\n\n[…pedí completo=True para el texto entero]"
    )


def t_list_experts(args):
    ok, d = api("/api/expertos")
    if not ok:
        return texto_error(d)
    out = [f"EXPERTOS ({len(d.get('expertos', []))}) · carpeta: {d.get('carpeta')}"]
    for e in d.get("expertos", []):
        marca = "" if e.get("valido") else "  [TIPO INVÁLIDO]"
        out.append(f"  · {e.get('nombre')}  → {e.get('tipo_artefacto')}  "
                   f"({e.get('caracteres_system')} car. de prompt)"
                   f"{' · proveedor: ' + e['proveedor'] if e.get('proveedor') else ''}{marca}")
        if e.get("descripcion"):
            out.append(f"      {e['descripcion']}")
    out.append("\nTipos de artefacto y su destino:")
    for t in d.get("tipos", []):
        out.append(f"  · {t.get('tipo')} → {t.get('destino')}: {t.get('descripcion')}")
    return "\n".join(out)


def t_run_expert(args):
    nodo = str(args.get("nodo") or "").strip()
    experto = str(args.get("experto") or "").strip()
    if not nodo or not experto:
        return "Faltan `nodo` y `experto`. Mirá `list_experts` para los disponibles."
    ok, d = api("/api/expert/run", {
        "nodo": nodo, "experto": experto, "extra": str(args.get("extra") or ""),
    }, "POST", timeout=300)
    if not ok:
        return texto_error(d)
    out = [
        f"{'ARTEFACTO VÁLIDO' if d.get('ok') else 'ARTEFACTO CON PROBLEMAS'} · {d.get('tipo')} "
        f"· experto «{d.get('experto')}»",
        f"nodo: {d.get('nodo', {}).get('titulo')}  ·  {d.get('proveedor')}  ·  "
        f"{d.get('ms')} ms  ·  {d.get('intentos')} intento(s)  ·  contexto {d.get('contexto_chars')} car.",
    ]
    for p in d.get("traza", []):
        est = "✓" if p.get("valido") else ("sin respuesta" if p.get("resultado") else "✗ contrato")
        out.append(f"  proveedor {p.get('proveedor')}: {est} ({p.get('ms')} ms)")
    if d.get("problemas"):
        out.append("PROBLEMAS QUE EL VALIDADOR MARCÓ:")
        out.extend(f"  - {x}" for x in d["problemas"])
    if d.get("fuentes"):
        out.append("contexto de la bóveda: " + " · ".join(
            (f.get("titulo") or "")[:40] for f in d["fuentes"][:3]))
    out.append("\n──── artefacto listo para su destino ────")
    out.append(d.get("texto", ""))
    return "\n".join(out)


def t_vault(args):
    ok, info = api("/api/vault/info")
    if not ok:
        return texto_error(info)
    ext = info.get("ultimos_cambios_externos") or []
    return (
        f"RUTA DEL VAULT: {info.get('vault')}\n"
        f"mapa: {info.get('mapa')} · revision: {info.get('revision')} · notas: {info.get('notas')}\n"
        f"nodos en disco: {info.get('nodos_en_disco')} · hay estado: {info.get('tiene_estado')}\n"
        f"últimos cambios externos (Obsidian): {', '.join(ext) if ext else 'ninguno'}"
    )


def texto_propuesta(data):
    v = data.get("vista") or {}
    ya = " (ya había una propuesta igual en la cola)" if data.get("accion") == "ya_propuesto" else ""
    return (
        f"PROPUESTA registrada{ya} — el lienzo NO cambió:\n"
        f"  {v.get('resumen')}\n"
        f"  id={data.get('id_pendiente')} · peligro={v.get('peligro')} · en cola: {data.get('pendientes')}\n"
        "Se aprueba o rechaza en el panel «Cambios del agente» de la app "
        "(o pedime que la apruebe/rechace por acá)."
    )


def texto_error(data):
    if isinstance(data, dict) and data.get("error"):
        return f"ERROR: {data['error']}"
    return f"ERROR: {data}"


# ── Fase 5.3: el registro de herramientas del cerebro ─────────────────────────────────────────
# El agente crea herramientas (scripts en la bóveda); el humano las aprueba; acá se exponen como tools
# MCP de primera clase, así el modelo las ve y las usa como cualquier otra. La ejecución la hace el
# backend de la app (jaula, tope de tiempo y auditoría viven ahí, no en este adaptador).
PREFIJO_HERRAMIENTA = "cerebro_"
# `None` = el cliente todavía no pidió la lista: en el primer listado no hay nada nuevo que avisar.
_ultimas_herramientas = None
# Bandera: el aviso «la lista cambió» se emite DESPUÉS de la respuesta, nunca antes. Una notificación
# metida entre el request y su respuesta desalinea a los clientes simples (nos pasó midiendo).
_avisar_lista = False


def registro():
    """Las herramientas ya aprobadas (vacío si la app no responde)."""
    ok, d = api("/api/cerebro/herramientas")
    if not ok:
        return []
    return d.get("herramientas") or []


def tools_del_registro():
    """Cada herramienta del registro se expone como una tool MCP más."""
    out = []
    for h in registro():
        nombre = str(h.get("nombre") or "").strip()
        if not nombre:
            continue
        out.append({
            "name": PREFIJO_HERRAMIENTA + nombre,
            "description": f"[herramienta del cerebro] {h.get('descripcion')} · riesgo: {h.get('riesgo')}",
            "inputSchema": h.get("parametros") or {"type": "object", "properties": {}},
        })
    return out


def llamar_herramienta(nombre, args):
    """Corre una herramienta del registro por el backend (que aplica la jaula y el tope)."""
    ok, d = api("/api/cerebro/herramienta", {"nombre": nombre, "parametros": args or {}}, "POST")
    if not ok:
        return f"«{nombre}» falló: {d.get('error') or d}", True
    return f"{d.get('salida')}\n\n({nombre} · {d.get('ms')} ms)", False


def t_mi_espacio(args):
    """Lee mi espacio: la bitácora (criterio acumulado) y mis planes. Es lo primero que conviene mirar
    antes de proponer algo grande: evita repetir una decisión ya tomada."""
    ok, d = api("/api/cerebro/espacio")
    if not ok:
        return f"No pude leer mi espacio: {d.get('error') or d}"
    bit = (d.get("bitacora") or {}).get("texto") or ""
    lineas = [l for l in bit.splitlines() if l.strip()]
    cola = "\n".join(lineas[-40:]) if lineas else "(la bitácora todavía está vacía)"
    planes = d.get("planes") or []
    lista = "\n".join(
        f"- {p.get('nombre')} · {p.get('chars')} chars · {p.get('titulo')}" for p in planes
    ) or "(todavía no hay planes míos)"
    return (
        f"# Mi espacio\n\nTurnos cumplidos: {d.get('turnos')} · carpeta: {d.get('carpeta')}\n\n"
        f"## Planes\n{lista}\n\n## Bitácora (últimas entradas)\n{cola}"
    )


def t_anotar_bitacora(args):
    """Escribe una entrada en mi bitácora: decisiones, criterios, por qué hice algo. Append-only."""
    ok, d = api(
        "/api/cerebro/espacio/nota",
        {"tipo": "bitacora", "quien": "cerebro", "contenido": args.get("texto", "")},
        "POST",
    )
    if not ok:
        return f"No pude anotar: {d.get('error') or d}"
    return f"Anotado en la bitácora ({d.get('ruta')})."


def t_escribir_plan(args):
    """Escribe (o reemplaza) uno de mis planes: `cerebro/planes/<nombre>.md`. Un plan por tema."""
    ok, d = api(
        "/api/cerebro/espacio/nota",
        {
            "tipo": "plan",
            "nombre": args.get("nombre", ""),
            "estado": args.get("estado", "borrador"),
            "contenido": args.get("contenido", ""),
        },
        "POST",
    )
    if not ok:
        return f"No pude escribir el plan: {d.get('error') or d}"
    return f"Plan escrito en {d.get('ruta')} (nombre: {d.get('slug')})."


def t_crear_herramienta(args):
    """Propone una herramienta nueva: entra a la cola de propuestas y el humano la aprueba."""
    ok, d = api("/api/cerebro/herramienta/proponer", args, "POST")
    if not ok:
        return f"No pude proponer la herramienta: {d.get('error') or d}"
    vista = d.get("vista") or {}
    return (
        f"Propuesta registrada ({d.get('id_pendiente')}): {vista.get('resumen')}\n"
        f"Riesgo: {vista.get('riesgo')} · se escribe en {vista.get('ubicacion')}\n"
        "Espera aprobación humana en «Cambios del agente»; cuando la aprueben, la vas a ver como tool."
    )


def avisar_lista_cambiada():
    """Marca que hay que avisar que el registro cambió (una herramienta nueva aprobada): Hermes
    re-registra en caliente. No escribe acá: `main` lo emite después de la respuesta."""
    global _avisar_lista
    _avisar_lista = True


TOOLS = [
    {
        "name": "mi_espacio",
        "description": (
            "Lee MI espacio (el del cerebro residente): la bitácora de decisiones y mis planes en la bóveda. "
            "Conviene mirarlo antes de proponer algo grande, para no repetir una decisión ya tomada."
        ),
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "anotar_bitacora",
        "description": (
            "Anota una entrada en mi bitácora de decisiones (bóveda: cerebro/bitacora.md). Usar para dejar "
            "criterio durable: qué decidí, por qué, y qué queda pendiente."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "texto": {"type": "string", "description": "La entrada (markdown corto, 1-6 líneas)."},
            },
            "required": ["texto"],
        },
    },
    {
        "name": "escribir_plan",
        "description": (
            "Escribe o reemplaza uno de mis planes en la bóveda (cerebro/planes/<nombre>.md). Un plan por tema, "
            "con estado. El briefing del turno siguiente lo lee."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "nombre": {"type": "string", "description": "Tema del plan (se convierte en slug)."},
                "contenido": {"type": "string", "description": "El plan en markdown."},
                "estado": {"type": "string", "description": "borrador | activo | hecho", "default": "borrador"},
            },
            "required": ["nombre", "contenido"],
        },
    },
    {
        "name": "crear_herramienta",
        "description": (
            "Propone una HERRAMIENTA nueva para el cerebro de NodeFlow: un script Python (run.py) que "
            "recibe los parámetros por stdin en JSON. Se guarda en <bóveda>/cerebro/herramientas/<nombre>/ "
            "y pasa a la cola de aprobación; cuando el humano la aprueba, queda disponible como tool "
            "`cerebro_<nombre>` en el próximo turno. Usala cuando una tarea se repita o el proyecto necesite "
            "una capacidad que hoy no existe. El nombre es un slug (minúsculas, números y _, 3-40)."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "nombre": {"type": "string", "description": "Slug: resumen_lienzo, contar_categorias…"},
                "descripcion": {"type": "string", "description": "Qué hace y cuándo usarla (lo lee el modelo)."},
                "parametros": {"type": "object", "description": "JSON-Schema de los parámetros que recibe."},
                "riesgo": {"type": "string", "description": "lectura · escritura · destructiva"},
                "codigo": {"type": "string", "description": "El código de run.py (lee el JSON de stdin)."},
            },
            "required": ["nombre", "descripcion", "codigo"],
        },
    },
    {
        "name": "canvas_summary",
        "description": (
            "Lee el lienzo NodeFlow completo como árbol desde el nodo núcleo: títulos, categorías, "
            "madurez, descripciones y conexiones con su etiqueta. Usalo ANTES de escribir para no "
            "duplicar conceptos. Devuelve también los ids (#id) que necesitan las otras herramientas."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "include_descriptions": {
                    "type": "boolean",
                    "description": "Incluir las descripciones de cada nodo (por defecto true).",
                }
            },
        },
    },
    {
        "name": "canvas_stats",
        "description": "Métricas del lienzo: nodos, aristas, aristas colgadas, nodos sin conexiones, madurez promedio y ruta del vault.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "search_nodes",
        "description": "Busca nodos por texto en título, descripción, categoría o tags.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Texto a buscar (sin distinguir mayúsculas)."},
                "limit": {"type": "integer", "description": "Máximo de resultados (por defecto 12)."},
            },
            "required": ["query"],
        },
    },
    {
        "name": "create_node",
        "description": (
            "Crea un nodo en el lienzo y, si pasás `parent`, lo conecta con una arista etiquetada. "
            "Si ya existe un nodo con ese título, lo actualiza en vez de duplicarlo. Does NOT touch "
            "the canvas by default: it registers a PROPOSAL the human approves in the app panel; "
            "pass mode='apply' to write it straight through."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "title": {"type": "string", "description": "Título del nodo (obligatorio)."},
                "description": {"type": "string", "description": "Cuerpo del nodo: la idea completa."},
                "category": {"type": "string", "description": "Etiqueta corta en mayúsculas, ej. ARQUITECTURA."},
                "maturity": {"type": "integer", "description": "Madurez 1-5 (1 semilla, 5 ejecutable)."},
                "parent": {"type": "string", "description": "id (#...) o título del nodo padre del que cuelga."},
                "link_label": {"type": "string", "description": "Etiqueta de la arista, ej. «habilita»."},
                "tags": {"type": "array", "items": {"type": "string"}},
                "x": {"type": "number", "description": "Posición X (opcional; si no, se calcula)."},
                "y": {"type": "number", "description": "Posición Y (opcional)."},
                "mode": {"type": "string", "description": "\"propose\" (por defecto) deja la escritura como propuesta a aprobar; \"apply\" la aplica directo."},
            },
            "required": ["title"],
        },
    },
    {
        "name": "update_node",
        "description": "Actualiza título, descripción, categoría, madurez o tags de un nodo existente (por id o título). Por defecto la escritura queda como PROPUESTA pendiente de aprobación humana en la app; pasá mode='apply' para aplicarla directo.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "string", "description": "id del nodo (#...) o su título exacto."},
                "title": {"type": "string"},
                "description": {"type": "string"},
                "category": {"type": "string"},
                "maturity": {"type": "integer"},
                "tags": {"type": "array", "items": {"type": "string"}},
                "x": {"type": "number"},
                "y": {"type": "number"},
                "mode": {"type": "string", "description": "\"propose\" (por defecto) deja la escritura como propuesta a aprobar; \"apply\" la aplica directo."},
            },
            "required": ["id"],
        },
    },
    {
        "name": "connect_nodes",
        "description": "Conecta dos nodos existentes con una arista etiquetada (no duplica si ya existe). Por defecto la escritura queda como PROPUESTA pendiente de aprobación humana en la app; pasá mode='apply' para aplicarla directo.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "source": {"type": "string", "description": "id o título del origen."},
                "target": {"type": "string", "description": "id o título del destino."},
                "label": {"type": "string", "description": "Etiqueta de la relación."},
                "direction": {"type": "string", "description": "«<-» invierte el sentido (target→source)."},
            },
            "required": ["source", "target"],
        },
    },
    {
        "name": "delete_node",
        "description": "Borra un nodo y sus aristas del lienzo (nunca el núcleo). Operación destructiva: confirmá con el usuario antes. Por defecto la escritura queda como PROPUESTA pendiente de aprobación humana en la app; pasá mode='apply' para aplicarla directo.",
        "inputSchema": {
            "type": "object",
            "properties": {"id": {"type": "string", "description": "id (#...) o título del nodo a borrar."}},
            "required": ["id"],
        },
    },
    {
        "name": "garden_scan",
        "description": (
            "Diagnóstico del lienzo (solo lectura): invariantes (aristas colgadas, ids repetidos, "
            "islas, nodos basura, sin madurez) y sugerencias de a quién conectar cada nodo huérfano "
            "por afinidad de contenido. Es el punto de partida de una sesión de orden: escaneá antes "
            "de proponer nada."
        ),
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "garden_fix",
        "description": (
            "Convierte los hallazgos del jardín en PROPUESTAS listas para aprobar: saneo de "
            "integridad, borrado de nodos que son archivos generados, y las conexiones sugeridas por "
            "afinidad. No toca el lienzo."
        ),
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "tidy_canvas",
        "description": (
            "Calcula un layout por niveles (el árbol se lee de izquierda a derecha, sin "
            "solapamientos) y lo propone para aprobar. Reemplaza el apilado automático de nodos."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {"motivo": {"type": "string", "description": "Por qué lo proponés."}},
        },
    },
    {
        "name": "capture_knowledge",
        "description": (
            "Convierte texto crudo (una lista de temas, apuntes, un documento pegado) en NODOS "
            "PROPUESTOS para la bóveda y el lienzo. Segmenta localmente por secciones, descarta lo "
            "que no tiene densidad y deja todo en la cola de aprobación: no escribe nada. Usalo para "
            "alimentar la memoria del sistema sin engordarla."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "texto": {"type": "string", "description": "El texto a convertir en nodos."},
                "parent": {"type": "string", "description": "id o título del nodo del que cuelgan (opcional)."},
                "categoria": {"type": "string", "description": "Categoría por defecto (CONOCIMIENTO)."},
                "madurez": {"type": "integer", "description": "Madurez por defecto (2)."},
                "motivo": {"type": "string", "description": "Por qué se captura."},
            },
            "required": ["texto"],
        },
    },
    {
        "name": "export_document",
        "description": (
            "Genera el mapa como documento Markdown legible (en orden de lectura, con conexiones), "
            "listo para compartir o para el portafolio."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {"completo": {"type": "boolean", "description": "true = devolver el documento entero."}},
        },
    },
    {
        "name": "list_experts",
        "description": (
            "Lista los EXPERTOS disponibles (notas de la bóveda en expertos/ con un system prompt y un "
            "tipo de artefacto) y los tipos de artefacto con su destino. Usalo antes de run_expert."
        ),
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "run_expert",
        "description": (
            "Ejecuta un EXPERTO sobre un nodo del lienzo: arma el contexto (nodo + vecinos + memoria "
            "de la bóveda), llama al modelo y devuelve un ARTEFACTO VALIDADO contra su destino "
            "(prompt_visual para Flow, brief_documento para Copilot, spec_td para TouchDesigner, "
            "critica). Tarda entre 15 y 90 segundos. NO escribe en el lienzo: el artefacto vuelve a vos."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "nodo": {"type": "string", "description": "id o título del nodo del lienzo (el concepto)."},
                "experto": {"type": "string", "description": "nombre del experto (ver list_experts)."},
                "extra": {"type": "string", "description": "indicaciones extra para este artefacto."},
            },
            "required": ["nodo", "experto"],
        },
    },
    {
        "name": "pending_changes",
        "description": (
            "Lista las escrituras que propuse y todavía no fueron aprobadas ni rechazadas. "
            "Usalo para saber si el humano ya actuó sobre lo que propusiste."
        ),
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "approve_changes",
        "description": (
            "Aprueba propuestas pendientes y las aplica al lienzo. Usalo SOLO si el usuario te lo pidió "
            "explícitamente en el chat (si no, que las apruebe él en el panel)."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "string", "description": "id de la propuesta (p-...)."},
                "todos": {"type": "boolean", "description": "true = aprobar toda la cola."},
            },
        },
    },
    {
        "name": "reject_changes",
        "description": "Rechaza propuestas pendientes (las descarta sin tocar el lienzo).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "string", "description": "id de la propuesta (p-...)."},
                "todos": {"type": "boolean", "description": "true = rechazar toda la cola."},
            },
        },
    },
    {
        "name": "search_vault",
        "description": (
            "Busca en TODA la bóveda de Obsidian del usuario (sus notas y los nodos del lienzo) con "
            "BM25: acentos plegados, títulos priorizados, devuelve fragmento y ruta. Usalo para nutrir "
            "el lienzo con lo que el usuario ya escribió, en vez de inventar conceptos."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Términos a buscar (sin acentos también funciona)."},
                "limit": {"type": "integer", "description": "Máximo de coincidencias (por defecto 6)."},
            },
            "required": ["query"],
        },
    },
    {
        "name": "leer_nota",
        "description": "Lee el texto completo de una nota de la bóveda por su ruta relativa (la que devuelve search_vault).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "ruta": {"type": "string", "description": "Ruta relativa, ej. 02_Playbooks/stack.md"},
                "max_chars": {"type": "integer", "description": "Recorte máximo (por defecto 6000)."},
            },
            "required": ["ruta"],
        },
    },
    {
        "name": "valor_medido",
        "description": (
            "Métrica de valor del sistema: minutos entre el brain dump (T0) y el primer artefacto "
            "aprobado (T1). Es el número que decide si la herramienta acelera el trabajo de verdad; "
            "el objetivo declarado es menos de 3 minutos."
        ),
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "vault_status",
        "description": "Estado del vault en disco: ruta, revisión, cantidad de notas y últimos cambios hechos desde Obsidian.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "repair_canvas",
        "description": (
            "Saca del lienzo las aristas colgadas: las que apuntan a nodos que ya no existen "
            "(invisibles en pantalla, pero ensucian el grafo y el vault). Es una limpieza segura: "
            "no toca nodos ni conexiones válidas."
        ),
        "inputSchema": {"type": "object", "properties": {}},
    },
]

HANDLERS = {
    "crear_herramienta": t_crear_herramienta,
    "mi_espacio": t_mi_espacio,
    "anotar_bitacora": t_anotar_bitacora,
    "escribir_plan": t_escribir_plan,
    "canvas_summary": t_summary,
    "canvas_stats": t_stats,
    "search_nodes": t_search,
    "create_node": t_create,
    "update_node": t_update,
    "connect_nodes": t_connect,
    "delete_node": t_delete,
    "repair_canvas": t_repair,
    "valor_medido": t_valor_medido,
    "vault_status": t_vault,
    "garden_scan": t_garden_scan,
    "garden_fix": t_garden_fix,
    "tidy_canvas": t_tidy,
    "capture_knowledge": t_capture_knowledge,
    "list_experts": t_list_experts,
    "run_expert": t_run_expert,
    "export_document": t_export_document,
    "search_vault": t_search_vault,
    "leer_nota": t_leer_nota,
    "pending_changes": t_pending,
    "approve_changes": t_approve,
    "reject_changes": t_reject,
}


# ──────────────────────────── protocolo JSON-RPC ─────────────────────────────

def resultado(rid, texto, error=False):
    return {
        "jsonrpc": "2.0",
        "id": rid,
        "result": {"content": [{"type": "text", "text": texto}], "isError": bool(error)},
    }


def manejar(msg):
    metodo = msg.get("method")
    rid = msg.get("id")

    if metodo == "initialize":
        pedido = (msg.get("params") or {}).get("protocolVersion") or PROTOCOL
        return {
            "jsonrpc": "2.0",
            "id": rid,
            "result": {
                "protocolVersion": pedido,
                # listChanged: el registro puede crecer en caliente (una herramienta nueva aprobada),
                # así que el cliente tiene que poder re-registrar sin reiniciar nada.
                "capabilities": {"tools": {"listChanged": True}},
                "serverInfo": SERVER_INFO,
            },
        }
    if metodo in ("notifications/initialized", "initialized", "notifications/cancelled"):
        return None
    if metodo == "ping":
        return {"jsonrpc": "2.0", "id": rid, "result": {}}
    if metodo == "tools/list":
        global _ultimas_herramientas
        del_registro = tools_del_registro()
        todas = TOOLS + del_registro
        nombres = [t["name"] for t in todas]
        # Sólo se avisa si la lista cambió después de que el cliente ya la conocía.
        if _ultimas_herramientas is not None and nombres != _ultimas_herramientas:
            avisar_lista_cambiada()
        _ultimas_herramientas = nombres
        return {"jsonrpc": "2.0", "id": rid, "result": {"tools": todas}}
    if metodo == "resources/list":
        return {"jsonrpc": "2.0", "id": rid, "result": {"resources": []}}
    if metodo == "prompts/list":
        return {"jsonrpc": "2.0", "id": rid, "result": {"prompts": []}}
    if metodo == "tools/call":
        params = msg.get("params") or {}
        nombre = params.get("name")
        args = params.get("arguments") or {}
        if isinstance(nombre, str) and nombre.startswith(PREFIJO_HERRAMIENTA):
            texto, fallo = llamar_herramienta(nombre[len(PREFIJO_HERRAMIENTA):], args)
            return resultado(rid, texto, error=fallo)
        fn = HANDLERS.get(nombre)
        if fn is None:
            return resultado(rid, f"Herramienta desconocida: {nombre}", error=True)
        try:
            return resultado(rid, fn(args) if isinstance(args, dict) else fn({}))
        except Exception as e:
            return resultado(rid, f"ERROR en {nombre}: {type(e).__name__}: {e}", error=True)
    if rid is None:
        return None  # notificación desconocida: se ignora
    return {
        "jsonrpc": "2.0",
        "id": rid,
        "error": {"code": -32601, "message": f"método no soportado: {metodo}"},
    }


def _emitir_aviso_si_hace_falta(out):
    """Emite el `tools/list_changed` pendiente, siempre después de la respuesta del request."""
    global _avisar_lista
    if not _avisar_lista:
        return
    _avisar_lista = False
    try:
        out.write(
            json.dumps({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"}).encode("utf-8") + b"\n"
        )
        out.flush()
    except Exception:
        pass


def main():
    out = sys.stdout.buffer
    for linea in sys.stdin.buffer:
        linea = linea.strip()
        if not linea:
            continue
        try:
            msg = json.loads(linea.decode("utf-8"))
        except Exception:
            continue
        if isinstance(msg, list):  # batch
            respuestas = [r for r in (manejar(m) for m in msg) if r]
            if respuestas:
                out.write(json.dumps(respuestas, ensure_ascii=False).encode("utf-8") + b"\n")
                out.flush()
            continue
        respuesta = manejar(msg)
        if respuesta is not None:
            out.write(json.dumps(respuesta, ensure_ascii=False).encode("utf-8") + b"\n")
            out.flush()
        _emitir_aviso_si_hace_falta(out)


if __name__ == "__main__":
    main()

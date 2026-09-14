#!/usr/bin/env python3
"""NodeFlow · Generador del mapa del proyecto.

El mapa del lienzo no se mantiene a mano: se **genera de la evidencia**, igual que la bitácora sale
de git. Este script lee el historial (y las releases etiquetadas), arma el estado del proyecto y deja
**una propuesta** en la cola de la app — que el humano aprueba. Si nada cambió desde la última vez,
no dice nada: el silencio es la señal de que el mapa está al día.

Uso:
    scripts/mapa.py                 # genera y propone si cambió
    scripts/mapa.py --dry           # muestra lo que propondría, sin proponer
    scripts/mapa.py --dias 14       # ventana de historia (7 por defecto)
"""
from __future__ import annotations
import hashlib
import json
import subprocess
import sys
import urllib.error
import urllib.request
from datetime import date
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
API = "http://127.0.0.1:37371"
MEMORIA = RAIZ / ".git" / "mapa.ultimo"          # hash de lo último propuesto (no se versiona)
TITULO = "Estado del proyecto"
NORTE = "Norte Estratégico · NodeFlow"

TIPOS = [("feat", "Cerró"), ("fix", "Corrigió"), ("perf", "Aceleró"), ("docs", "Documentó")]


def sh(*args: str) -> str:
    return subprocess.run(args, cwd=RAIZ, capture_output=True, text=True,
                          encoding="utf-8", errors="replace").stdout.strip()


def historia(dias: int) -> tuple[str, str]:
    hasta = f"{dias} days ago"
    partes = [f"Estado al {date.today().isoformat()} · rama {sh('git','branch','--show-current') or '?'}"]
    total = sh("git", "rev-list", "--count", f"--since={hasta}", "HEAD")
    partes.append(f"{total or '0'} commits en {dias} días.")

    for tipo, etiqueta in TIPOS:
        lineas = [l for l in sh("git", "log", f"--since={hasta}", "--pretty=format:%s").split("\n")
                  if l.startswith(tipo) and l.strip()]
        if not lineas:
            continue
        limpio = [l.split(": ", 1)[-1].strip() for l in lineas][:12]
        partes.append(f"{etiqueta}: " + " · ".join(limpio))

    tags = [t for t in sh("git", "tag", "--sort=-creatordate").split("\n") if t.strip()]
    if tags:
        partes.append(f"Última release etiquetada: {tags[0]}.")

    # La huella es el estado derivado de git, ANTES de agregar nada volátil.
    base = "\n".join(partes)

    # Lo que está trabado en la cola: el mapa también dice qué espera decisión humana. Va en la
    # descripción pero NO en la huella: el conteo cambia solo con proponer esta misma propuesta.
    try:
        with urllib.request.urlopen(f"{API}/api/agent/pending", timeout=10) as r:
            pend = json.loads(r.read()).get("total", 0)
        if pend:
            partes.append(f"{pend} propuesta(s) esperando aprobación humana.")
    except Exception:
        pass
    return "\n".join(partes), base


def proponer(descripcion: str) -> str:
    req = urllib.request.Request(
        f"{API}/api/graph/node",
        data=json.dumps({"title": TITULO, "description": descripcion, "category": "ESTADO",
                         "maturity": 3, "parent": NORTE, "prompt_original": "scripts/mapa.py"},
                        ensure_ascii=False).encode(),
        headers={"Content-Type": "application/json"}, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            return json.loads(r.read()).get("accion", "?")
    except urllib.error.HTTPError as e:
        return f"HTTP {e.code}: {e.read().decode()[:120]}"
    except Exception as e:
        return f"sin backend ({type(e).__name__})"


def main() -> int:
    args = sys.argv[1:]
    dias = 7
    if "--dias" in args:
        dias = int(args[args.index("--dias") + 1])
    texto, base = historia(dias)
    huella = hashlib.sha256(base.encode()).hexdigest()[:16]

    if "--dry" in args:
        print(texto)
        return 0
    if MEMORIA.exists() and MEMORIA.read_text().strip() == huella:
        return 0                                   # nada nuevo: silencio
    accion = proponer(texto)
    if accion.startswith(("propuesto", "actualizado", "ya_propuesto")):
        MEMORIA.write_text(huella)
    print(f"mapa: {accion} · «{TITULO}» ({dias} días)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

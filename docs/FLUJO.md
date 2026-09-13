# Flujo de trabajo

La regla es simple: **el trabajo se guarda solo y cada versión es un punto al que se puede volver.**

## Guardado automático

```bash
scripts/checkpoint.sh            # una vez: verifica (tsc + tests de Rust) y guarda
scripts/checkpoint.sh --watch    # lo repite cada 5 minutos mientras trabajás
```

Si no hay cambios, no hace nada. Si hay, **primero corre los chequeos** y sólo si pasan
commitea y empuja. Si algo falla, no guarda y deja el motivo en `.git/checkpoint.log`:
nunca se sube algo roto.

En esta máquina corre además una tarea programada de Windows (`NodeFlow Checkpoint`) que lo
ejecuta cada 5 minutos, así que no hace falta acordarse de nada.

## Bitácora (lo que se muestra)

```bash
scripts/devlog.sh 14        # arma los últimos 14 días
```

Se genera **desde el historial de git**, no se escribe a mano, y sale en dos lugares:

- `docs/DEVLOG.md` — el registro del repositorio.
- `<bóveda>/NodeFlow/devlog/AAAA-MM-DD.md` — la nota del día en Obsidian, con frontmatter.

## Nueva versión

```bash
scripts/release.sh --dry-run     # ver qué versión saldría
scripts/release.sh patch         # sube patch|minor|major, compila instaladores, etiqueta y sube
```

Cada versión queda como **tag anotado** (`v0.1.0`, `v0.2.0`, …): es el punto exacto al que se
vuelve si algo sale mal (`git checkout v0.1.0`).

## Respaldo

```bash
scripts/backup.sh    # bundle completo del repositorio en ../backups (rota los últimos 10)
```

## Disciplina de commits

Conventional Commits (`feat`, `fix`, `refactor`, `docs`, `test`, `chore`) y SemVer en las
versiones. El mensaje explica **qué** y **por qué**; los números medidos van en el cuerpo.

## Red de seguridad completa

| Capa | Qué protege |
|---|---|
| Checkpoint cada 5 min | No se pierde trabajo entre sesiones |
| Chequeos antes de guardar | No entra código que no compila ni rompe tests |
| CI en cada push (GitHub Actions) | `npm ci` + `tsc` + `cargo test` en un entorno limpio |
| Tags por versión | Punto de retorno exacto |
| Bundle de respaldo | El historial completo, fuera del repositorio |
| Backups rotativos de la app | El lienzo y las propuestas (`.nodeflow/backup-*.json`) |

## Guardado automático de fondo (sin ventanas)

Una tarea de Windows llamada **NodeFlow Checkpoint** corre cada **10 minutos**:

```
wscript.exe //B "scripts\auto-oculto.vbs"   →   bash scripts/auto.sh
```

- El lanzador `.vbs` existe por una razón concreta: llamar a `bash.exe` directamente **abría una
  consola** cada vez. `WScript.Shell.Run` con estilo de ventana `0` la mantiene oculta.
- `scripts/auto.sh` = respaldo diario (si corresponde) + punto de guardado.
- **Sólo chequea lo que cambió**: si tocaste `.ts/.tsx/.json/.css` corre `tsc`; si tocaste
  `.rs/.toml` corre los tests de Rust; si sólo hay docs, no corre nada. El mensaje del commit
  dice qué se verificó.
- Sin cambios → no hace nada (sólo deja la marca en `.git/checkpoint.ultima`).

Comandos a mano:

```bash
bash scripts/checkpoint.sh           # verificar y guardar ahora
bash scripts/checkpoint.sh --watch   # modo vigilante en primer plano
schtasks -query -tn "NodeFlow Checkpoint"   # ver la tarea
schtasks -end    -tn "NodeFlow Checkpoint"  # pausar el automático
schtasks -run    -tn "NodeFlow Checkpoint"  # dispararlo ya
```

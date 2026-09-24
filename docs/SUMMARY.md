# NodeFlow – Resumen ejecutivo

## Qué es NodeFlow
NodeFlow es un **lienzo local‑first de pensamiento aumentado** donde cada nodo representa una pieza de lógica (IA, servicio externo, proceso Rust, etc.). El usuario interactúa mediante **voz‑first** (push‑to‑talk) y el motor orquesta la ejecución entre el cliente web y el backend (Edge/Cloud) sin necesidad de servidores externos para almacenar datos.

## Principales logros
- **Demo Vercel** totalmente operativa con el modal de bienvenida que solicita claves de IA y de voz.  
- **Auto‑merge de PR** vía GitHub Actions con CI completa (TS lint, Vite build, 294 Rust tests).  
- **Voice UI** con reuso de sesión (timeout 90 s) y 10 turnos exitosos sin pérdida de audio.  
- **Persistencia segura**: claves API guardadas sólo en `localStorage`, nunca transitadas a terceros.  
- **Triage de disco** recuperó **+16 GB** borrando artefactos de worktrees ya mergeados y automatizando la limpieza.

## Arquitectura esencial
```mermaid
graph TD
  UI[React + Vite] -->|calls| Backend[Tauri (Rust)]
  Backend -->|IA| DeepSeek[DeepSeek / OpenAI / Azure]
  Backend -->|Voz| AssemblyAI[AssemblyAI / Speechmatics]
  UI <-- localStorage --> Keys[Claves API]
  Backend -->|Git| GitHub[GitHub PR + auto‑merge]
  Backend -->|Deploy| Vercel[Vercel (demo)]
```

- **Frontend**: React 19, Vite 6, componentes Lucide, `WelcomeModal` y `ApiKeyModal`.  
- **Backend**: Tauri 2, Rust, gestor de voz, motor de IA, persistencia en archivos locales.  
- **Infra**: Vercel (static build), GitHub Actions (CI + auto‑merge), `localStorage` para credenciales.

## Métricas de calidad (CI)
| Paso | Resultado |
|------|----------|
| TS lint | ✅ 0 errores |
| Vite build | ✅ 0 warnings |
| Rust tests | ✅ 294 pasados |
| Demo log | ✅ 10 turnos `voz(ui).turno.cerrado` |

## Roadmap compacto
| Estado | Ítem |
|--------|------|
| ✅ | Auto‑merge PR (completo) |
| ✅ | Demo Vercel (completo) |
| ✅ | Voice UI con reuso de sesión |
| 🚧 | UI propia para configuración de voz (separar de `ApiKeyModal`) |
| 🚧 | Integrar modelo **quantizado** local (Tauri + Rust) |
| 📅 Q4 2026 | Publicar versión con modelo quantizado y documentación PDF/infografía |

## Archivo clave y scripts útiles
- `src/App.tsx` – punto de entrada, controla modales y estados.  
- `src/components/WelcomeModal.tsx` – dialogo inicial (IA + voz).  
- `scripts/limpiar-worktrees.sh` – poda worktrees mergeados automáticamente.  
- `scripts/limpiar-builds.sh` – elimina `target/` de worktrees inactivos.  
- `scripts/generate_doc_pdf.py` – genera PDF a partir de los markdown en `docs/`.

## Próximos pasos recomendados
1. **Crear UI dedicada para la clave de voz** (separar lógica y diseño).  
2. **Finalizar integración del modelo quantizado** (compilar con Tauri, exponer endpoint local).  
3. **Ejecutar `scripts/limpiar-worktrees.sh --ver`** periódicamente y habilitar la tarea programada en Vercel.  
4. **Generar la documentación final** ejecutando el script PDF (ver paso 5).  
5. **Ejecutar**:
   ```bash
   python scripts/generate_doc_pdf.py   # produce docs/NodeFlow_Summary.pdf
   ```
   y añadir ese PDF a la distribución.

## Enlaces rápidos
- Repo: <https://github.com/NeonEden/NodeFlow>  
- Demo Vercel: <https://nodeflow-demo.vercel.app>  
- Issue #22 (Voice UI) – seguimiento de mejoras.

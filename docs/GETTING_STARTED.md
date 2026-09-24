# Guía de inicio rápido

```bash
# Clonar
git clone https://github.com/NeonEden/NodeFlow.git
cd NodeFlow/nodeflow-desktop

# Instalar dependencias
pnpm install

# Ejecutar demo local
pnpm dev   # abre http://localhost:5173

# Probar voz
#   - pulsa Ctrl+Alt+Space para activar el micrófono.
#   - después de hablar, el turno se cierra y el log muestra `voz(ui).turno.cerrado`.

# Publicar demo en Vercel (solo push)
git push origin main   # Vercel detecta el `vercel.json` y despliega.
```

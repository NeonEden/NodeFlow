# Control‑point – NodeFlow + Hermes + Azure

## Lo que hemos completado

- **Curador** (fase 5.5) – API `POST /api/cerebro/curaduria`.
- **Tool `graph_auth_device`** – flujo Device‑Code para Microsoft Graph.
- **Esqueleto de sync** – endpoints `/api/graph/sync/start|stop|status` y `graph_client.rs`.
- **Tool `azure_foundry_agent`** – catálogo MCP para invocar agentes de Azure AI Projects.
- **Tests** – `cargo test` 216 / 216 OK.

## Próximos pasos (hoja de ruta)

1. **Refresh token** – añadir `refresh_token` y lógica de renovación.
2. **Polling delta‑query** – loop que lee cambios de OneDrive.
3. **Descarga PDFs** y **pipeline PDF → MD → embeddings**.
4. **Azure Blob Storage** + **Cognitive Search** para indexar documentos.
5. **UI – botón “Sync OneDrive”** en el panel.
6. **Métricas y alertas de coste** (Azure Cost Management).
7. **Usar `azure_foundry_agent`** para resumir documentos, generar planes y Q&A.
8. **Data Connect** (batch a gran escala) cuando la sync sea estable.

Este documento sirve como **punto de control** para recordar lo logrado y lo que falta.

# ADR 0001 — Native shell: Tauri v2 + Rust instead of Electron

- **Status:** accepted
- **Date:** 2026-09-12

## Context

NodeFlow must run local language models on the same machine it is displayed on. Any memory the shell wastes is memory the model cannot use, and GPU headroom is what makes on-device inference viable at all. The obvious choice for a canvas UI is Electron (Chromium + Node), which is heavy at rest and ships a full browser per app. The UI is React 18 + React Flow either way, so the choice only concerns the shell and the backend.

## Decision

Build the shell on **Tauri v2 (Rust)** with the frontend loaded in the OS WebView, and implement the application server in-process with **axum** on `127.0.0.1:37371`.

## Consequences

- Resident memory of the released binary is **28.6 MB** (measured with `tasklist`), an order of magnitude below typical Electron apps, leaving the GPU busy with inference instead of with the shell.
- Installers are small (MSI 4.9 MB, NSIS 3.3 MB) because no browser runtime is bundled.
- The backend is Rust, so the same process that serves HTTP also owns the vault, the graph and the cost ledger — no IPC hop to a Node process.
- Cost: the app is Windows-first in practice, needs WebView2 on the target machine, and every new integration must be written in Rust rather than reused from npm.

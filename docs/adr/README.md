# Architecture Decision Records

Short records of the decisions that shaped NodeFlow, with the measurements that justified them.

| ADR | Decision |
|---|---|
| [0001](0001-tauri-rust-over-electron.md) | Native Tauri v2 + Rust shell instead of Electron (28.6 MB resident) |
| [0002](0002-cache-de-inferencia-por-hash.md) | Inference cache keyed by hash, versioned contract, no clock in the key |
| [0003](0003-el-modelo-propone-el-codigo-valida.md) | The model proposes, the code validates against live graph vocabulary |
| [0004](0004-ruteo-hibrido-primero-local.md) | Per-task hybrid routing, local first, cost recorded per artifact |
| [0005](0005-sin-router-por-llm-en-el-camino-caliente.md) | No small-LLM router on the hot path (measured 1/5 accuracy) |

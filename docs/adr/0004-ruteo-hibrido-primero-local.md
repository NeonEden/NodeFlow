# ADR 0004 — Hybrid routing: local first, cloud on demand, cost per artifact

- **Status:** accepted
- **Date:** 2026-09-13

## Context

The workload is bimodal. Structural work (classify, tag, title, parse Markdown, summarise a node) is short and repetitive and does not benefit from a frontier model. Reasoning across several branches of the graph does. Sending both to the cloud costs money and latency and leaks private notes; keeping both local caps quality.

## Decision

Route **per task**, not per application: local models (`Ollama`, `granite3.3:2b`) own drafting and validation-adjacent work; a cloud endpoint owns deep reasoning over multi-node selection. Every call is priced from a per-provider rate table (`costo.rs`), and its **tokens and USD cost are recorded in the trace of the artifact it produced**.

## Consequences

- Cost is attributable: each artifact shows what it cost to generate, and cache hits show what was avoided.
- The app degrades gracefully: with no cloud key it still runs, fully local.
- The routing decision itself must stay deterministic (see ADR 0005); the rate table must be updated when providers change prices.

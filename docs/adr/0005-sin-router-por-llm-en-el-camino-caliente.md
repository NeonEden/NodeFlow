# ADR 0005 — No small-LLM router on the hot path

- **Status:** accepted
- **Date:** 2026-09-13

## Context

The natural way to decide local vs cloud is to ask a small model. Two local candidates were measured as routers: a 2B model classified **1 of 5** cases correctly (chance level, i.e. guessing), and a 1.7B reasoning model returned empty `content` when it routed through the reasoning channel. Cold-start latency (6.6 s) also made the router cost more than the decision was worth.

## Decision

The routing decision is **deterministic code**: task type, input size, whether a cloud key is configured, and the declared ceiling. Local models are used for classification *inside* a task (drafting fields), never to decide which engine executes the task.

## Consequences

- Routing behaviour is reproducible and unit-testable; there is no hidden nondeterminism in the hot path.
- Adding a new route means adding a rule, not a prompt.
- `qwen3:1.7b` was removed from the machine (1.4 GB) once measured; `granite3.3:2b` stayed, and `keep_alive` is set **per request** because a global `OLLAMA_KEEP_ALIVE=0s` was measured to be worse (13.2 s of 16.4 s was model load).

# ADR 0003 — The model proposes, the code validates

- **Status:** accepted
- **Date:** 2026-09-13

## Context

Local drafting (titles, categories, tags for a raw capture) was delegated to `granite3.3:2b` with the JSON schema supplied as a grammar. Measured behaviour: the grammar reliably produced well-formed JSON — including `madurez: 100` on a 0-5 scale, invented categories, and a 1-character title. In other words, **the grammar guarantees shape, not truth**.

## Decision

Every structured model output passes through a Rust validator (`borrador.rs`) that checks each field against rules and against the **live vocabulary of the graph** — categories read from the canvas at call time, not a hardcoded preset list. Rejected fields are reported individually and the draft is downgraded or refused (`fuente: heuristica`).

## Consequences

- An invalid draft is refused field by field instead of silently entering the knowledge base; a valid one becomes an approval-queue proposal.
- The validator's vocabulary must be read from the graph: the 8 editor presets would have rejected 15 of the 16 categories actually in use.
- With two or more valid tags the validator uses them and declares what it discarded, rather than rejecting the whole field.

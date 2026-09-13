# ADR 0002 — Inference cache keyed by hash, with a versioned contract

- **Status:** accepted
- **Date:** 2026-09-13

## Context

The same expert run over the same node content was being paid for repeatedly. A cache is the obvious fix, but the first implementation scored **6 misses / 0 hits across six identical runs** — because the prompt embedded `now_iso()`, so the payload (and therefore the key) changed on every call.

## Decision

Cache inference results in `<vault>/.nodeflow/ai-cache.json`, keyed by a hash of **node content + prompt + provider**, with a version field in the contract and least-recently-used eviction at a fixed ceiling (300 entries). The prompt carries the **day** and a fingerprint of the facts involved — never a millisecond timestamp.

## Consequences

- A repeated run dropped from **27.9 s / 9,061 tokens to 0.4 s / 0 tokens**; the ledger currently reports 23,893 tokens avoided across 17 entries.
- Any change to the prompt contract must bump the version, which discards the whole cache by design rather than serving stale artifacts.
- The cache lives in the vault, not in the repo or the app data directory, so it travels with the knowledge and stays out of version control.

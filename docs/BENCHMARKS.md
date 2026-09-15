# Benchmarks — measured, not estimated

Every number in this file was produced on one machine and says how it was produced. Where a figure is
old, the date is stated; where something was **not** measured, it says so instead of guessing.

**Hardware of record:** Windows 11 · AMD Ryzen 5 5600G (6 cores / 12 threads, **no CUDA GPU**) ·
15.8 GB RAM · 85 GB free disk. Local inference runs through **Ollama** on `127.0.0.1:11434`, all
models at **Q4_K_M** quantisation.

---

## 1. Local model throughput (measured Sep 14 2026)

| Model | Params | On disk | Prefill | Decode | Notes |
|---|---|---|---|---|---|
| `granite3.3:2b` | 2.5B | 1.55 GB | **1,375 tok/s** | **113-124 tok/s** | Fastest by far. On some summaries it switches to English — acceptable for drafts, not for output |
| `qwen3:4b-q4_K_M` | 4B | 2.6 GB | 450 tok/s | 90-125 tok/s | Hybrid reasoning model: with a small token budget it returns an **empty string** (all budget spent thinking). Must be called with `"think": false` or a large budget |
| `deepseek-r1:7b` | 7.6B | 4.7 GB | 461 tok/s | 63-67 tok/s | Best local quality in the engine planilla (§2) |
| `qwen2.5vl:7b` | 8.3B | 6.0 GB | 250 tok/s | 70 tok/s | Vision model (reads images attached to nodes) |

**Method.** `POST /api/generate` with a fixed prompt, `num_predict` 400-500, `temperature: 0`, after a
warm-up call so the model is resident. Prefill read from `prompt_eval_duration`, decode from
`eval_duration`, and the decode rate cross-checked against wall-clock time.

**Not viable on this machine:** `gpt-oss:20b` (14 GB) and `qwen3:30b` (19 GB) do not fit in 15.8 GB of
RAM. That is a hardware limit, not a preference.

**Why this matters for the router.** Prefill is the bottleneck, not decode: with a 12.5k-token prompt
(the agent context measured on this box) the times to first token are ~9 s for `granite3.3:2b` and
~27-28 s for the 4B/7B models. That is the measured reason the router sends *drafting* to the 2B and
reserves the bigger models for tasks where depth actually pays.

---

## 2. Engine planilla — both motors on the same real tasks

Source: [`EVALUACION.md`](EVALUACION.md). Each engine runs **the canvas's real tasks** through the same
path the app uses (its spec, its validation, its cache — skipped on purpose — and its cost accounting).
The verdict comes from **code**, not from a model judging another model: did the plan work, how long
did it take, what did it cost?

| Engine | Passed | Mean time | Tokens | Cost |
|---|---|---|---|---|
| `deepseek-r1:7b` | **4/5** | 15.2 s | 9,436 | US$0 |
| `granite3.3:2b` | **3/5** | 4.3 s | 8,308 | US$0 |

| Test | `deepseek-r1:7b` | `granite3.3:2b` | What it measures |
|---|---|---|---|
| `voz-enfocar` | pass · 22.4 s | pass · 9.4 s | understands a cleanup order (a single `enfocar`) |
| `voz-crear` | pass · 16.2 s | fail · 2.6 s | adds new ideas without destroying the canvas |
| `voz-delegar` | fail · 13.1 s | fail · 2.2 s | recognises when the deep engine should take over |
| `condensar` | pass · 13.0 s | pass · 2.3 s | condenses respecting the contract (title, description, match 0..1) |
| `braindump` | pass · 11.2 s | pass · 5.1 s | breaks one idea into a map with a root and branches |

Canvas used for the run: 15 nodes. One engine at a time, unloaded from memory at the end. This is the
kind of measurement that changes design decisions — `voz-delegar` failing on both models is exactly
why escalation is decided by the router's declared plan instead of asking the small model its opinion.

---

## 3. Cache effect (read off the app in normal use)

`GET /api/ai/cache` on the live graph (Sep 15 2026):

| Counter | Value |
|---|---|
| Entries | 80 (cap 300, `CACHE_VER 3`) |
| Exact hits | 2 |
| **Semantic hits** | **2** |
| Misses | 130 |
| Tokens avoided — exact tier | 3,488 |
| **Tokens avoided — semantic tier** | **6,555** |
| **Total tokens avoided** | **10,043** |

Earlier milestone measurement of the same mechanism: an identical repeat of one expert run went from
**27.9 s / 9,061 tokens → 0.4 s / 0 tokens**.

**How the two tiers work** (`costo.rs` + `semantica.rs`):

- **Exact tier** — key = node + prompt + provider + schema + `CACHE_VER`. The key deliberately carries
  no clock: the first implementation hashed a prompt containing `now_iso()` and scored 6 misses /
  0 hits on identical runs.
- **Semantic tier** — compares by meaning with cosine similarity over Gemini embeddings
  (`gemini-embedding-001`, 3072 dimensions), threshold **0.92**. Without a key, without network, or if
  the embedding call fails once, it falls back to a dependency-free normalised-token comparison
  (containment, with a length guard for short prompts) and disables the vector path for that run
  instead of retrying.
- **Cost of the cache miss never disappears silently:** a model without a declared rate reports
  `unknown`, **not** `$0` — "no rate declared" and "free" are different facts.

---

## 4. T0→T1 — the app times its own value

From the raw brain dump (T0) to the first AI proposal **approved by a human** (T1), recorded by the app
itself (`GET /api/metrics`):

| Conversions | Deltas (minutes) | Median |
|---|---|---|
| 5 | 3.8 · 24.7 · 24.8 · 61.6 (+1 same-session at 0.0) | **24.7** |

This is the metric the product is steered by: not "how much did the model generate", but how long it
took for generated work to become something a human accepted.

---

## 5. Runtime footprint

| Measurement | Value | How |
|---|---|---|
| Native shell RSS (`app.exe`, v0.3.4, 55 nodes loaded) | **38.9 MB** | `Get-Process app` (Sep 15 2026) |

**Reading it honestly:** this is the **native shell only**. The WebView2 child processes that render
the canvas are *not* attributed here, so it is not the total desktop footprint — it is the part the
Rust side owns. Reported this way on purpose.

---

## 6. What was NOT measured (and is therefore not claimed)

- Total desktop footprint including all WebView2 renderer children.
- macOS or Linux behaviour — there are no builds for those platforms yet.
- GPU inference: this machine has no CUDA GPU, so the local path is CPU-only.
- Peak memory while synthesising audio (Kokoro TTS) — the probe captured the virtualenv launcher, not
  the interpreter child, so the figure is pending rather than invented.

---

*Related: [`EVALUACION.md`](EVALUACION.md) (engine planilla, in Spanish), [`../README.md`](../README.md)
(measured numbers summary), [`adr/`](adr) (why these decisions were made).*

# 07 — Known Gaps

> Where the **spec is silent** and a decision was needed, plus **manual interventions** still
> required (with owner + deadline). The agent appends here instead of guessing (`04 §5`). Empty
> until the build hits a gap.

| # | Date | Gap (spec was silent on…) | Resolution / decision | Owner | Needed by |
|---|---|---|---|---|---|
| 1 | 2026-06-21 | `03 §8` listed `enrich.vision_timer_interval_ms` / `enrich.vision_idle_secs` **without defaults**, and modelled mode as a single enum. | **Resolved (user):** timed = **60 min** (`3_600_000` ms), idle = **5 min** (`300` s); both are **opt-in toggles, off by default** (see `06` patch #1). `03 §8` updated. | user | ✅ done |
| 2 | 2026-06-21 | Intake/spec don't fix a **bundle identifier** for installers. | **Resolved (user):** `app.screensearch.desktop` was taken → use **`app.screensearchv2c.desktop`** (`tauri.conf.json`). | user | ✅ done |
| 3 | 2026-06-21 | `03 §7` `get_readiness` returns a `Readiness` but `03` didn't define the per-component status enum. | **Resolved:** defined `ComponentStatus { Unknown, Disabled, Initializing, Ready, Unavailable, Error }` + `ComponentReadiness { status, detail? }`; `Readiness` now holds one per subsystem so the UI can show *why*. `03 §7` updated. | agent | ✅ done |
| 4 | 2026-06-21 | `02 §5` "WebView2/Vulkan/llama smoke-check" doesn't specify the form. | **Resolved:** `crates/doctor` is now a **library + thin CLI** (`cargo run -p doctor [-- --json]`) returning a structured `Report` — reusable by CI and (later) the app's readiness panel. Diagnostic only (never fails CI). | agent | ✅ done |
| 5 | 2026-06-21 | `03 §3` `Store::hybrid_search` takes only query **text**, but the vector arm needs the query *embedded* — and the store must not depend on the embeddings impl. The spec is silent on how the query vector reaches the store. | **Resolved (engineering):** `SqliteStore` optionally holds `Arc<dyn EmbeddingProvider>` (a **trait** → modularity intact). P1 builds & tests the full FTS+vec+RRF path (vector arm driven by a fake embedder); the live path is FTS-only until **P3** injects fastembed. No trait/IPC signature change. | agent | ✅ P1; fastembed P3 |
| 6 | 2026-06-21 | `03 §5` defines claim→`running` and fail→retry/dead, but not recovery for a job stuck in `running` after a worker dies mid-job (no lease / visibility timeout). | **Deferred:** the store implements exactly per spec (claim sets `running`; `fail_job` increments attempts). Re-queuing stale `running` jobs is the **kernel worker**'s concern (`03 §6` "restart + requeue") — to add a lease/heartbeat sweep in P3. Logged so it isn't lost. | agent | P3 |

Resolved engineering decisions (spec silent on *how*, recorded for traceability):
- **ts-rs 64-bit ints → TS `number`** via per-field `#[ts(type = "number")]` (Tauri JSON wire);
  `TS_RS_LARGE_INT` env is ignored by ts-rs 10.1's macro. Guarded by `no_bigint_in_ipc_types`.
- **ts-rs `export_to`** anchors at the source-file dir here → `../../../ui/src/bindings/`.
- **CSP** set to `null` in `tauri.conf.json` for P0 dev convenience — **harden in P5**.
- ~~**§9 file logging** deferred to P1~~ — **done in P1**: console + daily-rotating file sink under
  `<app-data>/logs/` (`tracing-appender`), wired once the app-data dir is resolved at launch.
- **Store concurrency (P1):** a single `rusqlite::Connection` behind a `Mutex`, every query run via
  `spawn_blocking`. SQLite is single-writer, so this is correct and simple, and `:memory:` persists
  for the store's lifetime (clean tests). Trade-off: no reader/writer concurrency — revisit a read
  pool only if search latency demands it.
- **RRF (P1):** `k = 60` (de-facto standard); per-arm candidate pool = `max(limit·5, 50)`;
  `time_range` filters both arms (vec arm over-fetches the pool then post-filters on the join).
- **Crates (P1):** `rusqlite` 0.40 `bundled` (FTS5 is in the amalgamation — no `fts5` feature in
  0.40); `sqlite-vec` pinned to **0.1.9** (the 0.1.10-alpha line ships a broken amalgamation —
  references a missing `sqlite-vec-diskann.c`); `blake3` for `embeddings.content_hash`.
- **Schema additions (P1, non-breaking):** `UNIQUE(frame_id, chunk_index)` on `embeddings` and
  `UNIQUE(frame_id)` on `image_embeddings` to support upsert; `AFTER DELETE` triggers on the
  embedding tables purge the `vec0` shadows (FK cascade + `recursive_triggers=ON`, proven by test).
- **`JobState::Failed` reserved (P1):** the store drives pending/running/done/dead; the worker
  decides retry-vs-dead so no intermediate `failed` is needed (kept in the enum/DDL per `03 §4`).
- **Inherent store reads beyond `03 §3` (P1):** `get_frame` (backs the P5 `get_frame` command) and
  `delete_frame` (retention-purge primitive) — real APIs, not test-only, that also make the write
  paths observable.

Manual steps still required (e.g. signing certs, first-run model download, CI secrets):
- **First-run model download** (vision/answer GGUF + mmproj, embedding models) — P3/P4, per
  `MODEL_REGISTRY §4`. Not bundled.
- **Code-signing certificate** for the installer — P5 packaging. Recommended path for this MIT OSS
  project (cheapest → most turnkey): **SignPath Foundation** (free Authenticode signing for
  qualifying OSS) → **Azure Trusted Signing** (~US$10/mo, Microsoft-run, builds SmartScreen
  reputation) → **Certum Open Source Code Signing** (cheap annual cloud cert for OSS devs). Note:
  since 2023 all OV certs require a hardware token or cloud HSM, so a plain file-based cert is no
  longer available; budget for that. Decide owner before P5.
- ~~esbuild `allow-scripts` postinstall~~ — **resolved** locally via `npm approve-scripts --all`.

# Architecture (as-built)

How ScreenSearch V2c is actually put together as of **P3** (capture → OCR → store → embeddings →
hybrid search). This describes the **implemented** system and how to navigate the code; the design
intent and the *why* live in [`specs/`](../specs) (`03_MASTER_PRODUCTION_SPEC.md` is authoritative
for schema/traits/protocols). Where they ever disagree, the specs win — open an issue.

- Implemented: P0 scaffold, P1 data spine, P2 capture, **P3 enrichment + search**.
- Not yet built: P4 inference sidecar (vision tagging, grounded `ask`), P5 full UI + packaging.

---

## 1. Principles

- **Capture-cheap, enrich-deferred.** The always-on path does only cheap work (capture → OCR →
  store). Everything expensive (embeddings now; vision/answers later) is pushed into a durable
  SQLite **job queue** and run by background workers on user-controlled triggers (`03 §1/§5`).
- **Trait-bounded modularity.** The `kernel` and module crates depend only on the contracts in
  `traits` — never on each other's concrete impls. `src-tauri` is the **composition root**: the one
  place that wires concrete impls into the kernel (`03 §2`).
- **Windows-native by design.** WGC capture, WinRT OCR, WebView2 — no cross-platform abstractions.
- **Rust-only ML runtime.** Embeddings via `fastembed` (in-process ONNX); no Python in the runtime.
- **Verify, never fabricate.** No stubs/hardcoded results; "done" means observed running. Schema
  changes are forward-only with a `schema_version` bump.

---

## 2. Crate map

```
                 ┌────────────── src-tauri (composition root) ──────────────┐
                 │  opens store · spawns OCR · capture factory · builds      │
                 │  Kernel · loads embedder off-thread · Tauri commands      │
                 └───────────────┬───────────────────────┬──────────────────┘
                                 │ wires impls            │ forwards events
                          ┌──────▼───────┐                │
                          │    kernel    │  event bus, capture loop, worker pool
                          └──┬───┬───┬───┘
          depends on traits │   │   │  (never on concrete impls)
        ┌───────────────────┘   │   └────────────────────┐
   ┌────▼────┐   ┌─────────┐  ┌──▼──────┐   ┌──────────┐  ┌──▼────────┐
   │ capture │   │   ocr   │  │  store  │   │embeddings│  │ inference │ (P4 scaffold)
   │  (WGC)  │   │ (WinRT) │  │ SQLite… │   │(fastembed)│  │ (llama.cpp)│
   └─────────┘   └─────────┘  └─────────┘   └──────────┘  └───────────┘
                                    ▲
                              traits (contracts + domain/IPC/job types — no impls)
```

| Crate | Role | Key files |
|---|---|---|
| `traits` | Contracts (`CaptureSource`, `OcrProvider`, `EmbeddingProvider`, `VisionProvider`, `AnswerProvider`, `Store`) + domain/IPC/job types. No impls. | `contracts.rs`, `domain.rs`, `ipc.rs`, `jobs.rs` |
| `store` | Data spine: SQLite (WAL) + sqlite-vec + FTS5, job queue, hybrid search. | `lib.rs`, `schema.rs`, `records.rs`, `embeddings.rs`, `jobs.rs`, `search.rs`, `settings.rs` |
| `kernel` | Orchestrator: typed event bus, capture loop, **enrichment worker pool**, settings loader, readiness. | `lib.rs`, `capture_loop.rs`, `worker_pool.rs`, `events.rs`, `settings.rs` |
| `capture` | `CaptureSource` via Windows.Graphics.Capture + diff gate + privacy gate. | `lib.rs`, `wgc.rs`, `diff.rs`, `privacy.rs`, `monitors.rs` |
| `ocr` | `OcrProvider` via WinRT `Media.Ocr` on a dedicated COM STA thread. | `lib.rs` |
| `embeddings` | `EmbeddingProvider` via `fastembed` (in-process ONNX). | `lib.rs` |
| `inference` | `VisionProvider` + `AnswerProvider` + model supervisor (P4 — scaffold). | `lib.rs` |
| `doctor` | WebView2 / Vulkan / llama-server smoke-check (library + thin CLI). | `lib.rs` |
| `src-tauri` | Tauri 2 shell + composition root + command handlers. | `src/lib.rs`, `src/main.rs` |

---

## 3. Data model (SQLite, WAL)

Single file `screensearch.db`; forward-only migrations tracked in `schema_version`
(`store::schema`, authoritative DDL in `03 §4`). Per-connection pragmas: `journal_mode=WAL`,
`foreign_keys=ON`, `recursive_triggers=ON`, `busy_timeout=5000`.

Core tables: `frames` (one row per stored changed capture), `ocr_text` + `ocr_text_fts` (FTS5
mirror, porter tokenizer), `embeddings` + `embedding_vectors` (vec0 `FLOAT[768]` cosine shadow),
`image_embeddings` + `image_embedding_vectors`, `vision_analysis` (P4), `jobs` (the durable
queue), `tags`/`frame_tags`, `settings`.

Each embedding lives in **two** lock-step places — a metadata row and its `vec0` shadow keyed by
the same id. Upserts do both in one transaction; deletes are handled by `AFTER DELETE` triggers +
the `frames` FK cascade (`store::embeddings`).

**Concurrency:** one `rusqlite::Connection` behind a `Mutex` for the store's lifetime; every async
`Store` method runs its SQL inside `spawn_blocking`, and the guard is never held across an `.await`
(`store::lib::with_conn`). SQLite is single-writer, so this is correct and simple.

---

## 4. Always-on capture pipeline (P2)

`kernel::Kernel::start_capture` builds a `CaptureSource` from the current `Settings` via the
composition root's factory and spawns `run_capture_loop` (`kernel::capture_loop`). Per changed
frame:

```
WgcCapture.next_frame()           # diff-gated + privacy-gated; only *changed* frames
  → WinRtOcr.recognize()          # on full-res pixels, before storage downscale
  → write JPEG (downscaled)       # <app-data>/frames/day-<n>/<captured_at>-<monitor>.jpg
  → store.insert_frame + insert_ocr
  → enqueue embed_text job        # if enrich.embed_text  (+ embed_image if enabled)
  → emit KernelEvent::CaptureTick # drives the live timeline
```

Capture is **off until the user starts it** (privacy-first). Per-frame errors are logged and the
frame skipped — capture keeps running. No screen content or OCR text is logged at info level.
`vision_tag` is **never** auto-enqueued (deferred to P4).

---

## 5. Deferred enrichment (P3)

The new half of the system. The `embed_text` jobs the capture loop enqueues are drained into
vectors by a background worker pool.

### 5.1 Job queue (`store::jobs`)

State machine `pending → running → done`, or `running →` (fail) `→ pending` (retry with backoff)
`→ … → dead` (dead-letter at `max_attempts`, never silently dropped). Claims are a single atomic
`UPDATE … RETURNING` so no job is handed to two workers. Key methods: `enqueue_job`, `claim_jobs`,
`complete_job`, `fail_job(err, retry_at)`, `job_stats`, and `reset_stale_running_jobs` (P3).

### 5.2 Embedding provider (`embeddings::FastEmbedProvider`)

`fastembed` 5.17.2 (in-process ONNX, no Python). Text = `EmbeddingModel::EmbeddingGemma300MQ`
(768-dim, quantized → **embeds one input at a time**, it cannot batch); optional image =
`ImageEmbeddingModel::NomicEmbedVisionV15`, loaded only when `enrich.image_embeddings` is on. Each
lane is an `Arc<Mutex<…>>` whose lock is taken **inside** a `spawn_blocking` closure (the models
are plain `Send` ONNX handles with no thread affinity, unlike COM-bound OCR). Models load eagerly
in `FastEmbedProvider::new` — called off the launch thread — into `<app-data>/models/fastembed`,
downloading from HuggingFace on first run.

### 5.3 Worker pool (`kernel::worker_pool`)

`Kernel::attach_embedder` injects the loaded provider into the store (lighting up the vector arm)
and starts the pool — **independent of capture**, so the queue's backlog drains in the background.
`N = enrich.worker_concurrency` workers each loop:

```
claim_jobs([EmbedText (+EmbedImage)], 1, now)
  → process_job:                       # public, so tests drive one job deterministically
      embed_text:  read OCR text → embed_texts → upsert_text_embedding(chunk 0, source=ocr)
      embed_image: load JPEG → embed_image → upsert_image_embedding
  → complete_job / fail_job(backoff) / dead-letter
  → emit KernelEvent::JobProgress(job_stats)
```

Outcome rules: missing `frame_id` or a missing JPEG → **dead-letter** (won't fix itself); a purged
frame or empty/whitespace OCR → **complete** (nothing to embed is success, not failure); embed/
upsert errors → **retry** with backoff `1 s · 2^attempts` (cap 60 s). Idle poll backs off
250 ms → 2 s; shutdown is a `watch` channel that lets in-flight jobs finish.

**Stale-job recovery** (`03 §6`, gap #6): there is no per-job lease. A **startup sweep**
(`reset_stale_running_jobs(0)`) requeues anything left `running` by a dead worker; a **periodic
60 s sweep** with a 5-minute visibility timeout catches a worker that died while the app stayed up.

---

## 6. Hybrid search (`store::search`)

`hybrid_search(SearchQuery) → Vec<SearchHit>` fuses two ranked arms with **Reciprocal Rank Fusion**
(`k = 60`):

- **FTS arm** — BM25 over `ocr_text_fts` (porter tokenizer), with highlighted snippets. User text is
  safely quoted per-term (no FTS-operator injection).
- **Vector arm** — embed the query once (via the injected `EmbeddingProvider`), then sqlite-vec
  cosine KNN over `embedding_vectors`, de-duped by frame. Active only once an embedder is attached;
  before that, search degrades to FTS-only.

Both arms over-fetch a candidate pool (`max(limit·5, 50)`) and filter to the half-open time range
`[start, end)`. Results hydrate in two bulk `IN` queries (frame context + fallback snippets).

The embedder is **runtime-settable** (`SqliteStore.embedder` is `Arc<RwLock<Option<…>>>` +
`set_embedder`), so the composition root can attach the model *after* the off-thread load without
rebuilding the store; the search hot path clones the `Arc` out from under the lock before the
`.await`.

**Performance:** the `#[ignore]`d fixture `crates/store/tests/perf.rs` seeds 10 000 frames + 768-dim
vectors and measures **p95 ≈ 33 ms** — well under the `03 §13.4` ~200 ms bar.

---

## 7. Events, readiness, settings

**Event bus** (`kernel::events::KernelEvent`, a `tokio::broadcast`): `CaptureTick`,
`ReadinessChanged`, `JobProgress`. The kernel is shell-agnostic; `src-tauri::forward_events` bridges
these to Tauri events (`capture_tick`, `readiness_changed`, `job_progress`) for the WebView2 UI.

**Readiness** (`03 §7`): one `ComponentReadiness { status, detail? }` per subsystem — `capture`,
`db`, `embed_model`, `sidecar` — where `status ∈ {unknown, disabled, initializing, ready,
unavailable, error}`. `embed_model` flows Initializing → Ready (model attached) / Unavailable
(load failed) / Disabled (embeddings off in settings).

**Settings** (`kernel::settings`): the strongly-typed `Settings` is assembled from the opaque
key/value `settings` table; a missing/unparsable value falls back to the per-key default (never an
error). P3-relevant keys: `enrich.embed_text` (true), `enrich.image_embeddings` (false),
`enrich.worker_concurrency` (2).

---

## 8. Startup sequence (`src-tauri::run`)

1. Resolve `<app-data>`; create `logs/`; init tracing (console + daily-rotating file).
2. Open the store (`open_store`) → `db` readiness Ready / Error.
3. Build the `Kernel` (store + OCR worker + WGC capture factory). Capture starts `Disabled`.
4. Spawn `forward_events`; set `embed_model = Initializing`; spawn `init_embeddings`:
   load settings → if embeddings disabled, set `Disabled`; else `spawn_blocking` the
   `FastEmbedProvider::new` (off the launch thread) → `attach_embedder` (sets the store embedder,
   runs the startup stale-job sweep, starts the worker pool, sets `embed_model = Ready`).
5. Register Tauri commands; run. On `ExitRequested`, best-effort `stop_workers` (correctness does
   not depend on it — the startup sweep requeues any interrupted job).

**Commands** (typed via `ts-rs`): `ping`, `get_readiness`, `get_job_stats`, `get_frame`, `search`
(P3), `capture_control`. (`ask`, `get_timeline`, `enqueue_vision`, settings/model-tier commands
arrive with P4/P5.)

---

## 9. Testing

- **Unit / integration, platform-agnostic (run in CI):** store state-machine + retrieval tests
  against `:memory:` SQLite; capture-loop and worker-pool tests with **fake** sources/OCR/embedders;
  the P3 end-to-end test (`crates/kernel/tests/enrichment.rs`) drives the real worker pool draining a
  job, then proves the **vector arm** specifically by a query that does not FTS-match the frame.
- **`#[ignore]`d (local / hardware / model-backed):** WGC + WinRT OCR smoke (`cfg(windows)`), the
  real-model embedding test (`cargo test -p embeddings -- --ignored`), and the 10k-frame perf
  fixture (`cargo test -p store --test perf -- --ignored`).
- **Gates:** `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test`, the UI
  build, and a `ts-rs` binding-drift guard — all on `windows-latest` (`03 §11`).

---

## 10. Deferred (P4 / P5)

- **P4 — inference sidecar:** one `llama-server` child, OpenAI-compatible HTTP, **bound to the app
  via a Windows Job Object** (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) so it can never orphan;
  lazy-spawn + idle-evict; health/restart. Powers `VisionProvider` (on-demand/timer/idle vision
  tagging — the timer/idle scheduler and `enqueue_vision` command land here too) and
  `AnswerProvider` (grounded, streaming, *thinking* `ask`). The no-orphan test must pass before P4
  ships. (`inference` crate is scaffold today.)
- **P5 — UI & packaging:** the full Command-Deck UI (search/ask/timeline/settings, the
  `UI_REFERENCE.md` identity + state matrix), Inno Setup installer + portable ZIP (must bundle
  `onnxruntime.dll`), and code signing.

---

*Pointers:* design rationale → [`specs/03_MASTER_PRODUCTION_SPEC.md`](../specs/03_MASTER_PRODUCTION_SPEC.md) ·
phase plan → [`specs/02_STRATEGIC_PLAN.md`](../specs/02_STRATEGIC_PLAN.md) ·
open decisions/gaps → [`specs/07_KNOWN_GAPS.md`](../specs/07_KNOWN_GAPS.md) ·
build records → [`specs/05_BUILD_REVIEW.md`](../specs/05_BUILD_REVIEW.md) ·
model pins → [`specs/MODEL_REGISTRY.md`](../specs/MODEL_REGISTRY.md).

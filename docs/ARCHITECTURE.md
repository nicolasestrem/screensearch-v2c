# Architecture (as-built)

How ScreenSearch V2c is actually put together as of **P4** (capture → OCR → store → embeddings →
hybrid search → **inference sidecar**: vision tagging + grounded `ask`). This describes the
**implemented** system and how to navigate the code; the design intent and the *why* live in
[`specs/`](../specs) (`03_MASTER_PRODUCTION_SPEC.md` is authoritative for schema/traits/protocols).
Where they ever disagree, the specs win — open an issue.

- Implemented: P0 scaffold, P1 data spine, P2 capture, P3 enrichment + search,
  **P4 inference sidecar** (Job-Object lifecycle, vision tagging, streaming RAG answers,
  tiered runtime-downloaded models).
- Not yet built: P5 full Command-Deck UI + packaging (installer / portable ZIP + signing).

---

## 1. Principles

- **Capture-cheap, enrich-deferred.** The always-on path does only cheap work (capture → OCR →
  store). Everything expensive (embeddings, vision tagging) is pushed into a durable SQLite **job
  queue** and run by background workers on user-controlled triggers (`03 §1/§5`).
- **Fault isolation by construction.** The only crash-prone, out-of-process component — the
  `llama-server` inference sidecar — is bound to the app via a Windows **Job Object** so it can
  never orphan; a failed enrichment job retries instead of taking capture down (`02 §2`, `03 §6`).
- **Trait-bounded modularity.** The `kernel` and module crates depend only on the contracts in
  `traits` — never on each other's concrete impls. `src-tauri` is the **composition root**: the one
  place that wires concrete impls into the kernel (`03 §2`).
- **Windows-native by design.** WGC capture, WinRT OCR, WebView2 — no cross-platform abstractions.
- **Rust-only ML runtime.** Embeddings via `fastembed` (in-process ONNX); vision/answers via the
  local `llama-server` sidecar (OpenAI-compatible HTTP over loopback). No Python in the runtime,
  no cloud calls — everything downloads from GitHub / HuggingFace and runs on-device.
- **Verify, never fabricate.** No stubs/hardcoded results; "done" means observed running. Schema
  changes are forward-only with a `schema_version` bump.

---

## 2. Crate map

```
                 ┌────────────── src-tauri (composition root) ──────────────┐
                 │  opens store · spawns OCR · capture factory · builds      │
                 │  Kernel · loads embedder + inference off-thread · commands │
                 └───────────────┬───────────────────────┬──────────────────┘
                                 │ wires impls            │ forwards events
                          ┌──────▼───────┐                │
                          │    kernel    │  event bus, capture loop, worker pool,
                          └──┬─┬─┬─┬──────┘  vision scheduler, readiness
       depends on traits │  │ │ │ │   (never on concrete impls)
     ┌────────────────────┘ │ │ │ └──────────────────┬───────────────┐
┌────▼────┐ ┌──────┐ ┌──────▼─┐ ┌──────────┐ ┌────────▼─────────────────────┐
│ capture │ │ ocr  │ │ store  │ │embeddings│ │          inference           │
│  (WGC)  │ │(WinRT)│ │SQLite… │ │(fastembed)│ │  ModelSupervisor → llama-server
│ + idle  │ └──────┘ └────────┘ └──────────┘ │  (Job-Object-bound, OpenAI HTTP)
└─────────┘                                   │  VisionProvider + AnswerProvider
                  ▲                           └──────────────────────────────┘
            traits (contracts + domain/IPC/job types — no impls)
```

| Crate | Role | Key files |
|---|---|---|
| `traits` | Contracts (`CaptureSource`, `OcrProvider`, `EmbeddingProvider`, `VisionProvider`, `AnswerProvider`, `Store`) + domain/IPC/job types. No impls. | `contracts.rs`, `domain.rs`, `ipc.rs`, `jobs.rs` |
| `store` | Data spine: SQLite (WAL) + sqlite-vec + FTS5, job queue, hybrid search, untagged-frame query. | `lib.rs`, `schema.rs`, `records.rs`, `embeddings.rs`, `jobs.rs`, `search.rs`, `settings.rs` |
| `kernel` | Orchestrator: typed event bus, capture loop, enrichment + **vision** worker pool, **vision scheduler**, settings loader, readiness, **inference attach**. | `lib.rs`, `capture_loop.rs`, `worker_pool.rs`, `vision_scheduler.rs`, `events.rs`, `settings.rs` |
| `capture` | `CaptureSource` via Windows.Graphics.Capture + diff/privacy gates; **`user_idle_ms`** idle probe. | `lib.rs`, `wgc.rs`, `diff.rs`, `privacy.rs`, `monitors.rs`, `idle.rs` |
| `ocr` | `OcrProvider` via WinRT `Media.Ocr` on a dedicated COM STA thread. | `lib.rs` |
| `embeddings` | `EmbeddingProvider` via `fastembed` (in-process ONNX). | `lib.rs` |
| `inference` | **(P4)** `ModelSupervisor` (Job-Object sidecar lifecycle) + `VisionSidecar`/`AnswerSidecar` providers + sidecar HTTP client + runtime model/binary downloaders. | `job_object.rs`, `process.rs`, `supervisor.rs`, `client.rs`, `models.rs`, `download.rs`, `vision.rs`, `answer.rs` |
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
`vision_tag` is **never** auto-enqueued per frame — it is produced only on-demand (the
`enqueue_vision` command) or by the opt-in timer/idle scheduler (§7), so vision work never runs in
the always-on hot path.

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
claim_jobs([EmbedText (+EmbedImage), VisionTag], 1, now)
  → process_job:                       # public, so tests drive one job deterministically
      embed_text:  read OCR text → embed_texts → upsert_text_embedding(chunk 0, source=ocr)
      embed_image: load JPEG → embed_image → upsert_image_embedding
      vision_tag:  load JPEG → VisionProvider.analyze → insert_vision   (P4)
  → complete_job / fail_job(backoff) / dead-letter
  → emit KernelEvent::JobProgress(job_stats)
```

The pool always claims `VisionTag` too. The vision provider is attached **after** the pool may have
started (the sidecar resolves later than fastembed), so it lives in a shared `Arc<RwLock<Option<…>>>`
**slot** that `process_job` reads per job — no pool restart on attach. `vision_tag` jobs only exist
once a producer enqueues them, by which point the provider is in the slot; if it somehow isn't, the
job **retries** (not fails) so the backlog drains when the sidecar comes up.

Outcome rules: missing `frame_id` or a missing JPEG → **dead-letter** (won't fix itself); a purged
frame or empty/whitespace OCR → **complete** (nothing to embed is success, not failure); embed/
upsert/analyze errors → **retry** with backoff `1 s · 2^attempts` (cap 60 s). Idle poll backs off
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

## 7. Inference sidecar (P4)

The only out-of-process component. One `llama-server` child serves an OpenAI-compatible HTTP API on
`127.0.0.1:<ephemeral>`; the `inference` crate owns its whole lifecycle and exposes the two providers
the kernel drives. Built **lifecycle-first**: the no-orphan binding is proven before any real
inference (`04 §3`).

### 7.1 No-orphan guarantee (`inference::job_object`, `inference::process`)

A `ModelSupervisor` creates a Windows **Job Object** with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
Every `llama-server` is spawned **suspended** (raw `CreateProcessW` with `CREATE_SUSPENDED` — `std`
can't do this), **assigned to the job before its main thread is resumed**, then resumed. Because the
OS closes every handle a process owns when it dies — including the last handle to the job — the
sidecar is terminated whenever the app exits for *any* reason (clean exit, panic, kill, power loss
after resume). This is the hard requirement (`03 §6`, DoD #7); it's proven by a cross-process test
(`tests/no_orphan.rs`): a helper holds the job + a grandchild, the test kills the helper, and asserts
the grandchild dies.

**Startup reap** (`supervisor::reap_stray`): on launch, a stray sidecar from a prior run is killed,
identified by a **pidfile** plus an **image-path sentinel** — the recorded pid is terminated only if
its running image is the `llama-server.exe` we installed under app-data, never an unrelated process
that recycled the pid.

### 7.2 Supervisor (`inference::supervisor`)

`ModelSupervisor` is the one process at a time. `acquire(spec)` ensures the requested model is
running and returns a `Lease` carrying a cloned HTTP client; the lease counts the request **in-flight**
so the idle-evictor can't pull a model out from under it. Lifecycle:

- **Lazy spawn + `/health` gate** — the process starts on the first request that needs it; the
  supervisor polls `/health` (up to a generous timeout — first-run model load is slow) before serving.
- **Idle evict** — a background task stops the sidecar after `sidecar.idle_ttl_secs` of no in-flight
  requests, freeing GPU/RAM (the footprint control). It respawns on the next request.
- **Model switch** — a request for a different tier resolves a different GGUF; `needs_restart`
  detects the change and the supervisor stops + respawns (vision adds `--mmproj`).
- **Status** — every transition (`Starting/Ready/Evicted/Crashed/Stopped`) is broadcast as
  `SidecarStatus`; the composition root bridges it into the kernel (§8).

### 7.3 Tiered models, runtime-downloaded (`inference::models`, `inference::download`)

Vision and answer each offer **Default / Quality / Beta** (`MODEL_REGISTRY`). Nothing is bundled —
everything downloads on first use, Rust-only (no Python in the runtime):

- **Binary** — `ensure_binary` fetches a prebuilt llama.cpp **Vulkan** Windows release zip
  from GitHub into `<app-data>/sidecar/llama` (asset selected by a unit-tested
  `*-win-vulkan-x64.zip` matcher; overridable via `SSV2C_LLAMA_RELEASE_URL`). It scans the
  recent-releases list rather than `/releases/latest` and takes the **newest release that
  actually carries** the Vulkan asset — llama.cpp's CI sometimes publishes a release with an
  incomplete asset set, which a single-`latest` lookup would fail on outright.
- **Models** — `ensure_model` lists the tier's HuggingFace repo via `hf-hub`, picks the `Q4_K_M`
  weights (+ the **same-repo** `mmproj` for vision — a mismatched projector crashes the server), and
  copies them into `<app-data>/models/<lane>/<tier>`. Idempotent (skips files already present).
- `resolve_spec` scans the local dir for the weights/projector and builds the `ModelSpec
  { lane, tier, gguf_path, mmproj_path?, ngl }` the supervisor launches.

### 7.4 Providers (`inference::vision`, `inference::answer`)

- **`VisionSidecar` (`VisionProvider`)** — encodes the frame as a JPEG base64 data URL, sends a
  non-streaming chat completion asking for compact JSON (`description`/`activity_type`/`app_hint`/
  `confidence`), and parses it into a `VisionAnalysis`. A non-JSON reply falls back to raw text as the
  description with a `-1.0` "unknown" confidence sentinel — **never a fabricated score**.
- **`AnswerSidecar` (`AnswerProvider`)** — builds a grounded RAG prompt from the retrieved chunks
  (each tagged with its frame id), streams the SSE reply, and maps it to typed `AnswerDelta`s:
  reasoning → `Thinking`, answer text → `Token`, one `Citation` per grounding frame, then `Done`
  (or `Error`). Reasoning arrives two ways depending on the build — a `reasoning_content` delta field,
  or inline `<think>…</think>` tags, which a `ThinkSplitter` separates even when a tag is split across
  SSE chunks.

Both providers hold the active tier (changed via `set_tier`) and **lazily download** their model on
first use, mirroring fastembed's first-run UX.

### 7.5 Vision scheduling (`kernel::vision_scheduler`)

Vision is never real-time. Three triggers feed `vision_tag` jobs:

- **On-demand** — the `enqueue_vision` command (a frame, or all still-untagged frames in a time
  range). Always available.
- **Timer** — opt-in (`enrich.vision_timer_enabled`): every `enrich.vision_timer_interval_ms`, enqueue
  up to a batch (N = 20) of untagged frames (`Store::untagged_frame_ids`).
- **Idle** — opt-in (`enrich.vision_idle_enabled`): when the OS reports the user idle ≥
  `enrich.vision_idle_secs`, enqueue a batch (on the transition into idle, not every poll). Idle time
  comes from `capture::user_idle_ms` (`GetLastInputInfo`), injected as an `IdleSource` because the
  kernel forbids `unsafe`.

There is no pending-job dedup; a frame enqueued-but-not-yet-processed can be re-enqueued, but
`insert_vision` is an idempotent upsert so the only cost is a redundant analyze (logged, `07` #19).

---

## 8. Events, readiness, settings

**Event bus** (`kernel::events::KernelEvent`, a `tokio::broadcast`): `CaptureTick`,
`ReadinessChanged`, `JobProgress`, **`SidecarStatus`** (P4). The kernel is shell-agnostic;
`src-tauri::forward_events` bridges these to Tauri events (`capture_tick`, `readiness_changed`,
`job_progress`, `sidecar_status`). The `ask` command streams **`answer_delta`** events directly from
its forwarding task.

**Readiness** (`03 §7`): one `ComponentReadiness { status, detail? }` per subsystem — `capture`,
`db`, `embed_model`, `sidecar` — where `status ∈ {unknown, disabled, initializing, ready,
unavailable, error}`. `embed_model` flows Initializing → Ready / Unavailable / Disabled. **`sidecar`
(P4)** flows Initializing (resolving binary) → Ready (binary present; model downloads + spawns on
demand) / Unavailable (binary or supervisor init failed); thereafter the supervisor's `SidecarStatus`
maps live — `Starting`→Initializing, `Ready`→Ready, `Evicted`→Ready ("respawns on demand"),
`Crashed`→Error, `Stopped`→Disabled (`kernel::sidecar_component`).

**Settings** (`kernel::settings`): the strongly-typed `Settings` is assembled from the opaque
key/value `settings` table; a missing/unparsable value falls back to the per-key default (never an
error). Enrichment keys: `enrich.embed_text` (true), `enrich.image_embeddings` (false),
`enrich.worker_concurrency` (2). **P4 keys:** `enrich.vision_timer_enabled` (false) +
`enrich.vision_timer_interval_ms` (60 min), `enrich.vision_idle_enabled` (false) +
`enrich.vision_idle_secs` (5 min), `models.vision_tier` / `models.answer_tier` (`default`),
`answer.thinking` (true), `sidecar.idle_ttl_secs` (180), `sidecar.ngl` (99).

---

## 9. Query → answer path (`ask`)

```
ask(AskRequest{query, thinking, max_tokens})
  → store.hybrid_search(query, top-K = 8)                     # grounding candidates
  → per hit: get_enrichment_input → full OCR text (fallback: snippet) → RetrievedChunk
  → AnswerProvider.answer(query, context, opts, tx)           # background task
       supervisor.acquire(answer spec) → SidecarClient.stream(SSE)
       → AnswerDelta::Thinking / Token / Citation(per frame) / Done|Error
  → forwarder emits each delta as an `answer_delta` Tauri event
```

The command returns immediately; the answer streams asynchronously. The lease is held for the whole
stream, so the idle-evictor never stops the model mid-answer.

---

## 10. Startup sequence (`src-tauri::run`)

1. Resolve `<app-data>`; create `logs/`; init tracing (console + daily-rotating file).
2. Open the store (`open_store`) → `db` readiness Ready / Error.
3. Build the `Kernel` (store + OCR worker + WGC capture factory). Capture starts `Disabled`.
4. Spawn `forward_events`. Set `embed_model = Initializing` and spawn `init_embeddings` (load model
   off-thread → `attach_embedder`: store embedder, startup stale-job sweep, start worker pool,
   `embed_model = Ready`). Set `sidecar = Initializing` and spawn **`init_inference`**:
   `ensure_binary` (off-thread) → build `SupervisorConfig` + `ModelSupervisor::new` (creates the job,
   reaps a stray) → build `VisionSidecar`/`AnswerSidecar` → fill the supervisor/vision/answer slots →
   bridge `supervisor.subscribe()` into `kernel.emit_sidecar_status` → `attach_inference`
   (vision into the worker slot, answer for `ask`, start the vision scheduler with the idle source) →
   `sidecar = Ready`. Failure at any step sets `sidecar = Unavailable` with a reason.
5. Register Tauri commands; run. On `ExitRequested`: `stop_vision_scheduler` + `stop_workers`, then
   `supervisor.shutdown()` (kills the sidecar; the Job Object would anyway). All best-effort —
   correctness doesn't depend on it (the startup sweep requeues interrupted jobs).

**Commands** (typed via `ts-rs`): `ping`, `get_readiness`, `get_job_stats`, `get_frame`, `search`,
`capture_control`, **`enqueue_vision`**, **`ask`**, **`set_model_tier`** (P4). (`get_timeline` +
settings commands arrive with P5.)

---

## 11. Testing

- **Unit / integration, platform-agnostic (run in CI):** store state-machine + retrieval + the
  `untagged_frame_ids` query against `:memory:` SQLite; capture-loop and worker-pool tests with
  **fake** sources/OCR/embedders/vision; the P3 end-to-end test
  (`crates/kernel/tests/enrichment.rs`) drains a real job and proves the vector arm via a
  non-FTS-matching query; the P4 `vision_tag` routing tests drive `process_job` with a fake
  `VisionProvider` (writes the analysis; retries with no provider).
- **Inference, deterministic (run in CI, no GPU/network):** the **no-orphan gate**
  (`tests/no_orphan.rs` — kill a parent, assert the Job-Object child dies), startup **reap**
  (`tests/reap.rs` — reaps a matching stray, never a foreign pid), the HTTP **client** against a
  `wiremock` sidecar (`tests/sidecar_client.rs` — vision parse + ordered SSE deltas), and the pure
  logic (model/asset selection, `ThinkSplitter`, vision JSON parse, supervisor decisions).
- **`#[ignore]`d (local / hardware / model-backed):** WGC + WinRT OCR smoke (`cfg(windows)`), the
  real-model embedding test (`-p embeddings`), the 10k-frame perf fixture (`-p store --test perf`),
  and the **real-llama-server smoke** (`cargo test -p inference --test smoke -- --ignored` — downloads
  a Vulkan build + GGUFs and runs a real vision tag + streamed answer on the GPU).
- **Gates:** `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test`, the UI build, and a `ts-rs` binding-drift guard — all on `windows-latest` (`03 §11`).

---

## 12. Deferred — P5 (UI & packaging)

P4 completed the inference sidecar (vision tagging + grounded `ask`); the no-orphan gate passes. What
remains for v1.0:

- **Full Command-Deck UI** — search / ask / timeline / settings screens with the `UI_REFERENCE.md`
  identity and the complete state matrix (the P2/P3 UI is a minimal live timeline).
- **Packaging** — Inno Setup installer + portable ZIP; must bundle `onnxruntime.dll` (the `ort`
  build-time artifact) beside the exe; code signing (see `07` — SignPath Foundation / Azure Trusted
  Signing / Certum). The `llama-server` binary and GGUF models are *not* bundled — they download at
  runtime.
- **Polish carried from P4** (`07` #19): a download-progress %% in `sidecar` readiness, optional
  pending-job dedup for the vision scheduler, and multi-GPU device selection if `-ngl 99` picks the
  wrong Vulkan device.

---

*Pointers:* design rationale → [`specs/03_MASTER_PRODUCTION_SPEC.md`](../specs/03_MASTER_PRODUCTION_SPEC.md) ·
phase plan → [`specs/02_STRATEGIC_PLAN.md`](../specs/02_STRATEGIC_PLAN.md) ·
open decisions/gaps → [`specs/07_KNOWN_GAPS.md`](../specs/07_KNOWN_GAPS.md) ·
build records → [`specs/05_BUILD_REVIEW.md`](../specs/05_BUILD_REVIEW.md) ·
model pins → [`specs/MODEL_REGISTRY.md`](../specs/MODEL_REGISTRY.md).

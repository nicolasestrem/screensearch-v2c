# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

### Fixed — P3 review (PR #7, 2026-06-21)
- **Stale-job recovery** never misses a job now: the startup sweep requeues every
  `running` job unconditionally, and `claim` + the periodic sweep share one DB clock
  (no Rust-millisecond vs SQLite-second mismatch).
- **Image embedding** retries on a transient file lock (AV / indexer / backup) and
  only dead-letters a genuinely missing JPEG — robuster on Windows.
- The worker pool now stops its background tasks even if its handle is dropped
  without a graceful shutdown (`Drop` signals stop), so it can't keep draining the
  queue after an unexpected teardown.

### Added — P3 Deferred enrichment (2026-06-21)
- **Embedding worker, end-to-end** (`02 §5`, `03 §5/§13.2`): a bounded worker pool
  drains the `embed_text` jobs capture enqueues into vectors via **fastembed**
  (in-process ONNX, no Python) — `EmbeddingGemma-300M` text embeddings (768-dim),
  with optional `nomic-embed-vision-v1.5` image embeddings behind
  `enrich.image_embeddings`. Workers run in the background, draining the backlog
  independent of capture; concurrency is `enrich.worker_concurrency`.
- **Hybrid search is live** (`03 §13.4`): once the model loads, the vector arm of
  `hybrid_search` (FTS5 + sqlite-vec KNN → RRF) lights up. Measured **p95 ≈ 33 ms
  over 10 000 frames** — well under the ~200 ms bar. A typed `search` command
  (`SearchQuery → SearchHit[]`) makes it reachable (UI lands in P5).
- **Durable, self-healing queue**: jobs retry with exponential backoff and
  dead-letter at `max_attempts`; a job a crashed worker left `running` is requeued
  by a startup + periodic sweep (`03 §6`). A `job_progress` event surfaces queue
  depth; `embed_model` readiness reports loading → ready/unavailable.
- The embedding model loads off the launch thread, so app start never blocks on the
  first-run model download.
- Vision tagging stays fully deferred to P4 (no `vision_tag` is ever auto-enqueued).

### Added — P2 Capture happy path (2026-06-21)
- Always-on **capture → OCR → store** pipeline (`02 §5`, `03 §3/§5`): screen capture
  via Windows.Graphics.Capture with a diff gate that stores only changed frames,
  text recognition via WinRT `Media.Ocr` on a COM STA worker, JPEG storage, and a
  row written to `frames` + `ocr_text` — each stored frame enqueueing an
  `embed_text` job (consumed in P3). Vision is never auto-run (`13.3`).
- `kernel` orchestrator: the capture loop, a typed event bus (`capture_tick` /
  `readiness_changed`), a typed-`Settings` loader, and idempotent
  `start_capture`/`stop_capture`. Capture is **off until you start it**
  (privacy-first); `privacy.excluded_apps` and `privacy.pause_on_lock` are honored.
- App: `capture_control` (start/stop) and `get_frame` commands, live `db` +
  `capture` readiness, and a **minimal live timeline** UI (Start/Stop, readiness
  strip, a row per captured frame). The full Command-Deck UI lands in P5.
- Captured frames carry the foreground app/window as `app_hint` / `window_title`.
- OCR `mean_confidence` is recorded as `-1.0` ("unknown") — WinRT OCR exposes no
  confidence; no value is fabricated (see `specs/06_PATCH_PLAN.md` #2).

### Documentation (P0/P1 review pass)
- Reviewed merged P0 + P1 against the spec — both complete and compliant, no
  correctness bugs (record in `specs/05_BUILD_REVIEW.md` Pass 3). Doc/clarity
  touch-ups only: `TimeRange` now states its half-open `[start, end)` semantics
  explicitly; the concurrent-claim test documents what it actually proves; and the
  hybrid-search latency (`03 §13`) and vector-arm time-range approximations are now
  tracked gaps for P3 (`specs/07_KNOWN_GAPS.md` #7/#8). No behavior change.

### Fixed (post-P1 review, PR #4)
- `open_state` reports `db = Error` (not `Ready`) if the post-open schema-version
  probe fails — no "ready but unqueryable" store reaches the UI.
- `complete_job` / `fail_job` now error on an unknown job id instead of silently
  doing nothing (consistent with the queue's no-silent-loss contract).
- `insert_vision` also fills `frames.activity_type` (`03 §4`: "filled by vision")
  so the timeline can filter by activity without a join.
- Hybrid-search hydration uses two bulk `IN` queries instead of per-hit queries
  (removes an N+1 pattern); shared `f32_blob` / `dedup_keep_order` helpers.

### Added — P1 Data Spine (2026-06-21)
- `store` crate — the durable data spine on SQLite (WAL) + sqlite-vec (`vec0`) + FTS5
  (`03 §4/§5`): forward-only schema migrations tracked in `schema_version`, and the
  full `Store` contract — frame / OCR / vision inserts, key-value settings, text &
  image embedding upserts with synchronized vector shadows, and `get_frame` /
  `delete_frame`.
- Durable **job queue** — atomic claim (`UPDATE … RETURNING`), priority + scheduled
  (`not_before`) dispatch, retry-with-backoff, and dead-lettering at `max_attempts`
  (no silent loss).
- **Hybrid search** — FTS5 (BM25) fused with sqlite-vec cosine-KNN via Reciprocal
  Rank Fusion, with time-range filtering and highlighted snippets. The vector arm is
  driven by an injected embedding provider (FTS-only until P3 wires fastembed).
- App wiring — the desktop shell opens `screensearch.db` at the app-data dir on
  launch, reports real `db` readiness, exposes a `get_job_stats` command, and writes
  a daily-rotating file log under `<app-data>/logs/` (`03 §9`).

### Added — P0 Scaffold (2026-06-21)
- Cargo workspace with a modular kernel layout (`03 §2`): `traits` (contracts +
  domain/jobs/IPC types) and skeleton crates `kernel`, `store`, `capture`, `ocr`,
  `embeddings`, `inference`.
- `src-tauri` — Tauri 2 desktop shell (composition root) with typed `ping` and
  `get_readiness` commands.
- React 18 + TypeScript + Vite UI skeleton (`ui/`) with a P0 typed-IPC smoke screen
  and an ESLint flat config enforcing the Rules-of-Hooks gate at error level.
- Typed UI↔core contract generated with `ts-rs` into `ui/src/bindings/`, with a
  regression guard keeping 64-bit ids as TS `number` (Tauri JSON wire).
- `doctor` environment smoke-check (`cargo run -p doctor`) for WebView2 / Vulkan /
  llama-server.
- GitHub Actions CI (`.github/workflows/ci.yml`, windows-latest): UI lint/build,
  `cargo fmt`/`clippy -D warnings`/`build`/`test`, and a ts-rs binding-drift guard.
- Generated application icons from `assets/icon-source.png`.

### Fixed (post-P0 review)
- `doctor`: load `vulkan-1.dll` with `LOAD_LIBRARY_SEARCH_SYSTEM32` so the Windows
  loader resolves it only from System32 — prevents DLL search-order hijacking
  without trusting the (manipulable) `SystemRoot` env var or a hardcoded path.
- UI: issue the independent `ping` / `get_readiness` IPC calls in parallel.
- CI: Claude review now actually posts — added the `--comment` flag (the skill
  produces a review but only posts with it) and granted the workflows write
  permissions (posts were silently denied before); `concurrency` added; actions
  bumped to Node-24 majors.

_Data spine landed in P1; capture, embeddings, and inference still to come (P2–P4)._

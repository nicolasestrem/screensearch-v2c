# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

### Added — P5 (M5) Settings & Insights (2026-06-22)
- **Settings**: a single editable form over every persisted setting — capture (interval, change
  threshold, monitors), storage (JPEG quality, max width) and retention, model tiers, the answer
  "thinking" toggle, enrichment (text/image embeddings, worker concurrency) and the deferred-vision
  schedule, privacy (excluded apps, pause-on-lock), and advanced sidecar knobs. Saves are optimistic
  and reconcile against the backend; model tiers additionally **hot-apply the instant you pick them**
  (the running provider switches without waiting for Save). Every field states *when* it takes effect
  — tiers now, the thinking flag on your next question, capture/storage/privacy on the next capture
  start, enrichment/sidecar on restart — and retention is labelled honestly as recorded-but-not-yet-
  enforced. A failed save keeps your edits and explains.
- **Insights**: truthful activity analytics computed entirely from your own history — total and
  vision-tagged capture counts, a captures-over-time density chart, your top foreground apps, and the
  vision activity-type breakdown. No fabricated numbers: when there's nothing yet it says "not enough
  history yet", and while vision tagging is still catching up the activity breakdown is labelled
  "tagged only" with the count it's based on. Lightweight inline charts (no chart library) keep the
  bundle small — Settings and Insights are each their own lazily-loaded route chunk.
- New reusable controls: `ModelTierPicker`, `ScheduleControl`, `RetentionControl`, and the Insights
  `CapturesTrend` / `InsightsBars` charts.

### Added — P5 (M3+M4) Recall, Deck, Timeline & Moment (2026-06-22)
- **Deck (home)**: capture status with one-click start/stop, today's activity (capture count,
  density minimap, top apps), the enrichment-queue meter, and a "jump back in" grid of your most
  recent captures. Onboards from empty ("start capture"), and flags when captures are recorded but
  not yet enriched.
- **Recall (search + ask)**: hybrid search over your screen text/meaning, shown as a virtualized
  result list that links straight to the frame; and a grounded **Ask** that streams an answer with a
  collapsible "thinking" trace and clickable citation thumbnails to the source moments. Tells you
  honestly when it's "searching text only" (embeddings still indexing) or the answer model isn't
  loaded — never a dead end.
- **Timeline (the Scanline)**: the signature instrument — a canvas density ribbon over a chosen
  window (Today / 7 days / 30 days) with a sweeping signal-orange scan-head, hover thumbnails, and a
  faint scanline texture. Fully keyboard-operable (arrows scrub, Shift for big steps, Home/End jump,
  Enter opens the moment under the head); the open lands on the real nearest frame. Ambient motion
  honors "reduce motion".
- **Moment (one frame)**: the capture image, its recognized text, the vision description/tags and
  context, prev/next and a strip of nearby frames, plus an on-demand **"Tag with vision"** action
  when a frame hasn't been analyzed yet. Deep-linkable and resilient to a missing/deleted frame.
- **Frame browsing data** (`get_frames`, `get_nearest_frame`): the backend can now list frames in a
  window (newest-first) and resolve a point in time to the nearest captured frame — what the Timeline
  thumbnails, the Deck recents, and "Enter opens the moment" need. New typed `FrameMeta`.
- Every screen renders all of loading / empty / error / partial / populated — no mock data, no dead
  ends. Initial JS stays ~87 KB gzipped (the markdown renderer ships only with Recall).

### Fixed — P5 (M3+M4) review follow-ups (2026-06-22)
- **Moment Prev/Next & "around this moment" now work in real sessions.** They were sourced from a
  newest-first window that, in a busy capture session, returned only frames from the far edge of the
  window — so the buttons silently went dead and the strip showed unrelated frames. The backend now
  serves the captures immediately *bracketing* a moment (`get_frame_context`), so navigation always
  lands on the true neighbours.
- **Answer links open safely.** Links inside a streamed answer now open in your browser instead of
  navigating away inside the app window.
- **The answer's "Thinking" trace no longer snaps shut** the instant the answer finishes — it stays
  as you left it so you can read it.

### Added — P5 (M1+M2) UI foundation, shell & primitives (2026-06-22)
- **The "Command Deck" shell**: the app now opens to a real frame — a top StatusRail (live
  capture / data store / enrichment queue / inference status), a left NavRail (Deck · Recall ·
  Timeline · Insights · Settings), a ⌘K command palette (jump-to + start/stop capture), and a
  readiness banner that appears only while something is starting or needs attention.
- **Design system**: warm-graphite dark theme with a single signal-orange accent and Windows-native
  fonts, defined once as design tokens — every color, font, radius, and spacing in the UI references
  a token (no hardcoded styling). Ambient motion (the scanline texture) honors "reduce motion".
- **Typed data layer end-to-end**: every screen reads the backend through generated types only —
  typed command wrappers, a live-event subscription that keeps the status readouts fresh, and a
  streaming reducer for grounded answers. No hand-written API types.
- **Reusable building blocks**: Panel, Button, IconButton, Field, Select, Toggle, Chip, Toast,
  EmptyState, ErrorState, Skeleton, and Tooltip — accessible (visible focus, ≥32px hit targets,
  AA contrast) and built once for the screens that follow.
- **Routing & resilience**: real, deep-linkable routes for every screen (including a per-frame
  `/timeline/:id` and a friendly not-found), each wrapped in its own error boundary so a broken
  view never blanks the whole app. Routes are code-split to keep startup small (~86 KB gzipped).
- The data-bearing screen bodies (search, the Scanline Timeline, settings, insights) build on this
  foundation in the next milestones.

### Fixed — P5 (M1+M2) PR #11 review (2026-06-22)
- **Command palette recovers from a no-match search**: after typing a query that matches nothing
  and pressing the down arrow, the highlight no longer gets stuck — clearing back to a matching
  query re-highlights a command and Enter runs it again.
- **Correct keyboard hint**: the palette shortcut now reads **Ctrl+K** (this is a Windows-only app),
  not the Mac ⌘K.
- **Clearer error screens**: a route error now shows the actual status/detail of a failed navigation
  (not just a generic message), and the command-palette input no longer triggers browser
  autocomplete/spellcheck overlays.
- **Safer answers (forward-looking)**: starting a new question while one is still streaming is now
  blocked, so a previous answer's late text can't bleed into the next one.

### Added — P5 (M0) backend completion (2026-06-22)
- **Timeline data** (`get_timeline`): frame-density buckets over a time range — the data behind
  the Scanline Timeline. Sparse and half-open, with a presentation-driven bucket count.
- **Insights data** (`get_insights`): real activity aggregates — total and vision-tagged frame
  counts, capture density over time, top apps, and an activity-type breakdown. Honest-empty when
  there isn't enough history; never fabricated.
- **Settings read/write** (`get_settings` / `set_settings`): the full settings object round-trips
  through the key/value store. Model-tier changes apply live; other settings persist and take
  effect on the next capture start or app restart (the Settings screen will label which is which).
- **Frame images in the UI**: enabled Tauri's asset protocol (scoped to the capture-frames folder)
  so the interface can show stored screenshots, and replaced the permissive dev CSP with a tight,
  local-only content-security policy.
- New typed IPC: `InsightsSummary` / `AppCount` / `ActivityCount`.
- Packaging (installer + portable ZIP) is intentionally deferred to a later pass.

### Fixed — P5 (M0) PR #10 review (2026-06-22)
- **Timeline math is overflow-safe**: extreme or malformed time ranges from the UI can no longer
  panic the timeline query — an unrepresentable range simply returns no buckets.
- **Settings save is now atomic**: writing settings commits all keys in a single transaction, so a
  crash or error part-way through can never leave a half-updated settings store.
- **Insights skips needless work** on an empty or invalid time range, returning the honest-empty
  summary immediately.

### Added — P4 Inference sidecar (2026-06-21)
- **No-orphan guarantee, proven first** (`02 §2`, `03 §6`, DoD #7): the `llama-server`
  sidecar is bound to a Windows **Job Object** with `KILL_ON_JOB_CLOSE`, and the child
  is assigned to it **before** its main thread resumes — so if the app dies for any
  reason, the OS takes the sidecar down with it. A cross-process test (kill the parent,
  assert the child dies) is the gate; it passes.
- **Model supervisor** (`03 §6`): lazy spawn on first use, `/health`-gated startup,
  idle eviction after `sidecar.idle_ttl_secs`, crash restart, a startup **reap** of a
  stray sidecar from a prior run (pidfile + image-path sentinel — never an unrelated
  process), and model switching (stop + restart with a new GGUF; vision adds `--mmproj`).
- **Vision tagging** (`03 §3/§5`): on-demand (the `enqueue_vision` command), plus
  opt-in **timer** and **idle** producers (`GetLastInputInfo`) that batch still-untagged
  frames — never real-time. The worker runs the frame's JPEG through the sidecar and
  stores the analysis.
- **Grounded, streaming answers** (`03 §13.5`): the `ask` command retrieves top-K
  hybrid hits, grounds the answer in their OCR text, and streams typed `answer_delta`
  events — `thinking` trace, answer tokens, and one citation per source frame. Reasoning
  is handled both as a `reasoning_content` field and as inline `<think>` tags.
- **Tiered models, runtime-downloaded, Rust-only** (`MODEL_REGISTRY`, `01 §5`): vision +
  answer each offer Default/Quality/Beta via `set_model_tier`. The `llama-server` binary
  (prebuilt Vulkan release) and the GGUF weights (+ same-repo mmproj) download on first
  use — the binary from GitHub, the models via `hf-hub` (no Python in the runtime).
- New IPC: `ask`, `enqueue_vision`, `set_model_tier` commands; `answer_delta` +
  `sidecar_status` events; `sidecar` readiness now tracks the supervisor's lifecycle.
- Real end-to-end inference (download + GPU) is covered by `#[ignore]`d smoke tests
  (`cargo test -p inference --test smoke -- --ignored`); the always-green suite proves
  the lifecycle, the HTTP client (against a mock), and the providers deterministically.

### Fixed — P4 sidecar binary resolution (2026-06-22)
- **Resilient llama.cpp release selection** — `ensure_binary` previously read GitHub's
  single `/releases/latest`, which fails with `no win-vulkan-x64 asset in the latest
  llama.cpp release` whenever that release ships an **incomplete asset set** (llama.cpp's
  CI occasionally publishes a build with only a partial set of platform zips — observed:
  `b9753` had 1 asset, no Vulkan zip). Resolution now scans the recent-releases list and
  takes the newest release that actually carries the `*-win-vulkan-x64.zip` asset. Pure
  selector `pick_vulkan_from_releases` is unit-tested (incomplete-newest-then-complete,
  newest-wins, none-found). Vulkan stays the shipped lane — it is vendor-neutral and runs
  on Blackwell GPUs (RTX 50-series) where the prebuilt `cuda-12.4` asset would not.

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

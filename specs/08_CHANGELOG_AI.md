# 08 — AI Changelog

> Append-only record of what the agent changed during the build, **with reasons**. One entry per
> meaningful change set. Empty until P0 begins. (This tracks build work; the design-phase history
> lives in git.)

## <date> — <short title>
- **Change:** what was added/modified.
- **Why:** the reason, tied to a spec section.
- **Verification:** the command run + verbatim result.

---

## 2026-06-21 — P0 Scaffold (`p0-scaffold` branch)
- **Change:** Stood up the full workspace scaffold — Cargo workspace; `traits` crate with the six
  `03 §3` contracts + domain/jobs/IPC types; honestly-empty skeleton crates `kernel`, `store`,
  `capture`, `ocr`, `embeddings`, `inference`; `src-tauri` Tauri 2 shell with `ping` /
  `get_readiness` typed commands; React 18 + TS + Vite UI skeleton; `ts-rs` binding generation to
  `ui/src/bindings/`; ESLint flat config with the Rules-of-Hooks gate; `crates/doctor`
  environment smoke-check; `.github/workflows/ci.yml`; generated app icons.
- **Why:** P0 Scaffold per `02 §5` / `04 §3` — establish the modular kernel layout (`03 §2`),
  the typed UI↔core contract (`03 §7`), and CI before any phase that writes data.
- **Decisions / corrections:** ts-rs `i64/u64` forced to TS `number` via per-field
  `#[ts(type="number")]` (env override ignored by ts-rs 10.1) — guarded by a test;
  `export_to = "../../../ui/src/bindings/"` (anchors at source-file dir); provisional defaults for
  the two undocumented `03 §8` vision-schedule keys (see `07` gap #1). No fakes/stubs;
  Windows-native crates left un-`forbid`-ed for the P2/P4 `unsafe` FFI paths.
- **Verification:** `cargo fmt --all -- --check` (exit 0); `cargo clippy --workspace
  --all-targets -- -D warnings` (exit 0); `cargo test --workspace` (28 passed, 0 failed);
  `cargo run -p doctor` (WebView2 OK v149, Vulkan OK, llama-server WARN); `ui npm run build`
  (✓ built) + `npm run lint` (exit 0); `git diff --exit-code ui/src/bindings` (exit 0);
  `npx tauri dev` window observed rendering "Kernel says: pong" + readiness list.

## 2026-06-21 — P0 refinements (post-review, user decisions)
- **Change:**
  - Vision scheduling: replaced the single `enrich.vision_mode` enum with independent **opt-in
    toggles** — `enrich_vision_timer_enabled` (false) + `_interval_ms` (60 min) and
    `enrich_vision_idle_enabled` (false) + `_idle_secs` (5 min); removed `VisionMode`. On-demand
    is always available. (`06` patch #1; `03 §8` updated.)
  - Bundle identifier → `app.screensearchv2c.desktop` (`app.screensearch.desktop` was taken).
  - Readiness contract: defined `ComponentStatus { Unknown, Disabled, Initializing, Ready,
    Unavailable, Error }` + `ComponentReadiness { status, detail? }`; `Readiness` now carries one
    per subsystem; UI renders status + detail. (`07` gap #3; `03 §7` updated.)
  - `doctor` refactored into a **library + thin CLI** with a structured `Report` and a `--json`
    mode (reusable by CI and, later, the app). (`07` gap #4.)
- **Why:** user direction + closing the `07` silent-spec gaps with the spec kept authoritative.
- **Verification:** `cargo fmt --all -- --check` (exit 0); `cargo clippy --workspace
  --all-targets -- -D warnings` (exit 0); `cargo test --workspace` (traits 28 passed, 0 failed);
  `cargo run -p doctor` text + `--json` both OK; `ui npm run build` (✓ built) + `npm run lint`
  (exit 0).

## 2026-06-21 — P1 Data Spine (`p1-data-spine` branch)
- **Change:** Implemented the `store` crate end-to-end — the durable data spine (`03 §4/§5`):
  - SQLite (WAL) on `rusqlite` (bundled) + `sqlite-vec` (`vec0`) + FTS5; forward-only migrations
    tracked in `schema_version` (v1 = the full `03 §4` DDL, transcribed; FTS5 external-content
    sync triggers + vec0 cleanup triggers added per the spec's prose).
  - Full `Store` trait: frames / OCR / vision inserts, settings, text + image embedding upserts
    with synchronized `vec0` shadows, the durable **job queue** (atomic `UPDATE … RETURNING`
    claim, retry+backoff, dead-letter at `max_attempts`, stats), and **hybrid search**
    (FTS5 BM25 ⊕ cosine-KNN → **RRF**, k=60). Plus inherent `get_frame` (backs the `get_frame`
    command) and `delete_frame` (retention primitive).
  - Wired the store into `src-tauri`: opens `screensearch.db` at the app-data dir on launch,
    flips `db` readiness to Ready/Error, adds the daily-rotating **file log** (`03 §9`, the sink
    deferred in P0), and exposes `get_job_stats` over typed IPC.
- **Why:** P1 per `02 §5` / `04 §3` — build the data spine before any producer; *everything
  writes here*.
- **Decisions / corrections:** vector arm needs the query embedded but the store must stay
  impl-agnostic → it optionally holds `Arc<dyn EmbeddingProvider>` (a trait; FTS+vec+RRF is fully
  built and tested with a fake embedder, real fastembed injected in P3 — `07` gap #5).
  Single-connection + `spawn_blocking` concurrency model; `sqlite-vec` pinned to **0.1.9** (the
  0.1.10-alpha amalgamation is broken — missing `sqlite-vec-diskann.c`); `blake3` content-hash;
  non-breaking `UNIQUE`/trigger schema additions; `JobState::Failed` left reserved. Stuck-`running`
  recovery deferred to the kernel worker (`07` gap #6). No fakes/stubs in shipped code.
- **Verification:** `cargo fmt --all -- --check` (exit 0); `cargo clippy --workspace --all-targets
  -- -D warnings` (exit 0); `cargo test --workspace` (store **23**, traits 28, screensearch 2 =
  53 passed, 0 failed); `ui npm run build` (✓ built); `git status ui/src/bindings` (no drift);
  **observed running** — `cargo run -p screensearch` created `screensearch.db` + `-wal`/`-shm`
  (WAL active) and `logs/screensearch.log.2026-06-21` containing
  `INFO store: applied store migration schema_version=1` and `INFO screensearch_lib: store opened`.

## 2026-06-21 — P1 review fixes (PR #4, `p1-data-spine`)
- **Change:** Addressed all PR #4 review findings (Gemini + `@claude`):
  - **Correctness** — `open_state` now treats a `schema_version()` failure (after a successful
    open) as `db = Error` + `store = None`, instead of silently reporting `Ready` with a path-only
    detail. No more "Ready but unqueryable" state.
  - **Correctness** — `complete_job` / `fail_job` now error on a zero-row update (stale/unknown id)
    rather than silently no-op'ing, matching the queue's "never silently dropped" contract.
  - **Spec alignment** — `insert_vision` now also fills `frames.activity_type` (the `03 §4` column
    documented "filled by vision"), in one transaction with the `vision_analysis` write, so the
    timeline can filter by activity without a join.
  - **Performance** — `hybrid_search`'s `hydrate` replaced its per-hit N+1 queries with two bulk
    `IN (…)` queries (frame context + fallback OCR snippets), ≤2 round-trips regardless of result
    count.
  - **Maintainability** — `f32_blob` and `dedup_keep_order` made `pub(crate)` and reused in
    `search.rs` (removed the duplicated LE-serialization and inline-dedup).
- **Why:** external review (be skeptical, verify) — each finding was checked against the codebase
  and the spec; all four were valid for this stack, none warranted pushback.
- **Verification:** added 1 test (`completing_or_failing_an_unknown_job_is_an_error`) + updated the
  `insert_vision` test to assert `frames.activity_type`; the N+1/blob/dedup changes are
  behavior-preserving and covered by the existing search tests. `cargo fmt --all -- --check`
  (exit 0); `cargo clippy --workspace --all-targets -- -D warnings` (exit 0);
  `cargo test --workspace` (store **24**, traits 28, screensearch 2 = 54 passed, 0 failed).

## 2026-06-21 — CI fix: Claude review couldn't post (read-only token)
- **Change:** Granted `pull-requests: write` + `issues: write` to `claude-code-review.yml`
  (and `claude.yml`); added `concurrency` (cancel-in-progress) to the review workflow; bumped
  `actions/checkout`→v5 and `actions/setup-node`→v5 across all workflows.
- **Why:** The first PR #2 review *ran* (10 turns, 17m52s, $5.92) but posted nothing — the job's
  `GITHUB_TOKEN` had only read scopes, so ~25 post attempts were denied
  (`permission_denials_count: 25`, "No buffered inline comments"). The denial retries also
  inflated the runtime. `@claude` in `claude.yml` had the same latent read-only bug.
- **Verification:** `python -c yaml.safe_load` on all three workflows (OK); re-run on next push.

## 2026-06-21 — P0/P1 review pass (`review/p0-p1` branch)
- **Change:** Reviewed the merged P0 + P1 against `04`/`03` (full record in `05` Pass 3). Verdict:
  both **complete & compliant**, no correctness bugs. Applied four **additive** doc/clarity fixes
  for minor findings — no behavior change:
  - `concurrent_claims_never_double_claim` (`crates/store/tests/store.rs`): added a comment stating
    it proves single-shared-connection claim correctness under concurrent async callers, *not*
    multi-connection WAL contention.
  - `TimeRange` (`crates/traits/src/ipc.rs`): doc made explicit — half-open `[start, end)` (start
    inclusive, end exclusive); a matching note at the `hybrid_search` query site (`search.rs`).
  - `07` known-gaps: added #7 (`03 §13` "< ~200 ms" latency unverified until a realistic-DB fixture
    in P3) and #8 (vec-arm `time_range` post-filter can under-return on tight windows — tune in P3),
    promoting `05` Pass 2 "Still risky" prose into the tracked table with an owner.
- **Why:** review per `04 §3`/`§6`; close the gap between honestly-noted risks and *tracked* ones,
  and pin the half-open `TimeRange` contract before the P5 UI consumes it.
- **Verification:** see `05` Pass 3 — `fmt`/`clippy -D warnings` clean, `cargo test --workspace`
  unchanged-green (doc/comment-only changes), `ui npm run build`, no ts-rs drift.

## 2026-06-21 — P2 Capture happy path (`p2-capture` branch)
- **Change:** Implemented the always-on capture pipeline end-to-end (full record in `05` Pass 4):
  - `capture` — `WgcCapture: CaptureSource` on raw `windows-rs` 0.62 (per-monitor D3D11 + WGC frame
    pool on a COM(MTA) thread, BGRA→RGBA readback), the diff gate (`32×32` luma mean-abs-diff +
    `blake3` hash), and the privacy gate (`OpenInputDesktop` lock probe + foreground app/title for
    `privacy.excluded_apps`, also filling `app_hint`/`window_title`).
  - `ocr` — `WinRtOcr: OcrProvider` on a dedicated **STA** worker (WinRT `Media.Ocr`),
    `RecognizeAsync().join()`, `mean_confidence = -1.0` sentinel.
  - `kernel` — the capture loop (CaptureSource→OcrProvider→JPEG→`insert_frame`/`insert_ocr`→enqueue
    `embed_text`→emit `capture_tick`), a `broadcast` event bus, a typed-`Settings` loader, and
    idempotent `start_capture`/`stop_capture`. `vision_tag` is never auto-enqueued (`13.3`).
  - `src-tauri` — wires store + OCR + capture factory + kernel; `capture_control` + `get_frame`
    commands; live `get_readiness`; forwards `capture_tick`/`readiness_changed` to the UI.
  - `ui` — a minimal live timeline (Start/Stop + readiness strip + `capture_tick` rows).
  - `traits` — added `app_hint`/`window_title` to `CapturedFrame` (internal; no IPC change).
- **Why:** P2 per `02 §5` / `04 §3` — stand up the cheap, always-on half (capture→OCR→store) and
  prove the kernel. Heavy work stays deferred to the job queue (P3/P4).
- **Decisions:** four spec-silent items resolved with the user (`07` #9–#13): capture **off until
  Start** (privacy-first); **raw `windows-rs`** for WGC; **minimal live timeline**; **populate
  `app_hint`/`window_title`**. OCR-confidence contradiction → `-1.0` sentinel (`06` #2). Engineering
  notes (diff metric, day-bucket JPEG path, COM apartments, windows-rs feature gotchas) in `07`.
- **Verification:** `fmt`/`clippy -D warnings` clean; `cargo test --workspace` **66 passed, 0
  failed, 3 ignored**; `ui npm run build` + `lint` clean; no ts-rs drift. **Observed running** —
  the `#[ignore]`d `e2e_capture` test drove the real Kernel (WGC + WinRT OCR + on-disk store) and
  stored a frame + OCR row + JPEG and enqueued an `embed_text` job (no `vision_tag`), 3.55s; real
  WGC and WinRT OCR smoke tests also pass locally on Win11 26200.

## 2026-06-21 — P3 Deferred Enrichment (`p3-enrichment` branch)
- **Change:** Lit up the deferred half of the system — the `embed_text` jobs P2 enqueues are now
  drained into vectors and hybrid search returns them.
  - `embeddings::FastEmbedProvider` — the real `EmbeddingProvider` via **fastembed 5.17.2** (ONNX,
    no Python): text `EmbeddingGemma300MQ` (768-dim, embeds one-at-a-time), optional image
    `NomicEmbedVisionV15`; lanes behind `Arc<Mutex>` run inside `spawn_blocking`; models load
    eagerly into `<app-data>/models/fastembed`.
  - `kernel::worker_pool` — a bounded pool (N = `enrich.worker_concurrency`) that claims/runs/
    completes `embed_text` + `embed_image` jobs with exponential backoff, an idle poll, a periodic
    + startup stale-`running` sweep (gap #6), graceful shutdown, and a `job_progress` event. Public
    `process_job` lets tests drive one job deterministically. **Never** handles `vision_tag` (P4).
  - `store` — `embedder` made runtime-settable (`Arc<RwLock<Option<…>>>` + `set_embedder`);
    `frame_enrichment_input` (lightweight worker read); `reset_stale_running_jobs`. Three matching
    `Store` trait methods (`set_embedder` defaulted no-op). Vector arm of `hybrid_search` now goes
    live the moment the embedder is attached.
  - `kernel` — `attach_embedder`/`start_workers`/`stop_workers`/`set_embed_readiness`; capture loop
    enqueues `embed_image` when `enrich.image_embeddings`; `KernelEvent::JobProgress`.
  - `src-tauri` — loads the model off the launch thread (start never blocks on the download),
    flips `embed_model` readiness, `attach_embedder`s; adds the `search` command; forwards
    `job_progress`; stops workers on exit (best-effort).
  - `traits` — `FrameEnrichmentInput`; `Store::{get_enrichment_input, reset_stale_running_jobs,
    set_embedder}`; `EmbeddingProvider::{text_model_name, image_model_name}` (defaulted).
- **Why:** P3 per `02 §5` / `04 §3` — embedding worker (fastembed) + hybrid search end-to-end,
  satisfying DoD `03 §13.2` (deferred embeddings populate vectors via the job queue) and `13.4`
  (hybrid FTS5+vec→RRF returns correct frames < ~200 ms). First phase to exercise the durable queue
  end-to-end, proving the worker pattern before P4 adds the sidecar.
- **Decisions (user-confirmed):** vision fully deferred to P4; `search` backend command only (UI
  P5); model loads at launch with background workers (`02 §5`). Engineering: symmetric `embed_texts`
  for index+query (gap logged); runtime `set_embedder` via `RwLock` (no new dep); single shared
  model handle; backoff `1 s·2^attempts` cap 60 s; 5-min stale-job visibility timeout. New `07`
  entries for the prompt asymmetry, `onnxruntime.dll` bundling, and the shared-handle trade-off;
  gaps #6 and #7 resolved.
- **Verification:** `fmt`/`clippy --workspace --all-targets -D warnings` clean; `cargo test
  --workspace` all pass (0 failed) — store 27, traits 28, kernel 5+3, screensearch 2; `ui npm run
  build` clean, no ts-rs drift. **Observed running** — `embeddings -- --ignored` loaded the real
  EmbeddingGemma300MQ and produced deterministic 768-dim vectors (9.96 s); `store --test perf --
  --ignored` measured **p95 = 32.6 ms over 10 000 frames** (DoD 13.4 ✓); the `attach_embedder`
  integration test drove the real worker pool draining a job and the vector arm then finding the
  frame via a non-FTS-matching query.

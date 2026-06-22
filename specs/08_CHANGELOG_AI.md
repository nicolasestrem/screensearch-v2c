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

## 2026-06-21 — P3 review fixes (PR #7)
- **Change:** Addressed the three findings from the PR #7 code review (gemini-code-assist):
  1. **Stale-job sweep clock precision (high).** `reset_stale_running_jobs` now branches: the
     **startup sweep** (`older_than_ms <= 0`) requeues *every* `running` job unconditionally (no
     `updated_at` comparison, so it can't miss a job marked running in the last fraction of a second
     before a crash); the periodic sweep keeps the time filter. Additionally, `claim_jobs` now stamps
     `updated_at` with the `unixepoch()*1000` **DB clock** (not the caller's `now`), so the periodic
     sweep compares like-for-like — removing the Rust-ms-vs-SQLite-second mismatch at the root.
  2. **Image-load retries (medium).** `embed_image` no longer dead-letters on *any* load error — a
     transient Windows sharing violation (AV / indexer / backup briefly holding the JPEG) now
     **retries** (the file still `exists()`); only a genuinely missing file is dead-lettered.
  3. **`WorkerPool` Drop safety (medium).** Added `impl Drop for WorkerPool` that signals stop, so a
     pool dropped without a graceful `shutdown` (panic / early return) doesn't leave detached workers
     draining the whole queue; `shutdown` uses `mem::take` to avoid moving out of a `Drop` type.
- **Why:** robustness of the durable queue and the Windows file path under real conditions; review
  follow-through (`04 §5/§6`). The claude code-review action found no issues.
- **Verification:** `fmt`/`clippy --workspace --all-targets -D warnings` clean; `cargo test
  --workspace` all pass (0 failed) — kernel enrichment **7** (added two `embed_image` worker tests:
  load-from-disk happy path + missing-file dead-letter), store 27, the existing sweep tests still
  green. Also merged the updated `claude-code-review` workflow from `main`.

---

## 2026-06-21 — P4 Inference sidecar (`feat/p4-inference-sidecar` branch)
- **Change:** Built the inference sidecar end-to-end. New `crates/inference` modules: `job_object` +
  `process` (Windows Job Object `KILL_ON_JOB_CLOSE`; suspended `CreateProcessW` + assign-before-resume;
  pidfile + image-path reap), `client` (reqwest OpenAI: non-stream vision + SSE answer), `supervisor`
  (lazy spawn, idle-evict, `/health` gate, crash restart, startup reap, model switch, `Lease`
  in-flight guard, `SidecarStatus` broadcast), `models` + `download` (tier→repo map, `Q4_K_M`+mmproj
  pick, GitHub-release Vulkan binary + `hf-hub` GGUF downloaders — no Python), `vision` + `answer`
  providers (`VisionProvider`/`AnswerProvider`; JSON-or-rawtext vision parse; `ThinkSplitter` for
  inline `<think>` tags; one `Citation` per grounding frame). Wiring: kernel `attach_inference` + a
  shared vision slot into the worker pool + the `vision_tag` branch; `vision_scheduler` (timer + idle,
  opt-in); `Store::untagged_frame_ids`; `KernelEvent::SidecarStatus`→`sidecar` readiness;
  `capture::user_idle_ms`; composition root resolves the binary off-thread, builds the supervisor +
  providers, bridges status, attaches, shuts down on exit. New commands `ask` / `enqueue_vision` /
  `set_model_tier`; new events `answer_delta` / `sidecar_status`.
- **Why:** P4 per `02 §5` / `04 §3`, satisfying `03 §6` (sidecar lifecycle), `03 §5` (deferred vision),
  `03 §13.5` (grounded thinking answers), `03 §13.6` (tiered models), and **DoD #7** (no orphan). Built
  lifecycle-first: the Job-Object no-orphan binding before any real inference wiring.
- **Decisions / corrections (user-confirmed + logged in `07`):** runtime auto-download of *both* the
  `llama-server` binary (GitHub Vulkan release) and the GGUF models (`hf-hub`, no Python in runtime);
  acceptance bar = lifecycle + mock-tested inference, with real GPU end-to-end as `#[ignore]` smokes.
  Reap sentinel uses the child's full image path (under app-data) cross-checked with a pidfile — not
  a custom flag `llama-server` would reject; `KILL_ON_JOB_CLOSE` remains the primary guarantee.
  Citations = the retrieved context frames (reliable), not parsed from prose. Ask top-K = 8; vision
  timer/idle batch N = 20; vision/answer confidence fallback uses the `-1.0` "unknown" sentinel
  (consistent with the OCR-confidence decision). No new IPC types were needed — `ask`/`enqueue_vision`/
  `set_model_tier`/`AnswerDelta`/`SidecarStatus`/`VisionTarget` all pre-existed from P0.
- **Verification:** `cargo fmt --all -- --check` (exit 0); `cargo clippy --workspace --all-targets --
  -D warnings` (clean); `cargo build` (ok); `cargo test --workspace` all pass (0 failed) — inference
  23 unit + no-orphan 1 + reap 2 + client 4 (+ 2 smoke ignored), kernel enrichment 9 (two new
  `vision_tag` tests), store 28 (new `untagged_frame_ids` test), traits 28; `ui npm run build` ok
  (`tsc --noEmit` clean, no binding diffs). The no-orphan gate
  `killing_parent_terminates_job_bound_child` passes — DoD #7 demonstrated. Real vision-tag +
  streamed-answer on the RTX 5060 Ti remain the gated manual smoke (`cargo test -p inference --test
  smoke -- --ignored`).

## 2026-06-22 — P4 fix: sidecar binary resolution survives incomplete llama.cpp releases
- **Change:** `inference::download::resolve_binary_url` no longer reads GitHub's single
  `/releases/latest`. It now fetches the recent-releases list (`/releases?per_page=10`) and a new
  pure helper `pick_vulkan_from_releases` walks them newest→oldest, returning the
  `(download_url, name)` of the first release that actually carries a `*-win-vulkan-x64.zip` asset
  (the existing `pick_vulkan_asset` selector, reused). Error message on total miss is now "no
  win-vulkan-x64 asset in **any recent** llama.cpp release".
- **Why:** Sidecar start failed with `llama-server unavailable: no win-vulkan-x64 asset in the
  latest llama.cpp release`. Root cause (confirmed against the live GitHub API): llama.cpp's CI
  sometimes publishes a release with an **incomplete asset set** — `b9753` carried a single asset
  and **no** Windows Vulkan zip — and `/releases/latest` resolving to such a build broke startup
  outright. A network/rate-limit failure surfaces a *different* message, so the symptom uniquely
  implicated the selector running against a real-but-incomplete release. Scanning recent releases
  uses the newest *usable* build instead of failing. Vulkan stays the lane (vendor-neutral; runs on
  the user's Blackwell RTX 5060 Ti, where the prebuilt `cuda-12.4` asset would not). `SSV2C_LLAMA_RELEASE_URL`
  remains the override escape hatch.
- **Verification:** TDD — added `skips_release_with_incomplete_assets`, `prefers_the_newest_release_that_has_vulkan`,
  `no_vulkan_in_any_release_returns_none` (red first: `cannot find function pick_vulkan_from_releases`,
  exit 101). After implementing: `cargo test -p inference --lib download` → **8 passed, 0 failed**;
  `cargo test -p inference` full → all green (unit 8+others, no-orphan 1, reap 2, client 4, smoke 2
  ignored); `cargo fmt --all -- --check` (exit 0); `cargo clippy -p inference -- -D warnings`
  (clean). Live resolution against the current API selects `b9754` (newest complete) over the
  intervening incomplete `b9753`. **Real GPU end-to-end now confirmed** on an RTX 5060 Ti: the
  new resolver downloaded the Vulkan binary, `llama-server` launched, and the gated smoke passed —
  `cargo test -p inference --test smoke -- --ignored --nocapture --test-threads=1` →
  `test result: ok. 2 passed; 0 failed` in 15.77 s, with `real_answer_streams_tokens` returning
  `ANSWER: The deploy finished at 14:32. CITATIONS: [42]` and `real_vision_tags_an_image`
  describing the test image. DoD #5 (real sidecar) demonstrated.

## 2026-06-22 — P5 (M0) Backend completion (`feat/p5-backend` branch)
- **Change:** Implemented the three `03 §7` commands the Command-Deck UI requires that P4 left out,
  plus the queries behind them and frame-image serving:
  - `crates/store/src/timeline.rs` — `timeline_buckets(start, end, bucket_count)`: sparse,
    integer-index, half-open `[start, end)` frame-density buckets (backs `get_timeline`).
  - `crates/store/src/insights.rs` — `insights_summary(start, end)`: total/tagged counts, capture
    density, top apps, activity breakdown (backs `get_insights`, the Insights screen).
  - `crates/traits/src/ipc.rs` — new `InsightsSummary` / `AppCount` / `ActivityCount` ts-rs types;
    `contracts.rs` — defaulted `timeline_buckets` / `insights_summary` on `Store`; `store/src/lib.rs`
    forwards both.
  - `crates/kernel/src/settings.rs` — `save_settings`, the exact inverse of `load_settings`.
  - `src-tauri/src/lib.rs` — `get_timeline` / `get_insights` / `get_settings` / `set_settings`
    commands (registered); `set_settings` hot-applies model tiers to the live providers.
  - `src-tauri/tauri.conf.json` + `Cargo.toml` — enabled the asset protocol (`protocol-asset`
    feature, scope `$APPDATA/frames/**`) and a tight CSP (was `null`).
- **Why:** P5 (`02 §5`, `UI_REFERENCE`) — the UI consumes typed IPC only, so the timeline,
  settings, and insights screens cannot exist until these commands do; frame images need a way to
  reach the WebView (asset protocol). CSP hardening closes the `07` P0-dev gap. Packaging (DoD
  §13.9) is deferred to a follow-up per the user's decision.
- **Decisions (spec-silent, logged in `07` #21–#25):** asset protocol for frame images;
  `get_timeline` takes a presentation-driven `bucket_count`; `toast` event stays client-side;
  `storage.retention_days` persisted-not-enforced; `InsightsSummary` is a new contract shape.
- **Verification:** `cargo fmt --all -- --check` (exit 0); `cargo clippy --workspace --all-targets
  -- -D warnings` → `Finished … in 7.40s` (no warnings); `cargo test --workspace` → all green incl.
  the 4 new tests (`round_trips_defaults`, `round_trips_non_default_values`,
  `timeline_buckets_are_sparse_and_half_open`, `insights_summary_aggregates_truthfully`) and the
  regenerated `export_bindings_insightssummary`; new bindings `InsightsSummary.ts` / `AppCount.ts`
  / `ActivityCount.ts` emitted with `number` (not `bigint`) fields. Live asset-protocol image
  render is deferred to M4 (no UI surface yet) — recorded honestly.

## 2026-06-22 — P5 (M0) PR #10 review fixes (`feat/p5-backend` branch)
- **Change:** Addressed the three actionable comments on PR #10:
  - `crates/store/src/timeline.rs` — made `timeline_buckets` overflow-safe: `checked_sub` span,
    ceil as `span / n + (span % n != 0)`, bucket end via `checked_add(width).unwrap_or(end).min(end)`.
  - `crates/store/src/insights.rs` — early honest-empty return when `end <= start` or the span is
    unrepresentable, skipping four queries + a `timeline_buckets` call.
  - `crates/traits/src/contracts.rs` — new defaulted `Store::set_settings_batch`; `store/src/settings.rs`
    overrides it with a single `unchecked_transaction` + `commit`; `store/src/lib.rs` forwards it;
    `crates/kernel/src/settings.rs` — `save_settings` now builds every pair (incl. fallible JSON)
    up front and commits them atomically via the batch.
- **Why:** Review hardening. (1) hostile frontend timestamps could overflow the timeline math
  (panic in debug / wrap in release); (2) redundant DB work on invalid windows; (3) a crash or
  mid-loop `serde_json` error in the old 20-call `save_settings` could leave a partially-updated
  `settings` table that `load_settings`' per-key default fallback hides silently.
- **Tests added:** `set_settings_batch_writes_all_and_overwrites`, `timeline_buckets_survives_extreme_ranges`.
- **Verification:** `cargo fmt --all -- --check` (exit 0); `cargo clippy --workspace --all-targets
  -- -D warnings` → `Finished … in 1.23s` (no warnings); `cargo test --workspace` all green —
  `store` now **33 passed**, `kernel` settings **2 passed**, 0 failed across the workspace.
- **Notes:** `unchecked_transaction` is sound under `with_conn`'s exclusive mutex hold. A
  forced-rollback test isn't feasible via the public API (no fault seam; every upsert is a valid
  `(TEXT, TEXT)` write) — the all-or-nothing guarantee is structural (`?` before `commit`).
  Recorded honestly rather than faked. Other review verdicts were "clean" and needed no change.

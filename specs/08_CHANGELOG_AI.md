# 08 — AI Changelog

> Append-only record of what the agent changed during the build, **with reasons**. One entry per
> meaningful change set. Empty until P0 begins. (This tracks build work; the design-phase history
> lives in git.)

## <date> — <short title>
- **Change:** what was added/modified.
- **Why:** the reason, tied to a spec section.
- **Verification:** the command run + verbatim result.

---

## 2026-06-22 — P5 (M3+M4) PR #12 review fixes (`feat/p5-screens`)
- **Change:** Addressed the `claude-review` findings on PR #12. (1) Fixed a high-priority Moment bug:
  prev/next + the context strip were sourced from `get_frames([at−30m, at+30m), 24)`, but
  `frames_in_range` is newest-first capped, so a dense window returned only far-edge frames and
  dropped the anchor (`findIndex` = −1 → dead navigation). Added `SqliteStore::neighbour_frames(at,
  half_window_ms, limit_each)` (closest-before DESC + closest-after ASC, merged ascending, anchor
  excluded) exposed as `get_frame_context`; new `useFrameContext` hook + `frameContext` query keys
  (invalidated on `capture_tick`); Moment derives prev/next by capture-time. (2) `AnswerStream`
  markdown links now render `target="_blank" rel="noopener noreferrer"` so model output can't hijack
  the WebView. (3) `AnswerStream` "Thinking" trace is now a controlled `<details>` (auto-opens on a
  new stream, never auto-collapses), hooks hoisted above early returns.
- **Why:** Hard rule "prioritize accuracy over task completion" + CLAUDE.md "review and test
  thoroughly." The Moment bug breaks a headline feature in any real (non-trivial) session; the link
  and thinking-panel issues are correctness/UX regressions in the streamed-answer path. No new IPC
  type (reuses `FrameMeta`), so bindings are unchanged. Two findings acknowledged but deferred as
  minor (`07` #34 live-search invalidation; the cosmetic timeline clamp).
- **Verification:** `cargo fmt --all -- --check` exit 0 · `cargo clippy --workspace --all-targets --
  -D warnings` exit 0 · `cargo test -p store` 36 passed (incl. `neighbour_frames_brackets_anchor_
  with_closest_each_side`) · `cargo test -p traits` 32 passed · `git diff --stat -- ui/src/bindings`
  empty · `npm run typecheck`/`lint`/`build` all exit 0 (build ✓ in 1.85s, initial JS ≈ 87.5 KB gz).

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

## 2026-06-22 — P5 (M1+M2) UI foundation, shell & primitives (`feat/p5-ui-foundation` branch)
- **Change:** Replaced the minimal P2 `App.tsx` with the full "Command Deck" frontend foundation
  (UI_REFERENCE §1–9), consuming the M0 backend through the generated bindings only:
  - **Deps** (`ui/package.json`): `react-router-dom@6`, `@tanstack/react-query@5`, `zustand@5`,
    `@tanstack/react-virtual@3`, `react-markdown@9`, `remark-gfm@4`; dev `tailwindcss@3` +
    `@tailwindcss/typography`, `postcss`, `autoprefixer`.
  - **Tokens & Tailwind** (`ui/src/styles/{tokens.css,globals.css}`, `tailwind.config.js`,
    `postcss.config.js`): the palette/type/spacing/radius/z/motion of UI_REFERENCE §1–2 as CSS
    custom properties; Tailwind *replaces* color/radius/font scales with token refs (no off-palette
    hue or off-scale type reachable) but keeps the default `spacing` scale (its rem steps already
    equal 4·8·12·16·24·32·48 px and back the width/height/inset utilities); reduced-motion gates the
    scanline drift.
  - **Typed IPC layer** (`ui/src/lib/ipc/`): `commands.ts` (one wrapper per `03 §7` command, camel→
    snake args), `events.ts` (typed `listen` map), `queryKeys.ts`, `queries.ts`, `mutations.ts`
    (optimistic `useSetSettings`), `useAsk.ts` (a reducer folding `answer_delta` → phase/thinking/
    answer/citations), `useLiveEvents.ts` (one subscription manager: `readiness_changed`/
    `job_progress`/`sidecar_status` patch the Query cache, `capture_tick` debounce-invalidates
    timeline/insights), `frameSrc.ts` (memoized `appDataDir` + `convertFileSrc`, throws-safe in dev).
  - **State** (`ui/src/state/`): `uiStore` (palette/range/focused frame) and `toastStore`
    (client-side toast queue + a `toast.*` facade for mutation callbacks).
  - **Shell** (`ui/src/components/shell/`): `AppShell` (mounts `useLiveEvents` + the global ⌘K
    listener once), `StatusRail`, `NavRail`, `CommandPalette` (filter + ↑/↓/Enter/Esc), `ReadinessBanner`.
  - **Primitives** (`ui/src/components/primitives/`): the twelve §5 primitives + an inline-SVG icon
    set (no web fonts), all tokens-only with visible focus and ≥32px targets.
  - **App wiring** (`ui/src/app/`): `providers.tsx` (QueryClient) + `router.tsx` (`createBrowserRouter`,
    lazy routes, per-route `errorElement`); route scaffolds for all screens (data bodies land M3–M5);
    `vite.config.ts` vendor `manualChunks`. Removed the superseded `ui/src/styles.css`.
- **Why:** P5 M1+M2 per the approved plan and UI_REFERENCE — establish the shell, typed data plane,
  design tokens, routing/error boundaries, and primitives the data screens build on. No backend or
  bindings changed (`git diff --exit-code -- ui/src/bindings` → exit 0).
- **Decisions (logged):** (a) StatusRail shows the **DB readiness/status** chip, not a "DB size"
  number — no size field exists in the IPC contract, so showing a size would be fabricated (07 #27).
  (b) The `job_progress` event payload is a bare `JobStats` (the kernel emits the inner value), not
  the `JobProgress` wrapper binding — `events.ts` types it accordingly. (c) React Router's
  v7-future-flag console warning is informational and left as-is. (d) Route scaffolds state each
  screen's purpose (IA per §3) — not "Coming Soon"; the screen bodies are scheduled M3–M5.
- **Verification:**
  - `npm run build` (tsc --noEmit && vite build) → `✓ built in 1.27s`; initial JS gzipped ≈ 86 KB
    (`react-vendor` 67.89 + `query` 11.08 + `index` 7.24) + CSS 5.40 KB — well under the 250 KB
    budget (UI_REFERENCE §8); each route emitted as its own lazy chunk.
  - `npm run lint` (eslint .) → clean, **no errors/warnings** (Rules-of-Hooks error gate passes).
  - **Degraded (Playwright-MCP vs `npm run dev`, localhost:5173):** the shell renders (StatusRail +
    NavRail + content); with no Tauri runtime the readiness query errors and the rail shows the
    honest **"Kernel offline"** chip (the error state); routing works for `/`, `/recall`, `/timeline`,
    `/timeline/42` (Moment), `/insights`, `/settings`, and a bogus path → the **NotFound** invitation;
    global **Ctrl+K** opens the palette (input auto-focused), **↓ + Enter** selects "Go to Recall"
    and navigates + closes; console clean apart from a favicon 404 and the router future-flag note.
    Screenshots: `m1m2-shell-deck.png`, `m1m2-timeline-active.png`, `m1m2-command-palette.png`
    (gitignored verification artifacts).
  - **Authoritative (`npm run dev` = `tauri dev`):** the integrated app compiled (`Finished … in
    16.15s`) and booted — `store opened … schema_version=1`, `WinRT OCR ready`, `inference attached;
    sidecar ready (lazy spawn)`, `embedding model loaded; attaching to kernel`. So under the Tauri
    shell every subsystem comes up Ready (the live data the StatusRail consumes), vs. the
    "Kernel offline" error state the browser correctly shows without a runtime. No panic, no error.
    (The native WebView2 window can't be screenshotted with the available tools; the live StatusRail
    is evidenced by the backend boot log + the rail's verified render paths.)

## 2026-06-22 — P5 (M1+M2) PR #11 review fixes (`feat/p5-ui-foundation` branch)
- **Change:** Addressed the actionable findings from the PR #11 reviews (Claude code-review +
  gemini-code-assist; Codex posted no review):
  - **Bug (CommandPalette active-index latch).** `ui/src/components/shell/CommandPalette.tsx` — the
    clamp effect now resets to `0` when the filtered list is empty
    (`filtered.length > 0 ? Math.min(Math.max(0, a), len-1) : 0`). The old `Math.min(a, max(0,…))`
    latched `active` at `-1` after an ArrowDown over a zero-match query, so Enter then ran
    `filtered[-1]` (undefined) even once matches returned.
  - **Ctrl+K label.** `NavRail.tsx` — the shortcut hint reads `Ctrl+K`, not `⌘K`. Chose the literal
    Windows label over Gemini's `userAgent` platform-detection: CLAUDE.md forbids cross-platform
    abstractions for a Windows-only app, and a Mac branch is dead code here.
  - **Palette input hardening.** Added `type="text"` + `autoComplete/autoCorrect/autoCapitalize="off"`
    + `spellCheck={false}` so OS/browser overlays don't cover the command list.
  - **RouteError ErrorResponse.** `routes/RouteError.tsx` — extract the message via the official
    `isRouteErrorResponse` guard (status + statusText/data) before the `Error`/string/`{message}`
    fallbacks, so a thrown `Response`/404 shows real detail instead of the generic line.
  - **useAsk concurrency guard.** `lib/ipc/useAsk.ts` — `ask` returns early while
    `phase === "streaming"` (deps now `[state.phase]`). The `answer_delta` stream has no per-request
    id, so a second ask mid-stream would fold the first's late deltas into its state. A true
    concurrent/cancellable ask needs a backend request-id — logged as `07` #28.
  - **queryKeys prefixes (minor).** Added `timelinePrefix`/`insightsPrefix`; `useLiveEvents` uses
    them instead of raw `["timeline"]`/`["insights"]` arrays, keeping the invalidation prefix coupled
    to the key registry.
- **Why:** review follow-through (`04 §5/§6`). One real correctness bug (the palette latch) + four
  hardening items before M3 consumes these components.
- **Not changed (reviewer "no action needed" / deferred):** full ARIA combobox semantics for the
  palette (`07` #29 — revisit M3); `frameSrc` module-level cache (correct for a single-process app).
- **Verification:** `npm run build` → `✓ built in 1.49s` (initial JS still ≈ 86 KB gzip); `npm run
  lint` → clean (Rules-of-Hooks gate). **Observed running** — Playwright vs `npm run dev` reproduced
  the exact bug path: opened the palette, typed a no-match query `zzzz`, pressed **ArrowDown** over
  the empty list, replaced the query with `time` (→ "Go to Timeline"), pressed **Enter** → URL
  navigated to **`/timeline`** (the fix recovered `active` to 0; pre-fix it would have stayed `/`).
  The NavRail hint renders **"Ctrl+K"**.

## 2026-06-22 — P5 (M1+M2) PR #11 Codex follow-up (`feat/p5-ui-foundation` branch)
- **Change:** `ui/src/lib/ipc/useAsk.ts` — replaced the React-state concurrency guard with a
  synchronous **`useRef` in-flight flag** (set before `dispatch`/`cmd.ask`, released by an effect
  when `phase` reaches `done`/`error`, and on `reset`). Tidied the now-stale `⌘K` comment in
  `NavRail.tsx` → `Ctrl+K`.
- **Why:** Codex review of `f03fa58` (P2): the prior `if (state.phase === "streaming") return` guard
  reads React state, which lags within an event tick — two `ask()` calls in the *same* synchronous
  tick both observe the pre-`start` `phase` and both reach `cmd.ask`, so the single global
  `answer_delta` channel (no request id) could interleave two streams. A ref reflects the in-flight
  state immediately, blocking the second call synchronously. `ask` deps drop to `[]` (stable
  identity). The backend per-request-id needed for *true* concurrency/cancellation remains `07` #28.
- **Verification:** `npm run build` → `✓ built in 1.09s` (initial JS unchanged ≈ 86 KB gzip);
  `npm run lint` → clean (Rules-of-Hooks gate; ref/dispatch are stable so `[]` deps are exhaustive).
  No UI consumer yet (Recall is M3), so this is pre-emptive hardening verified by build/lint +
  reasoning, not a behavioral repro. Claude's re-review of `f03fa58` found "No issues … PR is clean".

## 2026-06-22 — P5 (M1+M2) PR #11 Codex follow-up #2 (`feat/p5-ui-foundation` branch)
- **Change:** `ui/src/lib/ipc/useLiveEvents.ts` — the `job_progress` handler now debounce-invalidates
  the data families a completed job changes (`framePrefix` / `searchPrefix` / `insightsPrefix`,
  `ENRICH_DEBOUNCE_MS = 1000`) in addition to the immediate `jobStats` counter update.
  `ui/src/lib/ipc/queryKeys.ts` — added `searchPrefix` / `framePrefix` (alongside the existing
  `timelinePrefix` / `insightsPrefix`).
- **Why:** Codex review of `db61a5f` (P2): a completed `vision_tag`/`embed_*` job's only UI signal
  is `job_progress` (counts only — no kind/frame id). With `refetchOnWindowFocus` off and no polling,
  an already-cached Moment (`get_frame`), Recall (`search`), or Insights query would never refetch,
  so new vision tags / indexed embeddings stayed invisible until a manual reload. Debounced family
  invalidation fixes it client-side; `invalidateQueries` refetches only *observed* queries and marks
  the rest stale, so an idle enrichment backlog drain stays cheap. Timeline is excluded (capture
  density, unaffected by enrichment — `capture_tick` already covers new frames). The surgical fix
  (a richer `job_completed { kind, frame_id }` event) is a backend change — logged as `07` #30.
- **Verification:** `npm run build` → `✓ built in 1.08s` (initial JS unchanged ≈ 86 KB gzip);
  `npm run lint` → clean. No UI consumer of these queries until M3/M4 (the screens are scaffolds), so
  this is foundation-plumbing hardening verified by build/lint + reasoning; the behavioral proof
  (Moment refetches after a vision tag) lands with the screens.

## 2026-06-22 — P5 (M3+M4) Recall · Deck · Timeline · Moment (`feat/p5-screens` branch, PR #12)
- **Change — backend slice (precursor commit):** new `crates/store/src/frames.rs` with inherent
  `SqliteStore::frames_in_range(start,end,limit) -> Vec<FrameMeta>` (newest-first over `[start,end)`)
  and `nearest_frame(at) -> Option<FrameMeta>` (closest frame either side of `at`, at-or-after wins an
  exact tie; `i128` distance avoids overflow). New ts-rs `FrameMeta { frame_id, captured_at,
  image_path, app_hint }` (`crates/traits/src/ipc.rs`, added to the `no_bigint_in_ipc_types` guard).
  Commands `get_frames` / `get_nearest_frame` (`src-tauri/src/lib.rs`, registered). Two `:memory:`
  tests in `crates/store/tests/store.rs`.
- **Change — frontend:** typed wrappers `getFrames`/`getNearestFrame`; `useFrames` query + `frames`
  key family (distinct from singular `frame`); `useLiveEvents` `capture_tick` also invalidates the
  frames list. New `lib/time.ts` (human-relative + absolute timestamps) and `lib/timeRanges.ts`
  (day-snapped windows). New `components/domain/`: `FrameImage`, `FrameTile`, `SearchResult`,
  `AnswerStream`, `JobQueueMeter`, `ScanlineTimeline` (+ shared `timelineDraw.ts`), `TimelineMinimap`,
  `MomentDetail`. Replaced the Deck/Recall/Timeline/Moment route scaffolds with full implementations,
  each covering loading/empty/error/partial/populated. New token `--glow-scan` + Tailwind
  `shadow-scan` (scan-head halo). New icons (image, chevron-left, arrow-left, sparkle, tag).
- **Why:** M3+M4 of the P5 plan — the "Command Deck" data screens. The frame-browsing slice was
  required because the merged M0 backend exposed only density buckets, which can't drive the Timeline
  hover thumbnails / Enter-opens-a-Moment / Deck recents; the user approved adding it (`07` #31).
  Enter resolves to a concrete frame via `get_nearest_frame` (exact, server-side), not a sampled
  thumbnail. Search results are virtualized (`@tanstack/react-virtual`, `UI_REFERENCE §8`); the Ask
  answer streams via the `useAsk` reducer with collapsible thinking + citation tiles; the
  ScanlineTimeline is a real `role="slider"` (keyboard scrub + pointer drag), reduced-motion-gated.
- **Verification:** `cargo fmt --check` clean · `cargo build --workspace` 22.05s · `cargo test
  --workspace` all pass (store **35**, incl. 2 new frames tests) · `cargo clippy --workspace
  --all-targets -- -D warnings` clean · bindings regenerate clean (only the new `FrameMeta.ts`) ·
  `npm run typecheck` clean · `npm run lint` clean (Rules-of-Hooks gate) · `npm run build` ✓ 2.02s,
  **initial JS ≈ 87 KB gzip** (react-markdown isolated in the lazy Recall chunk). **Observed
  running:** `tauri dev` booted all subsystems Ready (no panic); live WebView captures showed the
  **populated** Deck (real frame thumbnails via the asset protocol, today minimap, top-apps, queue
  meter, live-updating across captures) and Timeline (real density ribbon + scan-head). Degraded
  states (Playwright vs the Vite dev server, no Tauri runtime) showed Deck error/retry, Recall search
  + ask invites, Timeline error + presets, and the ⌘K palette. (Native-window populated screenshots
  are a manual aid — Playwright can't attach to the Tauri WebView.)

## 2026-06-22 — P5 (M5): Settings & Insights (`feat/p5-settings-insights`, off `main` @ 2ecf038)
- **Change — frontend only (0 Rust files touched, no IPC type added → bindings unchanged):**
  Replaced the `Settings` and `Insights` route scaffolds with full implementations. New domain
  controls in `ui/src/components/domain/` (exported from `index.ts`): `ModelTierPicker` (segmented
  Default/Quality/Beta per lane), `ScheduleControl` (deferred-vision timer/idle opt-ins, minute
  thresholds, on-demand explainer), `RetentionControl` (honest not-enforced label), and the inline
  charts `CapturesTrend` (time-positioned density bars) + `InsightsBars` (ranked horizontal bars) —
  no chart library, to protect the bundle. Reused the M0 commands/queries/mutations as-is
  (`get_settings`/`set_settings`/`set_model_tier`/`get_insights`, `useSettings`/`useInsights`,
  `useSetSettings`/`useSetModelTier`).
- **Settings** edits one draft of the typed `Settings` binding across six panels; Save is optimistic
  + reconcile, Reset reverts to the saved snapshot, a dirty chip gates the actions. Tiers hot-apply
  the instant they're picked (set_model_tier → live provider) with optimistic revert on error; every
  field is labelled with *when* it applies. The two list fields (`capture_monitors`,
  `privacy_excluded_apps`) use a raw-text buffer (parsed array drives dirty/save) so typing isn't
  fought. All hooks precede the loading/error early returns.
- **Insights** renders only real `get_insights` aggregates with Today/7/30 presets: total + tagged
  counts, captures-over-time, top apps, activity breakdown — honest-empty ("not enough history yet")
  and partial-labelled ("tagged only" + the count the breakdown is based on) when tagging is behind.
- **Why:** final milestone of the P5 plan (`02 §5`), completing the Command Deck. Settings/Insights
  ship as their own lazily-loaded route chunks. Spec-silent decisions logged in `07` #35 (apply-
  timing mirrors the backend's honest policy — no live `reconfigure()`), #36 (no monitor-enumeration
  command → comma-separated index field), #37 (Insights' fixed 48-bucket grain, rendered by true
  time-offset so the chart isn't coupled to it).
- **Verification:** `cargo fmt --all -- --check` exit 0 · `cargo clippy --workspace --all-targets --
  -D warnings` exit 0 · `cargo test --workspace` exit 0 (no Rust changed — suite identical to merged
  `main` 2ecf038) · `git diff --stat -- ui/src/bindings` empty · `npm --prefix ui run lint` exit 0
  (Rules-of-Hooks gate) · `npm --prefix ui run build` ✓ 1.63s (`tsc --noEmit` + vite), Settings 3.56
  gz / Insights 1.99 gz own lazy chunks, initial JS ≈ 87.7 KB gzip. **Observed running:** Playwright
  vs the Vite dev server with `window.__TAURI_INTERNALS__` mocked to the exact binding shapes; all
  five states captured for both screens with 0 console errors (Settings populated incl. the tier
  pickers/thinking toggle/schedule control, loading skeleton, load-error+retry; Insights populated
  incl. the density chart + ranked bars, empty, partial, compute-error+retry, loading skeleton).
  Native-window screenshots remain impossible (Playwright can't attach to the Tauri WebView).

## 2026-06-22 — Vision-tagging quality fix (`feat/p5-m5-insights-settings-vision`, off `main` @ 39d5da8)
- **Context:** P5-M5 (Settings + Insights) was already merged (#13); the genuinely-remaining work was
  the vision-output honesty gap logged in `07` #19/#20. The gated GPU smoke had recorded a fabricated
  `confidence: 0.0` (echoed from a `"confidence": 0.0` placeholder in the prompt) and a free-form
  `activity_type` — both violate CLAUDE.md's never-fabricate-a-value rule.
- **Change — `crates/inference` only (no `traits`/schema/IPC change → ts-rs bindings unchanged):**
  - `client.rs`: `ChatRequest` gains an optional `response_format: Option<serde_json::Value>`
    (`skip_serializing_if`), threaded through `complete(messages, max_tokens, response_format)`; the
    streaming `answer` path passes `None`. Two unit tests assert the field serializes when set and is
    omitted when `None`.
  - `vision.rs`: `VISION_PROMPT` no longer shows a numeric `confidence` (the field is described, not
    demonstrated) and states the `activity_type` enum. The vision call now passes an OpenAI
    `response_format` JSON-schema (`vision_response_format()` — enum `activity_type`, numeric
    `confidence`) which `llama-server` enforces as a sampling grammar. `parse_vision` normalises
    defensively regardless of model compliance: `normalize_confidence` trusts only finite `(0.0, 1.0]`
    (else `CONFIDENCE_UNKNOWN = -1.0` — mirrors OCR), `normalize_activity` maps to the closed
    `ACTIVITY_TYPES` set (case/space-insensitive) or `None` (incl. the model's own "unknown"). Six new
    unit tests cover zero/missing/out-of-range confidence and off-enum/normalised activity.
  - `tests/smoke.rs`: the gated `real_vision_tags_an_image` now asserts confidence is `-1.0` or a real
    `(0.0, 1.0]` (never `0.0`) and `activity_type` is `None` or in the allowed set.
- **Why this approach:** the response-format grammar makes the model emit a well-shaped object, but
  the defensive parse is the guarantee — even a model that ignores the schema can't slip a free-form
  label or a fabricated `0.0` past `parse_vision`. No domain type changed, so no schema migration and
  no binding drift.
- **Verification (verbatim):** `cargo fmt --all -- --check` exit 0 · `cargo clippy --workspace
  --all-targets -- -D warnings` exit 0 · `cargo test --workspace` exit 0 (inference lib **33** incl.
  the 8 new tests; store 36; traits 32; all crates green) · `git diff --exit-code -- ui/src/bindings`
  exit 0 (no drift) · `npm --prefix ui run typecheck` exit 0 · `npm --prefix ui run lint` exit 0 ·
  `npm --prefix ui run build` ✓ (Insights/Settings chunks build; initial JS ≈ 87 KB gz).
  **Observed running:** the **real-GPU** gated smoke
  `cargo test -p inference --test smoke real_vision_tags_an_image -- --ignored --nocapture` **passed**
  on the RTX 5060 Ti (cached Qwen3-VL-4B-Instruct Q4_K_M + mmproj): `VISION: A split-screen view …
  | activity=Some("browsing") | conf=0.95` — a real enum activity and a genuine confidence, replacing
  the old `unknown`/`0.0`. `npm run tauri dev` booted the full app (store v1, WinRT OCR ready, vision
  scheduler started, inference attached/lazy, fastembed loaded, embedding workers started) with no
  panic; clean shutdown left **no orphaned `llama-server.exe`**.
- **Docs:** `07` #19/#20 marked resolved; `README.md` phase table refreshed (P0–P5 complete, packaging
  the only open DoD item) and its run command corrected to `npm run tauri dev` (the Tauri CLI ships as
  the npm dev-dependency here; `cargo tauri` needs a separate `cargo install tauri-cli`); `CHANGELOG.md`
  updated.

## 2026-06-22 — PR #14 review follow-up (`feat/p5-m5-insights-settings-vision`)
- **Context:** automated reviewers on PR #14 raised three correct points against the vision fix above
  (one from gemini-code-assist, two P2s from chatgpt-codex). All three addressed.
- **(1) Forced activity label on low-signal frames (codex P2 — the consequential one).** The grammar
  made `activity_type` a *required* enum of the eight labels, so the model could never decline and the
  off-enum→`None` safeguard in `parse_vision` was effectively dead code (the grammar forbade off-enum
  values). A blank desktop / lock screen / synthetic frame was therefore tagged with an arbitrary
  label and mirrored into `frames.activity_type`, skewing Insights. **Fix:** `vision_response_format()`
  now makes `activity_type` nullable (`"type":["string","null"]`, `null` appended to the `enum`) and
  drops it from `required`; `VISION_PROMPT` instructs the model to answer `null` when nothing clearly
  fits. **Not cosmetic** — the gated smoke proves it: the synthetic two-tone frame that the *forced*
  schema confidently mislabelled `browsing` @ `0.95` (see the run recorded above) now correctly returns
  `activity=None | conf=-1` — the model declines. New unit tests `explicit_null_activity_type_becomes_none`
  and `response_format_allows_null_activity_and_drops_it_from_required`.
- **(2) `app_hint` "null" filter was case-sensitive (gemini).** `s != "null"` let `"Null"`/`"NULL"`
  through as a literal app name. **Fix:** extracted `normalize_app_hint`, which trims and compares with
  `eq_ignore_ascii_case("null")`, returning the trimmed value. New tests
  `null_app_hint_string_is_dropped_case_insensitively` (covers `null`/`NULL`/`Null`/`  null  `) and
  `app_hint_is_trimmed_and_kept_when_real`.
- **(3) `-1.0` sentinel rendered as `-100%` (codex P2).** `MomentDetail.tsx` computed
  `Math.round(vision.confidence * 100)%` unconditionally, so the new "unknown" sentinel showed users
  `-100%`. **Fix:** the Vision panel now shows a neutral `n/a` chip when `confidence < 0` and the accent
  percentage only for a real score. UI-only; `VisionAnalysis` unchanged, so no binding drift.
- **Verification (verbatim):** `cargo fmt --all -- --check` exit 0 · `cargo clippy -p inference
  --all-targets -- -D warnings` exit 0 · `cargo test -p inference --lib` → **36 passed; 0 failed**
  (was 33; +3 net new) · `git diff --exit-code -- ui/src/bindings` exit 0 (no drift) ·
  `npm run typecheck` exit 0 · `npm run lint` exit 0 · `npm run build` ✓. **Observed running:** the
  real-GPU gated smoke `real_vision_tags_an_image` **passed** on the RTX 5060 Ti — `VISION: The screen
  is divided into two vertical sections … | activity=None | conf=-1` (honest decline on the low-signal
  synthetic frame; the test asserts confidence ∈ {-1.0} ∪ (0,1] and activity ∈ {none} ∪ the set).

## 2026-06-23 — P0/P1 review findings fix (`codex/fix-p0-p1-review-findings`)
- **Context:** a scrupulous review of Phase 0/Phase 1 found two P1 data-spine hardening issues: job
  finalization was keyed by id only (so pending/done/dead jobs could be rewritten), and a DB with a
  future `schema_version` opened as if it were compatible.
- **Change:** `complete_job` and `fail_job` now finalize only `state='running'` rows and return
  `"missing or not running"` errors otherwise. `bootstrap_and_migrate` now rejects
  `schema_version > LATEST_SCHEMA_VERSION` before applying migrations. Added regression tests:
  `complete_job_requires_running_state`, `fail_job_requires_running_state`, and
  `open_path_rejects_future_schema_version`.
- **Why:** this preserves the `03 §5` queue state machine (`claim_jobs` → run → finalize) and the
  `03 §12` forward-only migration guarantee. No IPC, `ts-rs`, schema, or trait interface changed.
- **Verification (verbatim):**
  `cargo test -p store --test store complete_job_requires_running_state -- --exact` →
  `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out`;
  `cargo test -p store --test store fail_job_requires_running_state -- --exact` →
  `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out`;
  `cargo test -p store --test store open_path_rejects_future_schema_version -- --exact` →
  `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out`;
  `cargo fmt --all -- --check` exit 0;
  `cargo clippy --workspace --all-targets -- -D warnings` exit 0;
  `cargo test --workspace` exit 0 (store integration tests now **39 passed**; traits **32 passed**);
  `npm --prefix ui run build` exit 0 (`✓ built in 2.21s`);
  `npm --prefix ui run lint` exit 0;
  `git diff --exit-code -- ui/src/bindings` exit 0.

## 2026-06-23 — PR #15 review follow-up (`codex/fix-p0-p1-review-findings`)
- **Context:** PR #15 received two unresolved Gemini review threads after the initial P0/P1 hardening
  commit. Both were actionable and compatible with the existing store design.
- **(1) Schema-version guard source of truth.** `bootstrap_and_migrate` now derives `max_version` from
  the maximum version in `schema::MIGRATIONS` and rejects `schema_version > max_version`. A
  `debug_assert_eq!` catches any drift between `LATEST_SCHEMA_VERSION` and the compiled migrations in
  development/test builds, while the public constant stays available for readiness/status and tests.
- **(2) Temp DB cleanup in regression test.** `open_path_rejects_future_schema_version` now uses
  `tempfile::tempdir()` instead of manually composing and removing a directory under the OS temp path.
  `tempfile` is now a workspace dependency for Rust tests, and the existing kernel dev-dependency was
  switched to the centralized version to avoid drift.
- **Why:** this tightens the future-schema rejection around the migration set the binary actually
  contains and makes the test robust to panics without changing IPC, `ts-rs`, schema SQL, or trait
  interfaces.

## 2026-06-23 — PR #15 Codex review follow-up (`codex/fix-p0-p1-review-findings`)
- **Context:** a new Codex review thread on `crates/store/src/jobs.rs` pointed out a production
  interaction between the stricter `fail_job(... state='running')` guard and the kernel's periodic
  stale-running sweep: a long but live provider call could be requeued to `pending` after the
  visibility timeout, then fail later and lose retry/dead-letter accounting because it no longer owned
  a `running` row.
- **Change:** `kernel::worker_pool` now tracks this process's active job ids in a small
  `Arc<Mutex<HashSet<i64>>>`. Each worker holds an RAII guard while `process_job` awaits provider work;
  the periodic sweep checks that set and skips requeueing while any current-pool job is in flight.
  Startup recovery still calls `reset_stale_running_jobs(0)` before workers are spawned, so prior-run
  leftovers are still recovered.
- **Why not loosen `fail_job`:** the store has no durable claim token. Allowing `fail_job` to mutate a
  `pending` row after stale requeue would reintroduce stale-worker finalization by id alone, and an old
  worker could also race with a newly claimed `running` row. The kernel-local guard preserves failure
  accounting for live long calls without changing IPC, `ts-rs`, schema SQL, or trait interfaces.
- **Verification (verbatim):** `cargo test -p kernel active_job_guard_tracks_in_flight_job_until_drop`
  → `test worker_pool::tests::active_job_guard_tracks_in_flight_job_until_drop ... ok`;
  `cargo fmt --all -- --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0;
  `cargo test --workspace` exit 0 (new kernel unit test, kernel enrichment **9 passed**, store
  integration **39 passed**, traits binding tests **32 passed**); `npm --prefix ui run build` exit 0
  (`✓ built in 1.48s`); `npm --prefix ui run lint` exit 0; `git diff --exit-code -- ui/src/bindings`
  exit 0.

## 2026-06-23 — P2 capture hardening (`codex/fix-p2-capture-hardening`)
- **Context:** a scrupulous Phase 2 review found three hardening gaps in the capture happy path:
  capture readiness could remain `Ready` after an unexpected source shutdown; the OCR fallback
  contradicted `07` by silently returning empty OCR rows; and backend settings trusted persisted/direct
  numeric values that the UI alone had clamped.
- **Change:** `run_capture_loop` now reports `CaptureLoopExit::{StopRequested, SourceShutdown}`.
  `Kernel::start_capture` wraps the loop with a generation-checked supervisor that clears the live
  handle and emits `capture = Error` when the source shuts down without a user Stop. The kernel can now
  be built with an OCR-unavailable reason; `start_capture` fails before WGC opens and marks
  `capture = Unavailable`, while the defensive `UnavailableOcr` returns an error if ever called.
  `kernel::settings::sanitize_settings` clamps numeric settings on both load and save, matching the
  Settings UI bounds. Added regression tests for all three behaviors.
- **Why:** P2 is the privacy-sensitive, always-on path. It must report its real lifecycle, avoid
  half-searchable empty-OCR data, and survive hand-edited or direct IPC settings without wedging the
  diff gate. No schema, IPC, `ts-rs`, or trait signature changes.
- **Docs:** `CHANGELOG.md`, `docs/ARCHITECTURE.md`, `specs/05_BUILD_REVIEW.md`, and the P2 OCR
  fallback note in `specs/07_KNOWN_GAPS.md` were updated.
- **Verification (verbatim status):** `cargo fmt --all -- --check` exit 0; `cargo clippy --workspace
  --all-targets -- -D warnings` exit 0; `cargo test -p kernel --test pipeline` → **5 passed**;
  `cargo test -p capture` → **9 passed, 1 ignored**; `cargo test -p ocr` → **1 ignored**;
  `cargo test --workspace` exit 0; `npm --prefix ui run build` → `✓ built in 1.52s`;
  `npm --prefix ui run lint` exit 0; `git diff --exit-code -- ui/src/bindings` exit 0; hardware-gated
  P2 checks all passed: WGC smoke **1 passed**, WinRT OCR smoke **1 passed**, real e2e capture **1
  passed**.

## 2026-06-23 — PR #16 review comment follow-up (`codex/fix-p2-capture-hardening`)
- **Context:** Gemini identified a remaining restart race in the unexpected-source-shutdown supervisor:
  after the loop cleared the capture handle, it dropped the capture mutex before publishing
  `capture = Error`, so a fast restart could publish a new `Ready` status and then be overwritten by
  the old loop's error.
- **Change:** the supervisor now holds the capture mutex until after it clears the stale handle and
  calls `set_capture_readiness(Error, ...)`. This keeps the generation-id guard and existing lock order
  intact while closing the gap.
- **Verification (verbatim status):** `cargo fmt --all -- --check` exit 0; `cargo clippy --workspace
  --all-targets -- -D warnings` exit 0; `cargo test -p kernel --test pipeline` → **5 passed**;
  `cargo test --workspace` exit 0, including kernel enrichment **9 passed**, kernel pipeline
  **5 passed**, kernel settings **4 passed**, store integration **39 passed**, and traits bindings
  **32 passed**.

## 2026-06-23 — P3 deferred enrichment hardening (`codex/review-p3-deferred-enrichment`)
- **Context:** a comprehensive P3 review found that `vision_tag` jobs could stall when embeddings were
  disabled/unavailable, because `init_embeddings` returns early and `Kernel::start_workers` previously
  required an embedder before any pool could start. The review also found that direct IPC could pass an
  unbounded `SearchQuery.limit`, and that worker/event docs had drifted from the P4/P5 implementation.
- **Failing-first evidence:** `cargo test -p kernel --test enrichment
  vision_jobs_drain_when_embeddings_disabled` initially failed with `worker pool did not drain the
  vision_tag job without an embedder`. `cargo test -p store --test store
  hybrid_search_clamps_excessive_limit` initially failed with `left: 150 right: 100`.
- **Change:** `kernel::worker_pool` now has shared `EmbedderSlot` and `VisionSlot` provider slots.
  Workers build claim kinds dynamically each loop: `EmbedText` / `EmbedImage` only when an embedder is
  attached and the matching setting is enabled; `VisionTag` when the vision provider is attached.
  `attach_embedder` and `attach_inference` both call the same idempotent `start_workers`, so the first
  available provider starts the pool and the other can attach later without restart. The first pool
  start still runs startup stale-running recovery before spawning workers.
- **Search hardening:** `store::search` now normalizes `SearchQuery.limit` to `1..=100` and caps the
  candidate pool at 500 (`MAX_SEARCH_LIMIT * 5`), with zero-limit normalization still returning one
  result. This is deliberately backend-local: no IPC, schema, `ts-rs`, or trait signature changes.
- **Docs:** `CHANGELOG.md`, `docs/ARCHITECTURE.md`, `specs/05_BUILD_REVIEW.md`, `07_KNOWN_GAPS.md`,
  this file, `src-tauri` module docs, and the `useLiveEvents` `job_progress` comment now match the
  current implementation. Known gap #8 remains open by design.
- **Verification (verbatim status):** `cargo fmt --all -- --check` exit 0; `cargo clippy --workspace
  --all-targets -- -D warnings` exit 0; `cargo test -p kernel --test enrichment` → **10 passed**;
  `cargo test -p store --test store` → **40 passed**; `cargo test --workspace` exit 0, including
  kernel enrichment **10 passed**, store integration **40 passed**, traits bindings **32 passed**;
  `npm --prefix ui run build` → `✓ built in 2.05s`; `npm --prefix ui run lint` exit 0;
  `git diff --exit-code -- ui/src/bindings` exit 0.

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

## 2026-06-21 — CI fix: Claude review couldn't post (read-only token)
- **Change:** Granted `pull-requests: write` + `issues: write` to `claude-code-review.yml`
  (and `claude.yml`); added `concurrency` (cancel-in-progress) to the review workflow; bumped
  `actions/checkout`→v5 and `actions/setup-node`→v5 across all workflows.
- **Why:** The first PR #2 review *ran* (10 turns, 17m52s, $5.92) but posted nothing — the job's
  `GITHUB_TOKEN` had only read scopes, so ~25 post attempts were denied
  (`permission_denials_count: 25`, "No buffered inline comments"). The denial retries also
  inflated the runtime. `@claude` in `claude.yml` had the same latent read-only bug.
- **Verification:** `python -c yaml.safe_load` on all three workflows (OK); re-run on next push.

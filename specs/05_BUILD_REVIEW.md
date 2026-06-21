# 05 — Build Review

> **Populated during the build**, after each meaningful pass (`04 §7`). Record what actually
> happened — honestly. Empty until P0 begins.

For each build pass, append an entry:

## Pass <n> — <date> — <phase, e.g. P0 Scaffold>
- **Implemented:** what now works (with the verbatim verification output that proves it).
- **Skipped / deferred:** what was intentionally not done, and why.
- **Hallucinated / corrected:** anything the agent assumed that turned out wrong.
- **Broke / regressed:** what stopped working, and the fix.
- **Still risky:** areas that compile/pass but warrant scrutiny.

---

## Pass 1 — 2026-06-21 — P0 Scaffold

**Branch:** `p0-scaffold`. Toolchain observed: rustc/cargo 1.96.0, Node v26.3.0, npm 11.16.0,
`@tauri-apps/cli` 2.11.3 (via npm), `tauri` crate 2.11.3, `ts-rs` 10.1.0.

### Implemented (with verbatim verification)
- **Cargo workspace** (`Cargo.toml`, resolver 2) with members `src-tauri`, `crates/{traits,
  kernel, store, capture, ocr, embeddings, inference, doctor}`. Dependency rule enforced: module
  crates depend only on `traits` (`03 §2`).
- **`traits` crate** — all six contracts from `03 §3` (`CaptureSource`, `OcrProvider`,
  `EmbeddingProvider`, `VisionProvider`, `AnswerProvider`, `Store`) + domain/jobs/ipc types. IPC
  types derive `ts_rs::TS` and export to `ui/src/bindings/` via `cargo test`.
- **Skeleton module crates** — honestly-empty libs (doc comment naming the phase that fills them;
  re-export of the contract they will implement). No fake impls.
- **`src-tauri` shell** — Tauri 2 app, `get_readiness` + `ping` typed commands, capabilities,
  icons generated from `assets/icon-source.png`, console tracing.
- **UI** — React 18 + TS + Vite skeleton; P0 screen exercises typed IPC; ESLint flat config with
  the Rules-of-Hooks gate at error level.
- **CI** — `.github/workflows/ci.yml` (windows-latest): UI install/lint/build → fmt, clippy
  `-D warnings`, build, test, ts-rs binding-drift guard, doctor.
- **`doctor`** — environment smoke-check for WebView2 / Vulkan / llama-server.

Verbatim verification (run 2026-06-21):
```
$ cargo fmt --all -- --check          # exit 0 (no diff)
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.73s   # exit 0
$ cargo test --workspace
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   # traits (27 export + 1 bigint guard); other crates 0 tests
$ cargo run -p doctor
[ OK ]  WebView2        Evergreen Runtime v149.0.4022.80
[ OK ]  Vulkan          vulkan-1.dll loadable (GPU acceleration available)
[WARN]  llama-server    not on PATH — bundled/downloaded for the sidecar in P4
$ cd ui && npm run build
✓ built in 380ms   (tsc --noEmit clean; dist/index.html + assets emitted)
$ cd ui && npm run lint               # exit 0 (no findings)
$ git diff --exit-code -- ui/src/bindings   # exit 0 (committed bindings current)
```
- **Observed running:** `npx tauri dev` launched the WebView2 window (title "ScreenSearch",
  app icon shown). The P0 screen rendered **"Kernel says: pong"** (the `ping` command) and the
  readiness list capture/db/embed_model/sidecar = `Unknown` (the `get_readiness` command),
  confirming the Tauri bridge + generated `ts-rs` bindings work end-to-end. Captured via
  `PrintWindow`; process verified by `MainWindowTitle = "ScreenSearch"`.

### Skipped / deferred (intentional, by phase)
- `Store`/`JobQueue`/schema → P1. WGC capture + WinRT OCR → P2. fastembed → P3. Sidecar
  Job-Object lifecycle + real inference → P4. Command Deck UI + packaging → P5.
- `03 §9` daily-rotating **file** logging — console only in P0 (file sink needs the app data dir;
  wired in P1).
- `03 §11` **tauri build artifact** job (Inno Setup installer + portable ZIP) — P5 packaging.
- `tauri.conf.json` `bundle.targets: "all"` is a placeholder; P5 switches to Inno + ZIP (`00 §G`).

### Hallucinated / corrected
- Assumed `ts-rs` `export_to` is relative to the crate manifest dir (per its docs); it actually
  anchored at the **source-file** dir here, so `../../ui/...` landed in `crates/ui/...`. Corrected
  to `../../../ui/src/bindings/`.
- Assumed `TS_RS_LARGE_INT=number` (env / `.cargo/config.toml`) would map `i64/u64` to TS
  `number`; ts-rs 10.1's macro ignored it. Corrected to per-field `#[ts(type = "number")]`
  (Tauri's JSON wire delivers 64-bit ints as JS `number`), guarded by a regression test.

### Broke / regressed
- None. (Transient: the `tauri dev` background task reports exit 127 because I terminated its
  process tree after observing the window — expected, not a failure of the app.)

### Still risky / to watch
- `csp: null` in `tauri.conf.json` (dev convenience) — must be hardened in P5 (`07`).
- Local npm `allow-scripts` policy blocked esbuild's postinstall; the build still worked (binary
  resolved via the platform optional-dependency). Could matter on locked-down dev machines.
- `forbid(unsafe_code)` is intentionally **absent** on `capture`/`ocr`/`inference` (P2/P4 need
  `unsafe` FFI) and present on the pure crates.

---

## Pass 2 — 2026-06-21 — P1 Data Spine

**Branch:** `p1-data-spine`. Stack added: `rusqlite` 0.40 (bundled, FTS5), `sqlite-vec` 0.1.9,
`blake3` 1, `tracing-appender` 0.2.

### Implemented (with verbatim verification)
- **`store` crate** — full `Store` (`03 §3`) on SQLite (WAL) + sqlite-vec + FTS5:
  - **Schema/migrations** (`schema.rs`): forward-only runner keyed on `schema_version`; v1 = the
    `03 §4` DDL verbatim + the FTS5 external-content sync triggers and the vec0 cleanup triggers
    the spec describes in prose. `vec0` virtual tables created (extension auto-registered once).
  - **Records** (`records.rs`): `insert_frame` / `insert_ocr` (FTS kept in sync by trigger) /
    `insert_vision`, plus `get_frame` assembling `FrameDetail` (frame + OCR + vision + tags).
  - **Embeddings** (`embeddings.rs`): `upsert_text_embedding` / `upsert_image_embedding` with
    atomic metadata-+-`vec0`-shadow writes (insert *and* in-place replace), dim validated == 768;
    cosine-KNN building blocks; `delete_frame` (retention primitive) → FK cascade + triggers purge
    the vec0 shadows (proven by test).
  - **Search** (`search.rs`): `hybrid_search` fuses the FTS5 BM25 arm and the cosine-KNN arm via
    **RRF** (k=60, per-arm pool = max(limit·5, 50)); honors `time_range`; FTS-highlighted snippets
    with a truncated-OCR fallback. Vector arm active only when an `Arc<dyn EmbeddingProvider>` is
    injected.
  - **Jobs** (`jobs.rs`): durable queue — `enqueue_job`, atomic `claim_jobs`
    (`UPDATE … RETURNING`, priority-ordered, `not_before`/kind filtered), `complete_job`,
    `fail_job` (attempts++; retry+backoff or dead-letter at `max_attempts`), `job_stats`.
  - **Settings** (`settings.rs`): key/value upsert + read.
- **Composition root** (`src-tauri`): opens `screensearch.db` at the app-data dir on launch
  (managed state), `get_readiness` reports real `db` Ready/Error, daily-rotating **file log**
  (`03 §9`, deferred-from-P0 sink), `get_job_stats` IPC command.

Verbatim verification (run 2026-06-21):
```
$ cargo fmt --all -- --check                                   # exit 0 (no diff)
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.93s   # exit 0
$ cargo test --workspace
    store    tests\store.rs   test result: ok. 23 passed; 0 failed
    traits   (lib)            test result: ok. 28 passed; 0 failed
    screensearch (lib)        test result: ok.  2 passed; 0 failed       # 53 total, 0 failed
$ cd ui && npm run build                                       # ✓ built in 388ms (tsc clean), exit 0
$ git status --porcelain -- ui/src/bindings                    # empty (no ts-rs drift)
```
- **Observed running:** `cargo run -p screensearch` launched the app, which created
  `%APPDATA%\app.screensearchv2c.desktop\screensearch.db` + `-wal` + `-shm` (WAL mode active on
  the file DB) and `logs\screensearch.log.2026-06-21`. The log contained:
  `INFO store: applied store migration schema_version=1` and
  `INFO screensearch_lib: store opened db=…\screensearch.db` — proving the migration ran and the
  store is wired, with no screen/OCR content logged (privacy, `03 §9`).

### Skipped / deferred (intentional, by phase)
- Live `search` / `ask` / `get_timeline` / `enqueue_vision` IPC commands → P2+ (no data to serve
  until capture exists); only `get_job_stats` wired now as a liveness proof.
- The vector arm of `hybrid_search` runs against an **injected** embedder; in P1 none is wired, so
  the live path is FTS-only. The full FTS+vec+RRF code is implemented and tested with a fake
  embedder; fastembed is injected in P3.
- Stuck-`running` job recovery (lease/visibility timeout) — belongs to the kernel worker (P3),
  see `07` gap #6.

### Hallucinated / corrected
- Assumed `rusqlite` needs an `fts5` feature for FTS5 — it does **not** in 0.40 (the `bundled`
  amalgamation enables FTS5); removed the feature.
- Assumed the latest `sqlite-vec` (0.1.10-alpha.4) would build — its amalgamation references a
  missing `sqlite-vec-diskann.c` and fails `cc`. Pinned to the latest stable **0.1.9**.
- `u64` is not a rusqlite `FromSql` type — read `COUNT(*)` as `i64` and cast.

### Still risky / to watch
- **Single-connection** store (Mutex + `spawn_blocking`): correct and simple (SQLite single-writer)
  but serializes reads too; revisit with a read pool if search latency needs it. The concurrent
  job-claim test passes because the atomic SQL + serialization both guarantee no double-claim — it
  does not exercise true multi-connection WAL contention.
- RRF `k`/pool and the vec-arm time-range **post-filter** (over-fetch `pool`, then filter) are
  reasonable defaults but untuned against a realistic DB; revisit when P3 has real embeddings and
  the `03 §13` "< ~200 ms" target can be measured.

### Post-review fixes (PR #4 — Gemini + `@claude`)
All findings verified against the codebase/spec before applying (none warranted pushback):
- **[correctness]** `open_state`: a failed post-open `schema_version()` probe now → `db = Error` +
  `store = None` (was silently `Ready`). **[correctness]** `complete_job`/`fail_job` now error on a
  zero-row update (unknown id) instead of a silent no-op.
- **[spec]** `insert_vision` now also fills `frames.activity_type` (`03 §4`), in one txn with the
  `vision_analysis` write.
- **[perf]** `hydrate` N+1 → two bulk `IN` queries. **[maint]** `f32_blob`/`dedup_keep_order` made
  `pub(crate)` and reused in `search.rs`.
- **Verification:** +1 job test, updated the vision test; `fmt`/`clippy -D warnings` clean;
  `cargo test --workspace` **54 passed, 0 failed** (store 24, traits 28, screensearch 2).

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

## Audit — 2026-06-26 — 0.2.0 PR3 attention-first filtering

**Branch:** `codex/0.2.0-pr3-audit`. Runtime: `npm run tauri dev` launching
`target/debug/screensearch.exe`. DB policy: existing
`%APPDATA%\app.screensearchv2c.desktop\screensearch.db`, online backup to
`.playwright-mcp/pr3-2026-06-26/screensearch-pr3-before.sqlite`, no reset/backfill/destructive SQL.

### Implemented / audited
- Added the audit artifact `docs/AUDIT_0.2.0_PR3_2026-06-26.md`.
- Verified PR3's storage/retrieval plumbing: raw text is preserved, filtered content/spans/filter
  version are written, embeddings read `content_text`, default search uses content FTS, and
  `include_chrome=true` keeps raw/static recovery available.
- Verified Settings text-filter thresholds and per-app suppression readout load and match grouped
  SQL for the audited corpus.

### Broke / regressed / release blocker
- **Release blocker:** strict PR3 acceptance is not met. Default content search still has content FTS
  hits for static/app chrome terms (`Firefox` 24, `Steam` 24, `Deck` 68, `Recall` 42,
  `GPU Memory` 15) on the baseline DB. A fresh Notepad capture preserved the deliberate foreground
  content, but also indexed `Firefox`, `Deck`, `Recall`, and `COMMAND` in default `content_text`.
  See `docs/AUDIT_0.2.0_PR3_2026-06-26.md`, `06` patch #8, and `07` gap #64.

### Verbatim verification
Raw logs are preserved under `.playwright-mcp/pr3-2026-06-26/29-verify-ui-npm-ci-lint-build.txt`
through `34-verify-bindings-diff.txt`; the audit report includes the command output summary and
the exact evidence paths. All required commands exited 0:
`cd ui && npm ci && npm run lint && npm run build`, `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace`,
`cargo test --workspace`, and `git diff --exit-code -- ui/src/bindings`.

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

---

## Pass 3 — 2026-06-21 — P0/P1 review pass (`review/p0-p1` branch)

A compliance review of the merged P0 + P1 against `04`/`03`, by close reading of the workspace,
the `store` crate, the `traits` contracts, the tests, and `05`–`08`.

### Verdict
- **P0 — complete & compliant.** Workspace layout matches `03 §2` (incl. the `doctor` smoke-check
  crate); dependency rule respected (`src-tauri` is the composition root); CI runs the full gate.
- **P1 — complete & compliant.** `Store` matches `03 §3` verbatim; schema matches `03 §4` (tables,
  `porter` external-content FTS5, `vec0 FLOAT[768] cosine`, indexes, FTS + vec-purge triggers);
  forward-only migrations with `schema_version`; durable queue (atomic `UPDATE … RETURNING` claim,
  retry/backoff, dead-letter); hybrid FTS5+vec+RRF. No correctness bugs found.

Checked by reading, not assumed: the `fail_job` retry boundary (`attempts + 1 < max_attempts`) is
correct — 3 attempts then dead-letter; FTS `MATCH` input is quoted/escaped (no operator injection);
the embedding upsert keeps the `vec0` shadow in lock-step inside one transaction; the `frames`
cascade + `recursive_triggers=ON` purges `vec0` rows (covered by a test). Tests assert real
behavior; `FakeEmbedder` is a legitimate deterministic test double (`03 §10`), not a faked result.

### Findings (all minor — no bugs)
1. The concurrent-claim test proves single-shared-connection correctness, not multi-connection WAL
   contention (already noted here in Pass 2) → caveat now also stated **in the test itself**.
2. `TimeRange` is half-open but didn't spell out which bound is exclusive → doc made explicit
   (`[start, end)`, start inclusive / end exclusive) on the contract + a note at the query site.
3. `03 §13` "< ~200 ms on a realistic DB" is unverified at P1 (tiny `:memory:` fixtures) → promoted
   from Pass 2 prose to a tracked gap (`07` #7, P3).
4. The vec-arm `time_range` post-filter can under-return on tight windows → tracked gap (`07` #8, P3).

### Changes applied (additive only — comments/docs/spec; no behavior change)
- `crates/store/tests/store.rs`, `crates/traits/src/ipc.rs`, `crates/store/src/search.rs` (comments
  + doc-comment); `07` gaps #7/#8; this entry + `08`; `CHANGELOG.md`.

Verbatim verification (run 2026-06-21, after the doc/comment changes):
```
$ cargo fmt --all -- --check                                   # fmt exit code: 0
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.49s   # clippy exit code: 0
$ cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.45s   # build exit code: 0
$ cargo test --workspace
    store    tests\store.rs   test result: ok. 24 passed; 0 failed
    traits   (lib)            test result: ok. 28 passed; 0 failed
    screensearch_lib (lib)    test result: ok.  2 passed; 0 failed        # 54 total, 0 failed
$ cd ui && npm run build                                       # ✓ built in 339ms (tsc --noEmit clean), exit 0
$ git status --porcelain -- ui/src/bindings
 M ui/src/bindings/TimeRange.ts   # doc-comment regenerated by ts-rs; the TS *type* is unchanged
                                  # (`{ start: number, end: number }`). Committed with the source,
                                  # so the CI regenerate-then-diff guard is clean.
```
Behavior unchanged vs. Pass 2 (54 green, same counts) — confirming these were doc/comment-only
edits, and that the merged-`main` state actually holds the green the docs claim.

---

## Pass 4 — 2026-06-21 — P2 Capture happy path (`p2-capture` branch)

**Stack added:** `windows` 0.62 (WGC + D3D11 + WinRT Media.Ocr + Win32 foreground/lock), `image`
(JPEG encode), `tempfile` (dev). Four spec-silent decisions resolved with the user up front
(`07` #9–#13).

### Implemented (with verbatim verification)
- **`capture` crate** — `WgcCapture: CaptureSource` on raw `windows-rs`: per-monitor D3D11 device +
  free-threaded WGC frame pool on a dedicated COM(MTA) thread, BGRA→RGBA staging readback; a pure
  **diff gate** (`32×32` luma mean-abs-diff vs `capture.diff_threshold`, `blake3` content hash); a
  **privacy gate** (`OpenInputDesktop` lock probe + foreground app/title match for
  `privacy.excluded_apps`, also filling `app_hint`/`window_title`). `next_frame` paces by
  `capture.interval_ms` and drains changed frames one at a time.
- **`ocr` crate** — `WinRtOcr: OcrProvider` on a dedicated **STA** worker thread
  (`CoInitializeEx`), one reusable `OcrEngine`, RGBA→`SoftwareBitmap(Bgra8)`,
  `RecognizeAsync().join()`; `mean_confidence = -1.0` sentinel (engine gives none, `06` #2).
- **`kernel` crate** — the capture loop (CaptureSource→OcrProvider→JPEG encode→`insert_frame`/
  `insert_ocr`→enqueue `embed_text`→emit `capture_tick`), a typed `broadcast` event bus, a
  typed-`Settings` loader over the key/value table, and `Kernel::start_capture`/`stop_capture`
  (idempotent; off until started). **`vision_tag` is never auto-enqueued** (`13.3`).
- **`src-tauri`** — wires the store + OCR worker + capture factory + kernel; adds `capture_control`
  + `get_frame` commands; `get_readiness` is now live; forwards `capture_tick`/`readiness_changed`
  to the WebView2 UI.
- **`ui`** — a minimal **live timeline**: Start/Stop, a live readiness strip, and a row per stored
  frame from `capture_tick` (all view states; typed `ts-rs` bindings; lint-clean).

Verbatim verification (run 2026-06-21):
```
$ cargo fmt --all -- --check                                   # exit 0
$ cargo clippy --workspace --all-targets -- -D warnings        # exit 0
$ cargo build --workspace                                      # exit 0
$ cargo test --workspace
    capture  (lib)            test result: ok.  9 passed; 0 failed   # diff + privacy matchers
    kernel   tests\pipeline    test result: ok.  3 passed; 0 failed   # fake capture→ocr→store→job
    store    tests\store.rs   test result: ok. 24 passed; 0 failed
    traits   (lib)            test result: ok. 28 passed; 0 failed
    screensearch_lib (lib)    test result: ok.  2 passed; 0 failed
    # 66 passed; 0 failed; 3 ignored (Windows-gated real-hardware tests below)
$ cd ui && npm run build      # ✓ built in 334ms (tsc --noEmit clean), exit 0
$ cd ui && npm run lint       # exit 0
$ git status --porcelain -- ui/src/bindings   # empty (no ts-rs drift; no IPC type changed)
```
- **Observed running** (the P2 "done = observed" proof, `#[ignore]`d real-hardware tests run
  locally on Win11 26200):
```
$ cargo test -p capture --test wgc_smoke -- --ignored          # 1 passed (real frame, correct dims)
$ cargo test -p ocr -- --ignored                               # 1 passed (real WinRT OCR)
$ cargo test -p screensearch --test e2e_capture -- --ignored
    test capture_pipeline_stores_frames_ocr_and_enqueues_embed_jobs ... ok   # 1 passed in 3.55s
```
  The e2e test drives the **real** Kernel (WGC + WinRT OCR + on-disk store): a frame is captured,
  OCR'd, written to `frames` + `ocr_text` with a JPEG on disk, and an `embed_text` job is enqueued —
  while **no `vision_tag` job is** (`13.1` + `13.3`, demonstrated, not just compiled).

### Skipped / deferred (intentional, by phase)
- Embedding worker (P3) consumes the `embed_text` jobs P2 enqueues; vision/answer → P4.
- Live `search`/`ask`/`get_timeline`/`enqueue_vision` IPC → P3+. Thumbnails in the timeline (needs
  the Tauri asset protocol) + full Command-Deck UI → P5.
- `storage.retention_days` purge → P5. Single-shot-per-tick WGC optimization → later (`07`).

### Hallucinated / corrected
- Assumed windows-rs `IAsyncOperation::get()` — windows-future 0.3 renamed it to `.join()`.
- Assumed `SoftwareBitmap::CreateCopyFromBuffer` / `CryptographicBuffer::CreateFromByteArray` were
  always available — both need the `Storage_Streams` feature.
- Assumed `BOOL` in `Win32::Foundation` and `MONITORINFOF_PRIMARY` in `Graphics::Gdi` — moved to
  `windows::core::BOOL` and `Win32::UI::WindowsAndMessaging`.

### Still risky / to watch
- WGC `TryGetNextFrame` null/empty handling on cold start is treated as "no frame this cycle"
  (`Surface()` errors → `Ok(None)`); the first cycle or two on a fresh session may be skipped.
- Lock detection is an `OpenInputDesktop` heuristic (`07`); verify behaviour if ever run elevated.
- Continuous per-monitor WGC sessions sample every `interval_ms` — fine, but idle GPU work could be
  trimmed with single-shot capture later.

---

## Pass 5 — 2026-06-21 — P3 Deferred Enrichment

**Branch:** `p3-enrichment`. New external dep: `fastembed` 5.17.2 (pulls `ort` 2.0.0-rc.12 / ONNX
Runtime). Built clean on the pinned toolchain (rust-version 1.82) — **no MSRV bump needed**.

### Implemented (with verbatim verification)
- **`embeddings::FastEmbedProvider`** — the real `EmbeddingProvider` (`03 §3`): text via
  `EmbeddingModel::EmbeddingGemma300MQ` (768-dim, quantized → embeds one input at a time per
  `MODEL_REGISTRY §5`), optional image via `ImageEmbeddingModel::NomicEmbedVisionV15`. Each lane is
  `Arc<Mutex<…>>` accessed inside `spawn_blocking` (no thread affinity, unlike OCR). Models load
  eagerly off the launch thread into `<app-data>/models/fastembed`.
- **Bounded worker pool (`kernel::worker_pool`)** — N = `enrich.worker_concurrency` workers each
  `claim_jobs`→`process_job`→`complete`/`fail` with exponential backoff (`1 s·2^attempts`, cap 60 s);
  idle poll 250 ms→2 s; graceful `watch`-channel shutdown. Handles `embed_text` + (when enabled)
  `embed_image`; **never** `vision_tag` (P4). Empty-OCR / purged-frame → complete (no-op); missing
  `frame_id` / missing JPEG → dead-letter; embed/upsert error → retry.
- **Stale-`running` recovery (`03 §6`, gap #6)** — `Store::reset_stale_running_jobs`; startup sweep
  (requeue all `running`) + periodic 60 s sweep (5-min visibility timeout).
- **Vector arm live end-to-end** — `SqliteStore` embedder made runtime-settable
  (`Arc<RwLock<Option<…>>>` + `set_embedder`); the composition root loads the model off-thread, then
  `Kernel::attach_embedder` injects it (lighting the vector arm) and starts the pool — independent of
  capture (`02 §5` background trigger). `embed_model` readiness flows Initializing→Ready/Unavailable/
  Disabled; new `job_progress` event drives queue depth.
- **`search` command** (`SearchQuery`→`SearchHit[]`, `03 §7`) so hybrid search is reachable; the
  capture loop now also enqueues `embed_image` when `enrich.image_embeddings` is on.

Verbatim verification (run 2026-06-21):
```
cargo fmt --all -- --check                            → exit 0
cargo clippy --workspace --all-targets -- -D warnings → exit 0 (Finished in 19.68s)
cargo test --workspace                                → all pass, 0 failed
    store        27 passed   (incl. enrichment-input + stale-sweep)
    traits       28 passed
    kernel       5 passed (enrichment) + 3 passed (pipeline)
    screensearch 2 passed
    (ocr 1 / e2e_capture 1 / embeddings real-model 1 / store perf 1  → #[ignore]d)
cargo test -p store --test perf -- --ignored
    → hybrid_search over 10000 frames: median = 30.9ms, p95 = 32.6ms (20 queries)  [DoD 13.4 ✓]
cargo test -p embeddings -- --ignored
    → loads_and_embeds_text ... ok  (real EmbeddingGemma300MQ: 768-dim, deterministic) in 9.96s
npm --prefix ui run build  → tsc --noEmit clean; vite built in 393ms; no ts-rs binding drift
```

### Skipped / deferred (intentional, by phase)
- **All vision scheduling → P4** (user-confirmed): no `vision_tag` enqueue, no timer/idle scheduler,
  no `enqueue_vision` command — the consumer needs the P4 sidecar.
- **Text chunking** — one chunk per frame (`chunk_index = 0`); the schema's `UNIQUE(frame_id,
  chunk_index)` already supports multi-chunk as a non-breaking later addition.
- **Query/document prompt asymmetry** — `embed_texts` is used symmetrically for index and query (the
  trait has only `embed_texts`); an EmbeddingGemma `query:`/document-prefix path is a later
  retrieval-quality refinement (`07`).
- **Search UI** → P5 (backend command only this phase).

### Hallucinated / corrected
- Pre-flight worry that fastembed lacked a quantized EmbeddingGemma variant — **false**: docs.rs
  confirmed `EmbeddingModel::EmbeddingGemma300MQ` exists in 5.17.2; the `MODEL_REGISTRY §3` name is
  correct. The build proved it (real-model test passed).
- Expected a possible `ort` MSRV bump — not needed; built on 1.82.

### Still risky / to watch
- **`onnxruntime.dll` bundling** — `ort` fetches a prebuilt ONNX Runtime at build time; the
  installer must bundle the DLL (P5 packaging, logged in `07`).
- **Single shared model handle** serializes concurrent embeds on the `Mutex` (fine for the cheap text
  model; per-worker handles would 2× RAM — deferred).
- **Perf bar** measured on an in-memory DB (the query-algorithm cost, not disk); 32.6 ms p95 leaves
  wide margin, but a future on-disk fixture at scale is worth adding. Gap #8 (vec arm post-KNN time
  filter) stays open — unobserved here, revisit if recall on tight windows matters.
- **Stop-workers-on-exit** is best-effort (`block_on` in the Tauri exit hook); correctness does not
  depend on it (the startup sweep requeues any interrupted job).

---

## Pass 6 — 2026-06-21 — P3 review fixes (PR #7)

Three findings from the PR #7 review (gemini-code-assist; the claude code-review action found no
issues), all verified valid and fixed:

- **Stale-job sweep clock precision (high)** — `reset_stale_running_jobs(older_than_ms <= 0)` now
  requeues *all* `running` jobs unconditionally (the startup sweep can't miss a job marked running
  in the last sub-second before a crash); and `claim_jobs` stamps `updated_at` with the
  `unixepoch()*1000` DB clock, so the periodic sweep no longer mixes Rust-ms with SQLite-second time.
- **Image-load retries (medium)** — `embed_image` retries on a transient file error when the JPEG
  still `exists()` (Windows sharing violations from AV/indexer/backup), dead-lettering only a
  genuinely missing file — instead of dead-lettering on any error.
- **`WorkerPool` Drop (medium)** — `impl Drop` signals stop so a pool dropped without `shutdown`
  doesn't leave detached workers draining the queue; `shutdown` uses `mem::take` (can't move out of a
  `Drop` type). *Note: the reviewer's snippet kept `for join in self.joins`, which would not compile
  once `Drop` is implemented — corrected with `mem::take`.*

**Verification:** `fmt` + `clippy --workspace --all-targets -- -D warnings` clean; `cargo test
--workspace` all pass (kernel enrichment **7** — two new `embed_image` tests added; store 27; all
prior tests green). Merged the updated `claude-code-review` workflow from `main`.

---

## Pass 7 — 2026-06-21 — P4 Inference sidecar (`feat/p4-inference-sidecar` branch)

Built the full P4 layer **lifecycle-first** (`04 §3`): the no-orphan Job-Object binding before any
real inference wiring. User decisions taken up front (recorded in `07`): runtime auto-download of
both the `llama-server` binary and the GGUF models, with the acceptance bar = lifecycle + mock-tested
inference (real GPU end-to-end as a gated `#[ignore]` smoke).

**Implemented (`crates/inference`, all new modules):**
- `job_object` + `process` — `CreateJobObjectW(KILL_ON_JOB_CLOSE)`, suspended `CreateProcessW`,
  **assign-before-resume**, pidfile, image-path reap predicate, liveness/terminate helpers. Raw
  `windows-rs` (not `std::process`) because only that can spawn suspended and recover the thread
  handle to resume.
- `client` — `reqwest` OpenAI client: non-stream completion (vision) + SSE stream (answer),
  normalizing `reasoning_content` and `content` deltas into neutral `StreamPiece`s.
- `supervisor` — `ModelSupervisor`: lazy spawn, idle-evict, `/health` gating, crash restart, startup
  reap, model switch; `SidecarStatus` broadcast; an RAII `Lease` counts in-flight requests so the
  evictor never pulls a model out from under a live request.
- `models` + `download` — tier→repo map (`MODEL_REGISTRY`), app-data layout, `Q4_K_M`+mmproj
  selection; GitHub-release (Vulkan zip) + `hf-hub` downloaders (no Python), idempotent.
- `vision` + `answer` — the two providers; JSON-or-rawtext vision parse; `ThinkSplitter` that splits
  inline `<think>` tags across SSE chunk boundaries; one `Citation` per grounding frame.

**Wired:** kernel `attach_inference` + a shared vision slot into the worker pool + the `vision_tag`
branch; `vision_scheduler` (timer + idle, opt-in); `Store::untagged_frame_ids`;
`KernelEvent::SidecarStatus` → `sidecar` readiness; `capture::user_idle_ms` (kernel forbids
`unsafe`, so the idle probe is injected); composition root resolves the binary off-thread, builds the
supervisor + providers, bridges sidecar status, attaches, and shuts the sidecar down on exit.
New commands `ask` / `enqueue_vision` / `set_model_tier`; new events `answer_delta` / `sidecar_status`.

**Verification (verbatim, this machine — RTX 5060 Ti + Vulkan, no binary/models present):**
`cargo fmt --all -- --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` clean;
`cargo build` ok; `cargo test --workspace` all pass — **inference 23 unit + no-orphan 1 + reap 2 +
client 4 (+ 2 smoke ignored)**, **kernel enrichment 9** (two new `vision_tag` tests), **store 28**
(new `untagged_frame_ids` test), traits 28; `ui npm run build` ok (`tsc --noEmit` clean, no binding
changes — P4 added no new IPC types). The no-orphan gate
(`killing_parent_terminates_job_bound_child`) passes — **DoD #7 met**.

**Deferred / gated:**
- Real vision-tag + streamed-answer on the GPU → the `#[ignore]` smoke (`tests/smoke.rs`); run
  manually with `--ignored`. The lifecycle, HTTP client (mock), and provider logic are proven
  deterministically without it.
- `llama.cpp` release-asset name can drift; resolution scans the recent-releases list and
  takes the newest release that carries a unit-tested `*-win-vulkan-x64.zip` asset (not the
  single `/releases/latest`, which can be an incomplete CI publish), overridable via
  `SSV2C_LLAMA_RELEASE_URL`. Fixed 2026-06-22 after a live `latest` (`b9753`) shipped no
  Vulkan zip and broke sidecar start.

**Still risky / notes:**
- No pending-job dedup for the timer/idle vision producers — a frame enqueued-but-not-yet-processed
  can be re-enqueued next tick; harmless (`insert_vision` is an idempotent upsert) but wasteful.
  Logged in `07`.
- Multi-GPU (AMD iGPU + NVIDIA): `-ngl 99` offloads to Vulkan device 0; if the wrong device is
  picked, a device-select env/flag may be needed (logged in `07`).
- Sidecar readiness is set `Ready` after the binary resolves even though the model downloads lazily
  on first request; a model-missing failure surfaces as a `Crashed`/`Error` status + an answer
  `Error` delta rather than up-front.

---

## Pass 8 — 2026-06-22 — P5 (M0) Backend completion

**Branch:** `feat/p5-backend`. First milestone of P5: the three spec-`§7` commands the Command-Deck
UI needs but P4 never implemented, the queries behind them, the Insights aggregate, frame-image
serving, and CSP hardening. No UI work yet (the P2 `App.tsx` is untouched; the full UI lands in
M1–M5).

### Implemented (with verbatim verification)
- **`timeline_buckets(start, end, bucket_count)`** (`crates/store/src/timeline.rs`) — integer-index
  bucketing (`(captured_at - start) / width`, `GROUP BY`), **sparse** (occupied buckets only),
  half-open `[start, end)`, ceil-width so the last bucket reaches `end`. Backs `get_timeline`.
- **`insights_summary(start, end)`** (`crates/store/src/insights.rs`) — real aggregates: total +
  vision-tagged counts, capture density (reuses `timeline_buckets`), top apps (`GROUP BY app_hint`),
  activity breakdown (`GROUP BY activity_type`). Honest-empty when the window is bare. Backs
  `get_insights`.
- **New IPC types** (`crates/traits/src/ipc.rs`): `InsightsSummary` (+`Default`), `AppCount`,
  `ActivityCount` — ts-rs-exported (`ui/src/bindings/{InsightsSummary,AppCount,ActivityCount}.ts`),
  64-bit fields guarded as `number` (added `InsightsSummary` to `no_bigint_in_ipc_types`).
- **`Store` trait** gained defaulted `timeline_buckets` / `insights_summary`; `SqliteStore`
  forwards both.
- **`kernel::settings::save_settings`** — exact inverse of `load_settings` (same key strings;
  numbers→`to_string`, bools→`"true"/"false"`, JSON for composites). Round-trip tested.
- **Commands** (`src-tauri/src/lib.rs`): `get_timeline`, `get_insights`, `get_settings`,
  `set_settings` (persists all; hot-applies model tiers to the live providers like
  `set_model_tier`) — registered in `generate_handler!`.
- **Frame-image serving + CSP** (`tauri.conf.json` + `src-tauri/Cargo.toml`): enabled the Tauri
  **asset protocol** (`protocol-asset` feature + `assetProtocol.scope = ["$APPDATA/frames/**"]`)
  and replaced `csp: null` with a tight policy (`img-src 'self' asset: http://asset.localhost
  data:`, …). Closes the `07` CSP gap.

Verbatim verification (run 2026-06-22):
```
$ cargo fmt --all -- --check
=== fmt check clean ===                                  # exit 0
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.40s   # exit 0, no warnings
$ cargo test --workspace
     Running tests\settings.rs (kernel)
test round_trips_defaults ... ok
test round_trips_non_default_values ... ok
test result: ok. 2 passed; 0 failed; 0 ignored
     Running tests\store.rs (store)
test insights_summary_aggregates_truthfully ... ok
test timeline_buckets_are_sparse_and_half_open ... ok
test result: ok. 31 passed; 0 failed; 0 ignored
     Running unittests src\lib.rs (traits)
test ipc::export_bindings_insightssummary ... ok
test result: ok. 31 passed; 0 failed; 0 ignored
# all other crates green; 0 failed across the workspace
$ ls ui/src/bindings | grep -iE 'Insight|AppCount|ActivityCount'
ActivityCount.ts
AppCount.ts
InsightsSummary.ts
```

### Skipped / deferred
- **Live frame-serving screenshot** (asset protocol rendering a real JPEG in the WebView) — there
  is no UI surface to show a frame yet (still the P2 timeline). The asset protocol is configured
  and compiles; the visual proof happens in M4 (Moment screen) under `cargo tauri dev`. Recorded
  honestly rather than claimed.
- **Packaging** (installer/ZIP, DoD §13.9) — deferred to a follow-up pass per the user's call;
  stays open in `07`.

### Decisions (spec-silent — logged in `07`)
- Frame images via the **asset protocol** (not a custom scheme or base64). `get_timeline` gains a
  `bucket_count` arg beyond the literal §7 signature. The `toast` event is **not** emitted by the
  backend (toasts are client-side). `storage.retention_days` is persisted but **not enforced** (no
  purge job). `InsightsSummary` is a **new** IPC contract (spec defines none).

### Still risky / notes
- `set_settings` persists everything but most subsystems re-read config only on restart / next
  capture start (no live `reconfigure()`); the Settings UI (M5) must label each field honestly.
- The asset-protocol `$APPDATA` scope must resolve to the same dir as `app_data_dir()` (the bundle
  identifier subdir). Expected to match; confirm during the M4 live render.

---

## Pass 9 — 2026-06-22 — P5 (M0) PR #10 review fixes

**Branch:** `feat/p5-backend` (same PR, follow-up commit). Addressed all three actionable review
comments on PR #10 (two from `gemini-code-assist`, one from `claude[bot]`).

### Fixed
1. **`timeline_buckets` integer overflow** (gemini, *high*) — `crates/store/src/timeline.rs`.
   Hostile/malformed timestamps from the frontend could overflow `end - start` (panic in debug,
   wrap in release) and the `(span + n - 1)` ceil could overflow near `i64::MAX`. Now: `span` via
   `checked_sub` (unrepresentable → empty window), ceil rewritten as `span / n + (span % n != 0)`
   (no intermediate overflow), and the bucket end via `checked_add(width).unwrap_or(end).min(end)`.
2. **`insights_summary` redundant work on invalid range** (gemini, *medium*) —
   `crates/store/src/insights.rs`. Added an early `end <= start || checked_sub.is_none()` return of
   the honest-empty summary, skipping four queries + a `timeline_buckets` call that would all
   return zero/empty anyway.
3. **`save_settings` non-atomic multi-write** (claude[bot]) — `crates/kernel/src/settings.rs`,
   `crates/traits/src/contracts.rs`, `crates/store/src/{settings,lib}.rs`. The 20 separate
   `set_setting` upserts could leave the `settings` table half-updated on a crash or a mid-loop
   `serde_json` error (silently hidden by `load_settings`' per-key default fallback). Now: a new
   `Store::set_settings_batch` (defaulted to per-key for non-transactional stores; `SqliteStore`
   overrides it with a single `unchecked_transaction` + `commit`), and `save_settings` builds every
   pair — including the fallible JSON encodings — *before* any write, then commits atomically.

### Tests added
- `set_settings_batch_writes_all_and_overwrites` (store) — batch upserts all keys + overwrites.
- `timeline_buckets_survives_extreme_ranges` (store) — `i64::MIN..i64::MAX` → empty (no panic);
  `0..i64::MAX` (which the old ceil would overflow) → one correct bucket.

### Verification (verbatim)
```
$ cargo fmt --all -- --check          # exit 0
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.23s   # exit 0, no warnings
$ cargo test -p store --test store
test set_settings_batch_writes_all_and_overwrites ... ok
test insights_summary_aggregates_truthfully ... ok
test timeline_buckets_are_sparse_and_half_open ... ok
test timeline_buckets_survives_extreme_ranges ... ok
test result: ok. 33 passed; 0 failed; 0 ignored
$ cargo test -p kernel --test settings
test round_trips_non_default_values ... ok
test round_trips_defaults ... ok
test result: ok. 2 passed; 0 failed; 0 ignored
# full `cargo test --workspace` also green (0 failed across all crates)
```

### Notes
- `unchecked_transaction` is sound here: `with_conn` holds the store mutex exclusively for the
  closure, so no other borrow of the connection exists.
- A forced-crash/rollback test isn't feasible through the public API (no fault-injection seam, and
  every settings upsert is a valid `(TEXT, TEXT)` write); the all-or-nothing guarantee is by
  construction (`?`-return before `commit`). Recorded honestly rather than faked.
- Other review verdicts were "clean" (SQL correctness, IPC types, CSP/asset scope, CLAUDE.md
  compliance) — no changes needed there.

## Pass 10 — 2026-06-22 — P5 (M1+M2) UI foundation, shell & primitives (`feat/p5-ui-foundation` branch)

### Implemented (with verbatim verification)
Replaced the P2 live-timeline `App.tsx` with the "Command Deck" foundation (UI_REFERENCE §1–9),
frontend-only — **no Rust/bindings change** (`git diff --exit-code -- ui/src/bindings` → exit 0):
- **Design tokens** (`ui/src/styles/tokens.css`) + Tailwind theme mapped to them
  (`tailwind.config.js`, `postcss.config.js`, `globals.css`); colors/radius/fonts are token-only,
  default spacing kept (rem steps already equal the 4·8·12·16·24·32·48 scale).
- **Typed IPC plane** (`ui/src/lib/ipc/`): `commands.ts`, `events.ts`, `queryKeys.ts`, `queries.ts`,
  `mutations.ts`, `useAsk.ts`, `useLiveEvents.ts`, `frameSrc.ts` — bindings-only types (§6).
- **State** (`ui/src/state/`): `uiStore`, `toastStore`.
- **Shell** (`ui/src/components/shell/`): `AppShell`, `StatusRail`, `NavRail`, `CommandPalette`,
  `ReadinessBanner`.
- **Primitives** (`ui/src/components/primitives/`): the twelve §5 primitives + inline-SVG icons.
- **App wiring** (`ui/src/app/`): `providers.tsx`, `router.tsx` (lazy routes + per-route
  `errorElement`), route scaffolds, vendor `manualChunks`; removed `ui/src/styles.css`.

```
$ npm run build      # ui/  (tsc --noEmit && vite build)
✓ 125 modules transformed.
dist/assets/index-*.css            25.60 kB │ gzip:  5.40 kB
dist/assets/index-*.js             20.02 kB │ gzip:  7.24 kB
dist/assets/query-*.js             36.71 kB │ gzip: 11.08 kB
dist/assets/react-vendor-*.js     207.71 kB │ gzip: 67.89 kB
# + per-route lazy chunks (Deck/Recall/Timeline/Moment/Insights/Settings/NotFound ≤ 0.83 kB each)
✓ built in 1.27s
# initial JS ≈ 86 KB gzip ≪ 250 KB budget (UI_REFERENCE §8)

$ npm run lint       # ui/  (eslint .)
# clean — no errors, no warnings (Rules-of-Hooks error gate passes)

$ git diff --exit-code -- ui/src/bindings   # exit 0 — generated bindings untouched
```

**Observed running (not just compiled):**
- *Degraded — Playwright-MCP vs `npm run dev` (localhost:5173):* shell renders; no-Tauri runtime →
  readiness query errors → StatusRail shows the honest **"Kernel offline"** chip; routes
  `/ · /recall · /timeline · /timeline/42 · /insights · /settings` + bogus path → **NotFound**;
  global **Ctrl+K** opens the palette (auto-focus), **↓ + Enter** runs "Go to Recall" (navigates +
  closes). Console: only a favicon 404 + the router v7 future-flag note.
- *Authoritative — `tauri dev`:* compiled (`Finished … in 16.15s`) and booted — `store opened …
  schema_version=1`, `WinRT OCR ready`, `inference attached; sidecar ready (lazy spawn)`,
  `embedding model loaded; attaching to kernel`. Every subsystem comes up Ready (the live data the
  StatusRail consumes). No panic/error.

### Skipped / deferred (intentional, by milestone)
- **Screen bodies** (search list, AnswerStream, Scanline Timeline, Moment detail, Settings controls,
  Insights charts) → M3–M5. Routes are scaffolds stating each screen's purpose (IA per §3), not
  "Coming Soon".
- `frameSrc.ts` exists and is correct but isn't rendered yet (no `FrameImage`/`FrameTile` until M4).
- `@tanstack/react-virtual`, `react-markdown`, `remark-gfm` installed now but first consumed in M3/M4.

### Decisions / corrections (logged)
- **StatusRail "DB size"** → shows the **DB readiness/status** chip instead; no size field exists in
  the IPC contract and fabricating one is disallowed (new `07` gap #27).
- **`job_progress` payload is `JobStats`**, not the `JobProgress` wrapper binding — the kernel emits
  the inner value (`src-tauri` `forward_events`); `events.ts` types `job_progress` as `JobStats`.
- **Tailwind spacing** kept at default rather than replaced, so width/height/inset/max-* utilities
  survive while color/radius/font scales stay token-locked.
- **Sidecar live status** is an event-sourced Query cache entry (no fetch command) — `useSidecarStatus`'s
  queryFn preserves the last `sidecar_status`-written value so a refetch can't clobber it.

### Still risky / to watch
- Live StatusRail visuals are evidenced by the `tauri dev` boot log + the rail's verified render
  paths — the native WebView2 window can't be screenshotted with the available tools. A full
  populated-with-real-frames visual pass is scheduled with the M3/M4 screens (per the plan DoD).
- React Router v7 future-flag warning is benign now; opt into `v7_*` flags before any RR7 upgrade.

## Pass 11 — 2026-06-22 — P5 (M1+M2) PR #11 review fixes (`feat/p5-ui-foundation` branch)

Reviews: Claude code-review (after the `ANTHROPIC_API_KEY` secret was restored) + gemini-code-assist.
Codex was invoked (`@codex review`) but posted no review. One real bug + four hardening items.

### Fixed
- **Bug — CommandPalette active-index latch** (`CommandPalette.tsx`): the clamp effect now resets to
  `0` on an empty filtered list (`filtered.length > 0 ? Math.min(Math.max(0,a), len-1) : 0`). The
  old `Math.min(a, …)` latched `active` at `-1` after ArrowDown over a zero-match query, breaking
  Enter-to-run once matches returned. **Both** reviewers flagged it.
- **Ctrl+K label** (`NavRail.tsx`): `⌘K` → `Ctrl+K`. Chose the Windows literal over Gemini's
  `userAgent` Mac-detection branch — CLAUDE.md forbids cross-platform abstractions on a Windows-only app.
- **Palette input** (`CommandPalette.tsx`): added `type="text"` + `autoComplete/autoCorrect/
  autoCapitalize="off"` + `spellCheck={false}`.
- **RouteError** (`routes/RouteError.tsx`): extract via the official `isRouteErrorResponse` guard
  (status + statusText/data) before the Error/string/`{message}` fallbacks — a thrown Response/404
  now shows real detail. (Cleaner than the reviewers' nested-ternary duck-typing.)
- **useAsk concurrency guard** (`lib/ipc/useAsk.ts`): early-return while `phase === "streaming"`
  (deps `[state.phase]`) so a second ask can't mix the first's late `answer_delta`s. True
  concurrent/cancellable ask needs a backend request-id (`07` #28).
- **queryKeys prefixes** (minor): `timelinePrefix`/`insightsPrefix` used by `useLiveEvents` instead
  of raw magic arrays.

### Not changed (reviewer "no action needed" / deferred)
- Full ARIA combobox semantics for the palette → `07` #29 (revisit M3 with the search UX).
- `frameSrc` module-level cache (correct for a single-process desktop app).

### Verification (verbatim)
```
$ npm run build      # ui/
✓ built in 1.49s     # initial JS ≈ 86 KB gzip (react-vendor 67.90 + query 11.08 + index 7.40)
$ npm run lint       # ui/  — clean, no errors/warnings (Rules-of-Hooks gate)
```
**Observed running** — Playwright vs `npm run dev` reproduced the exact bug path: open palette →
type no-match `zzzz` → **ArrowDown** (empty list) → replace query with `time` → **Enter** → URL =
**`/timeline`** (fix recovered `active` to 0; pre-fix it would have stayed `/`). NavRail hint renders
**"Ctrl+K"**.

### Follow-up — Codex review of `f03fa58` (synchronous ask guard)
Codex flagged (P2) that the `useAsk` guard reads React state, which lags within an event tick: two
`ask()` calls in the same synchronous tick both observe the pre-`start` `phase` and both reach
`cmd.ask`, interleaving two streams on the id-less `answer_delta` channel. **Fixed** — `useAsk` now
guards on a synchronous `useRef` in-flight flag (set before dispatch, released when `phase` hits
`done`/`error`, and on `reset`); `ask` deps drop to `[]`. The backend per-request-id for true
concurrency/cancellation stays open as `07` #28. Also tidied the stale `⌘K` comment in `NavRail.tsx`.
Verify: `npm run build` `✓ built in 1.09s`, `npm run lint` clean. No UI consumer until M3, so this is
pre-emptive hardening (build/lint + reasoning). Claude's `f03fa58` re-review: "No issues … PR is clean".

### Follow-up — Codex review of `db61a5f` (enrichment-completion refresh)
Codex flagged (P2) that a completed `vision_tag`/`embed_*` job only signals `job_progress` (counts
only), so with focus-refetch off + no polling, a cached Moment/Recall/Insights query would never
refresh — new tags/embeddings invisible until reload. **Fixed** — `useLiveEvents` debounce-invalidates
`frame`/`search`/`insights` families on `job_progress` (`ENRICH_DEBOUNCE_MS = 1000`) on top of the
immediate `jobStats` update; `invalidateQueries` refetches only observed queries (others go stale), so
a backlog drain stays cheap. Timeline excluded (capture density). Added `searchPrefix`/`framePrefix`
to `queryKeys`. The surgical fix (a richer `job_completed { kind, frame_id }` event) needs a backend
change — `07` #30. Verify: `npm run build` `✓ built in 1.08s`, `npm run lint` clean. Behavioral proof
(Moment refetches after a vision tag) lands with the M3/M4 screens that consume these queries.

## P5 (M3+M4) — Recall, Deck, Timeline, Moment (`feat/p5-screens`, PR #12)

The data-bearing screen bodies on top of the M1+M2 foundation. One pivotal fork was surfaced to the
user before building (not guessed): the Timeline contract (hover thumbnails, Enter-opens-a-Moment)
and the Deck (jump-back-in) need to map a time/recency onto real frames, but the merged M0 backend
exposed only density buckets. The user approved adding a thin frame-browsing backend slice (`07`
#31) — so this PR opens with a backend precursor commit, then the screens.

### Implemented
- **Backend slice** (`crates/store/src/frames.rs`, inherent `SqliteStore` methods — mirrors
  `get_frame`, no `Store`-trait change): `frames_in_range(start,end,limit)` (newest-first, half-open)
  and `nearest_frame(at)` (closest on either side, at-or-after wins ties; `i128` distance so hostile
  timestamps can't overflow). Commands `get_frames` / `get_nearest_frame`; new ts-rs `FrameMeta`
  (added to the `no_bigint_in_ipc_types` guard). Two `:memory:` integration tests.
- **Frontend IPC** — `getFrames`/`getNearestFrame` wrappers, `useFrames` query, `frames` key family
  (distinct from the singular `frame` detail); `capture_tick` now also invalidates the frames list.
  Helpers `lib/time.ts` (relative + absolute, 24-h clock) and `lib/timeRanges.ts` (day-snapped,
  key-stable windows).
- **Domain components** (`components/domain/`): `FrameImage` (lazy, async-decoded, intrinsic w/h →
  no CLS, dev-safe placeholder), `FrameTile`, `SearchResult` (FTS `[match]` highlighting),
  `AnswerStream` (react-markdown + GFM `prose-deck`, collapsible `<details>` thinking, citation
  tiles), `JobQueueMeter`, `ScanlineTimeline` (canvas density ribbon via a shared `drawDensityRibbon`,
  devicePixelRatio-crisp, token colors read from computed style, DOM scan-head + glow + scanline
  texture, `role="slider"` with valuemin/max/now/text, pointer + keyboard scrub), `TimelineMinimap`
  (thin reuse of the same draw fn), `MomentDetail`.
- **Screens, all five states each** (`routes/`): Deck (capture hero + today aggregates + minimap +
  recents + queue meter; onboarding empty, enrichment-pending partial), Recall (mode-toggled
  virtualized search via `@tanstack/react-virtual` + streamed Ask, degraded-mode banners), Timeline
  (ScanlineTimeline + range presets + `?t=` deep link; Enter → `get_nearest_frame` → `/timeline/:id`),
  Moment (`MomentDetail` + prev/next + neighbour strip + on-demand "Tag with vision").
- **Token addition:** `--glow-scan` + Tailwind `shadow-scan` — the one place glow is spent (the
  scan-head halo), kept token-driven (no hardcoded shadow in components).

### Skipped / deferred
- Width-derived timeline `bucket_count` — fixed 240 is ample (`07` #32).
- A richer `job_completed { kind, frame_id }` event for surgical invalidation — still `07` #30.
- Insights + Settings screens are M5 (still `ScreenScaffold` placeholders, untouched here).

### Still risky / watch
- **Concurrent ask** unchanged — `useAsk`'s single-flight guard holds; true concurrency needs the
  backend request-id (`07` #28).
- **Hover-thumbnail bias** — preview thumbnails come from the newest-first window sample, so a very
  dense window biases the *preview* toward recent frames; the *open* is always exact via
  `get_nearest_frame` (`07` #31).
- **Native-window screenshots are not automatable** — Playwright drives the Vite browser (no Tauri
  IPC → degraded states only). Populated states were captured from the live WebView via `PrintWindow`
  for this pass; that is a manual verification aid, not a CI artifact.

### Verification (verbatim)
```
$ cargo fmt --all -- --check                 # clean
$ cargo build --workspace                    # Finished `dev` in 22.05s
$ cargo test --workspace                     # all pass; store: 35 passed (incl. 2 new frames tests)
$ cargo clippy --workspace --all-targets -- -D warnings   # Finished, no warnings
$ git status --porcelain ui/src/bindings/    # ?? ui/src/bindings/FrameMeta.ts (only the new file)
$ npm run typecheck   # ui/  — clean
$ npm run lint        # ui/  — clean (Rules-of-Hooks gate)
$ npm run build       # ui/  — ✓ built in 2.02s; initial JS ≈ 87 KB gzip
                      #   (react-vendor 68.14 + query 11.08 + index 8.24); react-markdown isolated
                      #   in the lazy Recall chunk (57.89 gz); route chunks 2–3 KB gz each
```
**Observed running** — `npm run dev` (root = `tauri dev`) booted the integrated app: store opened
(schema v1), WinRT OCR ready, vision scheduler started, inference attached (lazy sidecar), fastembed
loaded, embedding workers started — no panic; the new commands compiled into the registered handler.
Live WebView captures confirmed the **populated** path: the Deck rendered real frame thumbnails
(images served via the asset protocol), the today minimap, top-apps aggregates, and the queue meter,
live-updating across captures (71→82 frames, done 76→87 — proving the `capture_tick`/`job_progress`
cache invalidation refreshes in real time); the Timeline rendered the real density ribbon + scan-head.
Degraded states (Playwright vs `npm run dev` localhost:5173, no Tauri runtime): Deck error/retry,
Recall search + ask invites with the mode toggle, Timeline error + range presets, the ⌘K palette.

## Review response — PR #12 (2026-06-22)

The automated `claude-review` job exhausted its 20-turn budget on the 28-file diff
(`terminal_reason: max_turns`) before posting a verdict — a review-bot limit, not a branch failure;
the authoritative `build & test (windows)` gate **passed**. It did post inline findings first;
triaged here.

### Fixed (this commit, on `feat/p5-screens`)
- **[high] Moment prev/next + context broken in a busy session** (`07` #33). The single newest-first
  `get_frames([at−30m, at+30m), 24)` returns only the far-edge frames in a dense window, dropping
  the anchor and its true neighbours → `findIndex` = −1 → Prev/Next dead, strip ~30 min off. The cap
  can't express "closest-after" (DESC only), so added an anchored backend query
  `neighbour_frames(at, half_window_ms, limit_each)` (closest-before DESC + closest-after ASC, merged
  ascending, anchor excluded) → `get_frame_context` / `useFrameContext`; Moment derives prev/next by
  capture-time, not by locating the anchor. New regression test
  `neighbour_frames_brackets_anchor_with_closest_each_side`. No new IPC type (reuses `FrameMeta`).
- **[med] Markdown links in `AnswerStream` could hijack the WebView.** Model-output links rendered as
  bare `<a href>` would navigate the app's own window (unmounting the UI). Added an `a` component
  override → `target="_blank" rel="noopener noreferrer"` (opens in the OS browser).
- **[med] "Thinking" trace collapsed out from under the reader.** `<details open={streaming}>` snapped
  shut the instant streaming finished. Now a controlled `<details>` (`thinkingOpen` state): auto-opens
  on the rising edge of a new stream, never auto-collapses, respects the user's manual toggle. Hooks
  hoisted above the `idle`/`error` early returns (Rules-of-Hooks gate stays green).

### Acknowledged, not fixed (rationale recorded)
- **[P2] Live search doesn't refresh on `capture_tick` when text-enrichment is off** (`07` #34) —
  edge case; invalidating the whole `search` family every debounced tick would re-run the live query
  continuously during capture. The query re-runs on the next keystroke regardless. Deferred.
- **[minor] Timeline `ArrowRight` clamps to `range.end` vs `End` to `range.end−1`** — both resolve to
  the same frame via `get_nearest_frame` (the reviewer's own note); cosmetic, no behaviour change.

### Verification (verbatim)
```
$ cargo fmt --all -- --check                              # exit 0
$ cargo clippy --workspace --all-targets -- -D warnings   # exit 0 (Finished `dev`)
$ cargo test -p store                                     # 36 passed (incl. new neighbour_frames test)
$ cargo test -p traits                                    # 32 passed
$ git diff --stat -- ui/src/bindings                      # empty — no IPC type added, no drift
$ npm run typecheck   # ui/  — exit 0
$ npm run lint        # ui/  — exit 0 (Rules-of-Hooks gate)
$ npm run build       # ui/  — ✓ built in 1.85s; initial JS ≈ 87.5 KB gzip
                      #   (react-vendor 68.14 + query 11.08 + index 8.31); Recall chunk 58.01 gz
                      #   (react-markdown isolated); Moment 1.85 gz
```

## P5 — M5: Settings & Insights (feat/p5-settings-insights, 2026-06-22)

### Implemented
- **Settings route** (`ui/src/routes/Settings.tsx`) — a single editable draft of the typed `Settings`
  binding over six panels (Capture · Storage · Models · Enrichment · Privacy · Sidecar-advanced). Save
  is optimistic + reconcile (`useSetSettings`); **Reset** reverts to the last-saved snapshot; a dirty
  chip + disabled Save/Reset gate the action bar. Model tiers **hot-apply immediately** on pick
  (`useSetModelTier` — persists + switches the live provider) with optimistic draft update and revert
  on error. Each field carries an honest apply-point label (now / next question / next capture start /
  restart). All hooks (incl. the two free-text list buffers) are declared **before** the loading/error
  early returns — Rules-of-Hooks gate stays green. States: loading skeleton · load-error+retry ·
  populated · partial ("models loading…" chip driven by readiness). No empty state (matrix: "—").
- **Insights route** (`ui/src/routes/Insights.tsx`) — real `get_insights` aggregates with Today/7/30
  range presets. States: loading skeleton · compute-error+retry · empty ("Not enough history yet" +
  Go-to-Deck) · partial (tagged < total → "tagged only" badge + "based on N tagged frames" note) ·
  populated (stat chips + captures-over-time + top-apps + activity breakdown).
- **Domain controls** — `ModelTierPicker` (segmented Default/Quality/Beta per lane), `ScheduleControl`
  (on-demand note + timer/idle opt-ins with minute-thresholds), `RetentionControl` (honest
  not-enforced label), and the lightweight inline charts `CapturesTrend` (time-positioned density
  bars, no chart lib) + `InsightsBars` (ranked horizontal bars). Exported from `domain/index.ts`.

### Engineering defaults (spec-silent — logged in `07` #35/#36/#37)
- **Apply-timing** mirrors the backend's honest policy (tiers live, thinking per-ask, capture/storage/
  privacy on next capture, enrich/sidecar on restart); persisted always. No live `reconfigure()`.
- **`capture_monitors`** edited as a comma-separated 0-based index list (empty = all) — no
  monitor-enumeration command exists, so no fabricated monitor names.
- **List fields** (`capture_monitors`, `privacy_excluded_apps`) keep a raw-text buffer so typing
  (trailing commas etc.) isn't fought by re-serialising the parsed array back into the input; the
  parsed array still drives dirty-detection and the save payload.
- **Insights `captures`** rendered by true time-offset (not by bucket index), so the chart is
  decoupled from the backend's fixed 48-bucket grain.

### Verification (verbatim)
```
$ cargo fmt --all -- --check                              # fmt exit: 0
$ cargo clippy --workspace --all-targets -- -D warnings   # Finished `dev` … 5.82s; clippy exit: 0
$ cargo test --workspace                                  # test exit: 0  (M5 changed 0 Rust files —
                                                          #   suite identical to merged main 2ecf038)
$ git diff --stat -- ui/src/bindings                      # empty — no IPC type added, no drift
$ npm --prefix ui run lint                                # exit 0 (Rules-of-Hooks gate)
$ npm --prefix ui run build  # tsc --noEmit + vite build  — ✓ built in 1.63s
   Settings  10.46 kB │ gzip 3.56 kB   (own lazy chunk)
   Insights   5.16 kB │ gzip 1.99 kB   (own lazy chunk)
   initial JS ≈ 87.7 KB gzip (react-vendor 68.14 + query 11.08 + index 8.47); Recall 58.02 gz isolated
```
**Observed running:** Playwright vs the Vite dev server with `window.__TAURI_INTERNALS__` mocked to the
exact `Settings`/`InsightsSummary`/`Readiness`/`JobStats` binding shapes (the browser has no Tauri
bridge). Captured all five states for **both** screens with **0 console errors**: Settings populated
(form + the two `ModelTierPicker`s + thinking toggle + `ScheduleControl` with the timer field revealed,
honest apply labels), Settings loading skeleton, Settings load-error+retry; Insights populated
(`CapturesTrend` orange density chart + ranked `InsightsBars` for apps & activities + "tagged only"
partial labelling), Insights empty, Insights partial (18% tagged), Insights compute-error+retry,
Insights loading skeleton. (Native-window screenshots remain impossible — Playwright can't attach to
the Tauri WebView; the IPC mock is the dev-mode substitute the plan calls for.)

### Review response — PR #13 (2026-06-22)
Automated review (gemini-code-assist ×6, chatgpt-codex ×1, claude ×2) raised nine points; all
addressed (CI itself was green: build & test windows, CodeQL, claude-review).
- **[bug] Numeric handler sent floats to integer Rust fields** (claude) — `valueAsNumber` (e.g.
  `500.5`) passed `Number.isFinite` and would be rejected by serde for a Rust integer on save. Split
  the generic handler into `intHandler` (`Math.round`) for the six integer fields and `numHandler`
  (raw) for the one float (`capture_diff_threshold`).
- **[ux] Clearing a numeric input snapped back** (gemini ×3 — Settings, RetentionControl,
  ScheduleControl) — an empty field is `NaN`, which the guard ignored, so the controlled input
  reverted to its old value. All numeric handlers now fall back to `0` on empty (a transient value to
  type over). `ScheduleControl`'s minute display no longer floors at 1, so a cleared field shows `0`.
- **[correctness] Out-of-range values could be persisted** (codex, P2) — input `min`/`max` are
  advisory (typing/pasting bypasses them) and an out-of-range `capture_diff_threshold` (>1) would
  wedge the capture diff-gate. Added `sanitizeSettings()` — rounds every integer field and clamps all
  numerics to valid ranges — applied on **save**, with the clamped values reflected back into the form
  and a toast when anything was adjusted.
- **[robustness] `modelsLoading` partial state** (claude + gemini) — only checked `"initializing"`,
  missing `"unknown"` (pre-init) and the in-flight `readiness.isLoading` case (so the populated form
  showed no partial indicator during the readiness probe). Now also covers both, with optional
  chaining guarding a partially-populated payload.
- **[correctness] `baseline` could drift from the backend** (gemini) — the seed effect only set
  `baseline` once, so the `dirty` diff could go stale after a refetch. Now `baseline` re-syncs on
  every `settings.data` change; the editable `draft` is still seeded only once (no clobbering edits).

Verification (verbatim): `npm --prefix ui run lint` exit 0 · `npm --prefix ui run build` exit 0
(`tsc --noEmit` + vite, ✓ 2.20s; Settings chunk 3.86 KB gz, Insights 1.99 KB gz, initial JS unchanged
≈ 87.7 KB gz). **Observed running** (Playwright vs Vite dev, typed IPC mock): the clear-to-`0` fix
confirmed by interaction — `Capture interval (ms)` `"3000"` → clear → `"0"` (no snapback) → type →
`"4500"`; Settings form renders with 0 app-code console errors (only a favicon 404).

## Pass — Vision-tagging quality fix (2026-06-22, `feat/p5-m5-insights-settings-vision` off `main` @ 39d5da8)

**Scope.** With P5-M5 (Settings + Insights) already merged on `main` (#13), the remaining work was the
vision-output honesty gap (`07` #19/#20): the prompt pinned a literal `"confidence": 0.0` the model
echoed, and `activity_type` was free-form. Fix is confined to `crates/inference` — no `traits`/schema/
IPC change, so no migration and no ts-rs binding drift.

**Done.**
- `client.rs`: optional `response_format` on `ChatRequest`, threaded through `complete(...)`; streaming
  path unchanged (`None`). +2 unit tests (serialized-when-set / omitted-when-none).
- `vision.rs`: prompt no longer demonstrates a `confidence` value; vision call sends an OpenAI
  `response_format` JSON-schema (enum `activity_type`, numeric `confidence`) → `llama-server` grammar;
  `parse_vision` hardened with `normalize_confidence` (trust only finite `(0,1]`, else `-1.0`) and
  `normalize_activity` (closed `ACTIVITY_TYPES` set or `None`). +6 unit tests.
- `tests/smoke.rs`: gated smoke asserts confidence ≠ fabricated `0.0` and activity ∈ allowed∪{none}.

**Verification (verbatim).**
- `cargo fmt --all -- --check` → exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` → exit 0
- `cargo test --workspace` → exit 0 (inference lib **33** incl. 8 new; store 36; traits 32; 0 failed)
- `git diff --exit-code -- ui/src/bindings` → exit 0 (no drift)
- `npm --prefix ui run typecheck` → exit 0 · `lint` → exit 0 · `build` → exit 0
- **Real-GPU smoke** `cargo test -p inference --test smoke real_vision_tags_an_image -- --ignored
  --nocapture` (RTX 5060 Ti, cached Qwen3-VL-4B Q4_K_M+mmproj) → **1 passed in 11.52s**:
  `VISION: A split-screen view … | activity=Some("browsing") | conf=0.95` (was `unknown`/`0.0`).
- **Observed running:** `npm run tauri dev` booted all subsystems Ready (store v1, WinRT OCR, vision
  scheduler, inference attached/lazy, fastembed, embedding workers) with no panic; shutdown left no
  orphaned `llama-server.exe`.

**Note (QA finding).** The session's initial git snapshot was stale (`2ecf038`); `main` was already at
`39d5da8` with M5 merged, so Insights/Settings needed no rebuild — confirmed by reading the files +
the full UI build. `cargo tauri dev` is **not** runnable here (no `cargo-tauri`); the repo's Tauri CLI
is the npm dev-dependency, so `npm run tauri dev` is the working launch path (README corrected).

## Pass — PR #14 review follow-up (vision honesty hardening)

Addressed three correct automated-review findings on PR #14 (gemini-code-assist + two chatgpt-codex P2s).

### Changed
- `crates/inference/src/vision.rs` — `activity_type` made **nullable & optional** in
  `vision_response_format()` (the forced enum had made off-enum→`None` dead code and pushed arbitrary
  labels into Insights for low-signal frames); `VISION_PROMPT` now says answer `null` when unsure;
  `app_hint` filtering extracted to `normalize_app_hint` (trim + `eq_ignore_ascii_case("null")`, was a
  case-sensitive `!= "null"`). +3 net unit tests.
- `ui/src/components/domain/MomentDetail.tsx` — Vision panel shows a neutral **`n/a`** chip when
  `confidence < 0` (the `-1.0` unknown sentinel) instead of rendering it as `-100%`. UI-only; no
  binding drift.

### Verification (verbatim)
- `cargo fmt --all -- --check` → exit 0
- `cargo clippy -p inference --all-targets -- -D warnings` → exit 0
- `cargo test -p inference --lib` → **36 passed; 0 failed** (was 33)
- `git diff --exit-code -- ui/src/bindings` → exit 0 (no drift; `VisionAnalysis` unchanged)
- `npm run typecheck` → exit 0 · `npm run lint` → exit 0 · `npm run build` → ✓
- **Real-GPU smoke** `real_vision_tags_an_image` (RTX 5060 Ti) → **1 passed in 9.88s**:
  `VISION: The screen is divided into two vertical sections … | activity=None | conf=-1`. The synthetic
  two-tone frame — a genuinely low-signal image — now returns **no activity / unknown confidence** (an
  honest decline), where the *forced-enum* schema had it confidently report `browsing` @ `0.95`. This
  is the review's point demonstrated, not just patched.

---

## Pass — 2026-06-23 — P0/P1 review findings fix (`codex/fix-p0-p1-review-findings` branch)

### Implemented
- **Job finalization state safety (`03 §5`)** — `complete_job` / `fail_job` now update only
  `state='running'` rows. A pending, done, dead, stale, or unknown id returns an error instead of
  mutating the queue behind the worker state machine.
- **Forward-migration guard (`03 §12`)** — `SqliteStore::open_path` / `open_in_memory` now reject a
  database whose `schema_version` is newer than this build's `LATEST_SCHEMA_VERSION`, preventing an
  older binary from treating an unknown future schema as ready.
- **Regression coverage** — added `complete_job_requires_running_state`,
  `fail_job_requires_running_state`, and `open_path_rejects_future_schema_version`.

### Verification (verbatim)
```
$ cargo test -p store --test store complete_job_requires_running_state -- --exact

running 1 test
test complete_job_requires_running_state ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out; finished in 0.00s
```

```
$ cargo test -p store --test store fail_job_requires_running_state -- --exact

running 1 test
test fail_job_requires_running_state ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out; finished in 0.00s
```

```
$ cargo test -p store --test store open_path_rejects_future_schema_version -- --exact

running 1 test
test open_path_rejects_future_schema_version ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out; finished in 0.01s
```

```
$ cargo fmt --all -- --check
```

```
$ cargo clippy --workspace --all-targets -- -D warnings
    Checking store v0.0.0 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\crates\store)
    Checking screensearch v0.0.0 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\src-tauri)
    Checking kernel v0.0.0 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\crates\kernel)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.92s
```

```
$ cargo test --workspace
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s # store integration tests
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s # traits binding tests
Finished `test` profile [unoptimized + debuginfo] target(s) in 18.65s
```

```
$ npm --prefix ui run build
> screensearch-ui@0.0.0 build
> tsc --noEmit && vite build
✓ built in 2.21s
```

```
$ npm --prefix ui run lint
> screensearch-ui@0.0.0 lint
> eslint .
```

```
$ git diff --exit-code -- ui/src/bindings
```

---

## Pass — 2026-06-24 — PR #21 audit artifact review follow-up (`codex/run-audit-v2c` branch)

### Review Threads Addressed
- **Build-loop ledger completeness** — Claude and Codex both flagged that the audit artifact carried
  live follow-up gaps without mirroring them into `specs/05_BUILD_REVIEW.md` and
  `specs/07_KNOWN_GAPS.md`.
- **CI-runtime wording** — Codex flagged that the report called local frontend checks
  "CI-equivalent" while the preflight recorded Node `v26.3.0` / npm `11.16.0`, and the GitHub
  workflow uses Node 22.
- **Obsolete-term search reproducibility** — Codex flagged that a future repo-wide `rg` would match
  the audit file's recorded command string.

### Resolution
- Added this build-review pass entry.
- Logged PR #21's required documentation fix in `specs/06_PATCH_PLAN.md`.
- Added open gaps #40-#45 to `specs/07_KNOWN_GAPS.md` for OCR token normalization, no-evidence
  refusal citation tiles, incomplete P5 keyboard/focus matrix, deterministic route state triggers,
  VLM image-path logging, and future isolated app-data audit support.
- Changed the audit wording from "CI-equivalent" to "CI-order local" where appropriate and added the
  explicit Node 26 vs CI Node 22 caveat.
- Noted that the obsolete-term no-match search was captured before the audit artifact existed, and
  that future reruns should exclude the report file.

### Verification (verbatim)
```
$ git diff --check
git diff --check exited 0
```

## Pass — 2026-06-23 — P5 comprehensive review hardening (`codex/p5-comprehensive-review-fixes` branch)

**Scope.** Implemented the approved post-P5 review plan while keeping packaging deferred (`07` #26).
This pass closes P5-adjacent correctness, boundedness, accessibility, live-refresh, telemetry,
retention, and sidecar-device gaps without changing the packaging decision.

**Backend / contract changes.**
- Added range-aware nearest-frame lookup (`get_nearest_frame(at, range?)`) and store coverage so
  Timeline opens cannot escape the selected window.
- Clamped direct IPC sizes for frame lists, frame context, Timeline buckets, and Insights buckets.
- Added storage stats (`get_storage_stats`), monitor enumeration (`get_monitors`), sidecar device
  enumeration (`list_sidecar_devices` via `llama-server --list-devices`), optional
  `sidecar.device`, request-scoped `AnswerEvent`, and `cancel_ask`.
- Added `KernelEvent::Toast` and richer successful data-changing `JobCompleted` events; the UI uses
  these for operational notices and surgical query invalidation.
- Enforced `storage.retention_days` at startup and hourly in 1000-row batches, deleting DB rows and
  only containment-checked relative frame files under `<app-data>/frames`.
- Reconfigured enrichment workers live after settings changes and when an embedder attaches, so
  embedding lanes can be re-enabled without restarting the app.

**UI changes.**
- Reworked local-day helpers to use calendar midnights (`Date(year, month, day + n)`) instead of
  fixed 24-hour offsets.
- Added Deck panel-level error/retry states for readiness, insights, frames, timeline, and job
  stats; completed Command Palette ARIA combobox/listbox semantics.
- Removed hardcoded Timeline drawing colors in favor of CSS token values.
- Added StatusRail storage telemetry, monitor picker + manual fallback, sidecar device picker +
  manual fallback, request-id ask cancellation/stale-delta filtering, embeddings-disabled
  live-search invalidation, and adaptive Timeline/Insights bucket counts from measured chart width.

**Docs / tracking.** Updated `CHANGELOG.md`, `README.md`, `docs/ARCHITECTURE.md`, `CLAUDE.md`,
`specs/07_KNOWN_GAPS.md`, and `specs/08_CHANGELOG_AI.md`; `07` #26 remains open for packaging.

**Verification.** Final gates all exited 0 after the last code change:
- `cargo fmt --all -- --check`
- `npm --prefix ui run lint`
- `npm --prefix ui run typecheck`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm --prefix ui run build`

## Pass — 2026-06-23 — PR #19 review follow-up (`codex/p5-comprehensive-review-fixes` branch)

**Scope.** Addressed every actionable PR #19 comment from Gemini, Claude, and Codex on the P5
comprehensive review hardening PR.

**Changes.**
- Fixed the active ask-task race by locking the task map before spawning the provider task, so an
  immediately-completing task cannot miss its cleanup insertion/removal ordering.
- Made retention robust to a single frame delete failure: the sweeper logs the failed DB delete and
  continues with later candidates instead of letting one frame block all pruning.
- Fixed monitor toggling from the default empty/all-monitors state by expanding to all-but-clicked,
  and normalizing an explicit all-selected list back to empty.
- Refetched sidecar devices when readiness becomes ready and only runs the device query once the
  sidecar binary is resolved. Settings now shows either a device Select or a manual Field, not both.
- Clarified embed-toggle apply timing: worker claiming updates after save, while capture enqueueing
  changes on the next capture start. Updated docs/spec tracking accordingly.
- Cleaned smaller comments: stale toast-store transport comment, `uuid_like_id` → `next_ask_id`, and
  an explanatory comment for the llama.cpp device-id parser threshold.
- Verified the Claude `enrichTimer` cleanup comment was already addressed in `HEAD`; no extra code
  change was needed there.
- **Second Codex pass:** tracked whether the attached FastEmbed provider has the optional image lane
  and reloads it when `enrich_image_embeddings` is enabled after a text-only startup. Retention now
  removes each frame file before deleting its DB row, so a transient file-lock failure keeps the row
  available for retry instead of orphaning the JPEG.

**Verification.** Rerun after this follow-up before pushing:
- `cargo fmt --all -- --check`
- `npm --prefix ui run lint`
- `npm --prefix ui run typecheck`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm --prefix ui run build`

---

## Pass — 2026-06-23 — P4 sidecar hardening (`codex/p4-sidecar-hardening` branch)

### Review findings addressed
- **Model-switch request safety** — `ModelSupervisor::acquire` used to stop/restart the running
  sidecar immediately when the requested `ModelSpec` differed, even if an answer stream or
  `vision_tag` call still held a lease to that process. The supervisor now uses a `RequestGate`;
  every sidecar lease owns the permit until its provider finishes, so model switching waits for the
  active request to drop before killing the process.
- **Same-model crash/hang recovery** — reusing an already-running sidecar now validates both
  `pid_alive` and bounded `/health`. If either check fails, the supervisor emits `Crashed`, stops the
  stale process, and respawns for the current request.
- **Bounded HTTP waits** — `SidecarClient` now has explicit health, completion, and stream-idle
  deadlines. A hung localhost sidecar therefore returns an error to the caller instead of pinning a
  worker or answer stream indefinitely; `vision_tag` uses the existing retry path, and `answer`
  already maps provider errors to `AnswerDelta::Error`.
- **Binary override precedence** — `SSV2C_LLAMA_RELEASE_URL` now resolves before normal install reuse
  and extracts into `<app-data>/sidecar/llama-override/<url-fingerprint>`, preserving the app-managed
  Vulkan install while letting tests/operators pin a sidecar build.

### Failing-first checks
```
$ cargo test -p inference
error[E0433]: cannot find type `RequestGate` in this scope
error[E0425]: cannot find function `can_reuse_running_sidecar` in this scope
error[E0599]: no associated function or constant named `with_timeouts` found for struct `SidecarClient`
```

### Interface Review
- No schema, IPC, `ts-rs`, or trait signature changes.
- The real multi-GB GPU sidecar smokes remain `#[ignore]` and were not run in this pass.

### Verification (targeted during implementation)
```
$ cargo test -p inference
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

```
$ cargo clippy -p inference --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.33s
```

---

## Pass — 2026-06-23 — PR #18 P4 sidecar review follow-up (`codex/p4-sidecar-hardening` branch)

### Review Threads Addressed
- **Shared/exclusive sidecar gate** — `RequestGate` now uses a large semaphore capacity. Ordinary
  same-model requests acquire one permit and can overlap; model switches and same-model crash
  recovery acquire all permits before stopping the process, then downgrade to one permit before the
  request leaves `acquire()`.
- **Dead alias removed by behavior** — `enter_for_model_switch` now has distinct drain-all-permits
  semantics and is used by the stop/spawn paths instead of being a test-only alias for `enter`.
- **Stream timeout semantics** — `ClientTimeouts` now separates `stream_connect` from
  `stream_idle`; the initial streaming POST and later SSE chunk waits have independent deadlines.
- **Atomic override install** — release zips are extracted inside a `.partial` directory on a blocking
  task and renamed into place only after extraction completes; failed extractions clean up the partial
  directory.
- **Multi-install startup reap** — `SupervisorConfig` now carries exact installed binary candidates
  from the normal and override install roots, so toggling `SSV2C_LLAMA_RELEASE_URL` cannot leave an
  app-owned sidecar running from a previous install path.
- **Second review follow-up** — the crash-recovery path now emits `Crashed` for the model that was
  observed unhealthy even if another caller switched models before the exclusive recovery permit was
  acquired. The Tauri composition root also reaps installed binary candidates before `ensure_binary`,
  so an unreachable or invalid override URL cannot prevent startup cleanup of an old sidecar.

### Interface Review
- No schema, IPC, or `ts-rs` binding changes.
- Rust API shape changed only inside the workspace: `SupervisorConfig` gained `reap_binaries`, and
  `SidecarClient` gained `with_client_timeouts` plus a `stream_connect` timeout field.
- The ignored multi-GB GPU smokes remain optional gated verification and were not run during this
  follow-up.

### Verification (targeted during follow-up)
```
$ cargo test -p inference
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

---

## Pass — 2026-06-23 — PR #17 review comment follow-up (`codex/review-p3-deferred-enrichment` branch)

### Review Threads Addressed
- **Gemini inline threads (`crates/kernel/src/worker_pool.rs`)** — `claim_kinds` no longer allocates a
  `Vec` on each worker poll. It returns a fixed `[JobKind; 3]` plus active length, and the worker loop
  passes the active slice into `claim_jobs`.
- **Claude comment precision (`ui/src/lib/ipc/useLiveEvents.ts`)** — clarified that `job_progress` is
  emitted after each worker attempt completes, not only after terminal success.
- **Claude known-gap note (`specs/07_KNOWN_GAPS.md`)** — tracked that embedding lane enablement flags
  remain a pool-start snapshot. This matches the current Settings apply policy (enrichment changes
  take effect on restart) and belongs to a future live-reconfigure pass.

### Interface Review
- No schema, IPC, `ts-rs`, or trait signature changes.
- This follow-up is allocation/documentation cleanup only; the provider-slot behavior from the main
  P3 hardening commit is unchanged.

### Verification (verbatim status)
```
$ cargo fmt --all -- --check
```

```
$ cargo clippy --workspace --all-targets -- -D warnings
    Checking kernel v0.0.0 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\crates\kernel)
    Checking screensearch v0.0.0 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.65s
```

```
$ cargo test -p kernel --test enrichment
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```

```
$ npm --prefix ui run lint
> screensearch-ui@0.0.0 lint
> eslint .
```

```
$ git diff --exit-code -- ui/src/bindings
```

---

## Pass — 2026-06-23 — PR #15 review comment follow-up (`codex/fix-p0-p1-review-findings` branch)

### Review Threads Addressed
- **`crates/store/src/lib.rs` future-schema guard** — the rejection path now compares the DB
  `schema_version` against the maximum version present in the compiled `schema::MIGRATIONS` set, not
  a separately-maintained constant alone. A `debug_assert_eq!` keeps `LATEST_SCHEMA_VERSION` synced
  with the migration set during development and CI test runs.
- **`crates/store/tests/store.rs` temp DB setup** — the future-schema regression test now uses
  `tempfile::tempdir()` so cleanup is owned by the `TempDir` guard even if the test panics.

### Interface Review
- No IPC, `ts-rs`, schema SQL, or `Store` trait signatures changed.
- `LATEST_SCHEMA_VERSION` remains the exported readiness/status constant; the open-time guard now uses
  compiled migration reality as the source of truth and asserts the public constant matches it.
- Added `tempfile` only as a Rust test dependency and centralized its version in workspace
  dependencies, including the existing kernel test use.

---

## Pass — 2026-06-23 — PR #15 Codex review follow-up (`codex/fix-p0-p1-review-findings` branch)

### Review Thread Addressed
- **`crates/store/src/jobs.rs` / kernel stale requeue interaction** — Codex flagged that a provider
  call running past the 5-minute visibility timeout could be requeued to `pending`; if the same live
  call later returned a retryable/dead-letter failure, the new `fail_job(... state='running')` guard
  would reject it, skipping attempts/backoff accounting.

### Resolution
- Kept the store contract strict: `complete_job` and `fail_job` still mutate only `running` jobs.
  Without a durable claim token, weakening `fail_job` to touch `pending` jobs would let an old worker
  mutate a stale/reclaimed job by id alone.
- Added a process-local active-job set in `kernel::worker_pool`. Worker tasks insert the claimed job id
  while processing and remove it via an RAII guard on completion/panic. The periodic stale sweep skips
  while any current-pool job is active, so long-but-live provider calls remain `running` and can record
  their final retry/dead-letter result normally. Startup recovery is unchanged and still requeues
  leftover `running` rows before any worker is live.

### Interface Review
- No IPC, `ts-rs`, schema SQL, or `Store` trait signature changes.
- The fix is intentionally kernel-local because the durable store has no per-claim lease token.

### Verification (verbatim)
```
$ cargo test -p kernel active_job_guard_tracks_in_flight_job_until_drop

running 1 test
test worker_pool::tests::active_job_guard_tracks_in_flight_job_until_drop ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```
$ cargo fmt --all -- --check
```

```
$ cargo clippy --workspace --all-targets -- -D warnings
    Checking kernel v0.0.0 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\crates\kernel)
    Checking screensearch v0.0.0 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.95s
```

```
$ cargo test --workspace
test worker_pool::tests::active_job_guard_tracks_in_flight_job_until_drop ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out # kernel enrichment
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out # store integration
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out # traits binding tests
```

```
$ npm --prefix ui run build
> screensearch-ui@0.0.0 build
> tsc --noEmit && vite build
✓ built in 1.48s
```

```
$ npm --prefix ui run lint
> screensearch-ui@0.0.0 lint
> eslint .
```

```
$ git diff --exit-code -- ui/src/bindings
```

---

## Pass — 2026-06-23 — P2 capture hardening (`codex/fix-p2-capture-hardening` branch)

### Implemented
- **Capture lifecycle supervision** — `run_capture_loop` now returns `StopRequested` vs
  `SourceShutdown`. `Kernel::start_capture` wraps the loop, clears the live capture handle on an
  unexpected source shutdown, and emits `capture = Error` instead of leaving stale `Ready` readiness.
  User Stop still maps to `Disabled`.
- **OCR-unavailable start guard** — if WinRT OCR cannot be created, the app still boots, but
  `capture_control(Start)` fails before opening WGC with `capture = Unavailable`. The defensive
  `UnavailableOcr` now returns an error if ever reached, so it cannot silently write empty OCR rows.
- **Backend settings sanitizer** — `kernel::settings` clamps numeric settings on load and save,
  matching the Settings UI bounds. Malformed persisted values such as `capture.diff_threshold = NaN`
  become safe finite values before the capture config is built.
- **Regression coverage** — added tests for source shutdown readiness cleanup, OCR-unavailable start
  refusal/no empty rows, and persisted/direct numeric settings sanitization.

### Interface Review
- No schema, IPC, `ts-rs`, or trait signature changes.
- `07` OCR fallback wording updated to the stricter start-block behavior.
- Human changelog, AI changelog, and as-built architecture docs updated.

### Verification (verbatim highlights)
```
$ cargo test -p kernel --test pipeline
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

```
$ cargo test -p kernel --test settings
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

```
$ cargo clippy --workspace --all-targets -- -D warnings
    Checking screensearch v0.0.0 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.08s
```

```
$ cargo test --workspace
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s # store integration
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s # traits bindings
```

```
$ npm --prefix ui run build
> screensearch-ui@0.0.0 build
> tsc --noEmit && vite build
✓ built in 1.52s
```

```
$ cargo test -p capture --test wgc_smoke -- --ignored
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s

$ cargo test -p ocr -- --ignored
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

$ cargo test -p screensearch --test e2e_capture -- --ignored
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.55s
```

---

## Pass — 2026-06-23 — PR #16 review comment follow-up (`codex/fix-p2-capture-hardening` branch)

### Review Thread Addressed
- **`crates/kernel/src/lib.rs` capture supervisor race** — Gemini flagged that the unexpected-source-
  shutdown supervisor cleared the capture handle, dropped the capture mutex, and then published
  `capture = Error`. A fast restart could acquire the mutex in that gap, publish the new session as
  `Ready`, and then be overwritten by the old loop's error.

### Resolution
- Kept the existing generation-id guard, but now the supervisor keeps the capture mutex held while it
  clears the stale handle and publishes `capture = Error`. This preserves the established lock order
  (`capture` mutex before readiness lock) and prevents a new `start_capture` from entering until the
  old source-shutdown state is fully published.

### Verification (verbatim)
```
$ cargo fmt --all -- --check
```

```
$ cargo clippy --workspace --all-targets -- -D warnings
    Checking kernel v0.0.0 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\crates\kernel)
    Checking screensearch v0.0.0 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.98s
```

```
$ cargo test -p kernel --test pipeline
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

```
$ cargo test --workspace
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s # kernel enrichment
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s # kernel pipeline
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s # kernel settings
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s # store integration
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s # traits bindings
```

---

## Pass — 2026-06-23 — P3 deferred enrichment hardening (`codex/review-p3-deferred-enrichment` branch)

### Implemented
- **Provider-slot worker startup** — `kernel::worker_pool` now stores both the embedder and vision
  provider in shared runtime slots. Worker claim kinds are built on each poll: `EmbedText` /
  `EmbedImage` only when an embedder is attached and the matching setting is enabled; `VisionTag`
  once the vision provider is attached. `attach_embedder` and `attach_inference` both call the same
  idempotent `start_workers`, and the startup stale-running recovery still runs before the first pool
  spawn.
- **Vision-only regression fix** — `vision_tag` jobs now drain when inference is attached even if
  embeddings are disabled or unavailable. Added `vision_jobs_drain_when_embeddings_disabled`.
- **Backend search limit clamp** — `store::search` normalizes `SearchQuery.limit` to `1..=100` and
  caps the per-arm candidate pool at 500, matching the Recall UI and protecting direct IPC callers.
  Added `hybrid_search_clamps_excessive_limit` and extended zero-limit coverage.
- **Docs** — updated `CHANGELOG.md`, this build review, `08_CHANGELOG_AI.md`,
  `docs/ARCHITECTURE.md`, `07_KNOWN_GAPS.md`, the composition-root module comment, and the UI
  `job_progress` comment.

### Failing-first checks
```
$ cargo test -p kernel --test enrichment vision_jobs_drain_when_embeddings_disabled
test vision_jobs_drain_when_embeddings_disabled ... FAILED
worker pool did not drain the vision_tag job without an embedder
```

```
$ cargo test -p store --test store hybrid_search_clamps_excessive_limit
test hybrid_search_clamps_excessive_limit ... FAILED
assertion `left == right` failed
  left: 150
 right: 100
```

### Interface Review
- No schema, IPC, `ts-rs`, or trait signature changes.
- Known gap #8 (vector-arm tight-window post-KNN recall caveat) remains intentionally open.

### Hallucinated / corrected
- The first version of the new kernel fixture passed the app-data root to `Kernel::new`; the kernel
  expects the frames directory and derives app-data from its parent. Corrected the test fixture to
  pass `data_dir.join("frames")`.
- Clippy rejected the candidate-pool `.max().min()` form as `manual_clamp`; changed it to
  `clamp(50, MAX_CANDIDATE_POOL)`.

### Verification (verbatim status)
```
$ cargo fmt --all -- --check
```

```
$ cargo clippy --workspace --all-targets -- -D warnings
    Checking store v0.0.0 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\crates\store)
    Checking screensearch v0.0.0 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\src-tauri)
    Checking kernel v0.0.0 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\crates\kernel)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.51s
```

```
$ cargo test -p kernel --test enrichment
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```

```
$ cargo test -p store --test store
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

```
$ cargo test --workspace
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s # kernel enrichment
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s # store integration
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s # traits bindings
```

```
$ npm --prefix ui run build
> screensearch-ui@0.0.0 build
> tsc --noEmit && vite build
✓ built in 2.05s
```

```
$ npm --prefix ui run lint
> screensearch-ui@0.0.0 lint
> eslint .
```

```
$ git diff --exit-code -- ui/src/bindings
```

## Pass — 2026-06-24 — Vision scheduler: configurable batch size + pending-job dedup (`fix/vision-batch-size-dedup` branch)
- **Implemented:** Made the timer/idle vision batch size a user setting and stopped the scheduler
  re-enqueuing frames whose `vision_tag` job is already in flight (resolving the user-reported "count
  stuck at 20 for both timer and idle, no setting" and the timer+idle double-enqueue). New setting
  `Settings.enrich_vision_batch_size` (default 20, clamp 1–500) wired through load/save/sanitize and
  read fresh by the scheduler each run; new `NOT EXISTS` guard on `jobs(kind='vision_tag', state IN
  ('pending','running'))` in `Store::untagged_frame_ids`; "Frames per run" UI field in
  `ScheduleControl`. (Resolves the batch-size and pending-job-dedup threads of `07` #19; logged as
  `06` patch #4.)

  ```
  $ cargo fmt --all -- --check
  FMT_OK
  ```

  ```
  $ cargo clippy --workspace --all-targets -- -D warnings
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.79s
  ```

  ```
  $ cargo build --workspace
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.42s
  ```

  ```
  $ cargo test -p store --test store
  running 44 tests
  test untagged_frame_ids_excludes_tagged_and_honors_range ... ok
  test untagged_frame_ids_excludes_in_flight_vision_jobs ... ok
  test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
  ```

  ```
  $ cargo test -p kernel --test settings
  running 5 tests
  test round_trips_non_default_values ... ok
  test save_settings_persists_sanitized_numeric_values ... ok
  test load_settings_sanitizes_persisted_numeric_values ... ok
  test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
  ```

  ```
  $ cd ui && npm ci && npm run lint && npm run build
  > eslint .
  > tsc --noEmit && vite build
  ✓ built in 1.67s
  ```

  ```
  $ git diff --exit-code -- ui/src/bindings   # clean once the regenerated Settings.ts is committed
  ```
- **Skipped / deferred:** The 60s timer cadence is user-configured (`enrich_vision_timer_interval_ms`),
  not a bug — left untouched. Completed jobs accumulating in the `jobs` table (no purge) is a
  separate pre-existing concern — not addressed here.
- **Hallucinated / corrected:** None. The timer+idle firing ~1ms apart in the user's logs was a
  benign coincidence (an idle transition landing on a timer tick), not a coupling bug; the real
  defect it exposed was the missing pending-job dedup, now fixed.
- **Broke / regressed:** None. Adding the `Settings` field required updating the full-struct test
  literal in `crates/kernel/tests/settings.rs`; bindings regenerated via `cargo test`.

## Pass — 2026-06-24 — PR #23 review follow-up (`fix/vision-batch-size-dedup` branch)
- **Implemented:** Addressed all three actionable review threads (Gemini, Codex; Claude approved):
  (1) exclude `dead` `vision_tag` jobs from `untagged_frame_ids` so a poisoned/dead-lettered frame
  isn't re-enqueued forever (Gemini, high); (2) forward migration v2 adds
  `idx_jobs_frame_kind_state` on `jobs(frame_id, kind, state)` so the dedup `NOT EXISTS` is
  index-backed (Gemini/Claude); (3) the timer/idle producers share a `tokio::sync::Mutex` across
  read-then-enqueue, making their select-and-insert atomic and closing the simultaneous-wake
  double-queue race (Codex, P2); (4) documented `enrich.vision_batch_size` in `03` §8 (Codex, P1).

  ```
  $ cargo clippy --workspace --all-targets -- -D warnings
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.97s
  ```

  ```
  $ cargo test -p store
  running 44 tests
  test open_in_memory_migrates_to_latest_schema_version ... ok
  test untagged_frame_ids_excludes_in_flight_vision_jobs ... ok
  test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
  ```

  ```
  $ cargo test --workspace        # every crate: 0 failed
  ```
- **Skipped / deferred:** Did not add a DB-level UNIQUE constraint + conflict-ignored `enqueue_job`
  for cross-producer (on-demand-vs-scheduler) atomicity — it would change `enqueue_job` semantics for
  the embed lanes; the scheduler-scoped mutex covers the reported timer+idle scenario and the residual
  overlap stays bounded by `insert_vision`'s idempotent upsert. Jobs-table purge remains out of scope.
- **Hallucinated / corrected:** None. Verified each finding against the code before acting — the
  dead-job loop, the missing index, and the read-then-insert race were all real.
- **Broke / regressed:** None. The v2 migration is forward-only (`schema_version` 1→2); existing DBs
  add the index on next open.

---

## Pass — 2026-06-24 — Sidecar memory tuning + user settings (`feat/sidecar-memory-tuning` branch)

### Implemented (with verbatim verification)
- **Pinned context + KV quantization + flash attention for the sidecar.** `build_args`
  (`crates/inference/src/supervisor.rs`) now emits `--ctx-size`, `--flash-attn`, and
  `--cache-type-k/-v`, gated by a one-shot `--help` capability probe (`crates/inference/src/flags.rs`,
  `probe_caps`/`parse_caps`) so only flags the bundled (auto-updating) Vulkan binary accepts are
  passed; quantized KV is emitted only when flash attention ends up active. New
  `traits::SidecarParams` (built from `Settings`) threads `ngl/device/ctx_size/kv_cache_type/
  flash_attn` through both providers → `models::resolve_spec` (substitutes the `0 = auto` context
  sentinel with vision 4096 / answer 8192) → `ModelSpec` → `build_args`, and the new fields join
  `needs_restart` so a settings change relaunches on the next request. Three new `Settings` fields
  (`sidecar_ctx_size`, `sidecar_kv_cache_type: KvCacheType`, `sidecar_flash_attn: FlashAttnSetting`)
  are wired through `kernel::settings` load/save/sanitize and the Settings → Sidecar UI panel.
- **Automated gates:**
  ```text
  cargo fmt --all -- --check                              → exit 0
  cargo clippy --workspace --all-targets -- -D warnings   → exit 0
  cargo build --workspace                                 → Finished (exit 0)
  cargo test --workspace                                  → all suites ok (exit 0)
  cd ui && npm run lint && npm run build                  → eslint 0; tsc+vite built (exit 0)
  git diff ui/src/bindings → Settings.ts updated + KvCacheType.ts / FlashAttnSetting.ts generated
  ```
- **Live GPU proof** (RTX 5060 Ti, bundled Vulkan `llama-server` build 9754, default answer model,
  baseline VRAM 1660 MiB):
  ```text
  Untuned (old args  --model M -ngl 99):  n_ctx_seq 118272  → peak VRAM 14082 MiB
  Tuned (--ctx-size 8192 --flash-attn on --cache-type-k q8_0 --cache-type-v q8_0):
                                          n_ctx_seq   8192  → peak VRAM  4253 MiB
  → ~9.8 GB / ~70% reduction; sidecar exited cleanly, VRAM returned to 1660 MiB.
  ```

### Skipped / deferred (explicitly out of the approved plan)
- **Embedder idle-unload** (RAM lever): the ~0.5–1 GB resident fastembed model stays loaded — the
  user chose a VRAM-only scope.
- **`--parallel 1`**: `llama-server` auto-picks `n_parallel = 4` but with `kv_unified = true`, so the
  shared KV pool is sized by context, not multiplied by slots; context pinning already captures the
  win. Noted as a possible future lever.
- **`--batch/--ubatch`, `--no-mmap`/`--mlock`**: throughput trade or wrong direction for RAM.

### Hallucinated / corrected
- None. The root cause was confirmed against the real binary: its `--ctx-size` help reads
  `(default: 0, 0 = loaded from model)` and the default models report `n_ctx_train = 262144`, so the
  untuned launch genuinely allocated a ~118 k-token KV cache.

### Still risky
- KV quantization is gated behind flash attention and the `--help` probe, and degrades to f16 if
  unsupported; if a future Vulkan build accepts `--cache-type-k` in `--help` but rejects q8_0 K at
  model load, the escape hatch is the new `f16` KV setting. The live proof above used the currently
  bundled build, where q8_0 K+V loaded cleanly.

---

## Pass — 2026-06-24 — Sidecar memory tuning: PR #25 review round

Addressed the actionable findings from the `@claude` and `@codex`/`gemini-code-assist` reviews on
PR #25 (additive follow-up commit; no force-push, per ship-it).

- **`FlashAttnSetting::Auto` now defers to llama.cpp** (`supervisor.rs::push_flash_attn`). On a
  value-taking binary, `Auto` emits `--flash-attn auto` and `On` emits `--flash-attn on` — previously
  both emitted `on`, making the two settings indistinguishable. `auto` still counts as flash-active
  (it resolves to on for every build advertising the flag), so it keeps unlocking quantized KV. New
  test `build_args_distinguishes_auto_from_on_flash_attn`.
- **`probe_caps` no longer blocks the async executor** (`src-tauri/src/lib.rs::init_inference`). The
  `llama-server --help` syscall runs on `tokio::task::spawn_blocking`, falling back to
  `SidecarCaps::conservative()` if the join fails.
- **Silent KV-quant fallback now warns** (`supervisor.rs::push_kv_cache`). When quantization is
  configured but the binary advertises neither `--cache-type-k` nor `--cache-type-v`, it logs a warn
  instead of silently dropping to f16.
- **Probe recognizes a parenthesised value set** (`flags.rs::flash_line_takes_value`): added `(on` to
  the value hints so a future `--flash-attn (on/off/auto)` help line is detected as value-taking. New
  test `parses_parenthesised_value_taking_flash_attn`.
- **Doc nit:** the Settings "Context size" hint now states that clearing the field also means auto.

### Declined / not in this round
- **Stale-caps-on-auto-update (nit):** documented as a code comment rather than re-probing on every
  spawn — the binary download path is idempotent (skip-if-present), so a running app keeps one binary
  (hence one cap set) until restart.

### Verification (verbatim)
- UI: `npm run lint` exit 0; `npm run build` exit 0 (`✓ built in 1.43s`).
- Rust: `cargo fmt --all -- --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings`
  exit 0; `cargo test --workspace` all crates **0 failed**; `cargo test -p inference` **57 passed, 0
  failed** (incl. the two new tests).
- Binding guard: `git diff --exit-code -- ui/src/bindings` clean (no IPC type changes this round).

---

## Pass — 2026-06-24 — Sidecar memory tuning: PR #25 review round 2

Second review pass: `@codex` (`chatgpt-codex-connector`) posted inline findings and `@claude`'s
follow-up verified round 1 and surfaced two more diagnostic gaps. `gemini-code-assist` approved
("ready for merge"). Addressed the three remaining actionable items (additive commit).

- **Accurate KV-downgrade diagnostics** (claude `supervisor.rs:616`). `push_flash_attn` now returns a
  `FlashState` enum (`Active` / `BinaryUnsupported` / `UserDisabled`); `push_kv_cache` matches on it
  so the warn distinguishes "this build has no `--flash-attn`" from "you turned flash attention off"
  — previously both printed "build does not enable".
- **No silent drop of an explicit `On`** (claude `supervisor.rs:581`). `(Unsupported, On)` now logs a
  warn that the requested flag is unavailable and ignored, instead of returning `false` silently. New
  test `build_args_drops_explicit_on_flash_when_binary_unsupported`.
- **Sanitize before hot-apply** (codex + claude `lib.rs:479`). `set_settings` clamps the incoming
  `Settings` once up front, so the `SidecarParams` handed to the live providers match the values
  persisted to the DB; a direct IPC call with an out-of-range `sidecar_ctx_size` can no longer run the
  raw value until restart.

### Already addressed earlier in the round (replied + resolved)
- Blocking `--help` syscall on the executor (claude `flags.rs:83`) — handled by the round-1
  `spawn_blocking` wrap at the `probe_caps` call site.
- Flash-attn `auto` treated as forced-on (codex, outdated thread) — handled by the round-1 Auto→`auto`
  split.

### Verification (verbatim)
- Rust: `cargo fmt --all -- --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings`
  exit 0; `cargo test -p inference` **58 passed, 0 failed**; `cargo test --workspace` all crates **0
  failed**.
- No UI or IPC-type changes this round (no `ts-rs` regeneration; binding guard stays clean).

---

## 0.2.0 PR1 — 2026-06-25 — Specs contract (no code)

**Branch:** `docs/0.2.0-pr1-specs`. First PR of the 0.2.x arc (`docs/0.2.0.md`, `02 §5b`).

### Implemented
- Wrote the attention-first content-text + Recall-reports contract into `/specs/`: `02 §5b` (0.2.x
  arc / P6), `03 §3b` (raw vs content text, window semantics, spans, roles, suppression, new types),
  `03 §4` (`frame_text` + `frame_text_fts`, `text_spans`, `chrome_text_catalog`; `schema_version`
  2→3), `03 §7` (`generate_report`), `03 §8` (0.2.x settings keys), `03 §8b` (reports), `UI_REFERENCE`
  (Recall Search/Ask/Reports + toggle + premade cards + all five states), `07` (#47–#51 deferrals +
  PR2→PR3 interim), `04` (reading + build order), `CHANGELOG.md` + `08`.
- Per the user's decision, `03` carries the **full** authoritative contract (concepts + DDL + types),
  so PR2/PR3 implement `03` verbatim rather than re-deriving from the roadmap.

### Skipped / deferred
- No code, no schema migration, no OCR rewrite, no binding regen, no UI build — those are PR2–PR6.
- Settings defaults for suppression thresholds / report depths are listed as **provisional** in
  `03 §8`; PR2/PR3 finalize and tune them.

### Hallucinated / corrected
- An exploration agent proposed a new `specs/10_CONTENT_TEXT_DESIGN.md` file; rejected as out of
  scope — the content-text design lives **inside** `03`, per the roadmap's file list.

### Broke / regressed
- Nothing — specs-only; no code paths touched.

### Still risky
- The provisional settings defaults and the FTS-for-`include_chrome` choice (raw FTS vs role-filtered
  `text_spans` FTS) are explicitly left for PR2 to finalize; PR2 must document the decision.

### Review round 1 (PR #30, 2026-06-25) — Codex + Gemini auto-review
Four actionable threads, all addressed in the contract (still specs-only):
- **Gemini (`03 §3b`):** `suppress_reason` → `Option<SuppressReason>`, dropped the redundant in-enum
  `None` variant (maps directly to the nullable `text_spans.suppress_reason` column).
- **Gemini (`03 §4`):** added `CHECK` constraints to the `frame_text` / `text_spans` enum columns
  (`primary_source`, `source`, `role`, `is_searchable`, `suppress_reason`) so PR2's DDL enforces
  valid values at the DB layer.
- **Gemini (`03 §8`/`§8b`):** lowered `reports.map_reduce_min_frames` default 40→20 — at ~400 tok/frame
  the worst-case single-pass fit is ~20 frames, so 40 risked dropping frames (20–39) before map-reduce
  triggered; `§8b` now references the threshold instead of restating a contradictory range.
- **Codex (`04 §1`):** `04` now makes `docs/0.2.0.md` mandatory reading, but the roadmap's Status said
  "no implementation started / PR1 not done" — a future PR2 agent under stop-at-ambiguity would have
  hit a contradiction. Updated `docs/0.2.0.md` Status to record PR1 complete, PR2 next.

## 0.2.0 PR2 — 2026-06-25 — Text-signal data model + OCR spans (`feat/0.2.0-pr2-text-signal`)

### Implemented
- **OCR span geometry** (`crates/ocr`): `recognize_blocking` walks WinRT `Lines().Words()` → one
  `TextSpan`/word; pure `normalize_rect` maps `BoundingRect` (pixels) to `[0,1]` (clamped so
  `x+w≤1`, `y+h≤1`; zero-area frame → zero box). `CONFIDENCE_UNKNOWN` sentinel kept (no per-word
  confidence). PR2 spans: `source=ocr`, `role=unknown`, searchable, no suppression.
- **Schema v2→v3** (`crates/store/src/schema.rs`, forward-only): `frame_text` (+`frame_text_fts`
  over `content_text`, +`frame_text_raw_fts` over `raw_text` — both external-content, v1 trigger
  style), `text_spans` (CHECK-constrained), `chrome_text_catalog`; all per-frame tables
  `ON DELETE CASCADE`. Legacy `ocr_text`+FTS **dropped** (clean DB). `UNFILTERED_FILTER_VERSION = 0`
  marks the interim passthrough.
- **Types** (`crates/traits`): `TextSource`/`TextRole`/`SuppressReason` (ts-rs-exported +
  `as_db_str`/`from_db_str`), internal `TextSpan`, shared `normalize_text`; `OcrResult.spans`;
  `FrameDetail` (`text` → `raw_text`+`content_text`+`text_source`+`suppressed_text_count`);
  `SearchQuery.include_chrome`.
- **Store wiring**: `insert_ocr` writes `frame_text` (content = raw passthrough, `target_*` from the
  frame's foreground context) + `text_spans` atomically; `frame_enrichment_input`/`ocr_texts`/
  `get_frame`/search FTS + hydrate repoint to `frame_text`/`content_text`; new inherent `frame_spans`
  read for observability. Search: content FTS arm + opt-in raw FTS arm fused via the existing RRF.
- **UI**: `MomentDetail` renders `content_text` + a raw-text disclosure; `Recall` passes
  `include_chrome:false`; bindings regenerated.

### Decisions (user-approved before implementation)
- **D1 drop `ocr_text`** → `frame_text` is the single text store (`03 §4` "legacy ocr_text not
  required going forward"; empty on a clean DB → zero data loss).
- **D2 replace `FrameDetail.text`** (it duplicated `raw_text`).
- **D3 per-word** span granularity (most faithful to "walk `Lines().Words()`"; PR3 regroups).
- **D4 raw-search mechanism = a dedicated raw FTS5 table** (`frame_text_raw_fts`), **not** a
  role-filtered `text_spans` FTS: roles aren't populated until PR3 (a spans-FTS would index only
  `unknown` rows = meaningless), content==raw in PR2 (a spans-FTS would be redundant with content
  FTS), and raw FTS is stable across PR3 (raw_text never filtered). `include_chrome` semantics are
  "also search raw" (`03 §3b`), which raw FTS matches exactly.
- **D5 Recall toggle UI deferred to PR3** — backend `include_chrome` + raw arm land now (no visible
  effect in PR2 since content==raw); PR2's UI scope per `docs/0.2.0.md` is Moment raw-vs-content only.

### Skipped / deferred (correct for PR2)
- No PR3 classifier — `content_text` is a passthrough copy of `raw_text`, no backfill (`07` #51).
- `ASK_TOP_K`/`retrieval.default_top_k` untouched (PR6). `chrome_text_catalog` is created but unused
  until PR3. Suppression-threshold settings remain provisional (`03 §8`).

### Broke / regressed
- Nothing. All existing tests updated for the new `OcrResult.spans` / `SearchQuery.include_chrome`
  fields and the `FrameDetail` text-field replacement; full suite green.

### Still risky / watch in PR3
- **Span volume**: per-word spans on a busy screen = hundreds of `text_spans` rows/frame. Acceptable
  for PR3's classifier (needs word geometry); revisit if storage growth is observed.
- **`frame_text.target_*`** are copied from `frames.app_hint`/`window_title` (foreground = target in
  PR2); PR3 refines true target-window semantics.
- The interim `content_text == raw_text` means PR2 search/embeddings still include chrome until PR3 —
  by design (`07` #51).

### Verification (verbatim)
- `npm ci` (0 vuln) · `npm run lint` clean · `npm run build` → `✓ built in 1.75s`.
- `cargo fmt --all -- --check` clean · `cargo clippy --workspace --all-targets -- -D warnings`
  `Finished` · `cargo build --workspace` `Finished` 33.25s · `cargo test --workspace` all green.
- Store integration **45 passed** (incl. v3 migration, span persistence, delete cascade, raw-arm
  search); ocr `normalize_rect` unit test passed.
- **Live (gated):** `winrt_ocr_recognizes_blank_image ... ok` (bbox ∈ [0,1] asserted) and
  `capture_pipeline_stores_frames_ocr_and_enqueues_embed_jobs ... ok` (3.42s, real WGC+WinRT →
  `frame_text` → `get_frame` → embed jobs).

### Review follow-ups (PR #31, Gemini Code Assist)
Codex and Claude (CI) found no issues; Gemini raised two medium-priority items, both applied:
- **`chrome_text_catalog.suppressed` now `CHECK (suppressed IN (0,1))`** — enforces the documented
  0/1 boolean semantic, matching the `is_searchable` convention in `text_spans` (same v3 migration).
  v3 is unreleased (introduced in this PR), so tightening its DDL is not schema drift; `03 §4`'s
  canonical DDL updated in lockstep to keep spec ↔ code in sync.
- **`normalize_text` allocation** — rewritten to build the collapsed string in one capacity-hinted
  allocation (was: intermediate `Vec` + `join`), keeping the final `to_lowercase()` so casing
  semantics (incl. Greek final sigma) are byte-for-byte unchanged. Hot path: one call per OCR word.

---

## 0.2.0 PR3 — 2026-06-25 — Attention-first text filtering (`feat/0.2.0-pr3-text-filter`)

**Branch:** `feat/0.2.0-pr3-text-filter`. Third PR of the 0.2.x arc (`docs/0.2.0.md` PR3,
`03 §3b/§4/§8`). Replaces PR2's `content_text` passthrough with a real span-aware filter so search,
Ask, and embeddings stop ranking on chrome. The embed worker and search already read `content_text`,
so filtering it changes retrieval with **no** embed-worker or search change.

### Implemented (geometry before repetition; conservative by design)
- **New `crates/textfilter`** (depends on `traits` only — honors the module rule): a pure,
  deterministic `classify(input, catalog, config)`. Groups spans by `line_index` → lines (union bbox
  + centroid), assigns roles in priority order — `system` (short, bottom band `y > 0.95`, rect known
  & outside), `background` (rect known & outside), `chrome` (== normalized `target_window_title`),
  static-chrome candidate (short + not interior, `seen_count+1 ≥ min_seen`), else `content`/`unknown`.
  `content_text` built from kept spans (not string-subtraction); the title is metadata, never
  appended. Never-suppress guardrails: long lines (≥ `chrome_protect_min_chars`) are never catalogued
  or suppressed for repeating; short interior content is never catalogued. Signature =
  `app_hint ⏐ normalized_text ⏐ region_bucket` (sep `U+001F`, empty-string sentinel for null app);
  `region_bucket` = line centroid in an N×N grid.
- **Single-transaction filtered write** (`store::insert_ocr_filtered`): catalog read →
  `textfilter::classify` → filtered `frame_text` insert (`filter_version = 1`, real
  `suppressed_count`) → `replace_text_spans` (classified roles + `line_index`) → catalog upsert, all
  in **one** transaction so the content FTS is written once (no transient unfiltered window a
  concurrent search could match). `insert_ocr` passthrough kept for fakes/fallback.
- **Schema `schema_version` 3 → 4** (forward-only): `MIGRATION_V4` adds
  `text_spans.line_index INTEGER NOT NULL DEFAULT 0`. `reconcile_filter_version` (run once on store
  open) wipes `chrome_text_catalog` + rewrites the `text.catalog_filter_version` watermark when
  `FILTER_VERSION` changes; `text_filter_stats` groups by `target_app_hint` filtered on the current
  version. No backfill of old frames (clean-DB, `07` #51/#55).
- **Capture target rect** (`crates/capture`): per-frame normalized `target_rect` from the foreground
  window's visual bounds (`DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)`, `IsIconic` guard)
  mapped into the captured monitor's `rcMonitor` by the pure `normalize_window_rect` (center-point
  containment; `None` on another monitor / minimized / unresolved — the safe fallback).
- **Settings + UI:** 4 `text.*` settings (defaults: `include_chrome_default` false,
  `chrome_suppress_min_seen` 12, `chrome_protect_min_chars` 48, `chrome_region_buckets` 8) with
  load/save/sanitize clamps; Settings "Text filtering" panel + per-app suppression-rate readout (all
  states); Recall search `include_chrome` toggle; `get_text_filter_stats` command. Decisions logged
  as `07` gaps #52–57.

### Verification (verbatim)
```
$ cargo fmt --all -- --check
FMT CLEAN
```
```
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.04s
```
(fixed one `clippy::doc_lazy_continuation` — a doc line wrapped to a `+`-led continuation markdown
read as a bullet.)
```
$ cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.36s
```
```
$ cargo test -p textfilter
running 6 tests
test tests::default_frame_drops_system_and_background_keeps_content ... ok
test tests::empty_spans_produce_empty_output ... ok
test tests::no_target_rect_never_classifies_background_or_system ... ok
test tests::short_interior_body_is_never_catalogued ... ok
test tests::window_title_echoed_as_body_is_excluded ... ok
test tests::toolbar_becomes_chrome_at_the_seen_threshold ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
```
$ cargo test -p store --test store -- insert_ocr_filtered reconcile_filter
running 2 tests
test reconcile_filter_version_wipes_catalog_on_change ... ok
test insert_ocr_filtered_suppresses_repeated_chrome_after_threshold ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 45 filtered out; finished in 0.01s
```
```
$ cargo test --workspace   # 0 failed across all crates
store (lib 1, integration 47) · traits 43 · textfilter 6 · capture 13 · kernel (lib 2, pipeline 5,
settings 6) · ocr 1 · screensearch_lib 6 · inference (no_orphan 1, reap 3, sidecar 8) · embeddings 1
```
```
$ cd ui && npm run lint && npm run build
> eslint .
> tsc --noEmit && vite build
✓ built
```
```
$ git diff --exit-code -- ui/src/bindings   # clean once the regenerated Settings.ts + AppSuppression.ts are committed
```

### Skipped / deferred (intentional)
- **`CaptureTrigger` not threaded into the filter** — 0.2.0 capture is timer/idle-only, so it would
  be dead single-variant plumbing; the filter is trigger-agnostic. Arrives with event-driven capture
  (`07` #57 / #47, 0.2.1).
- **No backfill** of PR2-era frames — they keep their unfiltered `content_text`/`filter_version`
  (clean-DB assumption, `07` #51/#55).
- **Manual multi-DPI / secondary-monitor `target_rect` check** is the one acceptance item CI cannot
  cover (`07` #54) — to confirm in a live `npm run tauri dev` session.

### Still risky / to watch
- **`target_rect` DPI assumption (highest technical risk).** Correct mapping assumes the process is
  per-monitor-v2 DPI-aware so `GetWindowRect`, `rcMonitor`, and the WGC texture agree in physical
  pixels. A wrong rect can only *under*-suppress (recoverable via `raw_text`/`include_chrome`), never
  silently lose content — but the per-app suppression-rate readout is the alarm to watch on live data.

### Review fixes (PR #32, 2026-06-25)
- **N+1 catalog lookup removed (Claude P1 + Gemini high).** `SqlCatalog::seen_count` issued one
  `SELECT … WHERE signature = ?` per candidate line (10–30 round-trips per OCR frame on the hot
  path). Replaced with `load_chrome_catalog(conn, app_hint)` — a single
  `SELECT signature, seen_count … WHERE app_hint IS ?1` into a `HashMap<String, u32>` (which already
  implements `ChromeCatalog`), passed to `classify`. Scoped to the foreground app because every
  signature a frame can query is built with that `app_hint` prefix; `IS ?1` matches the NULL app.
- **Unknown rect no longer suppresses by repetition (Codex P2).** With `target_rect = None`,
  `centroid_interior` was `unwrap_or(false)`, so *every* short line became a chrome candidate and a
  repeated real body line could be dropped — contradicting the "unknown rect can only under-suppress"
  invariant. Static-chrome cataloguing/suppression now requires a known rect; rect-less short lines
  fall through to `unknown` (kept). New regression test
  `no_target_rect_never_suppresses_even_a_saturated_signature` (textfilter now 7 golden tests).
- **Verify:** `cargo fmt --all -- --check` clean · `cargo clippy --workspace --all-targets -D
  warnings` 0 warnings · `cargo test --workspace` 0 failures (textfilter 7, store 1+47) · bindings
  guard clean (no traits/IPC change).

---

## 0.2.0 PR6 — 2026-06-25 — Recall reports + Ask shortcuts (CGCMR) (`feat/0.2.0-pr6-recall-reports`)

**Branch:** `feat/0.2.0-pr6-recall-reports`. Sixth PR of the 0.2.x arc (`docs/0.2.0.md` PR6,
`03 §7`/`§8`/`§8b`). Adds `generate_report` over the attention-first `content_text`, removes the
hardcoded `ASK_TOP_K`, and ships a Reports UI mode + premade Ask cards. **Core user directive
(2026-06-25): report context must scale with the time range and guarantee temporal coverage, not
just relevance** — a weekly report must cover the *whole* week (every active day), not a flat
single window biased to the most-recent/most-relevant frames. Design = **Calendar-Grid Coverage
Map-Reduce (CGCMR)** (chosen via a multi-agent design panel + adversarial verification); deviations
from the literal `§8b` recorded in `06` patch #5.

### Implemented (coverage by construction; VRAM-flat)
- **New `crates/kernel/src/reports.rs`** — pure planner + I/O orchestrator over `dyn Store` +
  `dyn AnswerProvider` (GPU-free, unit-testable). `n_ctx` stays **flat 8192**; the *number* of
  8192-bounded passes scales with the range, not the window (KV-cache VRAM is unchanged; no sidecar
  relaunch). Flow: **(coverage path)** range → per-calendar-day grid → one `timeline_buckets` density
  probe → adaptive `plan_depth` (per-active-period budget, floored at `MIN_FRAMES_PER_PERIOD`, capped
  at the period count, floor-wins over the global cap) → per-period even ASC sample → MAP (one
  summarize pass per active day) → bounded hierarchical REDUCE (consecutive `REDUCE_FANOUT`=6 groups,
  time order preserved) → FINAL report pass. **(relevance path)** Custom-with-prompt →
  `hybrid_search(prompt, time_range)` → token-budget batches → same map-reduce. **Fast path:** a
  single group ≤ `map_reduce_min_frames` skips straight to one final pass (Daily common case = 1
  call). **Honest-empty:** zero frames in range → no-evidence report with **zero** sidecar calls.
  Cooperative `AtomicBool` cancel checked between passes; progress callback emits `summarizing day
  k/A` / `reducing`. Structural bounds are named constants (`07` #60).
- **ASC temporal sampler** (`store::sample_frames_in_range`, added to the `Store` trait) — even
  selection across `[start,end)` by `captured_at ASC` via windowed `row_number()`/`count()` (keep
  rows where `(rn-1) % max(1, total/limit) == 0`), all rows when `count ≤ limit`. `frames_in_range`
  is newest-first capped (unusable for even coverage), so this is a new primitive.
- **Summarize pass primitive** (`inference::answer`, added to `AnswerProvider`) —
  `summarize(system_prompt, instruction, context, opts) -> (String, Vec<i64>)`, non-streaming
  (collected, thinking dropped via `ThinkSplitter`). Reuses a `pack_context` helper **extracted** from
  `build_messages` so Ask output is byte-identical (its 8 tests pass unchanged). Three report prompts
  (map / reduce / final), each with the anti-fabrication + honest-empty clause. Default trait impl
  drives the existing `answer()` and drains the channel concurrently (`tokio::join!`) so the kernel
  fake + non-sidecar providers work. `answer_model_label()` fills the footer.
- **IPC + commands** — `ReportKind`/`ReportRequest`/`ReportResponse`/`ReportProgress` (ts-rs, i64
  fields annotated, added to the `no_bigint_in_ipc_types` guard). `generate_report` resolves the local
  range + label from the kind, builds `ReportConfig` from settings, registers an `AtomicBool` in
  `report_tasks`, emits `report_progress` events, and maps `ReportOutput → ReportResponse` (incl.
  `model`). `cancel_report(request_id)` flips the flag. `AskRequest` gains `top_k: Option<u32>`; `ask`
  reads `retrieval.default_top_k` (`ASK_TOP_K` const removed).
- **Settings (the four §8 keys, no surface bloat)** — `retrieval.default_top_k` (8),
  `reports.daily_top_k` (40, **per-active-period budget**), `reports.weekly_top_k` (200, **global
  cap**), `reports.map_reduce_min_frames` (20); struct + `Default` + load/save + `sanitize_settings`
  clamps (1–100 / 1–1000 / 1–2000 / 1–1000), mirrored in the Settings UI sanitizer.
- **UI** — `routes/Recall.tsx` gains a third **Reports** mode (search/ask/reports). `ReportBuilder`
  (Daily/Weekly/Custom; computes the concrete **local** `TimeRange` in JS so the backend never does
  TZ math; Custom = date inputs + optional prompt). `ReportView` (markdown via existing
  `react-markdown`+`remark-gfm`+`prose-deck`; capped citation chips + "+N more"; Copy; `.md` download;
  honest footer: model · ≈est tokens · passes · covered/total periods · summarized/sampled frames ·
  truncated notice). `CitationTile` extracted from `AnswerStream` into a shared component (reused by
  both). `PromptCardGrid` (5 premade Ask cards: Day Recap, Standup Update, Time Breakdown, Top of
  Mind, AI Habits) in the Ask idle state. `useReport` hook mirrors `useAsk` (idle/generating/done/
  error + live progress + cancel). Settings gains a "Reports & retrieval" panel. Bindings regenerated.

### Decisions (user-confirmed before implementation)
- **Weekly / large ranges use representative temporal coverage** (per-period grid + even intra-period
  stride), not most-recent-N.
- **Custom report *with* a prompt drives semantic retrieval** (`hybrid_search`); Daily / Weekly /
  Custom-*without*-prompt use grid coverage.
- **`n_ctx` stays flat 8192** — the ctx-boost alternative was considered and rejected (zero coverage
  benefit, forces a relaunch, no iGPU VRAM probe). The pass *count* scales instead.

### Skipped / deferred (intentional)
- **Scheduled / saved reports** — out of 0.2.0 scope (`07` #50); on-demand only, no saved-report
  table.
- **Real sidecar token usage in the footer** — the streaming `answer()` path doesn't surface
  prompt/eval token counts to the kernel, so the footer shows an `est_tokens` estimate plus the
  *exact* structural counts that matter for trust (`07` #61). `build_messages` remains the hard
  per-pass 8192 safety net regardless of the estimate.
- **True civil-calendar day slicing** — the internal grid uses fixed `DAY_MS`; the UI range is
  wall-clock-correct, so DST only shifts an internal boundary ±1 h on ≤2 days/yr (cosmetic, `07` #59).

### Broke / regressed
- Nothing. `ASK_TOP_K` removal is covered by `retrieval.default_top_k` + the per-request `top_k`
  override; Ask output is byte-identical (the extracted `pack_context` keeps the existing 8 Ask tests
  green). No schema change (the sampler is a query; the report types are additive IPC).

### Still risky / to watch
- **Token estimate vs reality** — `est_tokens` (`chars/4`) gates the reduce single-pass decision; if a
  real corpus packs denser than the heuristic, `build_messages` truncates an oversized chunk (citations
  reflect what fit) rather than overflowing — but a markedly low estimate could trigger one extra
  reduce level than strictly needed. Conservative-by-design; watch on live data.
- **Live GPU end-to-end is not in CI** — the orchestrator is fully unit-tested with a fake Store +
  fake `AnswerProvider`, but a real weekly report over a seeded multi-day DB at ctx 8192 (markdown
  mentions each active day; citations resolve; no relaunch) is a manual `npm run tauri dev` acceptance
  item, like the rest of the sidecar path.

### Verification (verbatim)
```
$ cd ui && npm run lint
> eslint .
   (exit 0, no output)
$ cd ui && npm run build
> tsc --noEmit && vite build
✓ 407 modules transformed.
✓ built in 1.44s          (exit 0)
```
```
$ cargo fmt --all -- --check          # exit 0
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.54s   # exit 0, 0 warnings
$ cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.43s   # exit 0
$ cargo test --workspace          # 0 failed across all crates (exit 0)
```
New/affected tests (all pass):
- **`crates/kernel` reports (10)** — `plan_depth_gives_every_active_period_its_budget`,
  `plan_depth_floor_wins_over_global_cap_on_long_ranges`,
  `grid_size_is_one_per_day_capped_at_max`, `split_chunks_batches_every_chunk_without_dropping`,
  `fits_single_pass_detects_overflow`, `daily_small_range_uses_single_pass`,
  `weekly_covers_every_active_day_and_cites_first_and_last`,
  `reduce_overflow_preserves_all_days_via_hierarchical_reduce`,
  `empty_range_is_honest_with_no_sidecar_call`, `cancellation_between_passes_returns_err` (real
  `SqliteStore` + `FakeAnswer`). **`crates/kernel` settings round-trip** extended with the four keys.
- **`crates/store` sampler (4)** — `sample_spreads_evenly_and_includes_the_earliest_frame`,
  `sample_returns_all_when_count_under_limit`, `sample_caps_at_limit_within_window`,
  `sample_degenerate_windows_are_empty`.
- **`crates/inference` (3 new)** — `report_summary_messages_use_the_given_system_prompt_and_tag_frames`,
  `report_summary_drops_overflow_and_cites_only_what_fit`,
  `report_model_label_extracts_the_gguf_filename` (Ask's 8 `pack_context`/`build_messages` tests
  unchanged & green).
```
$ git status --short -- ui/src/bindings
 M ui/src/bindings/AskRequest.ts
 M ui/src/bindings/Settings.ts
?? ui/src/bindings/ReportKind.ts
?? ui/src/bindings/ReportProgress.ts
?? ui/src/bindings/ReportRequest.ts
?? ui/src/bindings/ReportResponse.ts
# regenerated by cargo test; committed with the PR so the CI binding guard stays clean.
```

### Review fixes (PR #33, 2026-06-25)
All five actionable findings were bot-raised (Gemini ×2, Claude ×1, Codex ×2); each addressed.
- **N+1 over the period grid (Gemini, medium).** The coverage path called a per-period helper that
  ran its own `ocr_texts` query for every active day. Now it samples each period's frames (one
  windowed query per period — inherent to per-period coverage), collects **all** ids, and hydrates
  them with a **single** bulk `ocr_texts` read; the `sample_period_chunks` helper is removed.
- **Redundant reduce call on a trailing single node (Gemini, medium).** When `nodes.len()` isn't a
  multiple of `REDUCE_FANOUT`, the last fan-in group could be size 1; it now passes through untouched
  instead of spending a model call to "combine" one summary. (Citations are unaffected — they come
  from the MAP union, not the reduce passes.) The reduce-overflow test's pass-count comment updated
  (7 map + 1 reduce + 1 final = 9).
- **Prompted reports silently capped at 100 frames (Codex, P2).** `Store::hybrid_search` normalized
  every request through `MAX_SEARCH_LIMIT = 100` (the search-UI max), so a Custom-with-prompt report
  over a long range never considered more than 100 frames despite `reports.weekly_top_k` (up to 2000).
  Raised the backend ceiling to 2000 and **decoupled** `MAX_CANDIDATE_POOL` from it (so a big report
  limit can't explode per-arm scans); the Recall search UI is unaffected (it sends its own
  `SEARCH_LIMIT = 100`). New unit test `normalized_limit_clamps_to_the_backend_ceiling`; the
  integration `hybrid_search_clamps_excessive_limit` updated (150 seeded < ceiling → all 150 return).
- **Planner budgeted against a fixed 8192, not the actual context (Codex, P2).** `summarize` budgets
  against the resolved `sidecar.ctx_size`, which the user can lower; the planner assumed 8192, so a
  smaller window could make `fits_single_pass`/batch splitting think summaries fit while the final
  prompt silently truncates. Added `AnswerProvider::answer_context_budget()` (default `None`;
  `AnswerSidecar` returns its resolved answer-lane `ctx_size`), and the planner now budgets against
  it (falling back to the pinned default for the test fake). `fits_single_pass`/
  `split_chunks_into_batches` take `ctx_tokens`; `fits_single_pass_detects_overflow` extended to
  assert a lowered window tightens the gate.
- **Misleading default-`summarize` doc (Claude).** The default trait impl does **not** forward
  `system_prompt` (it falls back to `answer()`'s grounding prompt); the doc-comment now says so
  explicitly instead of "any provider works out of the box". No code change — `AnswerSidecar`
  overrides and honors the prompt.
- **Verify:** `cargo fmt --all -- --check` exit 0 · `cargo clippy --workspace --all-targets -- -D
  warnings` exit 0, 0 warnings · `cargo build --workspace` exit 0 · `cargo test --workspace` **0
  failed across all crates** · `git diff --exit-code -- ui/src/bindings` clean (no IPC type changed
  this round — the new `answer_context_budget` is a Rust trait method, not a wire type).

### Review fixes — round 2 (PR #33, 2026-06-25)
A second bot pass (Codex ×3, Claude ×2) re-reviewed the round-1 fix commit; round-1 findings were not
re-raised (confirmed resolved). The five new findings, each addressed:
- **Sampler returned ~half the quota just over the limit (Codex, P2 — `crates/store/src/frames.rs`).**
  `sample_frames_in_range` used a `ceil(total/limit)` integer stride, which doubles to 2 the moment
  `total > limit` — 41 frames at `limit = 40` yielded 21 rows, so reports summarized far fewer frames
  than the depth settings allow. Replaced with **even-rank bucketing** (`rn = 0 OR rn*limit/total <>
  (rn-1)*limit/total`): the first row of each of `limit` even buckets, returning exactly
  `min(total, limit)` rows still spread across the window. Existing tests unchanged (the 12→4 case is
  identical); new regression `sample_returns_full_quota_when_just_over_limit` (41 → 40, not 21).
- **`truncated` false-positive on the relevance path (Claude — `crates/kernel/src/reports.rs`).** For a
  prompted Custom report `total_in_range` is the **pre-hydration** hit count; `hydrate_hits` drops
  empty-text hits (no evidence), so `frames_sampled < total_in_range` fired — and the footer showed
  "range trimmed to fit" — even when nothing useful was lost. The prompt path now flags `truncated`
  only when the search cap was actually hit (`total_in_range >= weekly_top_k`, i.e. more relevant
  frames likely existed); the coverage path keeps its real-frame-count comparison.
- **Empty reports required the sidecar (Codex, P2 — `src-tauri/src/lib.rs`).** The command resolved an
  `AnswerProvider` before the empty-range check, so on first launch (while `llama-server` is still
  resolving/downloading) an honest no-evidence report failed with "inference sidecar not ready yet".
  It now probes `frames_in_range(.., 1)` first and returns a hand-built empty `ReportResponse` (new
  `empty_report_response`, body identical to the kernel's `empty_output`, `passes == 0`) **without**
  acquiring the provider; a probe error still falls through to surface the real error.
- **`frames_summarized` doc-comment inaccurate when the citation cap fires (Claude —
  `crates/traits/src/ipc.rs`).** `frames_summarized` is the full MAP-union size, which can exceed
  `cited_frame_ids.len()` once the union passes `MAX_REPORT_CITATIONS` (e.g. a long range where the
  per-period floor pushes it over). Doc now says "may exceed `cited_frame_ids.len()` when the citation
  list is capped" instead of claiming equality.
- **DST-incorrect local-day bounds (Codex, P2 — `ui/.../ReportBuilder.tsx`).** Daily/Weekly/Custom
  ranges added a fixed `DAY_MS`, so on a 23 h/25 h transition day the range could include or drop an
  hour. Bounds are now built from local calendar components (`localMidnight(now, ±n)` /
  `localDateMidnight(value, +1)`); the JS `Date` constructor normalizes day overflow, landing on the
  true next local midnight. Narrows gap #59 to the kernel's internal per-period split only.
- **Verify:** UI `npm run lint` + `npm run build` clean · `cargo fmt --all -- --check` exit 0 ·
  `cargo clippy --workspace --all-targets -- -D warnings` exit 0, 0 warnings · `cargo build --workspace`
  exit 0 · `cargo test --workspace` **0 failed** (new `frames` regression test) · `git diff
  --exit-code -- ui/src/bindings` (the `frames_summarized` doc-comment regenerated its binding if ts-rs
  emits doc comments — committed).

#### Coverage correctness fix — dense single periods no longer truncate (PR #33, follow-up)
Self-found while reviewing the round-2 fixes against a live Daily report footer ("10/40 frames
summarized — range trimmed to fit"). **Root cause:** a single calendar period is one map group
(always the case for Daily, and for short Custom ranges), and the map-reduce only fanned out *across*
groups — so a dense period's frames were packed into one 8192 pass and `build_summary_messages`
truncated them, silently dropping coverage despite the per-period guarantee. The old fast-path gate
`groups.len() <= 1 || frames_sampled <= map_reduce_min_frames` short-circuited a dense single period
straight into one truncated `summarize`. **Fix (`crates/kernel/src/reports.rs`):** before the map step,
`split_groups_to_fit` splits any group whose content overflows one window into pass-sized sub-batches
(reusing `split_chunks_into_batches`; chunk order preserved, parts labelled "(part k of n)"), so a
dense day fans out into several map passes and the reduce folds them back. The fast path now collapses
to one final pass **only** when the whole report genuinely fits one window (`groups.len() == 1`, or a
small report whose combined content fits via the new `chunks_fit_one_pass` gate) — never truncating.
`periods_covered` is captured before the split, so sub-splitting doesn't inflate the period count. New
test `dense_single_period_splits_into_passes_without_truncating` (40 large-text frames in one day →
`MapReduce`, ≥3 passes, all 40 summarized, `truncated == false`, first+last frame cited, still
1/1 periods). **Verify:** `cargo fmt --all -- --check` exit 0 · `cargo clippy --workspace
--all-targets -- -D warnings` exit 0, 0 warnings · `cargo build --workspace` exit 0 · `cargo test
--workspace` **0 failed** · `git diff --exit-code -- ui/src/bindings` clean (kernel-internal change,
no IPC type touched).

### PR7 integration audit (2026-06-25)
Executed the PR7 audit plan on `codex/0.2.0-pr7-integration-audit` with the user's populated
`%APPDATA%\app.screensearchv2c.desktop` store and `npm run tauri dev` driving the real debug
executable (`target/debug/screensearch.exe`). Evidence screenshots are local-only under
`.playwright-mcp/pr7-2026-06-25/` and are not tracked in Git.

- **Search:** content terms such as `Calendar-Grid` and `cargo test` returned useful project/terminal
  evidence, and `include app chrome + raw text` recovered raw labels. Default content search still
  returned static/app-chrome-heavy rows for `Firefox`, `Steam`, and `Deck` in the populated corpus.
  A fresh capture tick also showed some ScreenSearch labels inside `content_text`, even with many
  background spans suppressed. Recorded as known gap #62; no DB reset/backfill was performed.
- **Ask:** a Calendar-Grid question grounded on content text and showed cited frames. A unique-token
  no-evidence question refused honestly, but still rendered retrieved context under `CITED FRAMES`;
  recorded as known gap #63.
- **Reports:** Daily and Weekly both generated through the UI with source frames and footer metadata.
  Both observed runs used map/reduce-style bounded passes (`6 passes`, `39/40 frames summarized`,
  range-trim notice). The generating helper text incorrectly said "Weekly reports..." while Daily was
  selected; fixed in `ui/src/routes/Recall.tsx` with range-neutral bounded-pass copy.
- **Capture:** start/stop worked. The live step advanced from 432 to 433 captures today, added a
  visible `20:26` frame, and advanced the queue from `done 1192` to `done 1194`.

Audit artifact: `docs/AUDIT_0.2.0_PR7_2026-06-25.md`.

## 0.2.0 PR8 — 2026-06-26 — Parallel model download (`feat/0.2.0-pr8-parallel-download`)
Replaces the single-stream first-run model fetch (hf-hub, ~20 MB/s HF-CDN cap) with a
multi-connection **chunked downloader**. Confined to `crates/inference/src/download.rs` (+ its
tests) and `Cargo.toml` (+ a `sha2` direct dep, already in the tree transitively), independent of
the retrieval chain (`docs/0.2.0.md` PR8).

### Implemented
- **Probe → capability only (redirect-less, not a reusable URL).** `probe_range` issues
  `Range: bytes=0-0` against the HF `resolve` URL **without following the 302** and reads the
  redirect's own headers: total size (`X-Linked-Size` → `Content-Range` → `Content-Length`),
  `Accept-Ranges`, and the LFS sha256 (`X-Linked-ETag`). Reading the redirect directly (not the CDN
  response behind it) is what makes the integrity hash correct — see the second fix note below. The
  pure `range_plan(is_partial, accept_ranges, total)` is the chunked-vs-single-stream gate. It
  deliberately does **not** capture/reuse the resolved CDN URL — see the fix notes below.
- **Per-request resolve (the real-world fix).** First-cut code captured the probe's `resp.url()` and
  reused that one signed CDN URL for every chunk, which **403'd against live HF** (a storm of
  `server ignored Range … status 403`). Root cause, confirmed by decoding the CloudFront policy on
  the signed URL: HF's Xet-bridge URLs pin `"ByteRange":{"ExpectedHeader":"bytes=0-0"}` — valid
  **only** for the exact `Range` that minted them, so reuse with any other range fails the signature.
  Fix: `fetch_chunk` re-requests the stable `resolve` URL with its own `Range` and follows the
  redirect per request (reqwest preserves `Range` across the cross-origin hop), minting a fresh
  range-matched signed URL per chunk — the `hf_transfer` approach. A transient `403`/`401`/`429`/`5xx`
  retries with backoff (`CHUNK_RETRY_MAX_ATTEMPTS` × `CHUNK_RETRY_BACKOFF`). Verified live: reusing a
  `bytes=0-0` URL for another range → `403`; per-request resolve for two distinct ranges → `206`.
- **Correct integrity target (the second live-HF fix).** After the 403 fix, downloads *completed*
  but failed sha256 every time and looped at ~75%. Root cause: the probe followed the 302 and read
  the **CDN** response's bare `ETag`, which for a Xet-backed file is the **Xet content hash**
  (surfaced separately as `X-Xet-Hash`), not the file sha256 — yet it is a clean 64-hex string that
  passed `parse_sha256`, so the correctly-assembled blob (sha256 `66358cb1…`) was checked against the
  Xet hash (`d4ccbe2a…`) and rejected forever (the GGUF is ~75% of the GGUF+mmproj total, so it
  finished, failed, and restarted before the mmproj began — exactly the reported stall). The true
  sha256 lives in `X-Linked-ETag` on the `resolve` **redirect**, which reqwest discarded when it
  auto-followed. Fix: probe **redirect-less** (above) and read `X-Linked-ETag`; `lfs_sha256` trusts
  that header **only** — never the bare `ETag` (the Xet hash, or an S3 `md5-partcount` on classic
  LFS). Verified live: the `302` carries `X-Linked-ETag: 66358cb1…` = the HF-API LFS oid = our
  downloaded bytes, while the CDN `ETag`/`X-Xet-Hash` is the unrelated `d4ccbe2a…`.
- **Chunked, parallel, resumable.** `chunked_download` pre-allocates `<base>.part` (`set_len`), keeps
  one shared `std::fs::File`, and writes via positioned `FileExt::seek_write` (no shared seek cursor,
  race-free for non-overlapping regions). `DOWNLOAD_CONNECTIONS` (default 8, env
  `SSV2C_DOWNLOAD_CONNECTIONS`, clamp 1–16) chunk futures run via `buffer_unordered` **awaited inside
  the fetch task** so the stall watchdog's `task.abort()` cancels them all (the gap-#46 fix for this
  path). A per-chunk completion **bitmap** `<base>.parts` is fsync'd *after* its chunk data, so a
  crash never marks a chunk whose bytes are still in cache; on restart, completed chunks are skipped
  and their bytes seeded into the progress counter once up front.
- **Reuse, not bypass.** All chunks `fetch_add` into the same `downloaded` counter the existing
  watchdog reads — UI percentage and "no aggregate progress across all chunks" stall detection stay
  truthful. The finalize phase (fsync + optional hash) sets the existing `copying` flag so the
  network watchdog ignores it; the finished blob lands in the clean layout via atomic rename;
  `fetch_one`'s already-in-layout / already-a-cache-blob fast paths are still checked first.
- **Fallback + integrity.** No `Accept-Ranges`/no length/probe failure → single-stream
  `download_with_lock_retry` + `place_in_clean_layout_async`, unchanged. A per-chunk `206` assertion
  rejects a server that ignores `Range` (no corruption); `401`/`403` re-resolves the signed URL once.
  The assembled file is verified against the LFS sha256 (`X-Linked-ETag`) when advertised (else byte
  length); a mismatch discards the partial for a clean retry.

### Decisions
- **Connection count is a const + env override, not a Settings field** (user-approved 2026-06-26):
  keeps PR8 confined to `download.rs` and matches the file's existing convention (`STALL_TIMEOUT`,
  `LOCK_RETRY_MAX_ATTEMPTS` consts; the `SSV2C_LLAMA_RELEASE_URL` env override). No `Settings`/IPC/
  bindings/UI change → the binding guard stays clean by design.
- **Verification failure discards the partial** (vs the network-error path that leaves it for resume)
  because every chunk is already marked done — a resume would re-fail; a clean re-download is correct.

### Skipped / deferred (intentional)
- No real-network test (the gated `smoke` test still exercises a live fetch); the new tests are fully
  mocked. Manual saturation/speed check on a multi-connection link is the gated smoke, not `cargo test`.

### Verification (verbatim)
```
# UI
> eslint .                       (clean, no output)
> vite build                     ✓ built in 1.86s
# Rust
cargo fmt --all -- --check       → fmt: clean (exit 0)
cargo clippy --workspace --all-targets -- -D warnings
                                 → Finished (0 warnings)
cargo build --workspace          → Finished `dev` in 31.53s
cargo test --workspace           → all crates 0 failed; inference 84 passed
    (new: range_plan_requires_ranges_and_known_size, parse_sha256_normalizes_etag_forms,
     lfs_sha256_trusts_only_x_linked_etag_not_cdn_etag,
     content_range_total_parses_suffix, chunked_download_assembles_byte_identical_file,
     chunk_requests_follow_redirect_and_preserve_range, chunk_retries_transient_403_then_succeeds,
     resume_skips_already_completed_chunks, chunked_download_errors_when_server_ignores_range,
     chunked_download_fails_fast_on_stuck_chunk,
     integrity_accepts_matching_sha256_and_rejects_a_wrong_one)
git diff --exit-code -- ui/src/bindings   → bindings: clean (exit 0)

Live HF check (curl, real Qwen3-VL-4B GGUF):
  - reuse of a bytes=0-0 signed URL for another range → 403; per-request resolve for two distinct
    ranges → 206 + exact bytes — confirms the 403 root cause + fix.
  - the resolve 302's X-Linked-ETag = 66358cb1… = the HF-API LFS oid = our downloaded-bytes sha256
    (the prior "got"); the CDN ETag / X-Xet-Hash = d4ccbe2a… (the prior wrong "expected") — confirms
    the sha256 root cause + fix.
```

### PR8 review hardening — 2026-06-26 (PR #35 bot review)

Addressed the PR #35 bot review (Gemini Code Assist + the GitHub `claude` reviewer). Bots were not
replied to (per the request); the substance was applied. Five fixes in `download.rs`, two
cross-platform suggestions declined as out-of-policy (Windows-only by design — confirmed with the user).

- **#6 stale-manifest silent corruption (medium).** `open_preallocated` → `(File, created)` (atomic
  `create_new`, no `exists()` TOCTOU). A brand-new zero-filled `.part` under an all-done `.parts`
  bitmap is re-initialised (`Manifest::reinit`/`init_sync`) instead of skipping the download and
  publishing zeros. Proven: guard disabled → test publishes an all-zero file; guard restored →
  byte-identical.
- **#3 network-error retry (high).** A chunk request's transport error now feeds the bounded backoff
  loop instead of a bare `?` failing the whole download on the first hiccup.
- **#4 unreadable-manifest progress loss (high).** `load_or_init_sync` propagates a non-`NotFound`
  read error (Windows sharing violation) so the job retries, instead of truncating a valid bitmap.
- **#5 coalesced writes (medium).** Frames buffer to 256 KiB (`flush_chunk_writes`) before each
  positioned write; progress still accrues per frame.
- **#7 accurate terminal error (low).** Distinguishes "server ignored Range" (`200`) from "failed
  after N retries" (exhausted `403`/`429`).
- **Declined (two high):** `#[cfg(unix)]` `write_at` to compile on macOS/Linux — Windows-only hard
  rule, CI is `windows-latest`, matches `flags.rs`/`lib.rs` convention.

Verbatim verification (run 2026-06-26):

```
cargo fmt --all -- --check                          → exit 0 (clean)
cargo clippy --workspace --all-targets -- -D warnings → Finished (0 warnings)
cargo test --workspace                              → all crates 0 failed
cargo test -p inference --lib                       → 87 passed; 0 failed (was 84; +3)
    (new: fresh_part_discards_stale_all_done_manifest,
     exhausted_transient_is_not_reported_as_ignored_range,
     manifest_load_or_init_distinguishes_missing_valid_and_mismatched)
git diff --exit-code -- ui/src/bindings             → bindings clean (exit 0)
```

## 2026-06-26 — 0.2.0 PR6 audit checkpoint (`codex/0.2.0-pr6-audit`)

- **Scope:** audited PR6 (Recall reports + premade Ask shortcuts) as a scoped feature review on
  current `main` (`43053c4fabefa493d74184b3a0257fa269116017`) using the existing
  `%APPDATA%\app.screensearchv2c.desktop\screensearch.db`, no reset/backfill/destructive SQL.
- **Local evidence:** ignored by policy under `.playwright-mcp/pr6-2026-06-26/`; ignored local audit
  markdown at `docs/AUDIT_0.2.0_PR6_2026-06-26.md`.
- **Baseline:** schema version 4; `frames=105`; `frame_text=105`; `embeddings=102`; `jobs done=292`;
  all frames on active day `2026-06-26`; report settings `retrieval.default_top_k=8`,
  `reports.daily_top_k=40`, `reports.weekly_top_k=200`, `reports.map_reduce_min_frames=20`.
  Online SQLite backup created at
  `.playwright-mcp/pr6-2026-06-26/screensearch-pr6-before.sqlite`.
- **Static wiring:** `generate_report(ReportRequest) -> ReportResponse`, `report_progress`,
  `cancel_report`, `AskRequest.top_k`, report settings, generated TS bindings, Recall tabs,
  premade Ask cards, report builder/view, and Settings fields are wired. Reports hydrate
  `frame_text.content_text` through `Store::ocr_texts`; prompted Custom reports use
  `hybrid_search(... include_chrome=false)`; Ask defaults to `include_chrome=false`.
- **Hardening re-check:** static/targeted review covered coverage sampling, dense single-period
  splitting, prompted Custom cap, lowered sidecar context budgeting, empty report without sidecar,
  cancellation/progress, settings clamps, and absence of a hardcoded `ASK_TOP_K`.
- **Targeted verification:** raw outputs preserved locally:
  `04-targeted-cargo-test-kernel-reports.txt` (`cargo test -p kernel reports -- --nocapture`, 11
  passed), `05b-targeted-cargo-test-store-sampler-sample-prefix.txt` (`cargo test -p store sample_
  -- --nocapture`, 5 passed), and `06-targeted-cargo-test-inference-report-summary.txt`
  (`cargo test -p inference report_summary -- --nocapture`, 2 passed).
- **Live dev executable:** launched with `npm run tauri dev` and captured
  `target/debug/screensearch.exe` (PID 19588 in first pass; PID 5960 in the successful retry). The
  first Computer Use pass could screenshot but not activate the WebView; the retry succeeded and
  completed the live PR6 UI pass.
- **Live Recall/Ask:** Search mode rendered with `CONTENT TEXT ONLY`; Ask mode rendered all five
  premade cards (`Day Recap`, `Standup Update`, `Time Breakdown`, `Top of Mind`, `AI Habits`).
  Clicking `Day Recap` submitted through the normal Ask flow, loaded the sidecar on demand, returned
  an answer, and rendered `CITED FRAMES`.
- **Live Reports:** Daily generated with range-neutral progress and footer metadata (`5 passes`,
  `1/1 periods`, `40/40 frames summarized`, trimmed warning). Weekly generated through the Weekly
  path (`5 passes`, `1/7 periods`, `40/40 frames summarized`). Prompted Custom generated from
  `PR6 audit reports Ask shortcuts content_text` and showed nine bounded passes. Custom no-evidence
  for `06/25/2026` returned immediately with the honest empty message; code/tests verify the
  response is `passes=0`, and the UI intentionally renders that as message-only with no chips/footer.
- **Live Settings:** the "Reports & retrieval" fields showed the DB values:
  `Ask retrieval depth=8`, `Report frames per day=40`, `Report frame cap=200`,
  `Map-reduce threshold=20`.
- **Controlled capture probe:** a Windows Notepad tab with
  `PR6 PROBE TOKEN 20260626 8XK7 CONTENT TEXT CHECK` was captured while foreground. SQLite query
  `46-pr6-notepad-probe-db.txt` shows frame `107`, `app_hint=Notepad`, and token pieces present in
  `frame_text.content_text` (`PR6 PROBE TOKEN`, `20260626`, `8XK7`, `CONTENT TEXT CHECK` all count
  1). OCR misread one body `PR6` as `PRS`, but the title and body token parts landed in
  `content_text`.
- **Process cleanup:** stopped the dev run after evidence capture; the after-stop process evidence
  had no `screensearch.exe` or `llama-server.exe`, so no sidecar orphan was observed.
- **Findings:** no PR6 implementation blocker found so far. The existing DB still contains
  static/app-chrome terms in `content_text` (`Firefox`, `Steam`, `Deck`, `Recall`), which is the
  already-recorded upstream PR3 release blocker rather than a PR6 routing defect. Doc drift recorded
  in `06` #9 and `07` #65.
- **Full verification:** the first frontend verification attempt failed because the dev run left a
  repo-local Vite/esbuild service holding `ui/node_modules/@esbuild/win32-x64/esbuild.exe`
  (`EPERM unlink`; evidence `51-verify-ui-npm-ci-lint-build.txt`). After stopping only the matching
  repo-local Vite/esbuild processes (`52`/`53`), the exact frontend gate passed on retry
  (`51b-verify-ui-npm-ci-lint-build-retry.txt`). `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace`,
  `cargo test --workspace`, and `git diff --exit-code -- ui/src/bindings` all exited 0 with raw
  outputs preserved in `54` through `58`.

## 2026-06-26 — 0.2.0 PR8 audit checkpoint (`codex/0.2.0-pr8-audit`)

- **Scope:** audited PR8 (parallel model downloader) on current `main` after PR1/PR2/PR3/PR6/PR7/PR8
  had merged, including follow-up commit `d85331f` for stale partial manifests. PR3 static-chrome
  failure stayed out of scope except for release-status cross-reference.
- **Local evidence:** ignored local audit artifact `docs/AUDIT_0.2.0_PR8_2026-06-26.md` plus ignored
  evidence under `.playwright-mcp/pr8-2026-06-26/`. The evidence directory contains static
  `rg`/commit captures, the `npm run tauri dev` logs, resume state files, final model file inventory,
  and raw verification outputs.
- **Static review:** `crates/inference/src/download.rs` contains the PR8 chunked path: redirect-less
  HF resolve probe, `Range` planning, per-chunk resolve, positioned writes into one preallocated
  `.part`, bitmap manifest, aggregate progress/stall detection, `X-Linked-ETag` sha256 integrity,
  clean-layout publish, and hf-hub fallback for range-less servers. Follow-up hardening is present:
  stale fresh-part manifests (complete and partial) are reset, transport errors retry, unreadable
  manifests fail accurately, writes are coalesced, and exhausted transient errors are not mislabeled
  as ignored `Range`.
- **Surface check:** no PR8 schema, IPC, Tauri command, or generated-binding drift was found. The
  only expected public/dependency changes are `sha2` and the env overrides
  `SSV2C_DOWNLOAD_CONNECTIONS` / `SSV2C_DOWNLOAD_CHUNK_SIZE`.
- **Live dev executable:** launched with `npm run tauri dev`; Computer Use confirmed the active
  process as `target/debug/screensearch.exe`. From a reset app-data state, Settings -> Inference
  engine downloaded the default answer model and showed live progress (`11% · 218 MB / 2.0 GB` in
  the sampled chip). Vision Quality was selected, a fresh frame was captured, Moment -> `TAG WITH
  VISION` downloaded `Qwen3VL-8B-Instruct-Q4_K_M.gguf` plus
  `mmproj-Qwen3VL-8B-Instruct-F16.gguf`, removed `.part`/`.parts`, loaded the sidecar, and rendered a
  vision result. The user's Beta warning was honored: Vision Beta was used only for download/resume,
  not as a tag-quality assertion.
- **Resume:** with `SSV2C_DOWNLOAD_CONNECTIONS=1`, Vision Beta was interrupted after a manifest
  showed `Chunks: 170`, `Done: 73`, `Pending: 97`. Restarting normal `npm run tauri dev` did not
  return to zero: it resumed from those 73 completed chunks (about 43% of the GGUF) and showed `86%`
  at the next sampled UI state before finalizing both `Qwen3.5-9B-Q4_K_M.gguf` and `mmproj-F16.gguf`;
  partial artifacts were gone afterward.
- **Process cleanup:** after stopping the dev app, no `screensearch.exe` or `llama-server.exe`
  process remained.
- **Findings:** no PR8 release blocker. Gap #46 remains only for the hf-hub range-less fallback
  path. A new narrow hardening gap is tracked in `07`: a stale bitmap beside an existing-but-truncated
  `.part` can still be trusted because the current fresh-part guard only fires when the `.part` is
  newly created.
- **Verification:** raw outputs are preserved in `.playwright-mcp/pr8-2026-06-26/`:
  `verify-ui-npm-ci-lint-build.txt` (`cd ui && npm ci && npm run lint && npm run build`, exit 0),
  `verify-cargo-fmt.txt` (`cargo fmt --all -- --check`, exit 0),
  `verify-cargo-clippy.txt` (`cargo clippy --workspace --all-targets -- -D warnings`, exit 0),
  `verify-cargo-build.txt` (`cargo build --workspace`, exit 0),
  `verify-cargo-test-inference-lib.txt` (`cargo test -p inference --lib`, 88 passed),
  `verify-cargo-test-workspace.txt` (`cargo test --workspace`, all non-ignored tests passed), and
  `verify-bindings-diff.txt` (`git diff --exit-code -- ui/src/bindings`, exit 0).

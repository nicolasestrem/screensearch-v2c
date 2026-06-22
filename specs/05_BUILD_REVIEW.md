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

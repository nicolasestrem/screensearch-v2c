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

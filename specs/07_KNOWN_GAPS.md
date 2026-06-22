# 07 — Known Gaps

> Where the **spec is silent** and a decision was needed, plus **manual interventions** still
> required (with owner + deadline). The agent appends here instead of guessing (`04 §5`). Empty
> until the build hits a gap.

| # | Date | Gap (spec was silent on…) | Resolution / decision | Owner | Needed by |
|---|---|---|---|---|---|
| 1 | 2026-06-21 | `03 §8` listed `enrich.vision_timer_interval_ms` / `enrich.vision_idle_secs` **without defaults**, and modelled mode as a single enum. | **Resolved (user):** timed = **60 min** (`3_600_000` ms), idle = **5 min** (`300` s); both are **opt-in toggles, off by default** (see `06` patch #1). `03 §8` updated. | user | ✅ done |
| 2 | 2026-06-21 | Intake/spec don't fix a **bundle identifier** for installers. | **Resolved (user):** `app.screensearch.desktop` was taken → use **`app.screensearchv2c.desktop`** (`tauri.conf.json`). | user | ✅ done |
| 3 | 2026-06-21 | `03 §7` `get_readiness` returns a `Readiness` but `03` didn't define the per-component status enum. | **Resolved:** defined `ComponentStatus { Unknown, Disabled, Initializing, Ready, Unavailable, Error }` + `ComponentReadiness { status, detail? }`; `Readiness` now holds one per subsystem so the UI can show *why*. `03 §7` updated. | agent | ✅ done |
| 4 | 2026-06-21 | `02 §5` "WebView2/Vulkan/llama smoke-check" doesn't specify the form. | **Resolved:** `crates/doctor` is now a **library + thin CLI** (`cargo run -p doctor [-- --json]`) returning a structured `Report` — reusable by CI and (later) the app's readiness panel. Diagnostic only (never fails CI). | agent | ✅ done |
| 5 | 2026-06-21 | `03 §3` `Store::hybrid_search` takes only query **text**, but the vector arm needs the query *embedded* — and the store must not depend on the embeddings impl. The spec is silent on how the query vector reaches the store. | **Resolved (engineering):** `SqliteStore` optionally holds `Arc<dyn EmbeddingProvider>` (a **trait** → modularity intact). P1 builds & tests the full FTS+vec+RRF path (vector arm driven by a fake embedder); the live path is FTS-only until **P3** injects fastembed. No trait/IPC signature change. | agent | ✅ P1; fastembed P3 |
| 6 | 2026-06-21 | `03 §5` defines claim→`running` and fail→retry/dead, but not recovery for a job stuck in `running` after a worker dies mid-job (no lease / visibility timeout). | **Deferred:** the store implements exactly per spec (claim sets `running`; `fail_job` increments attempts). Re-queuing stale `running` jobs is the **kernel worker**'s concern (`03 §6` "restart + requeue") — to add a lease/heartbeat sweep in P3. Logged so it isn't lost. **Done (P3):** `Store::reset_stale_running_jobs(older_than_ms)` + a startup sweep (requeue all `running`) and a periodic 60 s sweep with a 5-min visibility timeout in `kernel::worker_pool`. No lease column added. | agent | ✅ P3 |
| 7 | 2026-06-21 | `03 §13` DoD requires hybrid search "< ~200 ms on a realistic DB", but P1 only tests tiny `:memory:` fixtures — the latency target is **unverified**. | **Deferred (tracked):** correct for P1 (no real embeddings or data volume until P3). When P3 wires fastembed, add a realistic-DB perf fixture and measure against the `< ~200 ms` bar; tune RRF `k`/pool if needed. Surfaced from `05` Pass 2 "Still risky" so it has an owner. **Done (P3):** `crates/store/tests/perf.rs` (`#[ignore]`d) seeds 10 000 frames + 768-dim vectors and measures `hybrid_search` — **p95 = 32.6 ms** (median 30.9 ms), well under 200 ms. | agent | ✅ P3 |
| 8 | 2026-06-21 | The vector arm post-filters `time_range` *after* the KNN (over-fetch `pool`, then filter on the join), so in-range matches that fall beyond the top-`pool` nearest vectors can be **missed** (recall under-counts on tight time windows). | **Deferred (tracked):** acceptable for P1 (FTS arm is unaffected and carries the live path; vec arm is dark until P3). Revisit in P3 with real embeddings — e.g. push the time filter into the KNN via a `vec0` partition/metadata column, or widen the pool adaptively. **P3 update:** left open — unobserved to cause a problem at the perf-fixture scale (32.6 ms p95 with `pool = max(limit·5, 50)`); revisit only if recall on tight time windows proves insufficient with real data. | agent | open (P3→P5) |
| 9 | 2026-06-21 | **P2 capture start behaviour** — `03 §7` defines `capture_control{start\|stop}` but is silent on whether capture is on at launch. | **Resolved (user):** capture is **off until the user starts it** (privacy-first). `capture` readiness = `Disabled` until `capture_control(Start)`. | user | ✅ done |
| 10 | 2026-06-21 | **P2 WGC implementation** — `03 §3` mandates WGC but not raw `windows-rs` vs the `windows-capture` wrapper (`02 §6` permits a simpler fallback). | **Resolved (user):** raw **`windows-rs` 0.62** (full control of the diff-gate at the GPU-copy boundary). `windows-capture` remains a permitted fallback if WGC proves unstable. | user | ✅ done |
| 11 | 2026-06-21 | **P2 UI scope** — `02 §5` lists a "live timeline in the UI" for P2, but `03 §13` DoD has no UI requirement. | **Resolved (user):** ship a **minimal live timeline** (Start/Stop + readiness + `capture_tick` stream); the full Command-Deck UI lands in P5. | user | ✅ done |
| 12 | 2026-06-21 | **P2 frame context columns** — `frames.app_hint`/`window_title` are nullable "context" with no producer specified for P2. | **Resolved (user):** **populate `app_hint` + `window_title`** from the foreground-window read the privacy gate already performs; `browser_url` stays null (needs UI Automation — deferred). | user | ✅ done |
| 13 | 2026-06-21 | **OCR confidence unavailable** — WinRT `Media.Ocr` exposes no confidence, but `OcrResult.mean_confidence` requires one (`06` #2). | **Resolved (user):** sentinel **`-1.0` = "unknown"** for WinRT rows (`ocr::CONFIDENCE_UNKNOWN`); no fabricated score, no schema change. | user | ✅ done |
| 14 | 2026-06-21 | **P4 `llama-server` binary acquisition** — `03 §6` assumes a `llama-server` child but never says where the binary comes from (doctor only warns "not on PATH"). | **Resolved (user):** **runtime auto-download** of the prebuilt llama.cpp **Vulkan** Windows release from GitHub into `<app-data>/sidecar/llama` (`inference::download::ensure_binary`); resolution order = `SSV2C_LLAMA_RELEASE_URL` env → existing install → download. Pinned to "latest" + a unit-tested `*-win-vulkan-x64.zip` asset selector. | user | ✅ done |
| 15 | 2026-06-21 | **P4 GGUF acquisition at runtime** — `MODEL_REGISTRY §4` describes the dev-time `hf` CLI, which is Python (banned in the shipped runtime, `01 §5`). | **Resolved (user):** **runtime auto-download via Rust `hf-hub`** (no Python): list repo siblings, pick `*Q4_K_M*.gguf` (+ same-repo `mmproj*.gguf` for vision), copy into `<app-data>/models/<lane>/<tier>` lazily on first use. | user | ✅ done |
| 16 | 2026-06-21 | **P4 acceptance bar** on a machine without the binary/models — `03 §10/§13` mix deterministic tests with a real-`llama-server` smoke. | **Resolved (user):** **lifecycle + mock-tested inference** must pass now (no-orphan Job-Object, reap, idle-evict decision, HTTP client vs a mock); real GPU end-to-end is a **`#[ignore]` smoke** (`tests/smoke.rs`), runnable manually. | user | ✅ done |
| 17 | 2026-06-21 | **Reap sentinel** — `03 §6` says identify a stray sidecar by a "unique command-line sentinel arg", but `llama-server` rejects unknown flags. | **Resolved (agent):** use the child's **full image path** (always under our app-data `sidecar/llama`) as the sentinel, cross-checked against a pidfile, before `TerminateProcess` — never kills an unrelated PID. `KILL_ON_JOB_CLOSE` stays the primary no-orphan guarantee; reap is defense-in-depth. | agent | ✅ done |
| 18 | 2026-06-21 | **Answer citations + retrieval depth** — `03 §7/§13.5` require citations + a grounded answer but don't define how citations are chosen or the retrieval K. | **Resolved (agent):** emit one `AnswerDelta::Citation` per **retrieved context frame** (the grounding set — reliable, not parsed from prose); `ask` retrieves **top-K = 8** hybrid hits and grounds on their OCR text. | agent | ✅ done |
| 19 | 2026-06-21 | **Vision tagging details** spec-silent: timer/idle batch size, vision confidence fallback, no pending-job dedup, multi-GPU device choice. | **Resolved (agent):** timer/idle batch **N = 20** untagged frames/tick; non-JSON vision reply → raw text as description with the **`-1.0` "unknown"** confidence sentinel (consistent with #13); **no dedup** of pending `vision_tag` jobs (re-enqueue is harmless — `insert_vision` is an idempotent upsert); `-ngl 99` offloads to **Vulkan device 0** (a device-select env/flag may be needed on the AMD-iGPU + NVIDIA box — revisit if the wrong device is picked). | agent | open (revisit P5) |
| 20 | 2026-06-22 | **Vision JSON quality** — the gated GPU smoke (`real_vision_tags_an_image`, RTX 5060 Ti) returned **well-formed JSON** but `confidence: 0.0` and `activity_type: "unknown"`. The model echoed the `VISION_PROMPT`'s literal `"confidence": 0.0` **placeholder** (so a fabricated-looking `0.0` is recorded, *not* the `-1.0` "unknown" sentinel — conflicts with the never-fabricate-a-score principle of #13/#19), and `activity_type` is free-form so off-list/`"unknown"` labels pass through. (A two-tone synthetic test image is genuinely low-signal, so "unknown" activity is partly expected there; the real defect is `confidence=0.0` masquerading as a real score.) | **Open (revisit P5 vision tuning):** (a) stop pinning a number in the prompt — describe the `confidence` field instead of showing `0.0`; (b) constrain the reply with llama.cpp `response_format` JSON-schema / GBNF grammar to force a genuine numeric confidence and an enum `activity_type`; (c) until then, map an off-enum or `"unknown"` `activity_type` to `None` and treat `confidence == 0.0` as the unknown sentinel in `parse_vision`. No code change yet — logged for the P5 vision pass (verify on real screenshots, not the synthetic smoke image). | agent | open (P5) |

Resolved engineering decisions (spec silent on *how*, recorded for traceability):
- **ts-rs 64-bit ints → TS `number`** via per-field `#[ts(type = "number")]` (Tauri JSON wire);
  `TS_RS_LARGE_INT` env is ignored by ts-rs 10.1's macro. Guarded by `no_bigint_in_ipc_types`.
- **ts-rs `export_to`** anchors at the source-file dir here → `../../../ui/src/bindings/`.
- **CSP** set to `null` in `tauri.conf.json` for P0 dev convenience — **harden in P5**.
- ~~**§9 file logging** deferred to P1~~ — **done in P1**: console + daily-rotating file sink under
  `<app-data>/logs/` (`tracing-appender`), wired once the app-data dir is resolved at launch.
- **Store concurrency (P1):** a single `rusqlite::Connection` behind a `Mutex`, every query run via
  `spawn_blocking`. SQLite is single-writer, so this is correct and simple, and `:memory:` persists
  for the store's lifetime (clean tests). Trade-off: no reader/writer concurrency — revisit a read
  pool only if search latency demands it.
- **RRF (P1):** `k = 60` (de-facto standard); per-arm candidate pool = `max(limit·5, 50)`;
  `time_range` filters both arms (vec arm over-fetches the pool then post-filters on the join).
- **Crates (P1):** `rusqlite` 0.40 `bundled` (FTS5 is in the amalgamation — no `fts5` feature in
  0.40); `sqlite-vec` pinned to **0.1.9** (the 0.1.10-alpha line ships a broken amalgamation —
  references a missing `sqlite-vec-diskann.c`); `blake3` for `embeddings.content_hash`.
- **Schema additions (P1, non-breaking):** `UNIQUE(frame_id, chunk_index)` on `embeddings` and
  `UNIQUE(frame_id)` on `image_embeddings` to support upsert; `AFTER DELETE` triggers on the
  embedding tables purge the `vec0` shadows (FK cascade + `recursive_triggers=ON`, proven by test).
- **`JobState::Failed` reserved (P1):** the store drives pending/running/done/dead; the worker
  decides retry-vs-dead so no intermediate `failed` is needed (kept in the enum/DDL per `03 §4`).
- **Inherent store reads beyond `03 §3` (P1):** `get_frame` (backs the P5 `get_frame` command) and
  `delete_frame` (retention-purge primitive) — real APIs, not test-only, that also make the write
  paths observable.
- **`frames.activity_type` (P1, post-review):** `03 §4` documents it "filled by vision", so
  `insert_vision` now writes it (denormalized copy of `vision_analysis.activity_type`) in the same
  transaction — lets the timeline filter by activity without a join. (PR #4 review.)
- **Job no-op guard (P1, post-review):** `complete_job`/`fail_job` error on a zero-row update
  (unknown/stale id) rather than silently succeeding — upholds `03 §5`'s "never silently dropped".
- **Diff gate (P2):** `capture.diff_threshold` is a normalized `[0,1]` mean-absolute **luma**
  difference over a `32×32` nearest-neighbour grid (`diff.rs`); the first frame per monitor and any
  resolution change always pass. `content_hash` = `blake3` of the raw RGBA bytes.
- **Frame JPEG layout (P2):** `frames/day-<n>/<captured_at>-<monitor>.jpg` where
  `n = captured_at / 86_400_000` — per-day sharding **without a calendar dependency** (chose this
  over `YYYY/MM/DD` to avoid adding `chrono`/`time` in P2); stored as a path relative to app-data.
- **`CapturedFrame` refinement (P2):** added `app_hint` / `window_title: Option<String>` (a
  spec-permitted shape refinement) so the capturer carries the foreground context it already reads
  for the privacy gate, without the kernel depending on the capture impl (`03 §2`).
- **windows-rs (P2):** `windows = "0.62"`, features per crate under `cfg(windows)`. WinRT async
  uses `IAsyncOperation::join()` (windows-future 0.3 renamed the old `.get()`); SoftwareBitmap from
  bytes needs the `Storage_Streams` feature; `BOOL`/`MONITORINFOF_PRIMARY` moved namespaces.
- **COM apartments (P2):** OCR runs on a dedicated **STA** thread (`COINIT_APARTMENTTHREADED`);
  capture runs on a dedicated **MTA** thread (`COINIT_MULTITHREADED`) with a free-threaded WGC frame
  pool. No `.await` ever runs inside COM code (bridged via channels + `spawn_blocking`).
- **Lock detection (P2):** `privacy.pause_on_lock` uses an `OpenInputDesktop` heuristic — a
  non-elevated process can't open the input (secure) desktop while locked, so a failed open ⇒
  "locked" (`privacy.rs`). Simpler/robust enough vs. a `WTS` session-notification window.
- **WGC sessions (P2):** per-monitor sessions stay running and are **sampled** each
  `capture.interval_ms` (drain-to-latest + diff gate), rather than start/stop per tick. Simple and
  correct; a single-shot-per-tick optimization to cut idle GPU work can come later.
- **OCR fallback (P2):** if the WinRT engine can't be created (no language pack), the composition
  root substitutes an `UnavailableOcr` that errors per frame (logged) so the app still runs and
  capture readiness/logs surface the problem — never a silent half-working state.
- **fastembed model variant (P3):** confirmed `fastembed` 5.17.2 exposes
  `EmbeddingModel::EmbeddingGemma300MQ` (768-dim, quantized — the `MODEL_REGISTRY §3` name is exact)
  and `ImageEmbeddingModel::NomicEmbedVisionV15`; both verified by the real-model `#[ignore]` test.
  Built clean on rust-version 1.82 — no MSRV bump for `ort` 2.0.0-rc.12.
- **Query/document prompt asymmetry (P3):** EmbeddingGemma is prompt-instructed (`query:` vs
  document prefixes improve retrieval), but the `EmbeddingProvider` trait has only `embed_texts`,
  used symmetrically for both indexing and the query. Kept symmetric for P3 (simplest correct, no
  trait change); a `query_embed` path is a later retrieval-quality refinement if needed.
- **Embedder injection (P3):** `SqliteStore.embedder` is `Arc<RwLock<Option<Arc<dyn
  EmbeddingProvider>>>>` with a runtime `set_embedder`, so the composition root attaches the model
  *after* the off-thread load without rebuilding the store (no new dep; the search hot path clones
  the `Arc` out before the `.await`). `Store::set_embedder` is a defaulted no-op on the trait.
- **Worker pool (P3):** one shared `Arc<Mutex<TextEmbedding>>` (concurrent embeds serialize on the
  lock — fine for the cheap text model; per-worker handles would 2× RAM, deferred). Backoff
  `1 s·2^attempts` capped 60 s; idle poll 250 ms→2 s. Workers start at `attach_embedder`, independent
  of capture (`02 §5` background trigger); stop-on-exit is best-effort (the startup sweep is the
  real safety net). `embed_text` embeds the whole OCR text as one chunk (`chunk_index = 0`);
  chunking is a non-breaking later addition.

Manual steps still required (e.g. signing certs, first-run model download, CI secrets):
- **First-run model download** — embedding models (P3) auto-download via fastembed into
  `<app-data>/models/fastembed`. **P4:** the `llama-server` Vulkan binary auto-downloads into
  `<app-data>/sidecar/llama` at launch (off-thread; `sidecar` readiness Initializing→Ready/
  Unavailable), and the vision/answer GGUF (+ mmproj) auto-download via `hf-hub` into
  `<app-data>/models/<lane>/<tier>` **lazily on first request** (gaps #14/#15). Nothing is bundled.
  No download-progress %% yet (readiness is coarse Initializing→Ready); a percentage is a P5 nicety.
- **`onnxruntime.dll` bundling (P3→P5):** `fastembed` → `ort` fetches a prebuilt ONNX Runtime at
  build time and ships `onnxruntime.dll`; the Inno Setup installer / portable ZIP must bundle it
  (P5 packaging). Verify it's present beside the exe in the `tauri build` artifact.
- **Code-signing certificate** for the installer — P5 packaging. Recommended path for this MIT OSS
  project (cheapest → most turnkey): **SignPath Foundation** (free Authenticode signing for
  qualifying OSS) → **Azure Trusted Signing** (~US$10/mo, Microsoft-run, builds SmartScreen
  reputation) → **Certum Open Source Code Signing** (cheap annual cloud cert for OSS devs). Note:
  since 2023 all OV certs require a hardware token or cloud HSM, so a plain file-based cert is no
  longer available; budget for that. Decide owner before P5.
- ~~esbuild `allow-scripts` postinstall~~ — **resolved** locally via `npm approve-scripts --all`.

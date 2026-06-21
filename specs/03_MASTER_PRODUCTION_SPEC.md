# 03 — Master Production Spec

> **Question this file answers:** *"Exactly how should this be built?"* — the engineering truth.
> If something matters in production, it belongs here. Builds on `01_PROJECT_CONTEXT.md` and
> `02_STRATEGIC_PLAN.md`. The agent treats this file as authoritative for *how*; when it is silent
> or self-contradictory, **stop and ask** (see `04`).

---

## 1. System overview

A single desktop process (Tauri 2) hosts a Rust **kernel** that owns a typed event bus and a
registry of **trait-bounded modules**. The only out-of-process component is the **llama.cpp
inference sidecar**, bound to the app via a Windows Job Object. Heavy work is **deferred** into a
durable SQLite **job queue** and run by workers on user-controlled triggers.

```
Tauri WebView2 UI ──(commands/events, ts-rs)── Kernel
                                                 ├── event bus (typed)
   always-on:  CaptureSource → OcrProvider → Store      (cheap)
   deferred:   Store.JobQueue → Workers:
                  • EmbeddingProvider (fastembed, in-proc)
                  • VisionProvider  ─┐
                  • AnswerProvider  ─┴─→ ModelSupervisor → llama.cpp sidecar (Job-Object-bound)
   query:      Store.hybrid_search (FTS5 + vec KNN → RRF) → AnswerProvider → stream to UI
```

## 2. Workspace layout (Cargo + Tauri)

```
screensearch-v2c/
├── src-tauri/                 # Tauri app shell + command handlers + main()
│   └── Cargo.toml
├── crates/
│   ├── kernel/                # orchestrator, event bus, ModelSupervisor, worker pool
│   ├── traits/                # the module contracts + shared domain types (no impls)
│   ├── store/                 # Store/JobQueue impl on SQLite + sqlite-vec + FTS5
│   ├── capture/               # CaptureSource (WGC) + diff gate
│   ├── ocr/                   # OcrProvider (WinRT Media.Ocr, STA worker)
│   ├── embeddings/            # EmbeddingProvider (fastembed)
│   └── inference/             # VisionProvider + AnswerProvider (sidecar HTTP client) + supervisor
├── ui/                        # React 18 + TS + Vite ("Command Deck")
├── specs/
├── Cargo.toml                 # workspace
└── README.md  LICENSE  .gitignore
```

**Dependency rule:** `kernel` and module crates depend on `traits` (contracts), never on each
other's concrete impls. `src-tauri` wires concrete impls into the kernel at startup (composition
root). This is the modularity guarantee.

## 3. Module contracts (`traits` crate)

Signatures are normative (names/shapes may be refined in impl, but the boundaries are fixed).
All fallible async; `Result<T>` = `anyhow::Result<T>` (or a crate error enum).

```rust
pub struct CapturedFrame { pub monitor_index: u32, pub width: u32, pub height: u32,
                           pub captured_at: i64 /*unix ms*/, pub pixels: Arc<RgbaImage>,
                           pub content_hash: String }

#[async_trait] pub trait CaptureSource: Send + Sync {
    fn monitors(&self) -> Vec<MonitorInfo>;
    /// Yields the next *changed* frame (diff-gated) or None on shutdown.
    async fn next_frame(&mut self) -> Result<Option<CapturedFrame>>;
}

pub struct OcrResult { pub text: String, pub mean_confidence: f32, pub engine: String }
#[async_trait] pub trait OcrProvider: Send + Sync {
    async fn recognize(&self, frame: &CapturedFrame) -> Result<OcrResult>;
}

pub struct Embedding(pub Vec<f32>); // len == dim()
#[async_trait] pub trait EmbeddingProvider: Send + Sync {
    fn dim(&self) -> usize;                 // 768
    /// NOTE: quantized text model cannot batch — impl embeds one input at a time.
    async fn embed_texts(&self, inputs: &[String]) -> Result<Vec<Embedding>>;
    async fn embed_image(&self, image: &RgbaImage) -> Result<Embedding>;
}

pub struct VisionAnalysis { pub description: String, pub activity_type: Option<String>,
                            pub app_hint: Option<String>, pub confidence: f32, pub model: String }
#[async_trait] pub trait VisionProvider: Send + Sync {
    async fn analyze(&self, image: &RgbaImage) -> Result<VisionAnalysis>;
}

pub struct RetrievedChunk { pub frame_id: i64, pub text: String, pub score: f32, pub captured_at: i64 }
pub struct AnswerOpts { pub thinking: bool, pub max_tokens: u32 }
#[async_trait] pub trait AnswerProvider: Send + Sync {
    /// Streams answer deltas over the channel; returns when complete.
    async fn answer(&self, query: &str, context: &[RetrievedChunk], opts: AnswerOpts,
                    tx: tokio::sync::mpsc::Sender<AnswerDelta>) -> Result<()>;
}

#[async_trait] pub trait Store: Send + Sync {
    // frames + ocr
    async fn insert_frame(&self, f: NewFrame) -> Result<i64>;
    async fn insert_ocr(&self, frame_id: i64, ocr: OcrResult) -> Result<()>;
    async fn insert_vision(&self, frame_id: i64, v: VisionAnalysis) -> Result<()>;
    // embeddings
    async fn upsert_text_embedding(&self, frame_id: i64, chunk_index: i32, chunk_text: &str,
                                   source: ChunkSource, emb: &Embedding, model: &str) -> Result<()>;
    async fn upsert_image_embedding(&self, frame_id: i64, emb: &Embedding, model: &str) -> Result<()>;
    // retrieval
    async fn hybrid_search(&self, q: &SearchQuery) -> Result<Vec<SearchHit>>;
    // job queue (see §5)
    async fn enqueue_job(&self, job: NewJob) -> Result<i64>;
    async fn claim_jobs(&self, kinds: &[JobKind], limit: u32, now: i64) -> Result<Vec<Job>>;
    async fn complete_job(&self, id: i64) -> Result<()>;
    async fn fail_job(&self, id: i64, err: &str, retry_at: Option<i64>) -> Result<()>;
    async fn job_stats(&self) -> Result<JobStats>;
    // settings
    async fn get_setting(&self, key: &str) -> Result<Option<String>>;
    async fn set_setting(&self, key: &str, value: &str) -> Result<()>;
}
```

## 4. Data model (SQLite, WAL) — authoritative DDL

Single file `screensearch.db`. Migrations are forward-only, tracked in `schema_version`.

```sql
-- frames: one row per stored (changed) capture
CREATE TABLE frames (
  id            INTEGER PRIMARY KEY,
  captured_at   INTEGER NOT NULL,          -- unix ms
  monitor_index INTEGER NOT NULL,
  width         INTEGER NOT NULL,
  height        INTEGER NOT NULL,
  image_path    TEXT    NOT NULL,          -- relative path to JPEG on disk
  content_hash  TEXT    NOT NULL,
  app_hint      TEXT, window_title TEXT, browser_url TEXT,  -- context (nullable)
  activity_type TEXT,                       -- filled by vision (nullable)
  created_at    INTEGER NOT NULL DEFAULT (unixepoch()*1000)
);
CREATE INDEX idx_frames_captured_at ON frames(captured_at);

-- OCR text (one row per frame) + FTS5 mirror
CREATE TABLE ocr_text (
  frame_id        INTEGER PRIMARY KEY REFERENCES frames(id) ON DELETE CASCADE,
  text            TEXT NOT NULL,
  mean_confidence REAL NOT NULL,
  engine          TEXT NOT NULL
);
CREATE VIRTUAL TABLE ocr_text_fts USING fts5(text, content='ocr_text', content_rowid='frame_id',
                                             tokenize='porter');
-- triggers keep FTS in sync (insert/delete/update) — standard external-content pattern

-- vision analysis (deferred, optional, one row per analyzed frame)
CREATE TABLE vision_analysis (
  frame_id     INTEGER PRIMARY KEY REFERENCES frames(id) ON DELETE CASCADE,
  description  TEXT NOT NULL, activity_type TEXT, app_hint TEXT,
  confidence   REAL NOT NULL, model TEXT NOT NULL,
  created_at   INTEGER NOT NULL DEFAULT (unixepoch()*1000)
);

-- text embeddings: metadata + sqlite-vec index
CREATE TABLE embeddings (
  id           INTEGER PRIMARY KEY,
  frame_id     INTEGER NOT NULL REFERENCES frames(id) ON DELETE CASCADE,
  chunk_index  INTEGER NOT NULL,
  chunk_text   TEXT NOT NULL,
  source       TEXT NOT NULL,               -- 'ocr' | 'vision_description'
  model        TEXT NOT NULL, dim INTEGER NOT NULL,
  content_hash TEXT NOT NULL                -- skip re-embed if unchanged
);
CREATE INDEX idx_embeddings_frame ON embeddings(frame_id);
CREATE VIRTUAL TABLE embedding_vectors USING vec0(
  embedding_id INTEGER PRIMARY KEY,         -- == embeddings.id
  embedding    FLOAT[768] distance_metric=cosine
);

-- image embeddings (optional visual recall): metadata + sqlite-vec index
CREATE TABLE image_embeddings (
  id        INTEGER PRIMARY KEY,
  frame_id  INTEGER NOT NULL REFERENCES frames(id) ON DELETE CASCADE,
  model     TEXT NOT NULL, dim INTEGER NOT NULL
);
CREATE VIRTUAL TABLE image_embedding_vectors USING vec0(
  image_embedding_id INTEGER PRIMARY KEY,   -- == image_embeddings.id
  embedding          FLOAT[768] distance_metric=cosine
);

-- durable job queue (the heart of enrich-deferred) — see §5
CREATE TABLE jobs (
  id           INTEGER PRIMARY KEY,
  kind         TEXT NOT NULL,               -- 'embed_text' | 'embed_image' | 'vision_tag'
  frame_id     INTEGER REFERENCES frames(id) ON DELETE CASCADE,
  state        TEXT NOT NULL DEFAULT 'pending', -- pending|running|done|failed|dead
  priority     INTEGER NOT NULL DEFAULT 0,  -- higher first
  attempts     INTEGER NOT NULL DEFAULT 0,
  max_attempts INTEGER NOT NULL DEFAULT 3,
  not_before   INTEGER NOT NULL DEFAULT 0,  -- unix ms (scheduling + backoff)
  last_error   TEXT,
  created_at   INTEGER NOT NULL DEFAULT (unixepoch()*1000),
  updated_at   INTEGER NOT NULL DEFAULT (unixepoch()*1000)
);
CREATE INDEX idx_jobs_ready ON jobs(state, not_before, priority DESC, id);

-- tagging, settings, schema version
CREATE TABLE tags (id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL);
CREATE TABLE frame_tags (frame_id INTEGER REFERENCES frames(id) ON DELETE CASCADE,
                         tag_id INTEGER REFERENCES tags(id) ON DELETE CASCADE,
                         PRIMARY KEY(frame_id, tag_id));
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE schema_version (version INTEGER NOT NULL);
```

**Vector sync:** on `upsert_text_embedding`, insert into `embeddings` then
`embedding_vectors(embedding_id, embedding)` with the new rowid; on frame delete, the cascade
removes `embeddings`, and an `AFTER DELETE` trigger (or app-side txn) removes the matching
`embedding_vectors` rows. Same for image embeddings.

## 5. Job queue & worker model (the core change)

**Producers**
- After `insert_ocr` succeeds → enqueue `embed_text` (priority normal).
- If image embeddings enabled → enqueue `embed_image`.
- `vision_tag` is **never auto-enqueued per frame.** It is enqueued only by:
  1. **On-demand** — a UI command for a frame or a time range.
  2. **Timer** — a scheduler enqueues up to *N* untagged frames every *interval*.
  3. **Idle** — when the OS reports user-idle ≥ threshold.

**Workers**
- A bounded worker pool (`kernel`) loops: `claim_jobs(kinds, batch, now)` →
  `UPDATE … SET state='running'` (atomic claim) → run provider → `complete_job` or
  `fail_job(err, retry_at)`.
- **Claim atomicity:** single `UPDATE … WHERE id IN (SELECT … state='pending' AND not_before<=now
  ORDER BY priority DESC, id LIMIT n) RETURNING *` under WAL.
- **Retry/backoff:** on failure `attempts++`; if `attempts < max_attempts` set
  `not_before = now + backoff(attempts)`, else `state='dead'` (dead-letter; surfaced in
  diagnostics, never silently dropped).
- **Resource control:** worker concurrency, enabled job kinds, and the vision trigger mode are all
  settings (§8). Embedding workers may run on a background trigger; vision workers honor the
  on-demand/timer/idle mode strictly.

## 6. Inference sidecar — protocol & lifecycle (hard requirements)

**Process:** one `llama-server` child, OpenAI-compatible HTTP on `127.0.0.1:<ephemeral>`.
**Model-agnostic, tiered:** the `ModelSupervisor` is given a `ModelSpec { lane, tier, gguf_path,
mmproj_path?, ngl }` and starts the server for it. Switching lane/tier that needs a different model
stops and restarts with the new GGUF (vision needs `--mmproj`; answer does not).

**Lifecycle (MUST):**
1. **Job Object binding.** On supervisor init, create a Windows **Job Object** with
   `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. Every spawned `llama-server` is assigned to it at spawn,
   before unsuspend. → If the app dies for *any* reason (crash, kill, power-loss-after-resume,
   clean exit), the OS terminates the child. **No orphaned inference, ever.**
2. **Startup reap.** On launch, detect and kill any stray `llama-server` from a prior run that
   this app owns (tracked via a pidfile + a unique command-line sentinel arg); never kill an
   unrelated process.
3. **Lazy spawn + idle evict.** Spawn on first request needing a model; stop after
   `sidecar.idle_ttl_secs` of no requests (frees GPU/RAM — the footprint control).
4. **Health + restart.** Poll `/health`; on hang/crash, restart and requeue the in-flight job.
5. **mmproj same-family invariant.** Never pair a vision model with a mismatched projector.

**Crate option:** `win32job`, or `windows`-rs `CreateJobObject`/`AssignProcessToJobObject`/
`SetInformationJobObject`.

## 7. UI ↔ Core contract (Tauri commands + events, `ts-rs`)

All request/response types are Rust structs exported to TS via `ts-rs` (no hand-written
duplicates). **Commands** (UI → core):

| Command | In → Out |
|---|---|
| `search` | `SearchQuery` → `SearchHit[]` |
| `ask` | `AskRequest` → `()` (answer streamed via `answer_delta` events) |
| `get_timeline` | `TimeRange` → `TimelineBucket[]` |
| `get_frame` | `frame_id` → `FrameDetail` |
| `enqueue_vision` | `frame_id \| TimeRange` → `enqueued_count` |
| `get_job_stats` | `()` → `JobStats` |
| `get_settings` / `set_settings` | `()` / `Settings` |
| `set_model_tier` | `{lane, tier}` → `()` |
| `capture_control` | `{start\|stop}` → `()` |
| `get_readiness` | `()` → `Readiness` (capture, db, embed model, sidecar) |

**Events** (core → UI): `capture_tick`, `job_progress`, `answer_delta`, `sidecar_status`,
`readiness_changed`, `toast`.

**`Readiness` shape** (defined 2026-06-21, was silent — see `07` gap #3): each of
`capture | db | embed_model | sidecar` is a `ComponentReadiness { status, detail? }`, where
`status ∈ { unknown, disabled, initializing, ready, unavailable, error }` and `detail` is an
optional human-readable explanation.

## 8. Configuration / settings (keys in `settings`)

`capture.interval_ms` (3000) · `capture.monitors` ([]=all) · `capture.diff_threshold` (0.006) ·
`storage.jpeg_quality` (80) · `storage.max_width` (1280) · `storage.retention_days` (0=keep) ·
`enrich.embed_text` (true) · `enrich.image_embeddings` (false) ·
`enrich.vision_timer_enabled` (false) · `enrich.vision_timer_interval_ms` (3600000) ·
`enrich.vision_idle_enabled` (false) · `enrich.vision_idle_secs` (300) ·
`enrich.worker_concurrency` (2) ·
`models.vision_tier` (`default`) · `models.answer_tier` (`default`) ·
`answer.thinking` (true) · `sidecar.idle_ttl_secs` (180) · `sidecar.ngl` (99) ·
`privacy.excluded_apps` (["1Password","KeePass","Bitwarden"]) · `privacy.pause_on_lock` (true).

Capture honors `privacy.excluded_apps` (skip frame if foreground app matches) and
`privacy.pause_on_lock`. OCR runs on the **full-res** frame before JPEG resize/storage.

## 9. Logging & observability
`tracing` + daily-rotating file (`tracing-appender`) and console. Job-queue depth, sidecar state
transitions, and model load/evict are logged at info. No screen content or OCR text at info level
(privacy). Diagnostics surface dead-letter jobs.

## 10. Testing requirements
- **Unit:** each module against `traits` with fakes; `Store` against in-memory SQLite (`:memory:`).
- **Job queue:** state-machine tests (claim atomicity, retry/backoff, dead-letter, concurrent
  claim with N workers).
- **Retrieval:** FTS5 + vec KNN + RRF fusion correctness on a seeded fixture.
- **Sidecar lifecycle:** spawn → kill parent → assert child terminated (the no-orphan guarantee);
  startup reap; idle evict.
- **Windows-gated** (`#[cfg(windows)]`, may be `#[ignore]` in CI without GPU): WGC capture, WinRT
  OCR, real llama-server smoke.
- **Integration:** capture(stub frames) → OCR → store → embed job → search returns the frame →
  ask streams an answer.

## 11. CI/CD
GitHub Actions on `windows-latest`: `cargo fmt --check`, `cargo clippy --workspace -D warnings`,
`cargo build`, `cargo test` (GPU/WinRT tests `#[ignore]`d), `ui` `npm ci && npm run build`, and a
`tauri build` artifact job. Release workflow (later): Inno Setup installer + portable ZIP.

## 12. Failure modes & rollback
- **Migrations** forward-only via `schema_version`; each ships an idempotent up-script.
- **Job failure** → retry/backoff → dead-letter (visible), never silent loss.
- **Sidecar crash** → restart + requeue; repeated failure on a tier → surface a toast, fall back to
  Default tier.
- **Corrupt/oversized frame** → mark + skip; capture continues.
- **DB busy** → WAL + bounded retry.
- **Beta model incompatibility** (e.g., hybrid-arch quirk) → confined to Beta; Default/Quality
  unaffected.

## 13. Definition of done (v1.0)
1. Always-on capture → OCR → store works across multiple monitors; honors privacy settings.
2. Deferred embeddings populate text (and optional image) vectors via the job queue.
3. Vision tagging runs **only** on-demand/timer/idle per setting — never real-time.
4. Hybrid search (FTS5 + vec → RRF) returns correct frames < ~200 ms on a realistic DB.
5. `ask` streams a grounded, *thinking* answer with citations to frames.
6. Model tiers (vision + answer: Default/Quality/Beta) selectable in settings and take effect via
   sidecar reload.
7. **No orphaned `llama-server` after a forced app crash** — verified by test and manually.
8. `cargo clippy -D warnings` clean; all non-ignored tests green.
9. Installer + portable ZIP build successfully.

---

*Next layer:* `04_CLAUDE_CODE_BUILD_PROMPT.md` — how the agent operates against `00`–`03`
(reading order, source-of-truth, build order, guardrails, stop-at-ambiguity).

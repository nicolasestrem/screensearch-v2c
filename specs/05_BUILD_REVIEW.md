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

> Pre-0.2.x (v0.1.0) history → `specs/archive/05_BUILD_REVIEW.v0.1.0.md`.
> Shipped 0.2.x history (0.2.0–0.2.2) → `specs/archive/05_BUILD_REVIEW.v0.2.x.md`.
> Live file holds only the current (post-0.2.2) arc.

---

## Pass — 2026-07-04 — 0.3.0 PR7: local HTTP API + export (`feat/pr7-local-api`)

The opt-in local HTTP API + streaming JSON export (`docs/0.3.0.md` Part III; `03 §7c`/`§7`/`§8`/
`§13b.6`; D11/D12). Built bottom-up: shared types → store cursor → the inference cancellation fix →
the `crates/api` crate → the composition-root wiring → the UI → docs. No schema change (stays v10).

### Implemented
- **Shared types** (`crates/traits`): `ApiStatus`/`ExportRequest`/`ExportResult` (ts-rs, exported to
  `ui/src/bindings`, added to the `no_bigint_in_ipc_types` guard) + `ExportFrameRow` (Serialize-only,
  in `domain`, shared by store↔api without a module→module dep). The bearer token rides `ApiStatus`
  (Settings reveal/copy) but **never** the `Settings` struct, so it can't leak via `get_settings`.
- **Store cursor** (`crates/store/src/records.rs::export_frames_page`): keyset `id > after_id` +
  half-open `[from,to)`, `frames LEFT JOIN frame_text` for `content_text`, one `with_conn` hop per
  page (connection released between pages). 4 unit tests (paging/window/left-join/zero-limit).
- **SSE cancellation fix** (`crates/inference/src/answer.rs`): the detached `stream_task` is wrapped
  in an `AbortOnDrop` guard and the consume loop became `pump_deltas` with a `tokio::select!` on
  `tx.closed()`. A dropped `/v1/ask` receiver (or `cancel_ask`) now aborts the sidecar stream,
  freeing the GPU instead of generating into a closed socket (the leak `§7c` calls out). Completed
  path byte-identical. 2 unit tests (cancel-aborts + normal-completes).
- **`crates/api`** (new; depends on `traits` only): `ApiHost` trait seam; `ApiServer` binds
  `127.0.0.1` from a `const BIND_IP` (no address parameter → a `0.0.0.0` bind is impossible by
  construction), binds **before** spawn (synchronous port-conflict `Err`), graceful shutdown (3s cap
  then abort); bearer-auth middleware on every `/v1` route with a constant-time compare and a live
  `RwLock` token (regenerate needs no restart); one `ApiError`/`ErrorBody` shape; routes for health/
  search/frames(+`?image=1`)/where-was-i/marks/ask(SSE)/export(streamed). axum 0.8 is the lone new
  dependency (reuses the in-tree hyper/tower/http stack). 14 integration/unit tests.
- **Composition root** (`src-tauri/src/local_api.rs`): `TauriApiHost` adapts the concrete store/kernel
  via the existing private helpers (`ask_context`, `safe_frame_path`, `kernel::resume::where_was_i`);
  `ApiRuntime` holds the server slot + live token; `apply_api_config` persists `api.*` KV keys,
  generates the token on first enable, (re)starts/stops the server, and leaves `enabled=true` +
  `running=false` + `last_error` on a bind conflict (loud + guided, `07` #80); the 4 commands
  (`set_api_config`/`get_api_status`/`regenerate_api_token`/`export_data`); autostart on boot; server
  stopped first on exit. 4 unit tests (fresh-off/clamp/token-once/bind-failure).
- **UI**: `ApiPanel` (five states, threat-model copy, token reveal/copy/regenerate, port-in-use
  retry) + a separate Data export button (Downloads, works with the API off); the `apiStatus`
  query/key + 3 mutations + 4 command wrappers. Fixed the stale "backend never emits toast" comments.
- **Docs**: `docs/API.md` (new, OpenAPI-lite); `docs/TESTING.md` PR7 manual-acceptance section;
  `CHANGELOG.md`; `06`/`07` (export-destination decision #88, v1 residuals #89, #80 marked shipped);
  `UI_REFERENCE §5` gains the `ExportPanel` line.

### Verification (Windows, verbatim)
Full suite green:
```
cargo fmt --all -- --check                                  → exit 0 (no diff)
cargo clippy --workspace --all-targets -- -D warnings       → Finished (exit 0)
cargo build --workspace                                     → Finished (exit 0)
cargo test --workspace                                      → all suites ok; new:
    api        (unit)          2 passed
    api        http_api.rs    12 passed
    inference  (unit)        104 passed  (incl. 2 new pump_deltas tests)
    store      store.rs       61 passed  (incl. 4 new export_frames_page tests)
    screensearch_lib (unit)   12 passed  (incl. 4 new local_api tests)
    traits     (unit)         65 passed  (incl. the 3 new ts-rs guards)
cd ui && npm run lint                                       → exit 0
cd ui && npm run build                                      → built (exit 0)
git diff --exit-code -- ui/src/bindings                     → clean (exit 0)
```
Live out-of-process check — a real `ApiServer` served the fixture store on `127.0.0.1:43210` and was
exercised by external **`curl`** (not the in-process reqwest tests):
```
GET  /v1/health         (no token)   → 401 {"error":"unauthorized",…}
GET  /v1/health         (wrong token)→ 401
GET  /v1/health         (token)      → 200 {"version":…,"uptime_secs":…,"capturing":false}
GET  /v1/search?q=invoice            → 200 [{"frame_id":1,"snippet":"quarterly [invoice] total",…}]
GET  /v1/frames/1                     → 200 {"frame_id":1,"width":1920,…}
GET  /v1/context/where-was-i          → 200 null
POST /v1/marks {"frame_id":1,…}       → 201 {"mark_id":1}
POST /v1/marks {"frame_id":1,"now":true} → 400
GET  /v1/marks                        → 200 [{"mark_id":1,"note":"live check",…}]
GET  /v1/export?format=json           → 200 {"schema":"screensearch.export.v1",…,"frames":[…×2],"marks":[…]}
GET  /v1/export?format=csv            → 400
GET  /v1/frames/999                    → 404
```
A second harness instance binding the occupied 43210 returned `AddrInUse` — the synchronous
port-conflict `Err` the loud/guided path relies on. The `live_server_for_curl` harness (`#[ignore]`)
reproduces this. Full interactive desktop pass (enable via the Settings UI → curl → exit-frees-port)
is documented in `docs/TESTING.md` for the PR9 audit.

### Skipped / deferred
- Live SSE `/v1/ask` over curl (needs a loaded answer model) — covered by the `pump_deltas` unit test
  (cancellation) + `ask_streams_sse_deltas` integration test (scripted provider). Non-JSON export
  formats, a rate limiter, and a `captured_at`-seeded export cursor are recorded deferrals (`07` #89).

### Hallucinated / corrected
- Initially wired the ask/export routes into `build_router` before their handlers existed; merged the
  route wiring with the handlers so the crate landed as one compiling unit. Forgot `anyhow` as a
  direct `src-tauri` dep (used by the `ApiHost` impl's `anyhow::Result`); added it. `Chip` has no
  `info`/`muted` tones — used `accent`/`neutral`.

### Still risky
- `pump_deltas` touches the Ask/reports hot path; the completed path is unchanged and gated by the
  existing inference suite, but a live long-answer + mid-stream cancel is worth the PR9 desktop pass.
- The export cursor scans ids from 0 even for a late window (`07` #89c) — correct, bounded, not
  optimal.

### Follow-up — PR #76 review fixes (Gemini + Codex bot comments)
Addressed the five applicable inline suggestions on the open PR (bots not replied to, per request):
1. **Malformed requests broke the JSON error contract (Codex P2).** Axum's stock `Query`/`Json`/
   `Path` extractors reject *before* the handler, returning a plaintext body that bypasses
   `ApiError` — so `GET /v1/search` (no `q`) or `/v1/frames/not-a-number` returned a non-`{error,
   message}` 400, contradicting `docs/API.md`. Added `crates/api/src/extract.rs` — `ApiQuery`/
   `ApiPath`/`ApiJson` wrappers that delegate to the stock extractors and map every rejection to
   `ApiError::BadRequest`. Swapped them into all routes (`routes.rs`, `export.rs`). Two integration
   assertions strengthened (`search` missing-`q` and `frames/not-a-number` now assert
   `error=="bad_request"` + JSON content-type).
2. **Port edits were inert while the API ran (Codex P2).** The port `Field` was editable when
   running but there was no apply action, and the hint claimed a change "restarts the server" — so
   `api.port` was effectively unconfigurable from the normal path. Added a "Restart on {port}"
   affordance (`ApiPanel.tsx`) shown when the drafted port differs from the live one; it reuses the
   bind-retry restart (the D6-mirror pattern), making the hint honest.
3. **Front-end port not clamped (Gemini).** `parsedPort` only checked finiteness, so a negative or
   >65535 value would fail IPC deserialization into `u16`. Now clamps to `1024..=65535` (the backend
   clamps to the floor too).
4. **`.partial` left on flush/rename failure (Gemini).** `export_to_file` cleaned up the partial on
   a stream/write error but the trailing `file.flush()` and `rename` still used bare `?`. Both now
   remove the partial before returning the error — a failed export never leaves a plausible file.
5. **Missing image file → 500 (Gemini).** `TauriApiHost::frame_image` used `tokio::fs::read(..)?`,
   so a file missing on disk (manual delete/sync gap) surfaced as 500. Now maps `NotFound` to
   `Ok(None)`, letting the route answer a clean 404.
- **Verification (verbatim):** `cargo fmt --all -- --check` → exit 0; `cargo clippy --workspace
  --all-targets -- -D warnings` → Finished (exit 0); `cargo test --workspace` → all suites ok
  (`api` http_api.rs **12 passed**, 1 ignored; `inference` **104 passed**; `store` **61 passed**;
  workspace 0 failed); `cd ui && npm run lint && npm run build` → exit 0 / built; `git diff
  --exit-code -- ui/src/bindings` → clean (no ts-rs type changed).

## Pass — 2026-07-04 — 0.3.0 PR6: where-was-i + mark-this-moment (`pr6-where-was-i-and-marks`)

The ADHD core of the arc: a where-was-i heuristic (overlay empty state + Deck card) and
mark-this-moment (global hotkey → `capture_now` past the diff gate + a mark). Data-spine-first,
then core plumbing, shell, UI, docs (`docs/0.3.0.md` Part II; `03 §7b`/`§4`/`§7`/`§13b.5`;
D8/D9/D10/D14/D15).

### Implemented
- **Schema v10** (`crates/store/src/schema.rs`, `LATEST_SCHEMA_VERSION 9→10`): the `marks` table +
  `idx_marks_open`, verbatim from `03 §4`. Plain additive DDL (no rebuild); `marks.frame_id` CASCADEs
  like every per-frame child. Proven by a populated-DB migration test (`migration_v10_adds_marks_with_cascade`,
  mirrors the v9 pattern) and the fresh-vs-migrated schema-agreement test stays green.
- **Marks store API** (`crates/store/src/marks.rs`): `insert_mark` (clear "frame not found" on FK miss),
  `list_marks` (join `frames`, canonical order `(resolved_at IS NOT NULL) ASC, created_at DESC, id DESC`),
  idempotent `resolve_mark`, `set_mark_note`; `recent_frame_contexts` in `frames.rs`. CRUD/ordering/
  purge-survival + context-ordering unit tests.
- **Where-was-i heuristic** (`crates/kernel/src/resume.rs`): a pure `last_sustained_context` over
  newest-first `FrameContextRow`s — context key = `app_hint` + browser domain; transient-excursion
  absorption via per-key presence-span; anchor = last non-self context; excludes anchor/ScreenSearch/
  `privacy.excluded_apps`. 15-case fixture suite. `where_was_i(store, settings)` convenience.
- **capture_now plumbing:** `crates/capture` gains a per-monitor diff-gate bypass
  (`CaptureRequest.bypass_for`) with a **static-screen frame-pool recreate** (so a demanded frame is
  never dropped), a `Wake`-enum demand seam in `next_frame` (privacy gates still apply; denials acked
  honestly), and `select_target_monitor` (foreground → primary → first). `crates/kernel`:
  `capture_now` (serialized gate, two-timeout ack→frame-id) + `add_mark`; `CaptureFactory` grows the
  demand receiver; the loop returns the demanded frame's id via a `pending_demand` slot. A pure
  `diff::gate_passes` helper. Kernel-level mark tests (demanded frame+mark, capture-off, denial,
  by-frame-id) + diff/target-monitor unit tests.
- **Shell** (`src-tauri`): `overlay.rs` marks hotkey (mirrors the overlay hotkey, loud D6 failure), a
  **non-focus-stealing** `show_mark_toast` (`set_focusable(false)`), `focus_overlay_for_note` /
  `dismiss_mark_toast`. Commands `where_was_i`/`add_mark`/`list_marks`/`resolve_mark`/`set_mark_note`
  (mutations emit `marks_changed`); setup + `set_settings` wiring.
- **UI**: IPC wrappers/queries/mutations/events; `OverlayRuntime` flow|mark view + `MarkToast`
  (optional note, ~6 s auto-dismiss paused while typing); overlay empty state → `WhereWasIStrip`
  (Enter jumps to resume); Deck `WhereWasICard` + `IntentionsStrip` (no badge counts); Settings mark
  hotkey + dwell fields; `IconMark`. Moved the pure `is_excluded` matcher to `traits::privacy` so the
  kernel reuses it without depending on `capture`.

### Verification
`cargo fmt --check` · `cargo clippy --workspace --all-targets -D warnings` · `cargo build --workspace`
· `cargo test --workspace` — all green. `ui` `npm run lint && npm run build` — green, bindings diff
clean. Live desktop pass per `docs/TESTING.md` PR6.

### Decisions / interpretations (recorded)
- Four spec-silent behaviours resolved by user decision (`07` #85): non-focus-stealing toast,
  capture-off honest failure, `set_mark_note` for the after-the-fact note, privacy-gates-still-apply.
- Heuristic edges pinned (`07` #86): per-key presence-span interruption test; no hard "ends before
  anchor" clamp; capture-gap dwell; single-frame runs fail dwell.
- `list_marks` order follows the `§7` prose, not the `idx_marks_open` comment (`06` #18 / `07` #87);
  WGC pool-recreate handles the static-screen demand (`07` #87).

### Still risky
- The static-screen `pool.Recreate` path and the non-focus-stealing `set_focusable(false)` summon are
  Windows-runtime behaviours that unit tests can't exercise — both are covered by the `docs/TESTING.md`
  PR6 live checks (mark a static fullscreen app; keep typing through the toast).

## Pass — 2026-07-03 — 0.3.0 PR4: image-embedding lane removal (`feat/pr4-image-lane-removal`)

Delete the dark-launched, flag-off nomic-embed-vision image-embedding lane, with a forward-only
v8 → v9 migration that drops only derived vectors (`docs/0.3.0.md` PR4, `03 §4`/`§13b.3`, D5/D15).
Three commits: (1) a test-only parity baseline on the 10k fixture, (2) the atomic removal + migration
+ its RED-first tests, (3) docs + build-loop.

### Implemented
- **Migration v9 (schema 8 → 9).** `crates/store/src/schema.rs`: `MIGRATION_V9` drops
  `image_embedding_vectors` + `image_embeddings` (the `image_embeddings_ad` trigger goes with its
  table; dropping the vec0 virtual table removes its shadow tables) and `DELETE FROM jobs WHERE
  kind = 'embed_image'`. `LATEST_SCHEMA_VERSION` bumped in lockstep (the runner `debug_assert` guards
  it). `MIGRATION_V1` left frozen — verified by `fresh_and_migrated_schemas_agree_at_latest`: a fresh
  V1..V9 bootstrap and a v8→v9 upgrade produce byte-identical `sqlite_master` object+DDL sets. **No**
  `jobs.kind` CHECK / **no** jobs rebuild (`07` #82 honored).
- **Populated-DB migration test** (TDD: written RED, failed on `version == 8`, went GREEN after the
  bump). `migration_v9_drops_image_lane_and_embed_image_jobs`: a frame with both embedding lanes + a
  mixed jobs queue (`embed_image` × {pending,running,done,dead} + `embed_text` + `vision_tag`) →
  after migrate, zero image-lane schema objects (incl. `image_embedding_vectors_%` shadows), zero
  `embed_image` jobs, surviving kinds `[embed_text, vision_tag]`, frame + text lane intact,
  `fk_violation_count == 0`.
- **Workspace sweep.** traits (`embed_image`/`image_model_name`/`upsert_image_embedding`/
  `JobKind::EmbedImage`/`enrich_image_embeddings` gone), embeddings (text-only provider; fastembed
  `default-features = false` drops `image-models`), store (image APIs + token arms gone; hybrid search
  untouched — it never had an image arm), kernel (enqueue/claim/dispatch/`embed_image_outcome` gone;
  throttle pauses `vision_tag` only; retired key added), src-tauri (`embedder_with_image` +
  `needs_image_embedder` branch gone), UI (toggle + dead job arm gone; bindings regenerated).
- **Throttle test redesign.** `crates/kernel/tests/throttle.rs` reframed around `vision_tag` as the
  single heavy kind, preserving the exact proof shape (L1 pauses heavy while `embed_text` drains →
  recovery drains it → disabled-throttle inert). The level-2 `embed_text` floor is unchanged by this
  PR and stays covered by the pure `ThrottleMachine` unit tests + the untouched worker gate.

### Verification — verbatim
- `cargo fmt --all -- --check` → `FMT_EXIT=0`
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished` / `CLIPPY_EXIT=0`
- `cargo test -p store --lib migration` → migration_v6/v7/v8/**v9**/**fresh_and_migrated** all `ok`;
  `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 21 filtered out`
- `cargo test --workspace` → **39** `test result: ok` suites, **0** `test result: FAILED`
- `npm run lint` → clean (`LINT EXIT 0`); `npm run build` → `✓ built`
- `git diff -- ui/src/bindings` → only `JobKind.ts` + `Settings.ts` (ts-rs regenerated, committed)
- **Grep gate.** `rg -i "nomic"` / `rg "EmbedImage"` over `crates src-tauri ui/src README.md
  docs/ARCHITECTURE.md docs/TESTING.md` → empty. `rg "embed_image|image_embedding" crates` → only the
  enumerated live-code exceptions: the frozen `MIGRATION_V1` DDL + jobs-kind comment (never edited,
  PR2/V3 append-only precedent), the new `MIGRATION_V9` + its doc, the `migration_tests` seed SQL, and
  the `"enrich.image_embeddings"` `RETIRED_SETTINGS_KEYS` literal + doc. The `JobKind`/settings doc
  notes that *mention* the removal are history, like the CHANGELOG.
- **Hybrid-search parity (acceptance).** `cargo test -p store --test perf -- --ignored --nocapture`
  before (commit 1) and after (HEAD): the 20 `parity-digest <query>: <hex>` lines are **identical**
  (`diff` empty); `p95 = 72.8 ms` after (< 200 ms). The image arm was never fused into
  `hybrid_search` (`git diff main...HEAD -- crates/store/src/search.rs` is empty), so parity is
  structural — and now shown.
- **Live runtime (real desktop).** (a) `cargo test -p embeddings -- --ignored` downloads + loads the
  **real** EmbeddingGemma-300M ONNX model and embeds text on the trimmed fastembed build
  (`default-features = false`) → `loads_and_embeds_text ... ok` — the feature trim does not break the
  text lane. (b) `npm run tauri dev` on a fresh profile: the log shows the full migration chain
  `applied store migration schema_version=1..9` then `store opened … schema_version=9`,
  `fastembed provider loaded` / `embedding model loaded; attaching to kernel`, `enrichment workers
  started` / `inference providers attached`. A read-only copy of the created on-disk DB:
  `schema_version = 9`, **zero** image-lane objects (`image_embeddings` / `image_embedding_vectors` /
  `image_embeddings_ad` / `image_embedding_vectors_%` shadows all absent — only the text
  `embedding_vectors` + its shadows remain).

### Skipped / deferred (intentional)
- **On-disk nomic GGUF cleanup** — not done (D5 is DB-only; the model lives outside the DB and users
  can delete it from the model cache). No cleanup logic added.
- **A new level-2 `embed_text`-floor integration test** — out of a *removal* PR's scope; existing unit
  + gate coverage is unchanged by this PR (noted above).

### Hallucinated / corrected
- None. Every contract line-reference (specs 02/03/MODEL_REGISTRY/00, `07` #81/#82) matched the tree;
  the fastembed `image-models` default feature was confirmed trimmable against the 5.17.2 manifest
  before editing root `Cargo.toml`, and the build + full test run confirm the text lane still loads.

### Still risky
- **Populated-profile migration not live-exercised** — no pre-existing real DB was present on this
  machine, so the on-disk *upgrade* of an old (v8, image-populated) profile was proven by the
  populated-DB unit test through the production `bootstrap_and_migrate` runner rather than a GUI boot;
  the live GUI boot exercised the *fresh* v1→v9 chain on disk (schema_version=9, no image lane). The
  retired-key-drops-once path (`enrich.image_embeddings`) reuses PR2's proven `drop_retired_settings`
  mechanism but was likewise not exercised on a real persisted key. Both are in the `docs/TESTING.md`
  PR4 manual section for the PR9 pass against a populated profile.

## Pass — 2026-07-03 — 0.3.0 arc specs contract (PR1, specs-only) (`feat/0.3.0-pr1-specs-contract`)

From `docs/0.3.0.md` (roadmap, decisions D1–D15) + a Plan-agent adversarial validation of the edit
map. Specs-first: PR1 pre-writes the whole-arc contract so PR2–PR9 implement from the specs alone.

### Implemented
- **Contract normalized across `00`–`04` + `UI_REFERENCE` + `MODEL_REGISTRY` + `07` + `CLAUDE.md`/
  `AGENTS.md` + `CHANGELOG.md`.** Subtractions (trigger trim / Beta retire / image-lane drop) and
  additions (Flow overlay §7b / where-was-i + marks §7b / localhost API + export §7c + MCP) are now
  contract language; every D1–D15 has a home (matrix in the plan file). Details: `08` entry above.
- **New ambiguity resolved:** API port-bind UX (not a D-decision) → "loud + guided port change"
  (`03 §7c`, `UI_REFERENCE`, `07` #80), surfaced to the user first.

### Verification (specs-only — the untouched tree must still build) — verbatim
- `cargo fmt --all -- --check` → `FMT_EXIT=0`
- `cargo build --workspace` → `Finished \`dev\` profile … in 19.79s` / `BUILD_EXIT=0`
- `git status --short` → only `specs/*` + `docs/0.3.0.md` + `CLAUDE.md`/`AGENTS.md`/`CHANGELOG.md`/
  `.gitignore`; **no `.rs`/`.ts`/`.tsx`/`.toml`/`ui/`** touched → `ts-rs` bindings untouched by
  construction (no `cargo test` regen needed for a docs-only change).

### Skipped / deferred (intentional — out of PR1 scope)
- **Pre-existing `03 §4` ↔ `crates/store/src/schema.rs` DDL drift** (mostly deferred): `§4`'s
  "authoritative DDL" predates the `capture_trigger` (v5/v6) and `image_purged` (v7) columns and still
  carries a stale "schema_version 2 → 3" comment (actual `LATEST_SCHEMA_VERSION = 8`). PR1's D2 contract
  is written against the **real** schema. Because a fresh PR2 implementer reads `§4` to ground D2, the
  verification workflow flagged the undocumented `capture_trigger` column, so PR1 **added it to the `§4`
  frames DDL verbatim from `schema.rs` (incl. its widened CHECK)** — the one targeted reconciliation D2
  directly needs. The rest of the drift (`image_purged`, the stale version comment, `frame_text`
  nuances) stays a future cleanup. The new `marks` DDL + the two 0.3.0 migration notes use the
  **relative** "+1, confirm against `LATEST_SCHEMA_VERSION`" rule so they stay correct regardless.
- `docs/ARCHITECTURE.md` / `docs/TESTING.md` (live docs naming removed subsystems) are **not** edited —
  PR1 is specs-only; assigned to PR2/PR4/PR9 via `07` #81.

### Still risky
- Nothing runtime (docs-only). The risk is contract completeness — mitigated by the Plan-agent map
  validation (High/Medium findings folded in: `03 §3` trait signatures, residual `embed_image` prose,
  D7 in `03`, `00 §E`/`01` image-model refs, `02 §8` Status). A whole-arc completeness sweep should run
  before PR2 (an adversarial grep that no live `specs/` reference to a removed subsystem survives).

### PR #70 review round (2026-07-03) — bot comments folded in (specs-only, still no code)
Three PR reviewers (claude/gemini/codex; all bots, not replied to per user instruction). Each inline
comment verified against the **real** code before acting — several made claims about the tree:
- **`capture_now` note misplaced inside the `CaptureSource` trait block** (claude) → moved outside the
  `}`, reworded "**NOT** a method on this trait — a per-request flag to the capture worker" (`03 §2`).
- **`list_marks` ordering stated 3 ways** (claude: IPC "unresolved-first" vs `§7b` "newest-first" vs
  the index comment) → settled one canonical order (**all marks, unresolved first then newest-first
  within each group**) in the `§7` IPC row, `§7c` `GET /v1/marks`, and fixed `idx_marks_open` to
  `(resolved_at, created_at DESC)` so "newest-first" is actually index-served.
- **where-was-i anchors on the wrong foreground** (codex P2): from the overlay the OS foreground *is*
  ScreenSearch, so "current foreground" would pick the detour, not the work context → `§7b` now
  anchors on the **last non-ScreenSearch foreground context**, derived core-side (no `where_was_i()`
  signature change); mirrored in `UI_REFERENCE`.
- **where-was-i fragile to transient focus switches** (gemini) → `§7b` absorbs brief excursions; an
  interruption breaks a run only if the interrupting context is **itself sustained** (≥
  `resume.min_dwell_secs`) — reuses the one D9 threshold, adds no new knob.
- **`capture_now` frame nondeterministic on multi-monitor** (codex P2): `capture_cycle` yields one
  frame per monitor → `§7b` pins the mark to the **foreground-window monitor** (the frame whose
  `target_rect` resolves, `crates/capture/src/lib.rs:302-309`), primary as fallback.
- **`POST /v1/ask` doesn't cancel on client disconnect** (gemini) → `§7c` now requires PR7 to
  propagate cancellation on disconnect (**abort the sidecar `stream_task`**, free GPU/CPU), spelled out
  because `AnswerProvider::answer` is driven by the sidecar stream and discards downstream `tx.send`
  errors today — so merely dropping the SSE receiver would *not* stop generation (caught by the
  review-round verification reading `crates/inference/src/answer.rs:118-137`).
- **`GET /v1/export` unbounded → OOM** (gemini) → `§7c` specifies streaming serialization (flat
  memory) + optional `from`/`to` bound; same for the Settings "Export…" file path.
- **Add `CHECK` to `jobs.kind`** (gemini) → **declined + recorded** (`07` #82): live `schema.rs:132`
  has no such `CHECK`; adding one would diverge the spec from code and force an unplanned jobs-table
  rebuild in PR4 (beyond D5). Kept as opt-in future hardening as its own migration.
- Verbatim re-verification below.

## Pass — 2026-07-03 — 0.3.0 PR2: event-trigger trim (`feat/pr2-trigger-trim`)

First 0.3.0 subtraction (D1/D2). Cut the six event-capture triggers to **foreground + idle**,
deleting the `WH_MOUSE_LL` mouse hook (click/scroll-stop), the clipboard listener, and the
typing-pause edge — plus their five settings fields. **No schema change** (D2): legacy
`capture_trigger` tokens stay readable end-to-end. Implemented against the PR1 contract (`03 §8`
L616–631, `§13b.1`); code map from two Explore agents + a Plan-agent design pass.

### Implemented
- **`crates/capture/src/trigger.rs`** — `InputEventKind` down to `Foreground`; `TriggerConfig`
  down to the 5 surviving fields; `poll()` idle-only (typing-pause re-arm + double-fire guard + the
  `else if` branch removed, idle `if` stands alone). Test module: 5 retired-only tests deleted, the
  two that exercised **surviving** min-interval/retry logic *via* the retired `Clipboard` kind
  **rewritten to `Foreground`** (coverage preserved), the typing-pause once-per-period test rewritten
  to the idle edge (`idle_fires_once_per_quiet_period`), idle-disabled rewritten idle-only — 9 tests.
- **`crates/capture/src/events.rs`** — **the message-only window is gone** (`CLASS_NAME`,
  `register_class_once`, `wndproc`, `CreateWindowExW`), and so are `mouse_proc`, `MOUSE_FLAGS`, the
  clipboard listener, and all mouse/window Win32 imports — a whole `unsafe` path deleted. `start()`
  takes no params; the hook thread forces its message queue with `PeekMessageW(…, PM_NOREMOVE)`
  **before** signaling ready (the window used to guarantee it — `Drop`'s `PostThreadMessageW(WM_QUIT)`
  depends on it), installs one out-of-context foreground WinEvent hook, pumps, unhooks.
- **`crates/traits`** — 5 fields dropped from `Settings` (`ipc.rs`) and `CaptureConfig` (`domain.rs`);
  `CaptureTrigger` enum + `as_db_str`/`from_db_str` **kept** (read path) with the 4 retired variants
  reworded as **legacy — no longer emitted, still read**. New required Store method
  `delete_settings(&[&str]) -> Vec<String>` (`contracts.rs`) — the tree has exactly one `impl Store`
  (`SqliteStore`), verified, so "required" is safe and forces the delegation wiring.
- **`crates/store`** — `delete_settings` inherent impl (one transaction, per-key DELETE, returns keys
  that existed) + the trait delegation line. **No `schema.rs` change** (v8 unchanged).
- **`crates/kernel/src/settings.rs`** — retired reads/writes/clamp/`capture_config` maps removed;
  new `RETIRED_SETTINGS_KEYS` (the 5 DB keys) + `drop_retired_settings(store)` (one `warn!` listing
  dropped keys, error-swallowing so startup never blocks). **`src-tauri/src/lib.rs`** — one edit:
  `drop_retired_settings` as the first line of the startup maintenance spawn (startup-only, not in the
  hot `load_settings` path).
- **UI** — `Settings.tsx` event panel down to master + app-switch + idle + 3 thresholds; the
  typing-pause clamp removed from `sanitizeSettings`; `MomentDetail.tsx` `TRIGGER_LABEL` **kept** (all
  legacy labels). `Settings.ts` binding regenerated by `cargo test` (5 fields gone); `CaptureTrigger.ts`
  **byte-identical** (enum untouched).
- **Docs** — `docs/ARCHITECTURE.md` (live doc that named the removed hooks as present — rewritten),
  `docs/TESTING.md` (test count 14→9; manual-acceptance section rewritten to foreground+idle + the
  legacy-render and retired-key-drop checks), `README.md` (2 feature lines), `CHANGELOG.md`.

### Verification — verbatim
- UI: `npm ci` clean; `npm run lint` → (no output, exit 0); `npm run build` → `✓ built in 1.85s`.
- `cargo fmt --all -- --check` → `FMT EXIT 0`.
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished … CLIPPY EXIT 0`.
- `cargo build --workspace` → `Finished \`dev\` … BUILD EXIT 0`.
- `cargo test --workspace` → all green; changed crates: **capture 22 passed / 1 ignored**,
  **kernel settings 8 passed** (was 6 + 2 new tolerance tests), **traits 53**, **store 24+58**.
- `git diff -- ui/src/bindings` → only `Settings.ts` (5 fields removed); `CaptureTrigger.ts` clean.
- Post-regen `npm run lint && npm run build` → exit 0 / `✓ built` (no stale TS refs to removed fields).
- **Grep gate:** `WH_MOUSE_LL|AddClipboardFormatListener` → only the two intentional "these were
  removed" history-note comments (`events.rs`, `docs/ARCHITECTURE.md`); the retired settings fields →
  only `RETIRED_SETTINGS_KEYS`; `typing_pause|TypingPause` → only the read-path exemptions
  (`traits/domain.rs`, `uia/classify.rs`, `store` CHECK + migration test, `MomentDetail.tsx`,
  `CaptureTrigger.ts`).

### Live checks (real desktop; `npm run tauri dev`) — verbatim
- **Foreground hook thread** (the rewritten window-less `events.rs`): `cargo test -p capture --lib --
  --ignored source_starts_and_stops_cleanly_repeatedly` → `test … ok. 1 passed` — 50× start/drop, no
  leak/hang on this desktop.
- **Retired-key drop:** seeded the fresh dev DB (`%APPDATA%\app.screensearchv2c.desktop`) with the 5
  retired keys + 2 survivors, relaunched → the 5 dropped in ~2 s, the 2 survivors kept, and one log
  line: `WARN kernel::settings: settings: dropped retired keys keys=["capture.event_on_clipboard",
  "capture.event_on_typing_pause", "capture.event_on_click", "capture.event_on_scroll_stop",
  "capture.event_typing_pause_ms"]`. Second relaunch → **no** new drop line (logged once); app boots
  clean (embedding model loaded, kernel workers running).
- **Legacy token, live schema:** the running-app DB is at `schema_version = 8` (unchanged — D2); the
  live `frames.capture_trigger` CHECK still lists all 8 tokens; a real
  `INSERT … capture_trigger='click'` **succeeded** and read back `1|click` (probe row then deleted).
  With `TRIGGER_LABEL['click']='Click'` retained, a pre-trim click frame renders "Click" in Moment.

### Skipped / deferred (intentional)
- Interactive alt-tab/idle *fire* through the GUI is the manual pass in `docs/TESTING.md` (formalized
  at PR9); the trigger **logic** is proven by the 9 unit tests and the **hook** by the ignored
  hardware test run above. No populated pre-trim dev DB existed on this machine (no Roaming dir before
  this run), so the legacy-render check was done at the DB/label layer rather than against real
  historical frames.
- `crates/uia/src/classify.rs` **untouched** (exhaustive match over the retained enum; compiles + 16
  tests pass). Consequence recorded in `07` (below): `capture.uia_run_on_interactive` and the
  `ScrollStop|Click` arm are now inert-in-practice — not "fixed" (spec §8 still lists the key).

### Still risky
- Nothing new runtime-wise. The one judgment call — reading the DoD's "`typing_pause` appears nowhere"
  as exempting the **legacy read path** (enum/labels/CHECK/union) — is forced by D2's "legacy tokens
  stay readable and Moment keeps rendering them"; `docs/0.3.0.md` itself asserts both, so it's an
  interpretation, not a contradiction (no `06` entry). Recorded under Interpretation in `07`.

## Pass — 2026-07-03 — 0.3.0 PR3: Beta model tier removal (`feat/pr3-beta-tier-removal`)

Second 0.3.0 subtraction (D3/D4). Retired the **Beta** tier from both inference lanes → **Default /
Quality** only, deleting the two Beta models (vision `Qwen3.5-9B-VLM`, answer `Nemotron-3-Nano-4B` —
the only non-Apache/OML license and the only hybrid-arch row). **No schema change** (tiers live in
the `settings` table). Implemented against the PR1 contract (`03 §8` L576–578, `§13b.2`); code map
from two Explore agents + a Plan-agent design pass that validated the one design decision (the remap
seam). TDD: RED (`left: Beta, right: Quality`) → GREEN.

### Implemented
- **`crates/traits/src/ipc.rs`** — `ModelTier` → `{ Default, Quality }` (dropped `Beta`); doc comment
  records the retirement + load-remap. ts-rs regenerated `ui/src/bindings/ModelTier.ts` →
  `"default" | "quality"` (committed; drift guard clean).
- **`crates/inference/src/models.rs`** — deleted the two `repo_for` Beta arms + the `tier_slug` `Beta`
  arm (the **only** two exhaustive matches on the enum in the workspace; everything else is
  tier-generic). Extended `repo_mapping_matches_registry` to all **four** surviving `(lane, tier)` →
  repo + `needs_mmproj` pairs per `MODEL_REGISTRY §1/§2`.
- **`crates/kernel/src/settings.rs`** — new `load_tier(store, key, default)` replaces the generic
  `json()` read for the two `models.*_tier` keys: a persisted `"beta"` maps to `Quality`, is
  **persisted** via `set_setting` (best-effort, warn-and-swallow on error), and returned — so it logs
  **once** (retired token leaves the DB; next load reads `"quality"`), the same mechanism as
  `drop_retired_settings`. **The remap lives in the load path, not the startup-maintenance sweep**,
  because the composition root builds the sidecars straight from `load_settings`' output (`src-tauri/
  src/lib.rs:1167→1210/1217`); a sweep-side remap would race it and the first post-upgrade session
  would run `Default` — a D3 violation. Any *other* unparsable tier keeps the old behavior (fall to
  default, no rewrite).
- **Tests** — `crates/kernel/tests/settings.rs`: `persisted_beta_tier_remaps_to_quality_and_persists`
  (seed `"beta"` both lanes → load = Quality, DB rewritten to `"quality"`, second load idempotent);
  `unknown_tier_falls_back_to_default_without_rewrite` (a non-Beta bad value is **not** migrated —
  pins the tolerant-load boundary); fixed `round_trips_non_default_values` (`Beta` → `Quality`).
- **UI** — `ModelTierPicker.tsx` (TIERS + MODEL_NAMES beta rows + two header comments) and
  `Settings.tsx` (`TIER_LABEL`) lose Beta; TypeScript's `Record<ModelTier, …>` is the tripwire that
  forces both (no UI unit tests exist). `Settings.ts` / `SetModelTier.ts` bindings unchanged.
- **Docs** — `README.md` (3-tier → 2-tier table), `docs/ARCHITECTURE.md` §7.3, `docs/TESTING.md`
  (new model-tier manual-acceptance section), `CHANGELOG.md`. Specs `MODEL_REGISTRY`/`UI_REFERENCE`/
  `00`/`02`/`03` already carried the "0.3.0 retired Beta" language from PR1 — no PR3 spec edits.

### Verification — verbatim (Windows, full CI sequence)
- RED first: `cargo test -p kernel --test settings persisted_beta_tier_remaps_to_quality_and_persists`
  → `FAILED … assertion left == right failed  left: Beta  right: Quality`.
- `cargo test -p kernel --test settings` → `ok. 10 passed; 0 failed` (incl. the 2 new).
- `cargo test -p inference --lib models` → `ok. 6 passed; 0 failed` (incl. extended
  `repo_mapping_matches_registry`).
- `cargo fmt --all -- --check` → `FMT_EXIT=0`.
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished … in 10.23s` / exit 0.
- `cargo build --workspace` → `Finished … in 19.95s` / exit 0.
- `cargo test --workspace` → all suites green, **0 failed** (kernel 27 + settings 10 + enrichment 11 +
  pipeline 6 + throttle 2; inference **102**; traits 53; store 24+58; capture 22/1-ign; uia 16/2-ign;
  sysmon 11; textfilter 12; screensearch_lib 8; embeddings 1; ocr 1; e2e/perf/smoke ignored).
- `cd ui && npm run lint` → clean (exit 0); `npm run build` → `✓ built in 2.27s`.
- `git diff -- ui/src/bindings` → only `ModelTier.ts` (regenerated to the two-variant union, committed).
- **Grep gate:** `Nemotron` / `Qwen3.5-9B` appear only in history/rationale docs (CHANGELOG entries,
  `CHANGELOG-ARCHIVE`, `specs/archive`, `docs/0.3.0.md`, `custom-llm-training.md`, `docs/audits`, specs
  `00/02/03/08/MODEL_REGISTRY` retirement language, `.remember/*`) — **zero** in `crates/`,
  `src-tauri/`, `ui/src/`, README, ARCHITECTURE. `beta` survives in source only as the `load_tier`
  migration literal (`s == "beta"`) + doc comments + incidental fixtures (`"alpha beta"`,
  `"beta banana"`).

### Live checks (real desktop, `npm run tauri dev`) — verbatim
- **Fresh DB created at `schema_version=8`** (unchanged — this PR has no migration), confirming tiers
  are a `settings`-table concern, not schema.
- **beta→quality remap:** seeded the fresh dev DB
  (`%APPDATA%\app.screensearchv2c.desktop\screensearch.db`) with
  `models.vision_tier = models.answer_tier = '"beta"'`, relaunched → the DB rows were rewritten to
  `'"quality"'` within ~2 s of boot, and the log carried exactly two lines:
  `WARN kernel::settings: settings: retired \`beta\` tier mapped to \`quality\` key="models.vision_tier"`
  and `… key="models.answer_tier"` — one per lane, **once** (no repeat despite the many in-session
  `load_settings` calls from the hot loops, because the first load persisted `quality`). The app then
  ran on the remapped tiers (`INFO … inference providers attached` / `sidecar ready (lazy spawn)`).
- **Tier resolution (line 3):** the extended `repo_mapping_matches_registry` pins all four surviving
  `(lane, tier)` → repo mappings; `download.rs`/`resolve_spec` are tier-generic and untouched by this
  pure subtraction, so Default and Quality traverse identical logic (only the pinned repo string
  differs). The live app ran on Quality for both lanes without a resolution error.

### Skipped / deferred (intentional)
- **End-to-end multi-GB download of both Quality GGUFs** — the fresh app-data had no vision/answer
  weights on disk (only the fastembed text model auto-downloads at boot), so a live tier download
  would be a several-GB fetch. It is the PR9 manual-acceptance pass (`docs/TESTING.md`); the
  resolution mapping is unit-pinned and the download machinery is unchanged, so the risk is nil.
- **Settings-UI visual** (picker shows only Default/Quality; a remapped lane reads Quality) — the DB
  is at `"quality"` and the binding has no `"beta"`, so the render is forced; the pixel check is the
  PR9 manual pass.

### Still risky
- A **stale IPC client** sending `set_model_tier` with `"beta"` now fails serde at the Tauri boundary
  (command error before the handler) — only our own UI calls it, so acceptable.
- The first post-upgrade session can emit a **bounded handful** of duplicate remap warns if several
  concurrent `load_settings` callers (composition root, embeddings init, throttle tick) race the
  read→write window — once ever, matching PR2's "log once = silent next launch" semantics; observed as
  exactly one-per-lane in this run. No process-local `Once` added (would over-engineer the precedent).

## Pass — 2026-07-01 — UIA cache-batched walk: efficiency lever (#71) (`fix/uia-findall-buildcache`)

From a `/superpowers:brainstorming` design (plan approved). Third of three (#8, #73a shipped). The `07`
#71 hang was already mitigated in 0.2.1; this closes the deferred **efficiency lever**, which the gap
required be **live-verified** (the walk path runs only in the `#[ignore]`d desktop test — CI-dark).

### Implemented — design shaped by live verification
- **Rejected `FindAllBuildCache(TreeScope_Subtree)`** — one uninterruptible fetch; measured **~1.4 s**
  on a large window, blew the hard timeout → OCR fallback. Live test caught it (would have shipped a
  regression).
- **Rejected `FindAllBuildCache(TreeScope_Children)` BFS** — 8× fewer calls and great on Chrome
  (98 fetches, 63 ms), but a single **wide-node** child fetch still overran the budget on a VS Code-
  scale window (429 spans, 924 ms full; timed out at 150 ms budget). Live test caught it.
- **Shipped: granular DFS + per-node `BuildUpdatedCache`** (user-chosen). Same walker navigation as the
  shipped DFS (small, deadline-interruptible calls; `MAX_DEPTH`/`MAX_STACK` restored), but each node's
  ~5 `Current*` reads collapse into **one `BuildUpdatedCache`** + cached getters (~2.5× fewer COM
  calls). `TextPattern` stays live/gated/capped; `Value`/`Name` from the cache.
- **Three adversarial-review defects fixed** (7-agent + 6-agent reviews): (1) cache the `ValueValue`
  *property* — caching the pattern object alone leaves `CachedValue()` failing, silently dropping
  edit-field text (URL/omnibox, search boxes); (2) a text control past the `TextPattern` cap is
  descended so its children's text isn't lost (moot in the final DFS — it descends into everything);
  (3) a `BuildUpdatedCache` failure skips only that node's text but **still descends** (a transient
  timeout must not prune a whole subtree — old-DFS parity).

### Verification — verbatim
- `cargo fmt --all -- --check` → EXIT 0; `cargo clippy --workspace --all-targets -- -D warnings` → EXIT 0
- `cargo test --workspace` → all green, **0 failed** (uia 16 + 2-ignored; store/traits/kernel/etc.)
- **`cargo test -p uia -- --ignored`** (real desktop) → passes **bounded** (312 ms on a heavy window
  that timed out the bulk-fetch variants; 103 ms on a light one), spans normalized + `TextSource::Uia`.
- **Live `npm run tauri dev`** (`RUST_LOG=info,uia=debug`, real captures): DB `frame_text.primary_source
  = 'uia'` on **Chrome** frames (1186–1748 chars); every UIA Chrome frame's `content_text` contains a
  URL (omnibox `ValuePattern` — the Finding-1 fix); **no over-budget warnings, no COM errors**; thin/
  gated frames fall to OCR as designed.

### Still risky
- The walk path stays CI-dark by nature (needs a real desktop); the `#[ignore]`d test is the live gate.
  The granular design has the same boundedness profile as the long-shipped DFS, lowering the risk.

### Review fixes — 2026-07-01 (PR #68)
- **Stale doc-comments (5 bot-flagged + 2 adjacent).** Comments in `worker.rs`/`lib.rs`/`classify.rs`
  still named the *rejected* `FindAllBuildCache` prototypes ("single-round-trip", "one tree level at a
  time") instead of the shipped per-node `BuildUpdatedCache` walk. Reworded; comment-only.
- **Raw-view cache filter (`chatgpt-codex-connector`, P2).** A cache request's `TreeFilter` defaults to
  the **control-view** condition ("caching is performed only for elements that appear in the control
  view" — MS docs). With `capture.uia_view_control_only` **off** the walk navigates via `RawViewWalker`,
  so a raw-only node's requested properties were skipped by the default filter → `Cached*` empty →
  text silently lost to OCR. Fix: `build_cache_request` now takes the view flag and calls
  `SetTreeFilter(ControlViewCondition | RawViewCondition)` to stay in lock-step with the walker. The
  control-view (default) path is unchanged — control view *is* the default filter, now set explicitly.
  Verify: `fmt`/`clippy`/`cargo test -p uia` → EXIT 0; live `--ignored` control-view path non-regressed
  (3× consecutive: **282 spans / 6316 chars / ~90 ms**, well inside the 300 ms ceiling). The raw-view
  path is off-by-default and CI-dark like the rest of the walk; the fix is MS-doc-recommended.
- **Don't prefetch field values before the privacy guard (`chatgpt-codex-connector`, P2).** Caching
  `UIA_ValueValuePropertyId` made `BuildUpdatedCache` prefetch every walked node's field value —
  password/offscreen fields included — **before** `should_emit` runs, so a masked/hidden value was
  pulled into-process even though it's never emitted. That regressed the crate's visible-only /
  "password fields are never read" guarantee vs. the pre-#71 live walk, which read `Value` only after
  the guard. Fix: dropped `ValueValue`/`ValuePattern` from the batched cache; `extract_text` reads
  `Value` **live** via `GetCurrentPattern(UIA_ValuePatternId)` + `CurrentValue()`, and it is only
  reached after `should_emit` — so masked/hidden values are never fetched (exact pre-#71 parity).
  `Name`/metadata stay batched (the bulk of nodes are static text); value-bearing inputs are a small
  live-read fraction, and `_Full` cache mode already keeps the live backing `GetCurrentPattern` needs.
  This supersedes the earlier "cache the `ValueValue` property" defect fix. Verify: `cargo fmt -p uia
  -- --check` EXIT 0; `cargo clippy -p uia --all-targets -- -D warnings` EXIT 0; `cargo test -p uia`
  16 passed / 2 ignored; live `--ignored` walk still yields text (4 spans / 30 chars, foreground app).

## Pass — 2026-07-01 — Degrade-to-text DB shrink: merge purged spans to lines (#73a) (`fix/degrade-to-text-db-growth`)

From a `/superpowers:brainstorming` design (plan approved). Second of three (#8 shipped, #71 next). TDD.

### Implemented
- **#73a — degrade-to-text now shrinks the DB, not just disk.** Discovery that reshaped the plan:
  `text_spans` are **not** dead weight for purged frames — they are the sole data source for
  `FrameReconstruction` (`MomentDetail.tsx` renders it in place of the image when `image_purged`). So
  instead of *pruning* spans (which would gut the just-shipped feature) we **merge** per-word spans to
  per-line for purged frames: new pure `merge_spans_to_lines` (group by `line_index`, union bbox,
  join text, content-wins role, any-searchable) + store `merge_frame_spans_to_lines` (one txn:
  read→merge→`replace_text_spans`). Wired into the retention sweep (`run_retention_once`, after
  `purge_frame_image`, non-fatal) + a one-time watermark-gated backfill `merge_purged_spans_once`
  (`maintenance.purged_spans_merged`) for the pre-existing backlog, backed by new
  `store::purged_frame_ids` (cursor-batched). Reclaims ~80% of span rows; search untouched (FTS reads
  `content_text`, vector arm reads `embeddings`).
- **Adversarial review fix (low, CONFIRMED).** The 3-lens review confirmed merge correctness, wiring,
  and consumer-safety, and found one real defect: `merge_purged_spans_once` set the completion
  watermark even when *individual* frames failed to merge (a divergence from the `purge_self_captures`
  retry pattern it cited). Fixed: track a `clean_drain` flag; the watermark is written only on a
  fully clean drain, so any list- or per-frame failure retries the whole (idempotent) backfill next
  launch. Added a happy-path + watermark + idempotency test in `screensearch_lib`.
- **PR #67 external review fix (Codex P2, CONFIRMED).** The retention sweep degraded a frame in two
  writes — `purge_frame_image` (sets `image_purged = 1`) then a non-fatal `merge_frame_spans_to_lines`.
  A transient merge failure *after* the flag was set stranded the frame's per-word rows: the sweep
  (`WHERE image_purged = 0`) and, once its watermark was set, the backfill both skip it forever. Fixed
  by making degrade **atomic** — new `store::degrade_frame_to_text` merges spans **and** sets the flag
  in one transaction; on failure nothing commits and the frame retries next sweep. Two new store tests
  (`degrade_frame_to_text_merges_spans_and_purges_atomically` / `_purges_even_without_spans`). The two
  Gemini "N+1 → bulk `IN`" comments were **declined** (embedded SQLite; cold paths; a bulk txn forfeits
  the per-frame failure isolation the backfill needs to converge) and recorded as `TODO.md` TODO-2.

### Verification (Windows, full CI sequence) — verbatim
_(original PR run below; re-verified verbatim after the PR #67 review fix, 2026-07-01: lint EXIT 0 /
build `✓ built in 1.70s`; fmt EXIT 0; clippy EXIT 0 `Finished … in 3.58s`; build EXIT 0
`Finished … in 16.26s`; `cargo test --workspace` **0 failed**, store integration now **54** incl. the 2
new `degrade_frame_to_text_*`; bindings diff clean.)_
- `cd ui && npm run lint` → `LINT_EXIT=0`; `npm run build` → `✓ built in 1.54s`
- `cargo fmt --all -- --check` → `FMT_EXIT=0`
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished … in 1.39s` / `CLIPPY_EXIT=0`
- `cargo build --workspace` → `Finished … in 20.90s` / `BUILD_EXIT=0`
- `cargo test --workspace` → all suites green, **0 failed** — store **18** lib (4 new
  `merge_spans_to_lines` unit) + **54** integration (`merge_frame_spans_to_lines_*`,
  `degrade_frame_to_text_*`, `purged_frame_ids_*`); `screensearch_lib` **8** (new
  `merge_purged_spans_once_merges_backlog_then_watermarks_and_is_idempotent`); traits 53; uia
  16/2-ignored; sysmon 11; textfilter 12; kernel; ocr 1
- `git diff --exit-code -- ui/src/bindings` → `BINDINGS_CLEAN_EXIT=0` (no ts-rs types touched)

### Skipped / deferred
- Live Moment-render confirmation (optional in the plan): `FrameReconstruction` is unchanged and pure
  — it renders whatever `get_frame_spans` returns, which the integration test verifies is line-level
  with intact text + geometry. Not staged live (would need a rebuilt binary + a forced-purged frame).
- Row #73 (b) lossy codec and (c) vision-on-degraded-frames remain deferred (unchanged).

### Still risky
- Coarser `text_filter_stats` / `backfill_filter_version` granularity for already-purged, past-
  retention frames (accepted tradeoff; `reconcile` reclassifies line-level spans without error).

## Pass — 2026-07-01 — Vector-arm time-range recall: adaptive KNN escalation (#8) (`fix/vector-arm-time-range-recall`)

From a `/superpowers:brainstorming` design (plan approved): close reachable `07` gaps. First of three
(then #73a, #71). TDD.

### Implemented
- **#8 — vector arm no longer misses in-range matches buried beyond the pool.** `text_knn_in_range`
  (`crates/store/src/search.rs`) previously ran one KNN at `k = pool` then post-filtered the time
  window on the join, silently dropping in-range vectors ranked beyond the top-`pool` nearest. Pushing
  the filter into `MATCH` is impossible on sqlite-vec 0.1.9 (no in-KNN filtering; 0.1.10-alpha is
  broken), so a **bounded** range now escalates `k` geometrically (`KNN_ESCALATION_FACTOR`=8, ceiling
  `MAX_TIME_RANGE_KNN`=20 000) until the pool fills with in-range frames, the KNN exhausts the table
  (returned `< k` rows), or `k` hits the ceiling; an **unbounded** range keeps the single `k = pool`
  pass (the time filter is then a no-op). Wrote `vector_arm_finds_in_range_match_buried_beyond_pool`
  first — 55 nearer out-of-window vectors bury the only in-window match at rank 56, past the `pool=50`
  floor — observed **red** (`got []`, want `[56]`), then green after the fix. Added `vec_at_angle`
  (graded cosine distances, unlike `one_hot`'s 0/1).
- **Adversarial review clean.** A 3-lens workflow (loop-termination/bounds, semantic parity with the
  old single-pass, exhaustion-signal + edge cases) returned **no findings**.

### Verification (Windows, full CI sequence) — verbatim
- `cd ui && npm run lint` → `LINT_EXIT=0`; `npm run build` → `✓ built in 2.11s` / `BUILD_EXIT=0`
- `cargo fmt --all -- --check` → `FMT_EXIT=0`
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished … in 9.12s` / `CLIPPY_EXIT=0`
- `cargo build --workspace` → `Finished … in 24.70s` / `BUILD_EXIT=0`
- `cargo test --workspace` → all suites green, **0 failed** (store **50** integration incl. the new
  test + 14 lib; traits 53; uia 16/2-ignored; sysmon 11; textfilter 12; kernel pipeline 6 / settings 6
  / throttle 2; screensearch_lib 7; ocr 1; e2e/perf ignored)
- `cargo test -p store --test perf -- --ignored` → `median = 31.3853ms, p95 = 80.3555ms` (< 200 ms bar)
- `git diff --exit-code -- ui/src/bindings` → `BINDINGS_CLEAN_EXIT=0` (store-only change, no ts-rs types)

### Skipped / deferred
- Pushing the time filter into the KNN via a vec0 metadata/partition column — still blocked by
  sqlite-vec 0.1.9 (documented in the `07` #8 row). The escalation is the schema-free close.

### Still risky
- A pathologically tight window on a very large vector table *past* the 20 000 `k` ceiling can still
  under-count (bounded residual, `07` #8). Unobserved at the 10k-fixture scale; the ceiling is a tunable
  constant.

### Review response — 2026-07-01 (PR #66, Codex P2 — count-capped escalation target)
Codex flagged that for a **sparse** bounded window (fewer distinct embedded frames than `pool`) on a DB
with more than `MAX_TIME_RANGE_KNN` vectors, neither the pool-fill (`ids.len() >= pool`) nor the
exhaustion gate (`raw < k`) can ever fire, so every such query/report climbed to the 20 000 `k` ceiling
even after already finding all in-window matches. Fixed by capping the escalation **target** at the
count of distinct embedded frames actually in the window:
- New `count_embedded_frames_in_range(conn, start, end, cap)` — an `EXISTS` semi-join, index-served
  (`EXPLAIN QUERY PLAN` → `SEARCH fr USING COVERING INDEX idx_frames_captured_at` +
  `SEARCH m EXISTS USING COVERING INDEX idx_embeddings_frame`), `LIMIT cap`-bounded so it stays O(pool)
  even on a wide, densely-embedded window (the P2 addressed the residual per-query cost of an uncapped
  count directly). `target = min(pool, count)`; `count == 0` skips the KNN entirely.
- Extracted the loop into a pure, unit-testable `escalate_in_range_knn(pool, target, fetch)`.
- TDD: 5 pure escalation unit tests (`escalating_knn_*`) written first — observed **3 red** (naive
  single-pass: no escalation / no ceiling climb / no truncation) → green after the real loop; new
  `count_embedded_frames_dedups_chunks_and_honors_cap` (distinct-frame count + `LIMIT` cap + empty
  window); integration `sparse_time_window_returns_every_in_window_match`,
  `dense_time_window_returns_the_pool_nearest_in_window_matches` (target caps to `pool`, nearest-first
  preserved), `empty_time_window_returns_nothing_via_vector_arm`.
- **Adversarial re-review (3-lens: target-correctness / SQL-race-index / edge-perf, refute-by-default
  verify pass):** one **LOW** finding — the *uncapped* count's O(window) cost — which the `LIMIT` cap
  above already resolves; no correctness findings.

Verification — verbatim (Windows, full CI):
- `cargo fmt --all -- --check` → clean (exit 0)
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished … in 2.28s` / exit 0
- `cargo build --workspace` → `Finished … in 8.89s` / exit 0
- `cargo test --workspace` → all suites **0 failed** (store 53 integration + 20 lib incl. the 5
  escalation + count-cap unit tests)
- `cargo test -p store --test perf -- --ignored` → `median = 27.3359ms, p95 = 65.8744ms` (< 200 ms)
- `git diff --exit-code -- ui/src/bindings` → clean (store-only change, no ts-rs types)

### Follow-up review response — 2026-07-01 (PR #66, Codex P2 — bound the pre-count scan)
Codex's next review correctly refuted the claim above that the `LIMIT cap` kept the count O(pool). The
`LIMIT` is on **matches**, so it only short-circuits after finding `cap` *embedded* frames. A window
with many captured frames but few embedded ones (an `embed_text` backlog, or a wide multi-day range)
never fills the `LIMIT`, so SQLite walked the whole `frames` range running the `EXISTS` probe per row —
O(frames-in-window), not O(cap). The intended guard didn't hold in exactly the sparse regime it targets.
Fixed by bounding the **frames examined**, not just the matches:
- `count_embedded_frames_in_range` now takes `(pool, scan_cap)` and returns `Option<usize>`. The inner
  select is `LIMIT scan_cap`; the outer query returns `(scanned, embedded)` via `COUNT(*)` +
  `COALESCE(SUM(has_emb), 0)`. If `scanned == scan_cap` the window is too large to prove sparse within
  budget, so it returns `Some(pool)` — the **dense** assumption, a safe over-estimate that can only
  *raise* the escalation target, never stop it early on an in-range match. Otherwise the whole window
  was scanned, so `embedded` is exact: `None` when zero (skip the KNN), else `Some(min(pool, embedded))`.
- `COUNT_SCAN_CAP = MAX_TIME_RANGE_KNN` (20 000): the count never examines more frames than a single
  ceiling KNN pass would examine vectors, and it still fully (exactly) scans any realistic short/medium
  window. Net: the pre-count is now genuinely O(cap) even on a wide, sparsely-embedded window.
- **Correctness argument (no missed matches):** the dense fallback only ever returns `pool ≥
  min(pool, n)`, so `target` is never *below* the true window count — the `ids.len() >= target` stop
  can't fire before every in-window frame is gathered. Escalation still terminates on `raw < k` or the
  `k` ceiling. Output-equivalent to the prior code on every window that fits the scan budget.
- TDD: rewrote `count_embedded_frames_dedups_chunks_caps_and_bounds_the_scan` with a scan-budget case
  (scan_cap 2 < 4 in-window frames → asserts `Some(pool)`, not the small exact count). Observed **red**
  (naive form returned `Some(2)`) → green after the scan-cap-hit branch. `escalate_in_range_knn` and all
  its unit tests are unchanged (the target contract is identical).

Verification — verbatim (Windows, full CI):
- `cargo fmt --all -- --check` → clean (exit 0)
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished … in 4.58s` / exit 0
- `cargo build --workspace` → `Finished … in 15.09s` / exit 0
- `cargo test -p store` → lib **20 passed / 0 failed**, integration `store.rs` **53 passed / 0 failed**
- `cargo test -p store --test perf -- --ignored` → `median = 26.9464ms, p95 = 68.57ms` (< 200 ms)
- `git diff --exit-code -- ui/src/bindings` → clean (store-only change, no ts-rs types)

## Pass — 2026-06-30 — Cancel Inno (#26) + single-instance focus + a11y matrix (#42) (`chore/cancel-inno-and-a11y-matrix`)

From a `/superpowers:brainstorming` design (plan approved): close three ready `07` gaps.

### Implemented
- **#26 — Inno/portable-ZIP/MSI dropped, gap closed.** Doc-only sweep of 9 live refs to the v0.1.0
  reality (unsigned NSIS); DoD §13.9 re-scoped to NSIS and met; code-signing kept as the lone open
  packaging item. Logged as the spec-contradiction resolution `06` #16.
- **Single-instance focus.** `src-tauri/src/lib.rs` callback now `show()`s before `unminimize()`/
  `set_focus()`. *(Defensive: the app has no tray-hide path today, so the distinct hidden-window effect
  isn't reachable to stage live; build/clippy/test clean and the ordering is correct.)*
- **#42 — five a11y fixes:** NavRail roving-tabindex + `aria-current="page"`; Command Palette
  focus-restore on close; Recall Ask focus-to-answer; Settings `<Panel group>` (`role="group"` +
  `aria-labelledby`). NavRail + Palette **live-verified** via a Playwright focus probe; Settings-group +
  Ask-focus build/code-verified (need live backend data).

### Verification (Windows) — verbatim
- `npm run lint` → `EXIT 0`; `npm run build` → `✓ built in 1.96s`
- `cargo fmt --all -- --check` → `EXIT 0`
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile … in 53.41s` / `EXIT 0`
- `cargo build --workspace` → `Finished dev profile … in 22.11s` / `EXIT 0`
- `cargo test --workspace` → every suite `ok`, **0 failed** (inference 102, traits 53, store 49+14,
  kernel 27, capture 27, uia 16/2-ignored, sysmon 11, textfilter 12, screensearch_lib 7, embeddings 1,
  ocr 1)
- `git diff --exit-code -- ui/src/bindings` → clean (`EXIT 0`)
- **Playwright focus probe:** NavRail `{Deck tabIndex 0, aria-current page}`, ArrowDown→Recall (tabIndex
  follows), End→Settings, ArrowDown wraps→Deck, ArrowUp wraps→Settings; Command Palette Ctrl+K→
  `role=combobox` input, Esc→focus restored to the ⌘K `BUTTON`; forced `?__devState=error` renders with
  NavRail un-trapped.

### Skipped / deferred
- Live data-backed keyboard pass of Timeline scrub/open, Recall results, Moment actions in the real
  WebView2 app — Playwright can't attach to WebView2 and plain Chrome has no captured frames; those
  handlers are pre-existing + ARIA-correct + unchanged, so it stays a low-risk manual residual (`07` #42).
- Recall results arrow-key roving nav — offered, declined as YAGNI (results are already Tab-reachable
  links; `UI_REFERENCE` §7 is met).

---

## Pass — 2026-06-30 — Model-downloader resume hardening (`fix/download-resume-hardening`)

Closed two open durability gaps in `crates/inference/src/download.rs` (download-hardening scope, per
user), both via TDD. The user also asked to drop the stale `#74` row (its only residual was re-tagging
dead `vision_tag` jobs in a throwaway dev DB — a don't-care; resolution history stays in the PR #61
records).

### Implemented
- **#69 — wrong-sized `.part` no longer publishes garbage.** `open_preallocated` now reports a part as
  `unbacked` when its pre-existing on-disk length is `!= total` (brand-new, externally truncated, or
  corruption-grown larger), not just when it created the file; the chunked-download caller discards the
  stale `.parts` bitmap in that case and refetches every chunk. No false positives on a legitimate
  resume — a real interrupted part is always preallocated to exactly `total`. Wrote
  `truncated_part_discards_stale_partial_manifest` first (observed red: published file all-zeros), then
  the fix (green); `oversized_part_discards_stale_manifest` covers the `> total` case (the `< total`→
  `!= total` broadening came from a PR #62 review note).
- **Cache re-check on lock retry (PR #27 Codex-P2).** Extracted the clean-layout + HF-cache fast paths
  into `place_if_cached`; folded the single-stream lock-retry into `fetch_one` so each `LockAcquisition`
  backoff re-checks the cache and short-circuits if the holder finished mid-sleep. Extended the backoff
  (`LOCK_RETRY_BACKOFF_CAP` 15 s; `LOCK_RETRY_MAX_ATTEMPTS` 5→24 ≈ 5 min) so a real multi-GB download is
  outlasted, not abandoned at ~20 s. Added `place_if_cached_*` unit tests. The doc-hidden
  `download_file_with_lock_retry_for_diagnostics` (the `examples/repro_8b.rs` entry point) keeps a
  minimal inline backoff loop.

### Verification (Windows, after the PR #62 review fix) — verbatim
- `cargo test -p inference --lib` → `test result: ok. 102 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.05s`
- `cargo fmt --all -- --check` → `EXIT 0`
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile … in 2.43s` / `EXIT 0`
- `cargo build --workspace` → `Finished dev profile … in 8.90s` / `EXIT 0`
- `cargo test --workspace` → every suite `ok` (store 49+14; inference 102 lib + integration; traits 53;
  uia 16/2-ignored; sysmon 11; textfilter 12; kernel; screensearch 7; e2e/perf/smoke ignored) / `EXIT 0`
- `git diff --exit-code -- ui/src/bindings` → clean (`EXIT 0`)

### Skipped / deferred
- The `#46` row proper (orphaned **detached** chunk writers in the single-stream hf-hub fallback) — left
  open; the real fix replaces hf-hub's high-level `download_with_progress`, out of this scope.
- "Gemini — single-instance focus" PR #27 follow-up — out of the download-hardening scope; still open.
- **Declined a PR #62 review note** (Gemini, medium): wrap `place_if_cached` in `tokio::task::spawn_blocking`.
  `place_if_cached` is a verbatim extraction of code that shipped throughout 0.2.x; its only heavy op (the
  multi-GB copy) is already offloaded via `place_in_clean_layout_async` (which exists precisely for that),
  and the rest are stat-level (`exists`/`metadata`/`Cache::get`) — the same inline-stat pattern used across
  `fetch_one`/`chunked_download`. Gemini's rewrite would re-implement that offload inline and diverge from
  the established pattern for no measurable gain. The sibling note (broaden the size check to `!= total`) was
  **applied**.

### Still risky
- The lock-retry / `place_if_cached` HF-cache branch can't be unit-tested portably (a valid hf-hub cache
  layout needs symlinked snapshots/blobs; Windows-restricted). It's an unchanged extraction of
  already-shipped working code; the dest-present and miss branches *are* unit-tested. The
  `LockAcquisition` path itself is network/contention-dependent and exercised by `examples/repro_8b.rs`,
  not CI.

## Pass — 2026-06-30 — Spec archival sweep + close reachable gaps #43/#44 (`chore/archive-known-gaps`)

From a `/superpowers:brainstorming` design (plan approved): shrink `07_KNOWN_GAPS.md` by restoring
the archive-on-release convention, and close the two reachable open gaps.

### Implemented
- **Archive-on-release brought current.** Only v0.1.0 had been archived; the shipped 0.2.0/0.2.1/0.2.2
  history still sat in the live logs. Moved it out **verbatim** (original `#N` ids preserved) into
  per-log `*.v0.2.x.md` archives: `07` (18 resolved + 5 accepted-as-is rows + the
  resolved-engineering-decisions list; 35 rows → 12), `05`, `08`, `06` (keeps its one open
  upstream-leak row #15), and `CHANGELOG.md` → `CHANGELOG-ARCHIVE.md` (the 0.2.x sections that had
  piled under `[Unreleased]`). Verified byte-identical to `git HEAD` (the project's own archival
  check) and that every `#N` cross-reference still resolves live-or-archived.
- **#44 — privacy-safe VLM image-path log.** One `tracing::info!(frame_id, image_path=…)` in
  `crates/kernel/src/worker_pool.rs::vision_tag_outcome`, immediately before `vision.analyze`. At
  `info` (the default `EnvFilter`), so it reaches `screensearch.log` — a `debug!` was why the audit
  scan found nothing. Logs only the frame id + relative path; no screen content (the inference client
  never sees the path, so the kernel is the only correct layer).
- **#43 — dev-only deterministic route-state triggers.** A `?__devState=loading|error` URL param
  forces any P5 route into that state, applied centrally at the `ui/src/lib/ipc/queries.ts` `useQuery`
  seam (all 17 read hooks; 0 mutations), dev-gated by `import.meta.env.DEV` and tree-shaken from prod.
  New `ui/src/lib/dev/{devState,useDevStateOverride}.ts` + `DevStateBadge.tsx`; documented in
  `docs/DEV_STATE_OVERRIDE.md`. Forces result *flags* only — empty/partial/populated stay real
  (no mocks in the production path).

### Verification (Windows, full CI sequence) — verbatim
- `cargo fmt --all -- --check` → `EXIT 0`
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile … in 14.12s` / `EXIT 0`
- `cargo build --workspace` → `Finished dev profile … in 24.95s` / `EXIT 0`
- `cargo test --workspace` → all suites green (kernel 27; `kernel --test enrichment` **10 passed**
  incl. `process_job_vision_tag_writes_analysis`; store 14+49; inference 95; traits 53; uia 16; sysmon
  11; textfilter 12; capture 27; e2e/perf/smoke ignored on this host) / `EXIT 0`
- `cd ui && npm run lint` → eslint clean (Rules-of-Hooks gate) / `EXIT 0`
- `npm run build` → `✓ built in 1.81s`
- prod-strip: `grep -rl __devState ui/dist/assets/` → **ABSENT** (dev override tree-shaken out)
- `git diff --exit-code -- ui/src/bindings` → clean (`EXIT 0`)

### Skipped / deferred
- The 10 still-open `07` rows (hardware-only checks, upstream fixes, future features, accepted
  trade-offs) are unchanged. The other reachable-but-heavier idea (`07` #43's seeded-fixture harness)
  was deliberately not built — the dev-only flag override is the lower-risk close the gap asked for.

### Still risky
- `#43` is dev-only by construction (stripped from prod, proven by the `dist` grep); the documented
  per-route manual screenshot pass (`docs/DEV_STATE_OVERRIDE.md`) is the live acceptance and was not
  run headless here.

### Follow-up — PR #60 review fix (`#43` prod-path correctness)
Three reviewers (Codex P3, Gemini high, Claude bot) independently flagged the same real defect:
`useMaybeOverride` called `useSearchParams()` **before** the `import.meta.env.DEV` guard. Because the
helper is invoked from the production `queries.ts` call-site, that hook call is *not* tree-shaken — so
release builds subscribed all 17 query consumers to router-history changes (extra re-renders on every
client-side navigation), and coupled every global read query to a `<Router>` context (crash risk in
hook usage outside a Router). The `dist` grep had only proven the `__devState` *string* was stripped,
not the hook call.
- **Fix:** drop `useSearchParams`; read `window.location.search` directly *inside* the DEV guard
  (`ui/src/lib/dev/useDevStateOverride.ts`). The helper now calls **no** React hook, so the early
  return makes the production path a plain `return result` identity — no subscription, no Router
  coupling, and no Rules-of-Hooks concern (nothing conditional is a hook). `readDevState` already
  accepted a raw `location.search` string (leading `?` handled by `URLSearchParams`). Doc + CHANGELOG
  updated to state the stronger guarantee.
- **Verification (verbatim):** `npm run lint` → `LINT_EXIT=0`; `npm run build` → `✓ built in 1.55s` /
  `BUILD_EXIT=0`; `grep -rl __devState dist/assets/` → **absent** (`GREP_EXIT=1`);
  `grep -rl "dev: forced route error" dist/assets/` → **absent** (the whole override module, not just
  the param string, is gone); `git diff --stat -- ui/src/bindings` → clean.

## Pass — 2026-06-30 — Fix vision context overflow on full-res frames (`fix/vision-fullres-ctx-overflow`)

User-reported symptom: vision tagging "failing for the first time" and "significantly slower," while
the model appeared to use "merely 3 GB RAM" (a "miracle?"). A live read of the running dev session
proved it was **not** a memory win — it was a regression.

### Diagnosis (evidence)
- The sidecar was fully GPU-resident (6.4 GB dedicated VRAM on the RTX 5060 Ti; `-ngl 99`,
  `--ctx-size 4096`), so the low footprint was the existing memory-tuning, not magic.
- A faithful reproduction (real captured frame → JPEG q80 → exact request body) returned
  `HTTP 400 — {"error":{"message":"request (4148 tokens) exceeds the available context size (4096
  tokens)…","type":"exceed_context_size_error","n_prompt_tokens":4148,"n_ctx":4096}}`. Frame
  3440×1440 (native, from #73). The DB had `vision_tag` **72 dead / 0 done**.
- The error was hidden: the worker recorded only the collapsed top context (`"vision completion"`),
  and the sidecar's stderr was discarded entirely (`process.rs` spawned with `CREATE_NO_WINDOW`,
  no redirect).

### Implemented (`07` #74; TDD red→green per change)
- **Downscale the VLM image** to a 1568 px longest edge in `vision::encode_data_url` (captures keep
  full resolution). Tests: oversized 3440×1440 → 1568×656; small frame passes through unscaled.
- **Vision auto-ctx left at the spec default 4096** (`models.rs`). _(An initial 4096 → 8192 "safety
  net" bump was reverted after the PR #61 Codex review — see the follow-up below.)_
- **Surface the real cause:** `vision_tag_outcome` formats with `{e:#}` (`worker_pool.rs`). Test:
  a failing provider's chained error now lands `exceed_context_size_error` in `jobs.last_error`
  (`kernel --test enrichment`).
- **Capture sidecar stdout/stderr** to `<sidecar dir>/llama-server.log` (`process.rs` inheritable
  log handle + `SupervisorConfig.sidecar_log`, wired in `src-tauri/src/lib.rs`). Test: a real child
  (`cmd /c echo …`) writes to the log and is read back.

### Verification (Windows, full CI sequence) — verbatim
- `cargo fmt --all -- --check` → `FMT_EXIT=0`
- `cargo clippy --workspace --all-targets -- -D warnings` → `CLIPPY_EXIT=0`
- `cargo build --workspace` → `Finished … in 27.08s` / `BUILD_EXIT=0`
- `cargo test --workspace` → `TEST_EXIT=0` (inference **98 passed** incl. new vision/process tests;
  `kernel --test enrichment` incl. `vision_tag_failure_records_full_error_chain`)
- `cd ui && npm run lint` → `EXIT 0`; `npm run build` → `✓ built in 1.64s`
- `git diff --exit-code -- ui/src/bindings` → `BINDINGS_DIFF_EXIT=0`
- **Live E2E** (`npm run tauri dev`, new binary): this run was on the interim 8192 build, so the
  sidecar launched `--ctx-size 8192` (`n_ctx_slot = 8192` in `llama-server.log`); `vision_tag done`
  0 → 8; faithful downscaled request → **HTTP 200** in 2.8 s, `prompt_tokens 1159`, content
  `{"description":"…Visual Studio Code…","activity_type":"coding","confidence":0.95}`. The measured
  **1159 prompt tokens** is what matters here: it sits far under the reverted **4096** default, so the
  downscale alone clears the overflow without the ctx bump.

### Still risky / follow-up
- The ~115 `vision_tag` rows that dead-lettered **before** the fix stay dead (terminal state) — a
  manual requeue is needed to re-tag those frames (`07` #74 residual).
- 1568 px lands ~1009 image tokens — right at the model's logged ≥1024 grounding recommendation;
  fine for holistic tagging. The worst case (a square frame) is ≤ 1568×1568 ≈ 2.46 MP → ~2.5 K
  prompt tokens, still under 4096; `sidecar.ctx_size` remains the power-user knob for more headroom.

### Follow-up — PR #61 review fix (drop the per-frame full-res clone)
Gemini flagged `downscale_for_vlm` (`vision.rs`) cloning the whole `RgbaImage` (~20 MB for a
3440×1440 capture) on every call, then discarding the clone during resize. Fixed by checking the
dimensions on the borrowed frame and, when it overflows, calling `image::imageops::resize(img, …)`
directly on the reference — no clone on the (common, ultra-wide) resize path; the pass-through
branch still clones the already-small frame. The fitted dimensions reuse the same round-to-nearest
scale `DynamicImage::resize` applied (`ratio = VISION_MAX_EDGE / longest edge`), so the cap math is
byte-for-byte identical and the two existing tests stay green unchanged.
- **Verification (verbatim):** `cargo fmt --all -- --check` → `FMT_EXIT=0`;
  `cargo clippy -p inference --all-targets -- -D warnings` → `CLIPPY_EXIT=0`; `cargo test -p
  inference --lib` → `98 passed; 0 failed` (incl. `downscales_oversized_frame_to_max_edge`
  3440×1440 → 1568×656 and `small_frame_passes_through_at_native_size`).

### Follow-up — PR #61 review fix (revert the vision auto-ctx bump; Codex P2)
Codex flagged that bumping the vision auto-context 4096 → 8192 contradicts the spec contract
(`03 §8:438` vision auto = 4096; `§:522` "`sidecar.ctx_size` … **not** bumped by default") and
raises KV-cache VRAM on weak GPUs for *every* tag — and since this PR already downscales the request
image, the bump is unnecessary. Reverted `default_ctx_for(ModelLane::Vision)` back to **4096**
(`models.rs`) and restored the per-lane test assertion (`vision auto ctx == 4096`; answer stays
8192). The downscale alone fixes the overflow: the 1568 px cap bounds the worst case (a square
frame) to ~2.5 K prompt tokens < 4096, and the live run measured only 1159. `sidecar.ctx_size`
remains the documented power-user VRAM knob. Doc/CHANGELOG updated to drop the bump.
- **Verification (verbatim):** `cargo fmt --all -- --check` → `FMT_EXIT=0`;
  `cargo clippy -p inference --all-targets -- -D warnings` → `CLIPPY_EXIT=0`; `cargo test -p
  inference --lib` → `98 passed; 0 failed` (incl. `auto_ctx_size_resolves_per_lane_and_override_passes_through`).

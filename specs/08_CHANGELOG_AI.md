# 08 — AI Changelog

> Append-only record of what the agent changed during the build, **with reasons**. One entry per
> meaningful change set. Empty until P0 begins. (This tracks build work; the design-phase history
> lives in git.)

## <date> — <short title>
- **Change:** what was added/modified.
- **Why:** the reason, tied to a spec section.
- **Verification:** the command run + verbatim result.

---

## 2026-06-27 — 0.2.0 PR3 audit fix: self-exclude + backfill + excluded-apps hot-apply (`fix/0.2.0-pr3-chrome-backfill`)
- **Change:** Resolved the PR3 attention-filter release blocker (`docs/AUDIT_0.2.0_PR3_2026-06-26.md`,
  gap #64) across four coordinated changes:
  1. **Self-exclude own window** — `capture::privacy::is_own_foreground_window()` (PID == current
     process) gates the capture loop so ScreenSearch never indexes its own UI; a gated one-time
     startup purge (`purge_self_captures` + `SqliteStore::frames_with_app_hint`) sweeps pre-existing
     own-window frames (CASCADE + JPEG cleanup), recorded by the `maintenance.self_capture_purged`
     watermark.
  2. **Backfill** — `FILTER_VERSION` 1→2; `SqliteStore::backfill_filter_version` replaces the old
     catalog-wipe `reconcile_filter_version`, re-cleaning sub-version frames against the warm catalog
     via the new pure, monotonic `textfilter::reconcile` (preserves positional roles; needs no
     `target_rect`; re-enqueues `embed_text` for changed frames). Runs batched in a background task at
     startup.
  3. **Cold-start** — `text.chrome_suppress_min_seen` default 12→4.
  4. **Excluded-apps hot-apply** — `Kernel::reload_capture()` + `set_settings` compares the derived
     `CaptureConfig` (now `PartialEq`) and restarts a running capture loop on change.
- **Why:** The audit proved (and live corpus replay confirmed) the dominant default-search chrome
  was ScreenSearch indexing its own window, plus a cold-start window the no-backfill design froze.
  The classifier's "rect unknown → suppress nothing" safety invariant means the catalog/backfill
  cannot place rect-None desktop chrome, so self-exclusion (user-chosen) carries the load for the
  app's own UI while the backfill cleans catalogued chrome. The excluded-apps bug (config frozen at
  capture start) was user-reported. `01 §5` Windows-only honored; no cross-platform stubs.
- **Verification:** Full suite green —
  `cd ui && npm ci && npm run lint && npm run build` (eslint clean, vite built),
  `cargo fmt --all -- --check` (clean), `cargo clippy --workspace --all-targets -- -D warnings`
  (clean), `cargo build --workspace` (ok), `cargo test --workspace` (all suites pass incl. new
  `textfilter` reconcile goldens, `store` backfill + `frames_with_app_hint`, `kernel`
  `reload_capture`), `git diff --exit-code -- ui/src/bindings` (clean). Live evidence: the shipped
  `backfill_filter_version` + self-purge replayed over a copy of the audit's 313-frame corpus
  (`.playwright-mcp/pr3-2026-06-26/screensearch-pr3-before.sqlite`) — default content FTS hits
  before→after: `Deck` 68→26, `Recall` 42→15, `Firefox`/`Steam` 24→15, `GPU Memory` 19→15; raw FTS
  still recovers all; `cargo test`=41 / `embeddings`=13 content hits preserved.

## 2026-06-26 — 0.2.0 PR6 audit checkpoint (`codex/0.2.0-pr6-audit`)
- **Change:** Created a PR6 audit checkpoint on `codex/0.2.0-pr6-audit` using the existing app DB.
  The ignored local audit file `docs/AUDIT_0.2.0_PR6_2026-06-26.md` and evidence directory
  `.playwright-mcp/pr6-2026-06-26/` record the baseline, online SQLite backup, static wiring review,
  targeted tests, and first real-dev-exe boot. Mirrored the tracked release notes into
  `docs/0.2.0.md`, `05`, `06`, `07`, this changelog, and `CHANGELOG.md`.
- **Why:** The 0.2.0 PR6 contract requires reports and premade Ask shortcuts to use current
  `content_text` paths by default, with honest empty/no-sidecar behavior and bounded map-reduce
  report coverage. This checkpoint found no PR6 wiring blocker by static review or targeted tests;
  the remaining release blocker is the upstream PR3 static/app-chrome leakage in default
  `content_text`.
- **Verification:** Targeted raw outputs are preserved under `.playwright-mcp/pr6-2026-06-26/`:
  `cargo test -p kernel reports -- --nocapture` (11 passed),
  `cargo test -p store sample_ -- --nocapture` (5 passed), and
  `cargo test -p inference report_summary -- --nocapture` (2 passed). The second live app session
  booted `target/debug/screensearch.exe` through `npm run tauri dev` and completed the UI pass:
  Search/Ask/Reports rendered, the five Ask cards were visible, Day Recap submitted with cited
  frames, Daily/Weekly/prompted-Custom/no-evidence-Custom reports generated, Settings showed
  `8/40/200/20`, a controlled Windows Notepad probe landed in `frame_text.content_text`, and the
  dev app/llama sidecar stopped without an observed orphan process. Full verification then passed:
  the first `cd ui && npm ci && npm run lint && npm run build` attempt failed with `EPERM unlink`
  because the dev run left a repo-local Vite/esbuild process holding `esbuild.exe`; after stopping
  only those matching repo-local processes, the same frontend gate passed on retry. The remaining
  gates all exited 0: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace`,
  `cargo test --workspace`, and `git diff --exit-code -- ui/src/bindings`.

---

## 2026-06-26 — 0.2.0 PR3 attention-first filtering audit (`codex/0.2.0-pr3-audit`)
- **Change:** Audited PR3 with the real dev app (`npm run tauri dev`) against the existing
  `%APPDATA%\app.screensearchv2c.desktop` DB, after an online backup and without reset/backfill.
  Added `docs/AUDIT_0.2.0_PR3_2026-06-26.md` and recorded the release-blocker notes in
  `docs/0.2.0.md`, `05`, `06`, `07`, this changelog, and `CHANGELOG.md`.
- **Why:** The 0.2.0 release plan requires PR3 to stop desktop icons, toolbars, app nav, and other
  static chrome from dominating default retrieval, while preserving raw recovery. The audit confirms
  the plumbing is mostly correct but the strict default-search acceptance still fails on both the
  populated corpus and a fresh Notepad capture.
- **Verification:** Raw output is preserved in `.playwright-mcp/pr3-2026-06-26/29-verify-ui-npm-ci-lint-build.txt`
  through `34-verify-bindings-diff.txt`; all required commands exited 0:
  `cd ui && npm ci && npm run lint && npm run build`, `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace`,
  `cargo test --workspace`, and `git diff --exit-code -- ui/src/bindings`.

---

## 2026-06-26 — Chunked download: reset stale manifest for ANY fresh part (`fix/chunked-download-partial-stale-manifest`)
- **Change:** Generalised the fresh-part stale-manifest guard in `crates/inference/src/download.rs`.
  The condition `part_created && manifest.pending_indices().is_empty()` (reinit only when the
  surviving bitmap is **all**-complete) became `part_created && manifest.any_complete()` (reinit when
  it marks **any** chunk complete). Added the `Manifest::any_complete()` helper, updated the guard's
  comment, and added the `fresh_part_discards_stale_partial_manifest` regression test.
- **Why:** A PR #35 bot review (codex P2) found the all-done guard missed the **partly-done** case: a
  header-matching `.parts` bitmap with some chunks marked `1` can survive over a brand-new zero-filled
  `.part` — e.g. an interrupted download whose multi-GB `.part` a user/cleanup tool later reclaims.
  The done-marked ranges were then skipped and left as zeros; the length check passes and sha256 is
  skipped when the CDN advertises no `X-Linked-ETag`, so a corrupt GGUF could be published (or, with a
  hash, an avoidable failed verify + retry). The code already asserts the invariant in its own comment
  ("a brand-new zero-filled `.part` cannot have any completed chunks") — the guard now enforces it
  fully. No schema/IPC/API change; Windows-only path unchanged.
- **Verification:**
  - `cargo test -p inference download`:
    ```
    test download::tests::fresh_part_discards_stale_all_done_manifest ... ok
    test download::tests::fresh_part_discards_stale_partial_manifest ... ok
    test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 59 filtered out; finished in 4.05s
    ```
    The new test is a genuine regression guard: with the old `pending_indices().is_empty()` condition
    restored it fails — `assertion left == right failed: published file must be the real bytes, not
    zeros in the done-marked ranges` — and passes only with the `any_complete()` fix.
  - `cargo fmt --all -- --check` → exit 0 (no diff).
  - `cargo clippy --workspace --all-targets -- -D warnings` → `Finished` with exit 0 (no warnings).

---

## 2026-06-25 — 0.2.0 PR7 integration audit (`codex/0.2.0-pr7-integration-audit`)
- **Change:** Ran the PR7 audit through the live dev app (`npm run tauri dev`, real
  `target/debug/screensearch.exe`) against the user's populated app-data DB. Captured local-only
  screenshots under `.playwright-mcp/pr7-2026-06-25/`, added
  `docs/AUDIT_0.2.0_PR7_2026-06-25.md`, updated architecture/testing docs, updated the human
  changelog, and recorded the PR7 findings in `05`/`06`/`07`.
- **Code change:** fixed one UI copy bug found during the audit: Daily report generation displayed a
  Weekly-only helper line. `ui/src/routes/Recall.tsx` now uses range-neutral bounded-pass copy.
- **Why:** `docs/0.2.0.md` PR7 requires a real integration audit over default content search,
  opt-in raw/app-chrome search, Ask, Reports, and a bounded live capture tick. The audit found two
  open acceptance/product-semantics gaps: strict static/app-chrome suppression is not fully closed
  on the populated corpus or fresh ScreenSearch UI captures (#62), and no-evidence Ask refusals
  still render retrieved context under `CITED FRAMES` (#63).
- **Verification:** final PR7 verification command output is recorded in the session response.

---

## 2026-06-25 — 0.2.0 PR6 — Recall reports + Ask shortcuts (CGCMR) (`feat/0.2.0-pr6-recall-reports`)
- **Change:** Added the third Recall capability — `generate_report(ReportRequest) -> ReportResponse`
  over the attention-first `content_text` — plus the removal of the hardcoded `ASK_TOP_K`, a Reports
  UI mode, and premade Ask cards.
  - **`crates/kernel/src/reports.rs` (new):** the **Calendar-Grid Coverage Map-Reduce (CGCMR)**
    orchestrator over `dyn Store` + `dyn AnswerProvider`. Range → per-calendar-day grid → one
    `timeline_buckets` density probe → adaptive `plan_depth` (per-active-period budget, floored at
    `MIN_FRAMES_PER_PERIOD`, capped at the period count, floor-wins over the global cap) → per-period
    even ASC sample → MAP (one summarize pass per active day) → bounded hierarchical REDUCE
    (`REDUCE_FANOUT`=6, time order preserved) → FINAL pass. Custom-with-prompt swaps the coverage
    sample for `hybrid_search(prompt, time_range)`. Single-pass fast path for small ranges;
    honest-empty with zero sidecar calls; cooperative `AtomicBool` cancel + progress callback.
  - **`crates/store` `sample_frames_in_range`** (Store trait + `SqliteStore`): even ASC stride across
    a window (`frames_in_range` is newest-first capped, so a new primitive was required).
  - **`crates/inference` `summarize`** (AnswerProvider trait + `AnswerSidecar`): non-streaming
    collected pass reusing a `pack_context` helper extracted from `build_messages` (Ask byte-identical);
    three report prompts; `answer_model_label`/`report_model_label` for the footer.
  - **IPC/commands:** `ReportKind`/`ReportRequest`/`ReportResponse`/`ReportProgress` (ts-rs, i64
    annotated, in the `no_bigint_in_ipc_types` guard); `generate_report` + `cancel_report`;
    `AskRequest.top_k` override; `ask` now reads `retrieval.default_top_k`.
  - **Settings:** the four §8 keys (`retrieval.default_top_k`, `reports.daily_top_k`,
    `reports.weekly_top_k`, `reports.map_reduce_min_frames`) with Default + load/save + clamps + UI.
  - **UI:** Reports mode (`ReportBuilder` computing a local `TimeRange`, `ReportView` with markdown +
    capped citation chips + Copy + `.md` download + honest footer), shared `CitationTile`,
    `PromptCardGrid` (5 premade Ask cards), `useReport` hook.
- **Why:** `03 §7` (`generate_report`), `§8` (the four settings keys, remove `ASK_TOP_K`), `§8b`
  (reports as map-reduce over `content_text`, cite frames), `docs/0.2.0.md` PR6. **The user's explicit
  directive (2026-06-25) — report context must scale with the time range and ensure temporal
  coverage, not just relevance** — drove the CGCMR design: a flat 8192 window for a week was rejected;
  instead `n_ctx` stays flat (VRAM-flat, `§8b`-compliant) and the *number* of 8192-bounded passes
  grows with the range, structurally guaranteeing per-active-day coverage. Deviations from the literal
  `§8b` text (per-day grid vs token-budget batches; strided coverage vs best-first relevance; recursive
  reduce; `daily/weekly_top_k` reinterpreted as per-period budget / global cap; additive
  `report_progress`/`cancel_report`; flat `n_ctx`) are logged in `06` patch #5, and three accepted
  limitations (DST grid skew, structural bounds as constants, estimated-token footer) in `07` #59–#61.
- **Verification:** UI `npm run lint` (exit 0) · `npm run build` → `✓ 407 modules transformed · ✓
  built in 1.44s` (exit 0); `cargo fmt --all -- --check` (exit 0); `cargo clippy --workspace
  --all-targets -- -D warnings` (exit 0, 0 warnings); `cargo build --workspace` (`Finished` 9.43s);
  `cargo test --workspace` (**0 failed across all crates**) incl. the 10 new `reports::` orchestrator
  tests (real `SqliteStore` + `FakeAnswer`: every active period ≥ floor, dense day capped, floor-wins
  on long ranges, weekly cites first+last day, reduce-overflow preserves all days, honest-empty = 0
  sidecar calls, cancel → Err), 4 store sampler tests, 3 inference summarize tests, and the extended
  kernel settings round-trip. `git diff --exit-code -- ui/src/bindings` is clean once the regenerated
  `AskRequest.ts`/`Settings.ts` + the 4 new `Report*.ts` are committed with the PR.

---

## 2026-06-25 — 0.2.0 PR3 review fixes (PR #32)
- **Change:** Addressed the two substantive bot findings on PR #32.
  1. **N+1 catalog query** (`crates/store/src/records.rs`) — replaced `SqlCatalog::seen_count` (one
     `SELECT` per candidate line, 10–30 per OCR frame on the hot path) with `load_chrome_catalog`,
     which pre-fetches the foreground app's catalog rows in a single
     `SELECT signature, seen_count … WHERE app_hint IS ?1` into a `HashMap<String, u32>` (already a
     `ChromeCatalog`). One bulk read replaces N point-lookups.
  2. **Unknown-rect over-suppression** (`crates/textfilter/src/lib.rs`) — static-chrome
     cataloguing/suppression now requires a known `target_rect`. Previously `target_rect = None`
     made `interior` false, so every short line became a chrome candidate and a repeated real body
     line could be dropped. Rect-less short lines now fall through to `unknown` (kept).
- **Why:** (1) hot-path DB efficiency (Claude P1 / Gemini high); (2) restores the documented
  invariant that a missing/wrong rect can only *under*-suppress, never silently lose content
  (Codex P2) — the project's top risk is false suppression (`03 §3b`).
- **Verification:**
  ```
  $ cargo fmt --all -- --check          # clean
  $ cargo clippy --workspace --all-targets -- -D warnings   # 0 warnings
  $ cargo test --workspace              # 0 failures; textfilter 7 (was 6), store 1+47
  $ git diff --exit-code -- ui/src/bindings   # clean (no traits/IPC change)
  ```
  New regression test `no_target_rect_never_suppresses_even_a_saturated_signature` asserts a
  catalog-saturated signature survives a rect-less frame.

---

## 2026-06-25 — 0.2.0 PR3: attention-first text filtering (`feat/0.2.0-pr3-text-filter`)
- **Change:** Implemented `03 §3b/§4/§8` (PR3 of `docs/0.2.0.md`) verbatim — replaced PR2's
  `content_text` passthrough with a real span-aware filter:
  1. **New crate `crates/textfilter`** — a pure, deterministic, `traits`-only classifier (no I/O, no
     Windows). `classify(input, catalog, config)` groups spans by `line_index` into lines (union
     bbox + centroid) and assigns one of five roles in priority order (**geometry before
     repetition**): `system` (short, bottom band `y > SYSTEM_BAND_TOP=0.95`, rect known & outside),
     `background` (rect known & outside), `chrome` (line == normalized `target_window_title`),
     static-chrome candidate (short + not interior, catalogue `seen_count+1 ≥ min_seen`),
     else `content` (rect known) / `unknown`. Builds the filtered `content_text` from the kept spans
     (not string-subtraction); long lines (≥ `chrome_protect_min_chars`) are never catalogued or
     suppressed for repeating; short interior content is never catalogued. Signature =
     `app_hint ⏐ normalized_text ⏐ region_bucket` joined by `U+001F`; `region_bucket` = line centroid
     in an N×N grid. 6 golden tests over an anonymized **synthetic** OCR fixture (`src/tests.rs`).
  2. **`crates/traits`** — `TextSpan` gains `line_index: u32`; `CapturedFrame` gains
     `target_rect: Option<[f32;4]>`; `Settings` gains `text_include_chrome_default` (false),
     `text_chrome_suppress_min_seen` (12), `text_chrome_protect_min_chars` (48),
     `text_chrome_region_buckets` (8); new IPC type `AppSuppression` (i64 fields `#[ts(type=number)]`,
     added to the `no_bigint_in_ipc_types` guard); new `TextFilterContext`. Three defaulted `Store`
     trait methods (`insert_ocr_filtered`, `text_filter_stats`, `reconcile_filter_version`) so fakes
     compile. ts-rs bindings regenerated (`Settings.ts`, new `AppSuppression.ts`).
  3. **`crates/ocr`** — `recognize_blocking` carries a per-line counter (`Lines().Words()`) into each
     word span's `line_index`.
  4. **`crates/store`** — schema `LATEST_SCHEMA_VERSION = 4`, `MIGRATION_V4` adds
     `text_spans.line_index INTEGER NOT NULL DEFAULT 0`; `pub const FILTER_VERSION = 1`.
     `insert_ocr_filtered` does the catalog read → `textfilter::classify` → filtered `frame_text`
     insert → `replace_text_spans` → catalog upsert (`CASE WHEN seen_count+1 ≥ ?min_seen`) in **one**
     transaction (content FTS written once — no transient unfiltered window). `text_filter_stats`
     groups `text_spans`+`frame_text` by `target_app_hint` filtered on the current `filter_version`;
     `reconcile_filter_version` wipes `chrome_text_catalog` + rewrites the `text.catalog_filter_version`
     watermark when it changes. `insert_ocr` passthrough kept for fakes/fallback.
  5. **`crates/capture`** — `monitors.rs` carries `rcMonitor` origin (`MonitorBounds`/`monitor_bounds`);
     `privacy.rs` adds `foreground_window_rect` (`GetForegroundWindow`/`DwmGetWindowAttribute`
     `DWMWA_EXTENDED_FRAME_BOUNDS`, `IsIconic` guard); `lib.rs` computes per-frame `target_rect` via a
     pure `normalize_window_rect` (maps the window into the captured monitor by center-point
     containment; `None` when on another monitor — the safe fallback). 4 new unit tests.
  6. **`crates/kernel`** — `settings.rs` load/save/sanitize the 4 `text.*` keys (clamps
     `min_seen 2..100000`, `protect 1..4096`, `buckets 1..32`); `capture_loop.rs` `process_frame`
     calls `insert_ocr_filtered` with a `TextFilterContext` from `frame.target_rect` + config.
  7. **`src-tauri`** — `get_text_filter_stats` command (registered in `generate_handler!`);
     `reconcile_filter_version(FILTER_VERSION)` once after `open_store`.
  8. **UI** — Settings "Text filtering" panel (toggle + 3 threshold fields with mirrored clamps +
     a per-app suppression-rate readout with all states); Recall search `include_chrome` toggle.
- **Why:** PR3 of `docs/0.2.0.md`. Today capture indexed raw full-screen OCR with no filtering, so
  search/Ask/embeddings were dominated by chrome (taskbar, icons, toolbars, background windows).
  Because the embed worker and search already consume `content_text`, filtering it is what makes
  retrieval stop ranking on chrome with **no** embed-worker or search change. The top risk —
  **false suppression (silent data loss)** — drove a conservative, fully recoverable design
  (`raw_text` preserved, `include_chrome` recovers, per-app suppression-rate alarm). Decisions
  recorded in `07` gaps #52–57.
- **Verification:** `cd ui && npm run lint` (Rules-of-Hooks gate, exit 0) + `npm run build`
  (`✓ built`); `cargo fmt --all -- --check` (exit 0); `cargo clippy --workspace --all-targets --
  -D warnings` (exit 0 — fixed a `doc_lazy_continuation` from a `+`-led wrapped doc line);
  `cargo build --workspace` (exit 0); `cargo test --workspace` — **0 failed** across all crates
  (textfilter **6**, store unit **1** + integration **47** incl. the new
  `insert_ocr_filtered_suppresses_repeated_chrome_after_threshold` /
  `reconcile_filter_version_wipes_catalog_on_change`, traits **43**, capture **13**, kernel **2** +
  pipeline **5** + settings **6**, ocr **1**, screensearch_lib **6**, inference no_orphan **1** /
  reap **3** / sidecar **8**, embeddings **1**). Bindings regenerated and committed (the post-`cargo
  test` `git diff -- ui/src/bindings` is clean once committed). **Manual multi-DPI / secondary-monitor
  `target_rect` check is the one acceptance item CI cannot cover** (gap #54).

## 2026-06-25 — 0.2.0 PR2: text-signal data model + OCR spans (`feat/0.2.0-pr2-text-signal`)
- **Change:** Implemented `03 §3/§3b/§4` (PR2 of `docs/0.2.0.md`) verbatim:
  1. `crates/traits` — `TextSource`/`TextRole`/`SuppressReason` enums (ts-rs-exported, with
     `as_db_str`/`from_db_str` DB-token helpers) + internal `TextSpan` + shared `normalize_text`;
     `OcrResult.spans`; `FrameDetail` drops `text`, gains `raw_text`/`content_text`/`text_source`/
     `suppressed_text_count`; `SearchQuery.include_chrome` (`#[serde(default)]`, default `false`).
  2. `crates/ocr` — `recognize_blocking` walks `Lines().Words()` emitting per-word `TextSpan`s; pure
     `normalize_rect` clamps `BoundingRect` (pixels) to `[0,1]` with `x+w≤1`, `y+h≤1`. Spans are
     `role=unknown`, searchable, unsuppressed (PR3 classifies).
  3. `crates/store` — migration `MIGRATION_V3` (`schema_version` 2→3): drops legacy `ocr_text`+FTS,
     creates `frame_text` (+`frame_text_fts` over content_text, +`frame_text_raw_fts` over raw_text,
     all external-content with the v1 trigger style), `text_spans`, `chrome_text_catalog`.
     `insert_ocr` writes `frame_text` (content_text = raw passthrough, `filter_version=0`,
     `suppressed_count=0`, `target_*` copied from the frame's foreground context) + `text_spans`
     atomically; `frame_enrichment_input`/`ocr_texts`/`get_frame`/search FTS + hydrate all repoint to
     `frame_text`/`content_text`. Added inherent `frame_spans` read (observability, mirrors the
     `get_frame`/`delete_frame` precedent).
  4. `crates/store/src/search.rs` — content FTS arm over `frame_text_fts`; `include_chrome=true` adds
     a raw FTS arm over `frame_text_raw_fts`, fused via the existing RRF (content snippet wins).
  5. `src-tauri` — `ask` passes `include_chrome:false` (`ASK_TOP_K` untouched — PR6 scope).
  6. `ui` — `MomentDetail` shows `content_text` with `raw_text` always viewable via a disclosure;
     `Recall` passes `include_chrome:false`; ts-rs bindings regenerated (new `TextSource`/`TextRole`/
     `SuppressReason`; changed `FrameDetail`/`SearchQuery`).
- **Why:** PR2 of `docs/0.2.0.md` / `02 §5b`. Splits the single unfiltered OCR string into a
  preserved raw layer + a filtered default-retrieval layer so search/Ask/embeddings stop being
  dominated by static chrome, and gives PR3's classifier the span geometry it needs. Interim
  passthrough (`content_text = raw_text`, no backfill) per `07` #51 — clean-DB assumption.
- **Decisions (user-approved, recorded for `05`):** (a) **drop `ocr_text`** on the clean DB →
  `frame_text` is the single text store (`03 §4`); (b) **replace** `FrameDetail.text` (it duplicated
  `raw_text`); (c) **per-word** span granularity; (d) raw-search mechanism = a **dedicated raw FTS5
  table** (not a role-filtered spans FTS — roles aren't populated until PR3, and raw FTS is stable
  across PR3); (e) Recall toggle UI **deferred to PR3** (backend `include_chrome` lands now).
- **Verification (verbatim, this branch):**
  - UI: `npm ci` clean (0 vuln); `npm run lint` clean; `npm run build` → `✓ built in 1.75s`.
  - `cargo fmt --all -- --check` → clean (`FMT_EXIT=0`).
  - `cargo clippy --workspace --all-targets -- -D warnings` → `Finished` (`CLIPPY_EXIT=0`).
  - `cargo build --workspace` → `Finished` in 33.25s.
  - `cargo test --workspace` → all green (`TEST_EXIT=0`); store integration **45 passed** incl.
    `insert_ocr_persists_spans_with_pr2_defaults`, `delete_frame_cascades_and_purges_vectors`,
    `open_in_memory_migrates_to_latest_schema_version` (v3); `search::tests::
    include_chrome_searches_raw_text_independently_of_content` passed; ocr
    `normalize_rect_maps_and_clamps_to_unit_square` passed.
  - **Live (gated, real Windows desktop):** `cargo test -p ocr -- --ignored` →
    `winrt_ocr_recognizes_blank_image ... ok` (asserts every span bbox ∈ [0,1]);
    `cargo test -p screensearch --test e2e_capture -- --ignored` →
    `capture_pipeline_stores_frames_ocr_and_enqueues_embed_jobs ... ok` in 3.42s — real WGC capture →
    WinRT OCR → `frame_text` (content_text passthrough) → `get_frame` → embed jobs.
  - Binding guard: `ui/src/bindings` regenerated by `cargo test` and committed (2 changed, 3 new).

---

## 2026-06-25 — Specs: 0.2.0 PR1 attention-first text contract (`docs/0.2.0-pr1-specs`)
- **Change:** Wrote the 0.2.0 contract into `/specs/` (no code):
  1. `02 §5b` — 0.2.x arc + **P6** (attention-first text signal + recall workflows) as a post-1.0 arc.
  2. `03` — `§3b` raw vs content text / window semantics / spans / roles / static chrome suppression
     + new types (`TextSource`/`TextRole`/`SuppressReason`, `TextSpan`, `OcrResult.spans`,
     `FrameDetail` + `SearchQuery.include_chrome`); `§4` `frame_text` + `frame_text_fts`, `text_spans`,
     `chrome_text_catalog` (`schema_version` 2→3); `§7` `generate_report`; `§8` 0.2.x settings keys;
     `§8b` Recall reports.
  3. `UI_REFERENCE` — Recall Search/Ask/Reports, content/raw toggle, premade Ask cards, all five
     states for new/changed screens.
  4. `07` — gaps #47–#51 (0.2.1 deferrals + the PR2→PR3 interim passthrough / no-backfill).
  5. `04` — reading order + build order extended to the 0.2.x PR sequence; `04` recycled as each PR's
     operating prompt.
  6. `CHANGELOG.md` `[Unreleased]` + this entry + `05` build-review entry.
- **Why:** PR1 of `docs/0.2.0.md`. Today capture indexes **raw full-screen OCR with no filtering**
  (`crates/store/src/schema.rs`: `ocr_text`→`ocr_text_fts`; `crates/kernel/src/worker_pool.rs` embeds
  the whole frame as one chunk), so search / Ask / embeddings are dominated by static chrome. The
  spec must state the content-text contract **before** PR2 code references it (`04 §1` reading order,
  `04 §5` stop-at-ambiguity). User decision: land the **full** authoritative contract (concepts +
  DDL + types) in `03` now so PR2/PR3 implement `03` verbatim.
- **Verification:** **Specs-only — no build/test delta.** `git diff --stat` touches only `docs/`,
  `specs/`, and `CHANGELOG.md` (nothing under `crates/`, `src-tauri/`, `ui/`). Contract
  self-consistency checked: `content_text` is filtered OCR/UIA (not vision); raw text is preserved
  but is **not** the default retrieval input; default search stays hybrid (FTS + vector) over content
  text.

---

## 2026-06-24 — Fix Quality (8B) vision download lock-race + storm (`release/v0.1.0`)
- **Change:**
  1. *Single-instance.* `src-tauri/src/lib.rs` registers `tauri-plugin-single-instance` (first plugin):
     a 2nd launch focuses the running window and exits. New dep in `src-tauri/Cargo.toml`.
  2. *Resilient downloader.* `crates/inference/src/download.rs` adds `download_with_lock_retry` — on
     hf-hub `ApiError::LockAcquisition` it backs off (linear 2 s·attempt, ≤5 tries) and retries instead
     of returning a hard error; `fetch_one` now routes through it. A `#[doc(hidden)]` diagnostics
     entry + `examples/repro_8b.rs` reproduce contention out-of-app.
- **Why:** User testing the v0.1.0 installer hit "Download failed: Qwen3VL-8B-Instruct-Q4_K_M.gguf" in a
  loop. **Root cause (reproduced):** two concurrent app instances (relaunch/spam during testing — the
  process list showed two `screensearch.exe`) share one app-data `.hf-cache`; hf-hub's per-blob
  **advisory** lock lets the loser fail with `LockAcquisition` after ~5 s, and the vision scheduler
  retried instantly → storm. The 8B suffered most (largest file → longest lock hold). A clean,
  single-downloader fetch always works; a lock left by a *dead* process is OS-released (so stale locks
  don't wedge a later run — verified). Single-instance removes the race; the backoff covers any residual
  startup-overlap contention without storming.
- **Verification:**
  - Repro: two concurrent downloaders on one cache → one streams, the other dies in **5.29 s** with
    `LockAcquisition` (the exact in-app symptom).
  - Fix, downloader: with the backoff, the loser logs `backing off and retrying … attempt=1/2/3
    backoff_secs=2/4/6` instead of hard-failing.
  - Fix, single-instance: launching the rebuilt `target/release/screensearch.exe` twice leaves **1**
    process (2nd launch `HasExited=True`).
  - Gates: `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test --workspace`
    124 passed / 0 failed; release `tauri build` re-emitted `ScreenSearch_0.1.0_x64-setup.exe` (11 MB).
  - **Pending: user retest of the rebuilt installer** (fresh install → Quality 8B downloads to completion).

## 2026-06-24 — Release v0.1.0: standalone unsigned NSIS installer (`release/v0.1.0`)
- **Change:**
  1. Version `0.0.0` → `0.1.0` across workspace `Cargo.toml`, `src-tauri/tauri.conf.json`, and root +
     `ui/package.json` (`Cargo.lock` regenerated to match).
  2. `bundle.targets` `"all"` → `["nsis"]` — ship a single unsigned NSIS installer; no MSI/WiX.
  3. `CHANGELOG.md` cut `[Unreleased]` → `[0.1.0]`; gap #26 marked **partial** (installer shipped,
     code signing still open); the `onnxruntime.dll` bundling item closed (static-linked).
- **Why:** First public release (specs `02 §5` / `03 §13.9`, gap #26). User decisions: **NSIS-only**,
  **local build + manual upload**, **GitHub pre-release**. ONNX Runtime is static-linked via `ort`'s
  `download-binaries` (no `load-dynamic`/`libloading` in `ort-sys`), so there is no DLL to bundle — the
  installer needs only the ~11 MB exe; sidecar + models download to the app-data dir on first run.
- **Verification:**
  - **UI:** `npm run lint` clean; `vite build` ok (`screensearch-ui@0.1.0`).
  - **Rust:** `cargo fmt --all -- --check` → `FMT_OK`; `cargo clippy --workspace --all-targets -- -D
    warnings` clean (all crates `v0.1.0`); `cargo test --workspace` → **124 passed, 0 failed**.
  - **Bundle:** `npm run build` → `Finished \`release\` profile [optimized] target(s) in 3m 03s`;
    `Finished 1 bundle at: target/release/bundle/nsis/ScreenSearch_0.1.0_x64-setup.exe` (**11 MB**). No
    `.dll` and no `onnxruntime*` beside `target/release/screensearch.exe` → static link confirmed.
  - **Live (user-verified, 2026-06-24):** installer installs to `%LOCALAPPDATA%\ScreenSearch`; app
    launches and works; **uninstall works** (Roaming app-data at `%APPDATA%\app.screensearchv2c.desktop`
    preserved); **fresh install after removing the data dir also works** (clean first-run path).

## 2026-06-24 — Ask answer truncated to nothing + download not visible globally (`fix/idle-backfill-sidecar-status`, round 4)
- **Change:**
  1. *Ask reply budget.* `ui/src/routes/Recall.tsx`: `ASK_MAX_TOKENS` 512 → 2048.
  2. *Global download indicator.* `ui/src/components/shell/StatusRail.tsx` now reads
     `useModelDownload()` and renders an accent "↓ <pct>%" chip (model + size tooltip) while a fetch
     is in `downloading` phase; new `IconDownload` in `ui/src/components/icons.tsx`.
- **Why:** User report (2026-06-24, round 4, two screenshots):
  - *Answer "always displayed as thinking … answer cut off probably because of the small context."*
    The default answer models (Ministral-3-3B-Reasoning, Qwen3-4B-Thinking) emit a `<think>` trace
    first; the visible trace in the screenshot ended mid-list ("4. "), i.e. generation hit the 512
    `n_predict` cap during reasoning and never reached the answer. `AnswerStream` renders thinking in
    the `<details>` box and the (empty) answer in the markdown body → "always thinking." The fix is
    purely the token budget — `answer.rs::build_messages` already reserves `max_tokens` from the ctx
    budget, and with the default 8K answer ctx, 2048 leaves ~6K for snippets.
  - *"I'd like some visual when a model is downloading"* — previously the progress bar lived only in
    Settings → Inference engine; the rail (visible on every screen) had no download cue.
  - (Positive: the round-2 thinking-box auto-scroll was confirmed "very good now.")
  - No Rust change, no schema/IPC change (no binding drift).
- **Verification:**
  - **Automated gates (all green):** `npm run lint` (`LINT_EXIT=0`); `npm run build`
    (`BUILD_EXIT=0`). Rust workspace untouched this round, so round-3's `cargo fmt/clippy/build/test`
    (inference lib 67 tests) still hold; binding guard shows no round-4 drift.
  - **Pending live (GUI) verification:** ask a question that triggers a long reasoning trace and
    confirm a non-empty answer renders after the Thinking box; start a model download and confirm the
    rail shows the "↓ %" chip from any screen.

## 2026-06-24 — Download hang on incomplete/resumed fetch + untruthful progress bar (`fix/idle-backfill-sidecar-status`, round 3)
- **Change:** Reworked `ModelDownloader`'s fetch path in `crates/inference/src/download.rs`:
  1. *Truthful, resume-aware progress.* Progress is now driven by hf-hub's `Progress` trait
     (a new `ByteCounter` sink accumulating real streamed bytes into a shared `AtomicU64`) via
     `ApiRepo::download_with_progress`, replacing the old `dir_size(blobs/)` poller. New
     `download_repo_files` / `fetch_one` orchestrate per-file fetch with a `hf_hub::Cache` pre-check
     (reuse a finalized-but-not-copied blob instead of re-downloading) and an off-runtime
     `spawn_blocking` clean-layout copy. Removed the now-dead `dir_size` / `hf_cache_repo_dir`.
  2. *Stall watchdog.* `run_download` fetches on a child task and a watchdog (`stall_step` /
     `stall_limit`, both pure + unit-tested) aborts it if the byte counter doesn't advance for
     `STALL_TIMEOUT = 180 s`, emitting a `Failed` `ModelDownloadStatus` with a retryable message;
     `finish_download` maps Done/Failed/JoinError. The partial `.sync.part` is left on disk so the
     next `ensure` resumes.
  3. *UI.* `ModelPanel` renders a failed-download banner (reason + "resumes from where it stopped")
     using only valid Command Deck tokens (`border-danger`, `bg-overlay`, `rounded-panel`).
- **Why:** User report (2026-06-24, round 3, with screenshot): "Inference engine download hanging
  probably because last attempt was incomplete — this is a use case to address." Investigation proved
  two defects in hf-hub 0.4.3's downloader: (a) it `set_len`s the `.sync.part` to the file's full
  length up front, so the old size-poll bar read ~81% (≈5 GB allocated of 6.2 GB) while only the
  committed-counter's **1.68 GB / 5.03 GB (33%)** was truly downloaded, then froze; (b) its `reqwest`
  client has **no socket timeout** and we ran it with no retries, so a dead CDN connection blocks
  `bytes_stream().next()` forever. No schema change; no new IPC types (no binding drift this round).
- **Verification:**
  - **Root cause confirmed from the live cache:** the 8B blob sat as a `*.sync.part` of
    `5,027,784,808` bytes = HF size `5,027,784,800` + hf-hub's 8-byte trailer; reading that trailer
    (the committed counter) showed `1,680,000,000` (33.4%) really downloaded; no finalized blob /
    `refs/main` for the repo; the app process was not running (an abandoned, resumable partial).
  - **Automated gates (all green):** `cargo fmt --all -- --check`; `cargo clippy --workspace
    --all-targets -- -D warnings`; `cargo build --workspace`; `cargo test --workspace` (inference lib
    67 tests incl. 3 new: `byte_counter_accumulates_streamed_bytes`,
    `stall_limit_is_timeout_over_poll_and_never_zero`, `stall_step_resets_on_progress_and_counts_otherwise`);
    `npm run lint`; `npm run build`; binding guard shows no round-3 drift.
  - **Pending live (GUI) verification:** with the existing 33% partial on disk, Load the quality
    vision model → bar should start near 33% and climb on real bytes; on a network drop it should
    fail with the stall message after ~180 s (not hang) and resume on the next Load.

## 2026-06-24 — Model-download progress + reliability, load-pinning, thinking auto-scroll (`fix/idle-backfill-sidecar-status`, round 2)
- **Change:** Follow-up fixes from a second user test.
  1. *Download progress + reliability.* New `inference::ModelDownloader` (shared by both lane
     providers) serializes downloads per lane via a `[Mutex; 2]` (no concurrent races) and
     broadcasts `ModelDownloadStatus` progress (polling the cache `blobs/` size against a total from
     the HF `tree/main` API). Bridged composition-root → `kernel.emit_model_download` →
     `model_download` Tauri event → `ModelPanel` progress bar + toasts. Providers' `ensure_spec` now
     calls `downloader.ensure(lane, tier)` instead of the bare `download::ensure_model`.
     New `models::model_files_present` fast-path. New IPC types `ModelDownloadStatus` /
     `ModelDownloadPhase` (u64 byte fields tagged `#[ts(type="number")]`).
  2. *Load-pinning (B).* `ModelSupervisor` gained a `pinned: AtomicBool`; `preload` sets it,
     `unload` and a model switch clear it, and `should_evict` honors it (idle-TTL won't evict a
     manually loaded model).
  3. *Thinking auto-scroll (C).* `AnswerStream` bounds the trace (`max-h-64 overflow-auto`) and
     auto-follows the latest line while streaming (only when already near the bottom).
- **Why:** User report (2026-06-24, round 2): clicking Load/Download gave no progress or error
  feedback ("if something takes >5s, communicate progress or error"); quality vision tagging
  "doesn't work" — the DB showed `vision_tag` jobs `dead` with "download vision model" (the 6 GB
  Qwen3-VL-8B fetch raced across workers and never completed); a manually loaded model was evicted
  right after downloading; and the streaming Thinking box was cut off. No schema change.
- **Verification:**
  - **Root cause confirmed from the live DB:** `jobs` had 3 `vision_tag` rows in state `dead`,
    `last_error = "vision analyze frame …: download vision model"`, and `models/vision/quality/` was
    empty. The HF tree API returns real sizes (Q4_K_M 5.03 GB + mmproj-F16 1.16 GB ≈ 6.2 GB), so the
    progress bar shows a true percentage.
  - **Automated gates (all green):** `npm run lint`; `npm run build`; `cargo fmt --all -- --check`;
    `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`; binding guard
    clean (regenerated `SidecarStatus.ts` + new `ModelDownloadStatus.ts` / `ModelDownloadPhase.ts`).
  - **Pending live (GUI) verification:** Load a quality model and watch the % bar + completion toast;
    confirm vision tagging works once downloaded; confirm a loaded model isn't idle-evicted; confirm
    the Thinking box follows the stream.

---

## 2026-06-24 — Idle backfill keep-warm + truthful sidecar status + Ask context budget (`fix/idle-backfill-sidecar-status`)
- **Change:** Four linked fixes from user testing of PR #25.
  1. *Idle full-drain, keep-warm.* `vision_scheduler::idle_loop` was rewritten from "enqueue one
     batch on the idle transition" to a continuous drain: while idle and a backlog remains, it tops
     the queue up to a batch whenever `Store::pending_vision_job_count` (new) falls below a
     `batch/4` watermark, polling at `DRAIN_POLL` (5s) instead of 30s. It sets a keep-warm flag while
     draining and clears it when the backlog is empty or the user resumes. The flag is the new
     `traits::BackfillControl` trait, implemented by `ModelSupervisor`, threaded
     kernel→`attach_inference`→`start_vision_scheduler`→`spawn` and wired at the composition root.
  2. *TTL vs idle.* `ModelSupervisor` gained a `backfill_active: AtomicBool`; `maybe_evict` now uses a
     pure, unit-tested `should_evict(in_flight, elapsed, ttl, backfill_active)` that holds the model
     warm during a backfill drain.
  3. *Truthful status + panel.* `sidecar_component` maps `Evicted → Disabled` (was `Ready`);
     `SidecarStatus` gained `lane: Option<ModelLane>` (supervisor `emit` now takes the spec); the
     StatusRail chip and a new **Inference engine** panel (`ModelPanel.tsx`) label from the raw
     `SidecarState` (`sidecarStateLabel`/`sidecarStateTone`). New `load_model`/`unload_model` commands
     back `ModelSupervisor::{preload, unload}` for manual control.
  4. *Ask context overflow.* `client.rs` now surfaces the real HTTP status+body (was
     `error_for_status().context("…error status")`); `answer.rs::build_messages` budgets included
     context to `ctx_size` (reserve = reply tokens + system prompt + question + template overhead),
     dropping/truncating chunks that don't fit and citing only included frames.
  5. *Kill verification.* All teardown paths route through `kill_and_confirm` (logs `killed`/
     `still_alive` after a bounded exit wait).
- **Why:** User report (2026-06-24): idle tagging did one batch then stopped (wanted "process as
  many as possible while idle", `03 §5`); the sidecar idle-TTL evicted the model and the UI showed
  "Ready" for a dead process (`03 §6/§7`); "Ask" failed with an opaque "sidecar returned an error
  status". No schema change.
- **Verification:**
  - **Root cause of the Ask failure, reproduced directly against the bundled `llama-server`** (answer
    model, `--ctx-size 8192`): a normal request returned `HTTP 200`; an oversized-context request
    returned the swallowed error:
    ```text
    [overflow] HTTP 400 ERROR; body={"error":{"code":400,"message":"request (29418 tokens) exceeds
    the available context size (8192 tokens), try increasing it","type":"exceed_context_size_error",
    "n_prompt_tokens":29418,"n_ctx":8192}}
    ```
  - **Automated gates (all green):** `npm run lint`; `npm run build`; `cargo fmt --all -- --check`;
    `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace` (incl. new
    `client::http_error_*`, `supervisor::evict_predicate_*`, `answer::{drops_chunks_…,
    truncates_an_oversized_top_chunk_…,builds_grounded_prompt_…}`); binding guard clean (only the
    regenerated `SidecarStatus.ts`).
  - **Pending live (GUI) verification:** in-app Ask now succeeds on large contexts; idle backfill
    drains a real backlog while keeping the model warm; the panel's Load/Unload + truthful labels;
    `killed=true,still_alive=false` after eviction. (Requires the running desktop app.)

---

## 2026-06-24 — Sidecar memory tuning + user settings (`feat/sidecar-memory-tuning`)
- **Change:** The llama.cpp sidecar now pins a small context window and quantizes the KV cache, and
  three new settings expose the knobs. `build_args` (`crates/inference/src/supervisor.rs`) emits
  `--ctx-size`, `--flash-attn`, and `--cache-type-k/-v` — gated by a new `--help` capability probe
  (`crates/inference/src/flags.rs`) so only flags the bundled binary accepts are passed. A new
  `traits::SidecarParams { ngl, device, ctx_size, kv_cache_type, flash_attn }` (built from `Settings`
  via `From<&Settings>`) threads the knobs through `VisionSidecar`/`AnswerSidecar` →
  `models::resolve_spec` (which substitutes the `0 = auto` context sentinel with a per-lane default,
  vision 4096 / answer 8192) → `ModelSpec` → `build_args`; the new fields also join `needs_restart`
  so a settings change relaunches the sidecar on the next request. New `Settings` fields:
  `sidecar_ctx_size` (u32, default 0, clamp 0 ∪ 512–32768), `sidecar_kv_cache_type` (`KvCacheType`
  enum, default `q8_0`), `sidecar_flash_attn` (`FlashAttnSetting` enum, default `auto`), wired through
  `kernel::settings` load/save/sanitize and surfaced in `ui/src/routes/Settings.tsx` (Sidecar panel).
- **Why:** `01 §1` (Windows-native, local-first, on-device) + the user report that processing used
  ~16 GB VRAM — 2–3× the models' own footprint. Root cause: with no `--ctx-size`, llama.cpp used the
  models' full trained context (262 144 tokens for the defaults), so the KV cache dominated VRAM.
  Pinning context + quantizing KV is the standard, low-risk lever; exposing it as settings (`03 §8`)
  lets a constrained user trade memory for quality. Default tuning is "balanced" (no expected quality
  loss); `f16` KV / larger context are escape hatches.
- **Verification:**
  - **Automated gates (all green):**
    ```text
    cargo fmt --all -- --check        → exit 0
    cargo clippy --workspace --all-targets -- -D warnings → exit 0
    cargo build --workspace           → Finished (exit 0)
    cargo test --workspace            → all suites ok (exit 0)
    cd ui && npm run lint && npm run build → eslint exit 0; tsc+vite built (exit 0)
    inference lib: 55 passed (incl. flags::* parser tests, supervisor::build_args_* and
      restart_on_tuning_change, models::auto_ctx_size_resolves_per_lane_and_override_passes_through)
    kernel --test settings: 6 passed (incl. sidecar_ctx_size_zero_is_preserved_as_auto_sentinel)
    ```
  - **Live GPU proof** (RTX 5060 Ti, bundled Vulkan `llama-server` build 9754, default answer model
    `Ministral-3-3B-Reasoning-2512-Q4_K_M`, baseline VRAM 1660 MiB):
    ```text
    # The probe matches the real binary's --help:
    -c,  --ctx-size N            (default: 0, 0 = loaded from model)
    -fa, --flash-attn [on|off|auto]
    -ctk,--cache-type-k TYPE   /  -ctv,--cache-type-v TYPE
      → parse_caps → { ctx_size: true, cache_type_k: true, cache_type_v: true,
                       flash_attn_kind: EnumOnOffAuto }

    # Untuned (old app args: --model M -ngl 99):
    llama_context: n_ctx_seq (118272) < n_ctx_train (262144)
    peak total VRAM used = 14082 MiB   (~12.4 GiB model footprint)

    # Tuned (new defaults: --ctx-size 8192 --flash-attn on --cache-type-k q8_0 --cache-type-v q8_0):
    llama_context: n_ctx_seq (8192) < n_ctx_train (262144)
    peak total VRAM used = 4253 MiB    (~2.5 GiB model footprint)

    → ~9.8 GB / ~70% VRAM reduction for the answer model; sidecar exited cleanly, VRAM back to 1660 MiB.
    ```

## 2026-06-23 — Add audit report artifact (`codex/run-audit-v2c`)
- **Change:** Added `docs/AUDIT_V2C_2026-06-23.md`, a Markdown record of the non-packaging V2c audit
  pass: preflight, static architecture checks, CI-order local gates, hardware/model-backed smokes,
  GPU runtime evidence, and the later live GUI follow-up for synthetic capture/search/Ask,
  no-evidence refusal, persisted Moment VLM analysis, and P5 route/state/a11y observations.
- **Why:** The audit was originally returned only in chat, which made it hard to find after the
  session. The repository now carries the audit artifact on the audit branch.
- **Verification:**
  ```text
  git diff --check exited 0
  ```

## 2026-06-24 — PR #21 audit-doc review follow-up (`codex/run-audit-v2c`)
- **Change:** Addressed all actionable PR #21 review comments. Added a `05_BUILD_REVIEW.md` pass
  entry, logged the review-required doc fix in `06_PATCH_PLAN.md`, mirrored six audit follow-ups into
  `07_KNOWN_GAPS.md`, clarified that the frontend gates were local CI-order smoke checks on Node 26
  rather than exact CI Node 22 reproduction, and noted that the obsolete-term search was captured
  before the audit artifact existed.
- **Why:** Reviewers correctly flagged that the build-loop docs were not complete and that the audit
  wording overstated the CI-runtime signal.
- **Verification:**
  ```text
  git diff --check exited 0
  ```

## 2026-06-23 — PR #19 review follow-up (`codex/p5-comprehensive-review-fixes`)
- **Change:** Addressed all actionable PR #19 comments. Fixed the ask-task insertion/removal race by
  locking the active-task map before spawning the provider task; made retention log and continue when
  a single DB delete fails; fixed monitor toggling from the empty/all-monitors state; gated/refreshed
  sidecar device listing on sidecar readiness; removed the simultaneous Select + manual sidecar
  device controls; updated stale toast comments; renamed `uuid_like_id` to `next_ask_id`; documented
  the sidecar-device parser heuristic; clarified embed-toggle apply timing in Settings/docs; reloaded
  FastEmbed with the image lane when image embeddings are enabled after a text-only startup; and made
  retention delete JPEG files before DB rows so a locked file remains retryable on the next sweep.
- **Why:** Reviewers found a real task-map leak race, a retention "poison pill" failure mode, and
  confusing Settings UX/labels. The `enrichTimer` cleanup comment was verified as already fixed in
  `HEAD` and required no code change.
- **Verification:** Final command output is recorded in the task response after rerunning the gates.

---

## 2026-06-23 — P5 comprehensive review hardening (`codex/p5-comprehensive-review-fixes`)
- **Change:** Implemented the approved P5 comprehensive review fix plan. Backend changes include
  range-aware `get_nearest_frame(at, range?)`, IPC clamping for frame/timeline/insights reads,
  `get_storage_stats`, startup/hourly retention enforcement, `get_monitors`, `list_sidecar_devices`,
  optional `sidecar.device`, request-scoped `AnswerEvent`, `cancel_ask`, backend `toast`, and richer
  `job_completed` events. UI changes include DST-safe local-day helpers, Deck panel errors/retries,
  Command Palette combobox/listbox ARIA, token-only Timeline drawing colors, StatusRail storage
  telemetry, request-id ask filtering/cancel, surgical live-event invalidation, embeddings-disabled
  live-search refresh, monitor/device pickers with manual fallback, and adaptive Timeline/Insights
  buckets. Regenerated ts-rs bindings for the new IPC shapes.
- **Why:** The P5 review found correctness and operability gaps that were not packaging work:
  calendar ranges could drift over DST; Timeline opens could jump outside the active range; direct IPC
  callers could request oversized reads; failed Deck queries rendered like empties; retention was only
  persisted; answer streams were global; enrichment refresh was too broad/late; monitor and llama.cpp
  device selection were manual-only; and chart grains were fixed. Packaging remains deferred in
  `07` #26 per user decision.
- **Verification:** Final gates run after the last code change: `cargo fmt --all -- --check`,
  `npm --prefix ui run lint`, `npm --prefix ui run typecheck`, `cargo test --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and `npm --prefix ui run build` all
  exited 0. The task final response includes the required verbatim command outputs.

---

## 2026-06-23 — Refresh root `CLAUDE.md` (`docs/refresh-claude-md`)
- **Change:** Docs-only refresh of the root `CLAUDE.md`. (1) Rewrote the "current state" line —
  was "specification complete, no application code yet; build starts at P0", now "P0–P5 complete
  and merged to `main`, in post-merge hardening". (2) Replaced the non-working app command
  `cargo tauri dev`/`cargo tauri build` with `npm run tauri dev`/`npm run build`. (3) Rewrote the
  Build/verify section to match `.github/workflows/ci.yml`: UI-before-cargo order, `npm run lint`
  gate, `clippy --all-targets`, `build/test --workspace`, the `git diff --exit-code -- ui/src/
  bindings` guard, Rust 1.82 / Node 22. (4) Added a "Where the code lives" 9-crate + `ui/` map.
- **Why:** The headline and app command were ~1 month stale and actively misleading (an agent
  could try to scaffold from scratch or run a command that fails — `cargo-tauri` is not installed).
  Brings the operating manual back in line with the code, CI, and `package.json` without weakening
  the "specs are the source of truth" stance. No code, schema, or behavior change.
- **Verification:** Cross-checked every command against `.github/workflows/ci.yml`, root
  `package.json`, and `Cargo.toml`; re-read `CLAUDE.md` — no remaining reference to "no application
  code yet", "build starts at P0", or `cargo tauri dev`.

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

## 2026-06-23 — PR #17 review comment follow-up (`codex/review-p3-deferred-enrichment`)
- **Context:** PR #17 received two unresolved Gemini inline comments and two actionable Claude notes.
  The Gemini threads asked to avoid allocating a `Vec` in the worker polling loop; Claude asked for a
  more precise `job_progress` comment and a known-gap note for startup-scoped embed lane flags.
- **Change:** `claim_kinds` now returns `([JobKind; 3], usize)` and the worker loop passes the active
  array slice into `claim_jobs`, eliminating the heap allocation on every idle poll. `useLiveEvents`
  now says `job_progress` is emitted after each job attempt completes. `07_KNOWN_GAPS.md` records
  that live re-enabling of embedding lanes requires a future worker restart/reconfigure path, matching
  the current Settings apply policy.
- **Interface review:** no schema, IPC, `ts-rs`, or trait signature changes.
- **Verification (verbatim status):** `cargo fmt --all -- --check` exit 0; `cargo clippy --workspace
  --all-targets -- -D warnings` exit 0; `cargo test -p kernel --test enrichment` → **10 passed**;
  `npm --prefix ui run lint` exit 0; `git diff --exit-code -- ui/src/bindings` exit 0.

## 2026-06-23 — P4 sidecar hardening (`codex/p4-sidecar-hardening`)
- **Context:** a scrupulous P4 review found four lifecycle gaps after the no-orphan foundation:
  model switches could kill an active request, same-model reuse did not re-check liveness/health,
  sidecar HTTP calls had no bounded deadlines, and `SSV2C_LLAMA_RELEASE_URL` was documented as
  highest precedence but lost to an existing normal install.
- **Failing-first evidence:** `cargo test -p inference` initially failed with missing
  `RequestGate`, missing `can_reuse_running_sidecar`, and missing `SidecarClient::with_timeouts`,
  proving the new tests were red before implementation.
- **Change:** `ModelSupervisor` now serializes sidecar leases with a `RequestGate`; the lease owns the
  permit until request completion, so lane/tier switches wait before killing the running process.
  Same-model reuse validates both OS liveness and bounded `/health`; unhealthy state emits
  `Crashed`, stops the stale process, and respawns for the current request. `SidecarClient` now has
  default production deadlines plus test-configurable timeouts for health, non-stream completion, and
  SSE idle waits. `ensure_binary` now checks `SSV2C_LLAMA_RELEASE_URL` first and extracts override
  zips into a URL-fingerprinted override directory, preserving the normal install.
- **Docs:** `CHANGELOG.md`, `specs/05_BUILD_REVIEW.md`, `specs/07_KNOWN_GAPS.md`, and this file.
- **Interface review:** no schema, IPC, `ts-rs`, or trait signature changes. Ignored GPU smokes were
  not run in this pass unless separately approved.
- **Verification (targeted during implementation):** `cargo test -p inference` → inference unit
  **39 passed**, sidecar client **7 passed**, no-orphan **1 passed**, reap **2 passed**, smoke **2
  ignored**; `cargo clippy -p inference --all-targets -- -D warnings` exit 0.

## 2026-06-23 — PR #18 P4 sidecar review follow-up (`codex/p4-sidecar-hardening`)
- **Context:** PR #18 reviewers found that the first `RequestGate` implementation over-serialized all
  sidecar use, that `enter_for_model_switch` was only an alias, that stream timeout naming collapsed
  initial POST latency with SSE idle waits, that override extraction was synchronous/non-atomic, and
  that startup reap only recognized the currently selected binary path.
- **Change:** `RequestGate` now behaves like a reader/writer gate: same-model requests acquire one of
  many permits and can run concurrently, while model switches and crash recovery drain every permit
  before stopping the child. The exclusive permit is downgraded to one request permit after the new
  model is ready. `ClientTimeouts` now has separate `stream_connect` and `stream_idle` deadlines.
  Override downloads extract on a blocking task into a `.partial` directory and rename into place only
  after a complete extraction, with failed partials cleaned up. Startup reap now scans exact
  app-owned binaries under both `<sidecar>/llama` and `<sidecar>/llama-override/*`.
- **Tests:** added/updated regressions for concurrent regular gate entry, all-permit model-switch
  draining, multi-path reaping, override partial cleanup, installed-binary candidate discovery, and
  separate stream connect vs SSE idle timeout behavior.
- **Interface review:** no schema, IPC, or `ts-rs` binding changes. Workspace Rust API additions are
  `SupervisorConfig::reap_binaries`, `ClientTimeouts::stream_connect`, and
  `SidecarClient::with_client_timeouts`.
- **Verification (targeted during follow-up):** `cargo test -p inference` → inference unit **42
  passed**, no-orphan **1 passed**, reap **3 passed**, sidecar client **8 passed**, smoke **2
  ignored**.

## 2026-06-23 — PR #18 second review follow-up (`codex/p4-sidecar-hardening`)
- **Context:** fresh PR comments after `df02070` found two remaining edge cases: a status-bus gap
  when crash recovery races a concurrent model switch, and a startup cleanup gap when a bad
  `SSV2C_LLAMA_RELEASE_URL` fails before the supervisor has a chance to reap a pidfile from an older
  normal/override install.
- **Change:** the crash-recovery branch records the model label observed unhealthy before dropping
  the shared request permit and emits that `Crashed` transition once the exclusive recovery path takes
  over. `init_inference` now scans installed normal/override binary candidates and calls
  `reap_stray_any` before `ensure_binary`, then reuses that candidate list for `SupervisorConfig`.
- **Interface review:** no schema, IPC, `ts-rs`, or public command changes. The optional multi-GB GPU
  smokes remain ignored.

## 2026-06-24 — Vision scheduler: configurable batch size + pending-job dedup (`fix/vision-batch-size-dedup`)
- **Context:** a user observed the vision scheduler logging a fixed `count=20` every tick for both
  the `timer` and `idle` triggers, with no setting to change it. Root cause was the two remaining
  sub-threads of `07` #19: the batch size was a hardcoded `const BATCH: u32 = 20` in
  `kernel::vision_scheduler`, and `Store::untagged_frame_ids` filtered only on "no `vision_analysis`
  row" — it ignored whether a `vision_tag` job was already in flight, so every tick re-queued the
  same oldest-untagged frames and the timer/idle lanes double-queued the same frames when they fired
  together.
- **Change (config):** added `Settings.enrich_vision_batch_size: u32` (default 20, sanitized to
  1–500) persisted as `enrich.vision_batch_size`. The scheduler reads it from the settings it already
  loads each loop and passes it into `enqueue_untagged`, so a change applies on the next scheduled
  run. Surfaced in the UI as a "Frames per run" field in `ScheduleControl`, shown when either lane is
  enabled, with a save-time clamp mirroring the backend.
- **Change (dedup):** `untagged_frame_ids` now also excludes frames with an in-flight
  (`pending`/`running`) `vision_tag` job via a `NOT EXISTS` subquery on `jobs`. A finished (`done`)
  job or a job of another kind still leaves an untagged frame eligible. This also benefits the
  on-demand `enqueue_vision` range path (shared query); single-frame on-demand re-tagging is
  unaffected (it bypasses the query). `insert_vision`'s idempotent upsert remains the backstop for a
  stray re-analyze after a job leaves the queue.
- **Why:** fixes the user-reported "count stuck at 20 / no setting" and the timer+idle double-enqueue;
  resolves the batch-size and pending-job-dedup threads of `07` gap #19 (the prior "re-enqueue is
  harmless" stance is reversed now that it can churn the queue at larger batch sizes).
- **Tests:** new store integration test `untagged_frame_ids_excludes_in_flight_vision_jobs`
  (pending/running excluded; done / other-kind / no-job eligible); extended the kernel settings
  round-trip and numeric-sanitize tests for the new field.
- **Interface review:** `Settings` gains one field → `ts-rs` regenerated `ui/src/bindings/Settings.ts`
  (committed). No trait-signature, command, or schema changes; the `jobs`/`frames`/`vision_analysis`
  tables are unchanged (the dedup is a query-only `NOT EXISTS`, no new index or migration).
- **Verification:** UI `npm ci && npm run lint && npm run build` clean; `cargo fmt --all -- --check`
  (exit 0); `cargo clippy --workspace --all-targets -- -D warnings` (exit 0); `cargo build
  --workspace` (ok); `cargo test --workspace` (all crates 0 failed, incl. store integration **45
  passed**, kernel settings **5 passed**); `git diff --exit-code -- ui/src/bindings` clean after the
  regenerated `Settings.ts` is committed.

## 2026-06-24 — PR #23 review follow-up (`fix/vision-batch-size-dedup`)
- **Context:** PR #23 drew three actionable review threads (Gemini, Codex; Claude approved). All three
  were valid and addressed.
- **Change (Gemini, high — infinite retry of poisoned frames):** a `vision_tag` job that exhausted
  retries goes `dead` without writing a `vision_analysis` row, so the prior `NOT EXISTS` (which only
  excluded `pending`/`running`) re-selected that frame every tick → enqueue → fail → dead → repeat
  forever. `untagged_frame_ids` now also excludes `state = 'dead'`. A `done` job still does **not**
  exclude (a frame whose job finished without persisting analysis remains eligible); on-demand
  single-frame re-tag bypasses the query, so a dead frame can be force-retagged.
- **Change (Gemini/Claude — perf):** the correlated `NOT EXISTS` scanned `jobs` per candidate frame.
  Added forward migration v2 (`schema_version` 1→2): `CREATE INDEX idx_jobs_frame_kind_state ON
  jobs(frame_id, kind, state)`. Index-only, no data change; existing DBs migrate on next open.
- **Change (Codex, P2 — atomic scheduled enqueue):** the read (`untagged_frame_ids`) and the
  per-frame `enqueue_job` calls are separate store ops, so a simultaneous timer+idle wake could both
  read the same frames before either inserts, defeating the guard in exactly the overlap scenario this
  PR targets. The timer and idle loops now share a `tokio::sync::Mutex` held across the whole
  read-then-enqueue, making the two producers mutually atomic. The rarer on-demand-vs-scheduler
  overlap stays bounded by `insert_vision`'s idempotent upsert (a scheduler-scoped guard, not a global
  DB constraint — chosen to avoid changing `enqueue_job` semantics for the embed lanes).
- **Change (Codex, P1 — docs):** added `enrich.vision_batch_size` (20, clamp 1–500) to
  `specs/03_MASTER_PRODUCTION_SPEC.md` §8, per AGENTS.md's "every setting recorded in §8" rule.
- **Tests:** extended `untagged_frame_ids_excludes_in_flight_vision_jobs` with a dead-lettered frame
  (claim → `fail_job(.., None)`) asserted excluded; the existing `open_in_memory_migrates_to_latest_
  schema_version` now exercises the v1→v2 path.
- **Interface review:** schema gains an index via a versioned forward migration (no drift). No
  trait-signature, IPC, command, or `ts-rs` changes in this round.
- **Verification:** `cargo fmt --all -- --check` (exit 0); `cargo clippy --workspace --all-targets --
  -D warnings` (exit 0); `cargo test -p store` (store integration **44 passed**, 0 failed, incl. the
  migration + dead-job tests); `cargo test -p kernel` (settings **5 passed**, enrichment **10
  passed**); `cargo test --workspace` (all crates 0 failed).

## 2026-06-26 — 0.2.0 PR8: parallel model download (`feat/0.2.0-pr8-parallel-download`)
- **Context:** first-run model download went single-stream via hf-hub, capped ~20 MB/s by HF's CDN
  throttling one connection, so the ~5 GB Quality (8B) vision model took minutes. PR8 (`docs/0.2.0.md`)
  adds a multi-connection chunked downloader. Confined to `crates/inference/src/download.rs` (+ tests)
  and `Cargo.toml`; independent of the retrieval chain.
- **Change (downloader):** `fetch_one` now probes the HF `resolve` URL with `Range: bytes=0-0`
  **without following the 302**, reading the redirect's own `X-Linked-Size` + `Accept-Ranges` +
  `X-Linked-ETag` (the LFS sha256), and — when ranges are supported — fetches the file with
  `DOWNLOAD_CONNECTIONS` (default 8, env `SSV2C_DOWNLOAD_CONNECTIONS`, clamp 1–16) parallel `Range`
  requests into one pre-allocated `<base>.part` via positioned `seek_write`. A per-chunk completion
  bitmap `<base>.parts` (fsync'd **after** its chunk data) drives crash-safe resume. Chunk futures run
  via `buffer_unordered` awaited inside the fetch task, so the existing stall watchdog's `task.abort()`
  cancels every in-flight chunk (no orphaned writers — the gap-#46 fix for this path).
- **Fix (live-HF 403 storm):** the first cut pinned one signed CDN URL (`resp.url()`) for all chunks
  and hit `403 server ignored Range` against real HF. Root cause (confirmed by decoding the
  CloudFront policy): HF Xet-bridge signed URLs pin `"ByteRange":{"ExpectedHeader":"bytes=0-0"}`, so a
  URL minted for one range is rejected for any other. Each chunk now re-requests the stable `resolve`
  URL with its own `Range` and follows the redirect per request (reqwest keeps `Range` across the
  hop) — minting a fresh range-matched signed URL, the `hf_transfer` approach. Transient
  `403`/`401`/`429`/`5xx` retries with backoff. Verified live with curl: signed-URL reuse → 403;
  per-request resolve → 206. Added tests `chunk_requests_follow_redirect_and_preserve_range` and
  `chunk_retries_transient_403_then_succeeds`.
- **Fix (live-HF sha256 / Xet-hash storm):** with the 403 fixed, downloads *completed* but failed
  sha256 every time and looped at ~75%. Root cause: the probe followed the 302 and read the **CDN**
  response's bare `ETag`, which on a Xet-backed repo is the **Xet content hash** (`X-Xet-Hash`), not
  the file sha256 — a clean 64-hex value that passed `parse_sha256`, so the correctly-assembled blob
  (`66358cb1…`) was verified against the Xet hash (`d4ccbe2a…`) and rejected forever (the GGUF is
  ~75% of the GGUF+mmproj total, so it finished and restarted before the mmproj began). The real
  sha256 is in `X-Linked-ETag` on the `resolve` redirect, discarded by reqwest's auto-follow. Fix:
  probe **redirect-less** and read the 302's own headers; `lfs_sha256` trusts `X-Linked-ETag`
  **only**, never the bare `ETag`. Verified live: the 302's `X-Linked-ETag` = the HF-API LFS oid =
  our downloaded bytes (`66358cb1…`); the CDN `ETag`/`X-Xet-Hash` = `d4ccbe2a…`. Added test
  `lfs_sha256_trusts_only_x_linked_etag_not_cdn_etag` (rejects a bare CDN/Xet `ETag`).
- **Change (reuse, not bypass):** all chunks feed the existing `downloaded` `AtomicU64`, so the UI
  percentage and aggregate-progress stall detection are unchanged; the finalize phase sets the
  existing `copying` flag; the blob lands in the clean layout via atomic rename; the already-in-layout
  / already-a-cache-blob fast paths are still first.
- **Change (fallback + integrity):** a range-less server (no `Accept-Ranges`/no length) or a probe
  failure falls back to the unchanged single-stream `download_with_lock_retry`. A per-chunk `206`
  assertion refuses a server that ignores `Range`; `401`/`403` re-resolves the signed URL once; the
  assembled file is verified against the LFS sha256 (`X-Linked-ETag`) when advertised (else byte
  length), and a mismatch discards the partial for a clean retry. Added `sha2` as a direct dep
  (already transitive).
- **Decision:** connection count is a const + env override (not a `Settings` field) — user-approved,
  keeps PR8 confined to `download.rs` and matches the file's existing const/env convention; no IPC/
  bindings/UI change.
- **Tests (PR8 owns them, fully mocked — wiremock + a local hang-after-206 server, no network):**
  `chunked_download_assembles_byte_identical_file`, `resume_skips_already_completed_chunks`,
  `chunked_download_fails_fast_on_stuck_chunk`, `chunked_download_errors_when_server_ignores_range`,
  `integrity_accepts_matching_sha256_and_rejects_a_wrong_one`, `range_plan_requires_ranges_and_known_
  size`, `parse_sha256_normalizes_etag_forms`, `lfs_sha256_trusts_only_x_linked_etag_not_cdn_etag`,
  `content_range_total_parses_suffix`.
- **Interface review:** no trait-signature, IPC, command, or `ts-rs` change (binding guard clean).
- **Verification:** `cd ui && npm ci && npm run lint && npm run build` (lint clean; `vite build` ✓);
  `cargo fmt --all -- --check` (exit 0); `cargo clippy --workspace --all-targets -- -D warnings`
  (0 warnings); `cargo build --workspace` (Finished); `cargo test --workspace` (all crates 0 failed,
  **inference 84 passed** incl. the 11 new tests); `git diff --exit-code -- ui/src/bindings` (clean).

### 2026-06-26 — PR8 review hardening (PR #35 bot review)

Addressed the PR #35 bot review (Gemini + the GitHub `claude` reviewer; bot replies not posted, per the
request). Five robustness fixes in `download.rs`, each with a test; two cross-platform suggestions
declined as out-of-policy. Still confined to `download.rs` (+ tests + docs); no IPC/binding change.

- **Stale-manifest silent corruption (claude, medium).** `open_preallocated` now returns
  `(File, created)` — `created` via an atomic `create_new` open, no `exists()` TOCTOU. `chunked_download`
  re-initialises an all-done `.parts` bitmap sitting over a **freshly created** (zero-filled) `.part`
  (`Manifest::reinit` / new `init_sync`), so a prior run's failed post-publish bitmap cleanup can't make
  the next run skip every chunk and publish zeros (length check passes; sha256 absent when no
  `X-Linked-ETag`). Proven: with the guard disabled the new test publishes an all-zero file; with it,
  byte-identical.
- **Network-error retry (gemini, high).** A chunk request's transport error (dropped connection /
  timeout / DNS) was propagated by `?`, failing the whole download on the first hiccup. It now feeds the
  same bounded backoff loop as a transient HTTP status.
- **Don't clobber progress on an unreadable manifest (gemini, high).** `Manifest::load_or_init_sync`
  now matches on the read: `NotFound` → init fresh; any other error (a Windows sharing violation from
  AV / the indexer) → propagate, so the job-queue retries instead of truncating a valid bitmap and
  losing real download progress.
- **Coalesced writes (gemini, medium).** Body frames buffer to `CHUNK_WRITE_BUFFER` (256 KiB) before
  each positioned `spawn_blocking` write (`flush_chunk_writes`), cutting blocking-task churn; bytes
  still accrue into the progress counter per frame.
- **Accurate terminal error (claude, low).** The chunk failure return now distinguishes "server ignored
  Range" (a `200`) from "failed after N retries" (an exhausted `403`/`429`).
- **Declined (gemini, two high):** add `#[cfg(unix)]` `write_at` / cfg-gate `FileExt` so `download.rs`
  compiles on macOS/Linux. The project is **Windows-only by design** (CLAUDE.md hard rule; CI is
  `windows-latest`; `flags.rs`/`lib.rs` already use Windows-native APIs without cross-platform branches).
  Confirmed with the user before declining.
- **New tests:** `fresh_part_discards_stale_all_done_manifest`,
  `exhausted_transient_is_not_reported_as_ignored_range`,
  `manifest_load_or_init_distinguishes_missing_valid_and_mismatched`. (The present-but-unreadable
  manifest branch is covered by inspection — a non-`NotFound` read error can't be injected
  deterministically without a real sharing violation.)
- **Verification:** `cargo fmt --all -- --check` (exit 0); `cargo clippy --workspace --all-targets -- -D
  warnings` (0 warnings); `cargo test --workspace` (all crates 0 failed; **inference lib 87 passed**,
  +3 new); `git diff --exit-code -- ui/src/bindings` (clean). The #6 test was confirmed to fail with the
  guard disabled (published an all-zero file) before being restored.

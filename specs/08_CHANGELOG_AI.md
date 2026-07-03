# 08 — AI Changelog

> Append-only record of what the agent changed during the build, **with reasons**. One entry per
> meaningful change set. Empty until P0 begins. (This tracks build work; the design-phase history
> lives in git.)

## <date> — <short title>
- **Change:** what was added/modified.
- **Why:** the reason, tied to a spec section.
- **Verification:** the command run + verbatim result.

---

> Pre-0.2.x (v0.1.0) history → `specs/archive/08_CHANGELOG_AI.v0.1.0.md`.
> Shipped 0.2.x history (0.2.0–0.2.2) → `specs/archive/08_CHANGELOG_AI.v0.2.x.md`.
> Live file holds only the current (post-0.2.2) arc.

---

## 2026-07-03 — 0.3.0 PR3: Beta model tier removal (`feat/pr3-beta-tier-removal`)
- **Change:** Retired the **Beta** tier from both inference lanes — **Default / Quality only** (D3/D4).
  Deleted the two Beta models (vision `jc-builds/Qwen3.5-9B-VLM-Q4_K_M-GGUF`, answer
  `nvidia/NVIDIA-Nemotron-3-Nano-4B-GGUF`). **No schema change** (tiers live in the `settings` table,
  not the schema).
  - `crates/traits/src/ipc.rs`: `ModelTier` → `{ Default, Quality }` (dropped `Beta`); doc comment
    records the retirement + load-remap. ts-rs regenerated `ui/src/bindings/ModelTier.ts` to
    `"default" | "quality"`.
  - `crates/inference/src/models.rs`: deleted the two `repo_for` Beta arms + the `tier_slug` `Beta`
    arm (the only two exhaustive matches on the enum in the workspace). Extended
    `repo_mapping_matches_registry` to assert all **four** surviving `(lane, tier)` → repo + mmproj
    pairs per `MODEL_REGISTRY §1/§2` (supports acceptance line 3).
  - `crates/kernel/src/settings.rs`: new `load_tier(store, key, default)` helper replaces the generic
    `json()` read for the two `models.*_tier` keys. A persisted `"beta"` is mapped to `Quality`,
    **persisted** via `set_setting` (best-effort, warn-and-swallow on error), and returned — so it
    logs **once** (the retired token leaves the DB; next load reads `"quality"`), the same mechanism
    as `drop_retired_settings`. The remap lives in the **load path**, not the startup-maintenance
    sweep, because the composition root builds the sidecars straight from `load_settings`' output; a
    sweep-side remap would race it and the first post-upgrade session would run `Default`. Any *other*
    unparsable tier value keeps the old behavior (fall back to default, no rewrite).
  - Tests: `crates/kernel/tests/settings.rs` — new `persisted_beta_tier_remaps_to_quality_and_persists`
    (seed `"beta"` both lanes → load = Quality, DB rewritten to `"quality"`, second load idempotent)
    and `unknown_tier_falls_back_to_default_without_rewrite` (a non-Beta bad value is not migrated);
    fixed `round_trips_non_default_values` (`Beta` → `Quality`).
  - UI: `ModelTierPicker.tsx` (TIERS + MODEL_NAMES beta rows + header comments) and `Settings.tsx`
    (`TIER_LABEL`) lose Beta — TypeScript's `Record<ModelTier, …>` is the tripwire that forced them.
  - Docs: `README.md` (2-tier table), `docs/ARCHITECTURE.md` §7.3, `docs/TESTING.md` (new model-tier
    manual-acceptance section), `CHANGELOG.md`.
- **Why:** `docs/0.3.0.md` PR3 + `02 §5c`/`03 §8`/`§13b.2` — cut the model-testing matrix by a third
  and make licensing uniformly Apache-2.0; Nemotron (OML license, hybrid arch) was the single riskiest
  registry row. D3 (beta→quality on load), D4 (leave on-disk GGUFs) settled in the roadmap.
- **Verification:** `cargo fmt --check` / `clippy --workspace --all-targets -D warnings` /
  `build --workspace` / `test --workspace` all exit 0 (kernel settings **10** incl. the 2 new; inference
  **102** incl. the extended registry test; traits 53; store 24+58; full workspace 0 failed);
  `npm run lint` (clean) / `npm run build` (`✓ built`); `git diff -- ui/src/bindings` = only
  `ModelTier.ts` (regenerated, committed). **Grep gate:** `Nemotron` / `Qwen3.5-9B` appear only in
  history/rationale docs (CHANGELOG entries, archives, `docs/0.3.0.md`, specs retirement language) —
  zero in `crates/`, `src-tauri/`, `ui/src/`, README, ARCHITECTURE; `beta` survives in source only as
  the `load_tier` migration literal + incidental test fixtures. **Live (real desktop, `npm run tauri
  dev`):** seeded the fresh dev DB with `models.vision_tier=models.answer_tier='"beta"'`, relaunched →
  two `WARN kernel::settings: settings: retired \`beta\` tier mapped to \`quality\`` lines (one per
  lane), the DB rows persisted to `'"quality"'`, and a second in-session load emits no further warn
  ("logged once"); the app ran on the remapped Quality tiers (`inference providers attached; sidecar
  ready`). Full verbatim in `05` (Pass 2026-07-03 PR3).

## 2026-07-03 — 0.3.0 PR2: event-trigger trim (`feat/pr2-trigger-trim`)
- **Change:** Cut the six opt-in event-capture triggers to **foreground + idle** (D1), deleting the
  `WH_MOUSE_LL` global mouse hook (click/scroll-stop), the `AddClipboardFormatListener` clipboard
  listener, and the typing-pause edge — plus their five `capture.event_*` settings fields. **No schema
  change** (D2); the `CaptureTrigger` enum, its DB-token maps, the `frames.capture_trigger` CHECK, and
  the Moment `TRIGGER_LABEL` all stay so legacy frames still render their trigger.
  - `crates/capture/src/trigger.rs`: `InputEventKind`→`{Foreground}`; `TriggerConfig`→5 surviving
    fields; `poll()` idle-only; 14 tests → 9 (retired-only deleted; two surviving-logic tests rewritten
    off the retired `Clipboard` kind; typing-pause test → idle-edge).
  - `crates/capture/src/events.rs`: **deleted the message-only window + the whole mouse-hook `unsafe`
    path + the clipboard listener**; `start()` is now param-less; the hook thread forces its message
    queue with `PeekMessageW(PM_NOREMOVE)` before signaling ready (the window used to guarantee it,
    which `Drop`'s `WM_QUIT` post depends on), installs one out-of-context foreground WinEvent hook.
  - `crates/traits`: 5 fields removed from `Settings` + `CaptureConfig`; new required Store method
    `delete_settings`; `CaptureTrigger` retired variants reworded **legacy — no longer emitted**.
  - `crates/store`: `delete_settings` impl + delegation (no schema change).
  - `crates/kernel/src/settings.rs`: retired reads/writes/clamp/maps removed; new
    `RETIRED_SETTINGS_KEYS` + `drop_retired_settings` (one `warn!`, error-swallowing).
    `src-tauri/src/lib.rs`: call it once at startup (before the maintenance sweep).
  - UI: `Settings.tsx` event panel → master + app-switch + idle + 3 thresholds; `Settings.ts` binding
    regenerated (5 fields gone); `CaptureTrigger.ts` unchanged.
  - Docs: `docs/ARCHITECTURE.md`, `docs/TESTING.md`, `README.md`, `CHANGELOG.md`.
- **Why:** `docs/0.3.0.md` PR2 + `02 §5c` — remove the invasive global mouse hook the 0.2.0 design
  avoided, the clipboard privacy-optics liability, and the idle-redundant typing-pause; every removal
  deletes user config surface, maintainer decision surface, and audit surface (`03 §8` L616–631,
  `§13b.1`; settings-load-tolerance = D1's "drop + log once, never crash").
- **Verification:** `npm run lint`/`build` (exit 0 / `✓ built`); `cargo fmt --check`/`clippy -D
  warnings`/`build`/`test --workspace` all exit 0 (capture 22+1ign, kernel settings 8, store 24+58);
  `git diff -- ui/src/bindings` = only `Settings.ts`. Grep gate clean (retired symbols only in
  history notes + the read-path exemptions). **Live (real desktop):** window-less foreground hook 50×
  start/drop `ok`; seeded dev DB with the 5 retired keys → dropped on load with one `warn` line, none
  on relaunch, boots clean; live DB `schema_version=8` (unchanged) accepts + reads back a
  `capture_trigger='click'` frame. Full verbatim in `05` (Pass 2026-07-03).

## 2026-07-03 — 0.3.0 arc specs contract (PR1, specs-only) (`feat/0.3.0-pr1-specs-contract`)
- **Change:** Normalized the 0.3.0 roadmap (`docs/0.3.0.md`, decisions D1–D15) into the spec contract
  so PR2–PR9 are implementable from the specs alone. **No code / no schema code / no UI** — only
  `specs/`, `docs/`, `CLAUDE.md`, `AGENTS.md`, `CHANGELOG.md`.
  - `02`: new **§5c** (0.3.0 arc — problem/thesis/additions/ships-in/deferred); two-tier §2/§3; §6 risk
    rows (drop the Nemotron row; add hotkey-conflict + API-token-leak rows); §7 non-goals (+ proactive
    nudges, audio *for now*); §8 Status → 0.3.0 active.
  - `03`: §8 removed the 5 retired event keys + `enrich.image_embeddings`; added `overlay.*`/`resume.*`/
    `marks.*`/`api.*` groups + the `beta`→`quality` load mapping (D3/D4) + the settings-load-tolerance /
    no-schema-change contract (D1/D2). §4 added the `marks` table + documented **both** forward-only
    migrations (PR4 image-lane drop, PR6 marks; D5/D10/D15) with the relative-version rule, and removed
    the image-embedding DDL + `embed_image` refs across §3/§4/§5. New **§7b** (where-was-i + marks +
    `capture_now` — D7/D8/D9) and **§7c** (localhost HTTP API + export + SSE + MCP — D11/D12/D13).
    §12/§13 reconciled to two tiers; new **§13b** DoD (PR2–PR9 acceptance).
  - `UI_REFERENCE`: Overlay screen (identity / five states / keyboard / <150 ms perf / reduced-motion /
    self-exclude), Deck where-was-i card + Intentions strip, Settings hotkeys + Local API row (threat
    model + loud port-in-use), `ModelTierPicker` → Default/Quality, `Domain (0.3.0)` components.
  - `MODEL_REGISTRY`: deleted both Beta rows + the image-embedding row + the Nemotron invariant.
  - `00`/`01`: two-tier consistency (**required** — `04 §2` routes model-tiers to `00`) + image-model
    strike; `00 §D` flags (image embeddings removed / reranker never implemented).
  - `04`: 0.3.0 reading-order line, source-of-truth row, PR1→PR9 build-order sequence.
  - `07`: five deferrals (#75–#79), the resolved API port-bind UX (#80 — "loud + guided change"), and a
    doc-sweep tracking row (#81 — `docs/ARCHITECTURE.md`/`TESTING.md` assigned to PR2/PR4/PR9).
  - `CLAUDE.md`/`AGENTS.md`: current-state paragraph → 0.3.0.
- **Why:** the arc ships specs-first (same method as the 0.2.x PR1); `docs/0.3.0.md` "What PR1 must
  change, file by file" + its acceptance ("a fresh agent can implement PR2 from the specs alone").
- **New ambiguity (not in D1–D15):** API port-bind failure UX — surfaced to the user, resolved to
  **"loud + guided port change"**, contract written into `03 §7c` + `UI_REFERENCE`, recorded in `07` #80.
- **PR #70 review round (bot comments; not replied to per user instruction; each verified vs. real
  code first):** `03 §2` moved the `capture_now` note outside the `CaptureSource` trait ("not a trait
  method"); `03 §7`/`§7c`/`§4` settled one canonical `list_marks` order (all marks, unresolved first
  then newest-first) + fixed `idx_marks_open` to `created_at DESC`; `03 §7b` anchors where-was-i on the
  **last non-ScreenSearch foreground** (overlay-focus bug) and absorbs transient excursions via the D9
  dwell threshold; `03 §7b` pins `capture_now` to the **foreground-window monitor** (multi-monitor
  determinism); `03 §7c` cancels `/v1/ask` inference on client disconnect and streams `/v1/export`
  (flat memory, bounded window). A `CHECK` on `jobs.kind` was **declined** (live `schema.rs` has none;
  would force an unplanned PR4 rebuild) and recorded as opt-in hardening in `07` #82. Full mapping: `05`.
- **Verification — verbatim** (specs-only PR — the untouched tree must still build):
  - `cargo fmt --all -- --check` → `FMT_EXIT=0`
  - `cargo build --workspace` → `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 19.79s` / `BUILD_EXIT=0`
  - `git status --short` diff limited to `specs/*`, `docs/0.3.0.md`, `CLAUDE.md`, `AGENTS.md`,
    `CHANGELOG.md`, `.gitignore` — **no `.rs`/`.ts`/`.tsx`/`.toml`/`ui/` files touched** (bindings
    untouched by construction).

## 2026-07-01 — UIA cache-batched walk: efficiency lever (#71) (`fix/uia-findall-buildcache`)
- **Change:** `crates/uia/src/worker.rs` — the foreground-window UIA walk now batches each node's ~5
  separate `Current*` property reads into **one `BuildUpdatedCache`** call + cached getters
  (`build_cache_request`: ControlType/Name/IsPassword/IsOffscreen/BoundingRectangle/ValueValue +
  ValuePattern, `_Element` scope, `_Full` mode). Same walker DFS structure/bounds as the shipped code;
  live `TextPattern` stays gated/capped; `Value`/`Name` read from the cache. ~2.5× fewer cross-process
  COM calls per walk.
- **Why:** `07` #71 efficiency lever (deferred from the 0.2.1 hang mitigation). The gap required live
  verification. Two bulk-fetch designs were live-tested and **rejected** as unbounded: a single
  `FindAllBuildCache(Subtree)` (~1.4 s on a large window) and a `FindAllBuildCache(Children)` BFS (a
  single wide-node fetch overran the budget on VS Code-scale trees). The granular per-node
  `BuildUpdatedCache` keeps small, deadline-interruptible calls — no wide-node cliff.
- **Review fixes (3, adversarial):** cache the `ValueValue` property (else `CachedValue()` fails and
  edit-field/omnibox text is silently dropped); descend past a `BuildUpdatedCache` failure (a transient
  timeout must not prune a subtree); full coverage parity (descend into everything, like the old DFS).
- **Verification — verbatim:** `cargo fmt --all -- --check` EXIT 0; `cargo clippy --workspace
  --all-targets -- -D warnings` EXIT 0; `cargo test --workspace` 0 failed (uia 16 + 2-ignored). Live:
  `cargo test -p uia -- --ignored` passes **bounded** on a heavy window that timed out the bulk-fetch
  variants; `npm run tauri dev` captured `primary_source='uia'` Chrome frames (1186–1748 chars, omnibox
  URL present, no over-budget warnings).
- **PR #68 review fixes (2026-07-01):** (1) reworded 7 stale `FindAllBuildCache` doc-comments to the
  shipped `BuildUpdatedCache` design (comment-only). (2) **Raw-view cache filter** — a cache request's
  `TreeFilter` defaults to control-view, so with `capture.uia_view_control_only` off the `RawViewWalker`
  navigated to raw-only nodes whose properties the filter skipped (`Cached*` empty → text lost to OCR).
  `build_cache_request` now takes the view flag and `SetTreeFilter(Control|Raw ViewCondition)` in
  lock-step with the walker; control-view default unchanged. Verify: `fmt`/`clippy`/`cargo test -p uia`
  EXIT 0; live `--ignored` control-view path non-regressed (3×: 282 spans / 6316 chars / ~90 ms).
  (3) **Don't cache field values before the privacy guard (Codex P2).** Caching `ValueValue` meant
  `BuildUpdatedCache` prefetched every node's field value — including password/offscreen fields —
  *before* `should_emit` runs, a visible-only/"password fields are never read" regression vs. the
  pre-#71 live walk (which read `Value` only after the guard). Removed `ValueValue`/`ValuePattern`
  from the batched cache; `extract_text` now reads `Value` **live** via `GetCurrentPattern`, and it is
  only called after the guard passes — so a masked/hidden value is never fetched. `Name`/metadata stay
  batched (the bulk of nodes are static text), and value-bearing inputs are a small live-read fraction.
  Verify: `cargo fmt -p uia -- --check` EXIT 0; `cargo clippy -p uia --all-targets -- -D warnings`
  EXIT 0; `cargo test -p uia` 16 passed/2 ignored; live `--ignored` walk yields text (4 spans / 30 chars).

## 2026-07-01 — Degrade-to-text DB shrink: merge purged spans to lines (#73a) (`fix/degrade-to-text-db-growth`)
- **Change:** Degrade-to-text retention now shrinks the DB too. For a purged frame, the per-word
  `text_spans` are merged into per-line spans: new pure `merge_spans_to_lines` (group by `line_index`,
  union bbox, join text, content-wins role/searchable) + store `merge_frame_spans_to_lines` (one
  transaction). Wired into `run_retention_once` (via the atomic `degrade_frame_to_text`, see the
  PR #67 review fix below) and a one-time watermark-gated backfill `merge_purged_spans_once`
  (`maintenance.purged_spans_merged`) over the pre-existing purged backlog, backed by new
  cursor-batched `store::purged_frame_ids`.
- **Why:** `07` #73 (a). The DB (~40% of growth) didn't shrink on retention. `text_spans` are the
  largest prunable artifact but power `FrameReconstruction` for purged frames (`MomentDetail.tsx`
  renders it in place of the purged image), so they're **merged** (keeps a line-level reconstruction),
  not pruned. Search is unaffected (FTS reads `content_text`; the vector arm reads `embeddings`).
- **Review fix (CONFIRMED low):** `merge_purged_spans_once` set the completion watermark even when
  individual frames failed to merge, diverging from the `purge_self_captures` retry pattern. Now a
  `clean_drain` flag withholds the watermark on any list- or per-frame failure, so the idempotent
  backfill retries next launch. Covered by a new `screensearch_lib` test.
- **PR #67 review fixes (2026-07-01):**
  - **Codex P2 — stranded per-word rows after a mid-sweep merge failure (fixed).** The sweep degraded
    a frame in two writes: `purge_frame_image` (sets `image_purged = 1`) then a non-fatal
    `merge_frame_spans_to_lines`. If the merge failed *after* the flag was set, the frame — now
    excluded from `frames_with_image_older_than` (`WHERE image_purged = 0`) and, once the backfill
    watermark was set, from the backfill too — kept its per-word rows forever. Replaced with the
    **atomic** `store::degrade_frame_to_text` (merge **and** flag in one transaction); on failure
    nothing commits, `image_purged` stays `0`, the whole frame retries next sweep. New store tests
    `degrade_frame_to_text_merges_spans_and_purges_atomically` / `_purges_even_without_spans`.
  - **Gemini "N+1 / bulk `IN`" ×2 — declined, recorded (`TODO.md` TODO-2).** Embedded SQLite has no
    network round-trip; neither the one-time backfill nor the hourly sweep is hot; and a single
    `IN`-clause transaction would forfeit the per-frame failure isolation the `clean_drain` backfill
    relies on to converge (one busy frame rolls back a whole 256-batch). Kept per-frame transactions
    with a documented deferral + how to batch safely if it ever matters.
- **Verification — verbatim:** RED then GREEN across `merge_spans_to_lines` (4 unit),
  `merge_frame_spans_to_lines_*` / `degrade_frame_to_text_*` / `purged_frame_ids_*` (store
  integration), and `merge_purged_spans_once_*` (`screensearch_lib`). Full CI (re-run after the
  PR #67 review fix, 2026-07-01): `npm run lint` EXIT 0 / `npm run build` `✓ built in 1.70s`;
  `cargo fmt --all -- --check` EXIT 0; `cargo clippy --workspace --all-targets -- -D warnings` EXIT 0
  (3.58s); `cargo build --workspace` EXIT 0; `cargo test --workspace` all green, 0 failed (store 18
  lib + **54** integration incl. 2 new `degrade_frame_to_text_*`; `screensearch_lib` 16/18, 2 ignored);
  `git diff --exit-code -- ui/src/bindings` clean. Adversarial 3-lens review: 1 low finding, fixed;
  PR #67 external review: 1 P2 fixed (atomic degrade), 2 N+1 declined + recorded.

## 2026-07-01 — Vector-arm time-range recall: adaptive KNN escalation (#8) (`fix/vector-arm-time-range-recall`)
- **Change:** `crates/store/src/search.rs::text_knn_in_range` now escalates the KNN `k` for time-windowed
  search instead of running a single `k = pool` pass. A bounded `time_range` re-runs the cosine KNN with a
  geometrically larger `k` (factor 8, ceiling 20 000) until the pool fills with in-range frames, the vector
  table is exhausted (KNN returned `< k` rows), or the ceiling is hit; an unbounded range is unchanged
  (one pass, the time filter a no-op). New constants `KNN_ESCALATION_FACTOR` / `MAX_TIME_RANGE_KNN`; the
  time filter + frame de-dup moved from SQL into Rust so the loop can see the raw KNN row count (its
  exhaustion signal). New test `vector_arm_finds_in_range_match_buried_beyond_pool` + `vec_at_angle` helper.
- **Why:** `07` #8 — sqlite-vec 0.1.9 can't filter inside a KNN `MATCH` (0.1.10-alpha is broken), so the
  old post-KNN time filter silently dropped in-range matches ranked beyond the top-`pool` nearest vectors
  (recall under-count on tight windows). `03 §4/§13`.
- **Verification — verbatim:** RED `vector_arm_finds_in_range_match_buried_beyond_pool` → `left: [] right: [56]`;
  after fix `cargo test -p store` → `50 passed; 0 failed`. Full CI: `npm run lint` EXIT 0 / `npm run build`
  `✓ built in 2.11s`; `cargo fmt --all -- --check` EXIT 0; `cargo clippy --workspace --all-targets -- -D
  warnings` EXIT 0; `cargo build --workspace` EXIT 0; `cargo test --workspace` all green 0 failed; perf
  `p95 = 80.3555ms` < 200 ms; `git diff --exit-code -- ui/src/bindings` clean. Adversarial 3-lens review
  workflow: **no findings**.
- **Review response (PR #66, Codex P2 — count-capped escalation target):** a *sparse* bounded window
  (fewer distinct embedded frames than `pool`) on a DB with > `MAX_TIME_RANGE_KNN` vectors trips neither
  the pool-fill nor the exhaustion gate, so it climbed to the 20 000 `k` ceiling on **every** query even
  after finding all in-window matches. Now the escalation `target` is capped at
  `count_embedded_frames_in_range(start, end, cap=pool)` — an index-served `EXISTS` semi-join
  (`idx_frames_captured_at` range + `idx_embeddings_frame`), `LIMIT`-bounded so it stays O(pool) not
  O(window) (resolves the reviewer's residual-cost concern). `target = min(pool, count)`; `count == 0`
  skips the KNN. Loop extracted into pure `escalate_in_range_knn(pool, target, fetch)`. New tests:
  5 `escalating_knn_*` unit tests (**3 observed red** on a naive single-pass first),
  `count_embedded_frames_dedups_chunks_and_honors_cap`, and integration `sparse_/dense_/empty_time_window_*`.
  Verbatim: `cargo test --workspace` all green **0 failed** (store 53 integration + 20 lib); `cargo fmt
  --all -- --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo build --workspace`
  all EXIT 0; perf `median = 27.3359ms, p95 = 65.8744ms` < 200 ms; bindings clean;
  `EXPLAIN QUERY PLAN` → `COVERING INDEX idx_frames_captured_at` + `EXISTS … idx_embeddings_frame`.
  Adversarial re-review (3-lens, refute-by-default verify): 1 **LOW** finding (uncapped-count cost)
  already resolved by the `LIMIT`; no correctness findings.
- **Follow-up review response (PR #66, Codex P2 — bound the pre-count scan):** Codex refuted the
  "O(pool) via `LIMIT`" claim above — the `LIMIT` caps *matches*, so a window with many captured frames
  but few embedded ones (embed backlog / wide range) never fills it and the count walked the whole frame
  range (O(frames-in-window)). Fixed by bounding frames *examined*: `count_embedded_frames_in_range` now
  takes `(pool, scan_cap)` and returns `Option<usize>`; the inner select is `LIMIT scan_cap`, the outer
  returns `(scanned, embedded)`. `scanned == scan_cap` → too large to prove sparse → `Some(pool)` (dense
  assumption; only *raises* the target, never drops a match); else exact → `None` if zero (skip KNN) or
  `Some(min(pool, embedded))`. `COUNT_SCAN_CAP = MAX_TIME_RANGE_KNN` (20 000) — the count never examines
  more frames than a ceiling KNN examines vectors, so it is now genuinely O(pool) even on a sparse wide
  window. `escalate_in_range_knn` + its unit tests unchanged. TDD: rewrote the count test with a
  scan-budget case (**observed red** `Some(2)` → green `Some(pool)`). Verbatim: `cargo test -p store`
  lib **20**/integration **53** all `ok`; fmt/clippy/build EXIT 0; perf `median = 26.9464ms, p95 =
  68.57ms` < 200 ms; bindings clean.

## 2026-06-30 — PR #63 review fixes: NavRail tab-stop sync + palette focus-on-navigate (#42)
- **Change:** Two real bugs in the #42 a11y work, flagged by reviewers (Gemini/Claude/Codex all caught
  the first):
  1. **NavRail roving tab-stop didn't follow external navigation.** `focusIndex` was seeded once at
     mount, so navigating via the Command Palette / an in-app link / browser back left `tabIndex=0` on a
     stale link. Added `useEffect(() => setFocusIndex(activeIndexFor(pathname)), [pathname])` — re-derives
     the tab stop only (never calls `.focus()`); arrow moves don't change the path so they're untouched.
  2. **Command Palette focus-restore stole focus on navigation.** Restoring focus to the opener on every
     close yanked it back to the ⌘K trigger when a command navigated to a route that autofocuses (Recall's
     search input). Fix: restore **only on dismiss** — `run()` sets `restoreFocusRef.current = false` so a
     command's destination/action owns focus; the cleanup also now guards `openerRef.current?.isConnected`.
     (Gemini's literal `document.body.contains` suggestion alone wouldn't fix it — the trigger lives in the
     always-mounted NavRail, so it's always connected; the dismiss-vs-run distinction is the real fix.)
- **Why:** Both regress keyboard/SR navigation introduced by the #42 changes; `UI_REFERENCE` §7.
- **Verification — verbatim:** `npm run lint` EXIT 0; `npm run build` `✓ built in 1.45s`. **Live Playwright
  probe:** (1) palette-navigate `/`→`/timeline` ⇒ NavRail tab stop = **Timeline** (was stale Deck before);
  (2) palette-navigate →`/recall` ⇒ `document.activeElement` = the **"Search query" `INPUT`** (not the ⌘K
  button); (3) focus ⌘K → `Ctrl+K` → `Esc` ⇒ focus **restored to the ⌘K button** (dismiss path intact).

## 2026-06-30 — Cancel Inno installer (#26) + single-instance focus + a11y matrix (#42) (`chore/cancel-inno-and-a11y-matrix`)
- **Change:** Three known-gap closures.
  1. **#26 packaging — Inno/portable-ZIP/MSI formally dropped, gap closed.** Tauri 2 shipped an
     unsigned NSIS installer in v0.1.0 (`bundle.targets=["nsis"]`); the specs still demanded an "Inno
     Setup installer + portable ZIP" in 9 live places. Rewrote every one to NSIS — `00` §A/§G, `01`,
     `02` P5, `03` §11 + DoD §13.9, `docs/ARCHITECTURE.md` §12, `.github/workflows/ci.yml`, `README.md`
     — re-scoped DoD §13.9 to "NSIS builds successfully" (met), and flipped `07` #26 to ✅ with
     **code-signing as the lone open packaging item** (already tracked under `07` "Manual steps").
  2. **Single-instance focus (Gemini PR #27 follow-up).** The `src-tauri/src/lib.rs` single-instance
     callback now calls `window.show()` before `unminimize()`/`set_focus()`, so a hidden/tray-minimized
     window is restored (not just unminimized) on a second launch.
  3. **#42 keyboard/focus matrix — five UI fixes.** NavRail roving-tabindex (Arrow/Home/End, wrap) +
     `aria-current="page"`; Command Palette focus restoration on close; Recall Ask focus-to-answer on
     stream completion; Settings `<Panel group>` (`role="group"` + `aria-labelledby`, the ARIA
     fieldset/legend equivalent, card layout untouched).
- **Why:** `07` #26/#42 + the `07` single-instance TODO. #26 was a standing spec-vs-reality
  contradiction (logged in `06` #16); #42 was an open P5 a11y audit follow-up; the single-instance
  bullet was a deferred PR #27 review note.
- **Verification (Windows) — verbatim:**
  - `npm run lint` → `EXIT 0`; `npm run build` → `✓ built in 1.96s`
  - `cargo fmt --all -- --check` → `EXIT 0`
  - `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile … in 53.41s` / `EXIT 0`
  - `cargo build --workspace` → `Finished … in 22.11s`; `cargo test --workspace` → every suite `ok`,
    **0 failed** (inference 102, traits 53, store 49+14, kernel 27, capture 27, uia 16/2-ignored,
    sysmon 11, textfilter 12, screensearch_lib 7, embeddings 1, ocr 1, doctor 0)
  - `git diff --exit-code -- ui/src/bindings` → clean (`EXIT 0`)
  - **Live focus probe (Playwright/Chromium vs the Vite dev server):** NavRail `Deck {tabIndex 0,
    aria-current page}`, ArrowDown→Recall (tabIndex follows), End→Settings, ArrowDown wraps→Deck,
    ArrowUp wraps→Settings, re-seeds to active route on navigation; Command Palette `Ctrl+K`→
    `role=combobox` input, `Esc`→focus restored to the ⌘K `BUTTON`. (Settings-group + Ask-focus need
    live backend data the IPC-less probe can't supply → build + code-verified.)

## 2026-06-30 — Model-downloader resume hardening (`fix/download-resume-hardening`)
- **Change:** Two localized fixes in `crates/inference/src/download.rs`, both TDD'd.
  1. **Gap #69 — wrong-sized `.part` no longer publishes garbage.** `open_preallocated` now returns
     `unbacked = (pre_existing_len != total)` instead of "created"; the chunked-download caller
     discards a header-matching `.parts` bitmap whenever the part is `unbacked`. This covers the
     external-cleanup case (a tool truncates an existing `.part`; `set_len` re-grows it with zeros)
     that the old "created"-only check missed, plus the corruption-grown (`> total`) case (broadened
     from `< total` to `!= total` per a PR #62 review note), while never flagging a legitimate resume
     (always preallocated to exactly `total`). New red→green tests
     `truncated_part_discards_stale_partial_manifest` + `oversized_part_discards_stale_manifest`.
  2. **PR #27 Codex-P2 — re-check cache before retrying a locked download.** Extracted the
     clean-layout + HF-cache fast paths into `place_if_cached`; folded the single-stream
     lock-retry into `fetch_one` so that after each `LockAcquisition` backoff it re-checks
     `place_if_cached` and short-circuits if the lock holder finished during the sleep (no
     re-download / publish collision). Extended the backoff (added `LOCK_RETRY_BACKOFF_CAP` 15 s,
     `LOCK_RETRY_MAX_ATTEMPTS` 5→24 ≈ 5 min total) so a real multi-GB download by the holder is
     outlasted rather than abandoned at ~20 s. New `place_if_cached_*` unit tests. The doc-hidden
     `download_file_with_lock_retry_for_diagnostics` (used by `examples/repro_8b.rs`) keeps its own
     minimal backoff loop.
- **Why:** Both are open durability gaps in `07_KNOWN_GAPS.md` — silent corruption (zeros published,
  length check passes, sha256 skipped when the CDN advertises no `X-Linked-ETag`) and wasted
  re-download/collision on lock contention. `03 §13` wants the downloader robust and resumable. The
  separate `#46` row (orphaned **detached** writers in the same fallback) stays open — it needs
  replacing hf-hub's high-level downloader, out of this scope.
- **Verification (Windows, after the PR #62 review fix) — verbatim:**
  - `cargo test -p inference --lib` → `test result: ok. 102 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.05s` (incl. `truncated_part_discards_stale_partial_manifest`, `oversized_part_discards_stale_manifest`, `place_if_cached_short_circuits_when_dest_already_present`, `place_if_cached_returns_false_when_nothing_cached`; existing `resume_*`/`fresh_part_*`/`integrity_*` unchanged)
  - `cargo fmt --all -- --check` → `EXIT 0`
  - `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile … in 2.43s` / `EXIT 0`
  - `cargo build --workspace` → `Finished dev profile … in 8.90s` / `EXIT 0`
  - `cargo test --workspace` → all suites green / `EXIT 0`
  - `git diff --exit-code -- ui/src/bindings` → clean (`EXIT 0`)

## 2026-06-30 — Spec archival sweep + close gaps #43/#44 (`chore/archive-known-gaps`)
- **Change:** (1) Restored the archive-on-release convention: moved the shipped 0.2.0/0.2.1/0.2.2
  entries out of the live build-loop logs (`05`/`06`/`07`/`08`) and `CHANGELOG.md` into per-arc
  `specs/archive/*.v0.2.x.md` files and `CHANGELOG-ARCHIVE.md`, verbatim with original `#N` ids. `07`
  also moved its resolved-engineering-decisions list and retired five accepted-as-is rows (#40, #45,
  #59, #60, #61); live `07` went 35 → 12 rows. (2) `#44`: added a privacy-safe `info` log of
  `frame_id` + relative capture path before each VLM request in `crates/kernel/src/worker_pool.rs`.
  (3) `#43`: added a dev-only `?__devState` route-state override at the `ui/src/lib/ipc/queries.ts`
  `useQuery` seam (+ `ui/src/lib/dev/*`, `DevStateBadge.tsx`, `docs/DEV_STATE_OVERRIDE.md`).
- **Why:** CLAUDE.md "Archive on release" had been applied to v0.1.0 only, so the live logs had
  re-bloated across the whole 0.2.x arc. `#44`/`#43` were the two reachable open gaps (`07`): the VLM
  log was missing because the only prior candidate was below the default `info` filter, and audit
  loading/error states couldn't be forced without mocking production data — the dev-gated flag
  override solves it without shipping any override or fake payload to production.
- **Verification:** `cargo fmt`/`clippy -D warnings`/`build`/`test --workspace` all `EXIT 0` (kernel
  enrichment 10 passed incl. `process_job_vision_tag_writes_analysis`); `npm run lint`+`build` clean
  (`✓ built in 1.81s`); `grep -rl __devState ui/dist/assets/` **absent** (prod tree-shake);
  `git diff --exit-code -- ui/src/bindings` clean; archived blocks diff **byte-identical** against
  `git HEAD`; all `#N` cross-references resolve live-or-archived.

## 2026-06-30 — PR #60 review fix: dev override truly hook-free in prod (`#43`)
- **Change:** `ui/src/lib/dev/useDevStateOverride.ts` now reads `window.location.search` inside the
  `import.meta.env.DEV` guard instead of calling `useSearchParams()` above it. Doc + CHANGELOG updated.
- **Why:** Codex/Gemini/Claude reviewers all caught that the pre-guard hook call survives tree-shaking
  (the helper is on the production `queries.ts` path), so release builds subscribed all 17 query
  consumers to router history and required a `<Router>` context. With no hook call, the production
  helper folds to `return result`. No Rules-of-Hooks concern (nothing conditional is a hook).
- **Verification:** `npm run lint` `EXIT 0`; `npm run build` `✓ built in 1.55s`;
  `grep -rl __devState dist/assets/` **absent**; `grep -rl "dev: forced route error" dist/assets/`
  **absent**; `git diff --stat -- ui/src/bindings` clean.

## 2026-06-30 — Fix vision context overflow on full-res frames (`fix/vision-fullres-ctx-overflow`)
- **Change:** (1) `crates/inference/src/vision.rs` — `encode_data_url` downscales the VLM request
  image to a 1568 px longest edge (`VISION_MAX_EDGE`) before JPEG-encoding; captures/timeline keep
  full resolution. `downscale_for_vlm` resizes the borrowed frame directly (no full-res clone — PR
  #61 Gemini review). (2) `crates/inference/src/models.rs` — vision auto-ctx **left at the spec
  default 4096**; an interim 4096 → 8192 bump was reverted after the PR #61 Codex P2 (it contradicted
  `03 §8`'s "not bumped by default" and added KV-cache VRAM on weak GPUs — the downscale already
  bounds the worst case to ~2.5 K < 4096 tokens). (3)
  `crates/kernel/src/worker_pool.rs` — `vision_tag` failure formats the error with `{e:#}` (full
  anyhow chain) into `jobs.last_error`. (4) `crates/inference/src/process.rs` (+ `supervisor.rs`
  `SupervisorConfig.sidecar_log`, `src-tauri/src/lib.rs`) — capture the sidecar's stdout/stderr to
  `<sidecar dir>/llama-server.log` via an inheritable log handle (only that handle is inheritable).
- **Why:** native full-res captures (`07` #73) made a 3440×1440 frame ~4148 vision tokens > the
  4096 ctx, so `llama-server` returned HTTP 400 `exceed_context_size_error` for every tag (DB: 72
  dead / 0 done). The low RAM/VRAM that looked like a "miracle" was the model rejecting requests in
  ~0.1 s without inference. The cause was invisible because the worker logged only the top context
  and the sidecar's stderr was discarded — both fixed here (`07` #74).
- **Verification:** TDD red→green per change (models ctx; vision downscale large/small; worker
  `{e:#}` end-to-end via a failing provider → `jobs.last_error` contains `exceed_context_size_error`;
  process.rs real-child stdout capture). Full gate all `EXIT 0`: `cargo fmt --check`,
  `clippy --workspace --all-targets -D warnings`, `build --workspace`, `test --workspace`
  (inference **98 passed**; kernel enrichment incl. the new chain test), `npm run lint` + `build`
  (`✓ built in 1.64s`), `git diff --exit-code -- ui/src/bindings` clean. **Live E2E:** rebuilt dev
  app → sidecar launches with `--ctx-size 8192`, `llama-server.log` written (`n_ctx_slot = 8192`),
  `vision_tag done` 0 → 8, and a faithful downscaled request returned **HTTP 200** in 2.8 s
  (`prompt_tokens 1159`) with `{"description":"…Visual Studio Code…","activity_type":"coding",
  "confidence":0.95}` — was HTTP 400 `4148 > 4096` before the fix.

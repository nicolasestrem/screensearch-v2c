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

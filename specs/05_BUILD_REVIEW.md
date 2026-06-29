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

## Pass — 2026-06-29 — CI toolchain pin + UI view-state test harness (`claude/codebase-improvements-847slc`)

Two easy reliability/quality wins from a code-quality survey (the rest — lock-poisoning
resilience, store-layer tracing, deeper textfilter tests — were surveyed and deferred).

### Implemented
- **CI determinism (`.github/workflows/ci.yml` + new `rust-toolchain.toml`):** the Rust step
  now installs `dtolnay/rust-toolchain@1.82.0` (was `@stable`) and a root `rust-toolchain.toml`
  pins local dev to `1.82.0`, so a `stable` roll-forward can't break CI under `-D warnings` for
  reasons unrelated to a code change. Added a `cargo build --workspace --release` step so the
  shipping profile (`lto="thin"`, `codegen-units=1`, `strip=true`) is exercised in CI.
- **UI test harness (Vitest + React Testing Library + jsdom):** first automated UI tests. New
  `ui/src/test/setup.ts` (jest-dom matchers + a ResizeObserver stub jsdom lacks) and a
  `renderRoute` helper (fresh QueryClient with retries off + MemoryRouter). Specs cover the
  view-state contract: `Timeline` (loading/empty/error/populated), `Insights`
  (loading/error/empty/populated), `Recall` search-mode invite + mode tabs. Extracted the pure
  `ask` state machine to `ui/src/lib/ipc/askReducer.ts` (re-exported from `useAsk.ts` so
  consumers are unaffected) and unit-tested its invariants (done-after-error doesn't resurrect,
  citation dedupe, accumulation, reset). CI now runs `npm test` in the UI job.

### Verification (verbatim, this Linux box)
- `cd ui && npm test` → `Test Files 4 passed (4) · Tests 15 passed (15)`.
- `npm run lint` → clean; `npm run typecheck` (`tsc --noEmit`) → clean; `npm run build` → built
  in ~3s (test files excluded from `dist`).
- Sanity-break check: flipping the Insights empty assertion to a non-matching string failed that
  one test (3 others still passed) — confirms the harness actually drives state — then reverted.

### Skipped / deferred
- The **Rust** workspace is Windows-only (`windows`/WinRT crates), so it does **not** compile on
  this Linux environment; the toolchain pin + release-build step are config-only and are verified
  by CI on `windows-latest`, not locally. YAML/TOML validated as well-formed here.

### Still risky
- None. UI changes are test-only + a no-behaviour reducer extraction; CI changes are config-only.

## Pass — 2026-06-29 — Sidecar recycle valve (docs-only, `fix/vision-sidecar-rss-recycle`)

**Branch:** `fix/vision-sidecar-rss-recycle`. Docs-only entry recording the upstream multimodal
leak and the shipped mitigation (`07` #72). No code, test, or binding changes in this pass.

### Implemented
- **`specs/03_MASTER_PRODUCTION_SPEC.md §8`** — added `sidecar.recycle_enabled` and
  `sidecar.recycle_rss_mb` rows in the sidecar settings block, mirroring neighboring rows.
- **`specs/07_KNOWN_GAPS.md`** — new row #72 recording the upstream llama.cpp multimodal leak
  (~149 MB committed host RAM per vision inference, confirmed 2026-06-29) and the local recycle
  valve mitigation.
- **`specs/05`/`06`/`08`/`CHANGELOG.md`** — one concise entry each.

### Skipped / deferred
- Real fix is upstream / a newer bundled `llama-server` build; tracked as a separate item.

---

## Pass — 2026-06-29 — Fix: UIA freezes Chromium/Electron apps (mitigation)

**Branch:** `fix/uia-chromium-hang`. Bug: with UIA text on (default), Chrome/Edge/Claude Desktop
froze ("Not responding") when scrolling content past ~1.5 pages (repro: the qBittorrent web UI's
208-row grid). Root cause confirmed in code + runtime evidence: `worker.rs::read_foreground` ran a
live, uncached **raw-view** DFS of the foreground window's whole a11y subtree (thousands of
synchronous cross-process COM calls the target app's UI thread must serve), and an unbounded worker
queue let every scroll/click pile on another walk. `07` #71. This is the agreed low-risk mitigation;
the cache-request rewrite (`FindAllBuildCache`) is the planned PR2.

### Implemented
- **Trigger gate.** New pure, CI-tested `uia::classify::trigger_runs_uia` (false for
  `ScrollStop`/`Click`); the composite (`src-tauri`) routes those frames to OCR.
- **Backlog killed.** Worker-owned `Arc<AtomicBool> in_flight` (RAII-cleared) + bounded
  `sync_channel(1)` + `try_send`: ≤1 walk running, ≤1 queued; busy frames → OCR (rate-limited warn).
- **Lighter walk.** `RawViewWalker → ControlViewWalker`.
- **Observability.** Per-walk `debug!(nodes, spans, elapsed_ms)` + rate-limited budget-hit `warn!`.

### Verification (verbatim)
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished` in 41.45s, no warnings.
- `cargo fmt --all -- --check` → clean (after `cargo fmt --all`).
- `cargo test --workspace` → all green; `uia` lib `running 12 tests` → `11 passed; 0 failed; 1
  ignored` (the 2 new gate tests + the `#[ignore]`d live walk); no `FAILED`/`error[`/`panicked`.
- `git diff --exit-code -- ui/src/bindings` → clean (no `Settings` change in this PR).

### Follow-up commits (same branch)
- **Configurable policy** — the choices above are now clamped Settings
  (`capture.uia_run_on_interactive` off, `…_view_control_only` on, `…_max_nodes`,
  `…_max_textpattern_calls`) with a Settings UI panel; bindings regenerated.
- **TextPattern gating** — `classify::control_type_wants_textpattern` (pure, CI-tested); the live
  `TextPattern` read now runs only on Document/Edit/Text controls, capped per walk.

### Still risky / deferred
- **Live desktop acceptance is the real gate** — browser must stay responsive while scrolling a
  large Chromium/Electron page (`npm run tauri dev` + `cargo test -p uia -- --ignored`). Run before
  merge.
- **`FindAllBuildCache` cached-round-trip rewrite deferred** — the plan's "real lever", held back
  because it's only exercisable via the `#[ignore]`d live test; a COM/cache bug would silently
  thin-yield with no CI signal. Do it with live verification (`07` #71). The hang is already fixed
  by PR1 + these follow-ups.

---

## Pass — 2026-06-29 — Event-driven capture review follow-up

**Branch:** `codex/event-capture-review-followups`. Addresses the review findings against the
implemented event-driven capture work in `docs/0.2.0.md`: stale Architecture/Testing wording after
click + scroll-stop landed, and incomplete `CaptureTrigger` token round-trip coverage.

### Implemented
- **Docs corrected.** `docs/ARCHITECTURE.md` now describes the full landed trigger set
  (`foreground_change`, `clipboard_change`, `idle`, `typing_pause`, `click`, `scroll_stop`, fallback
  `timer`), the default-off mouse-hook path, and schema v6. `docs/TESTING.md` now includes manual
  click/scroll-stop acceptance checks instead of saying those triggers are deferred.
- **Coverage hardened.** `crates/traits/src/domain.rs` `capture_trigger_db_str_round_trips` now covers
  `CaptureTrigger::Click` and `CaptureTrigger::ScrollStop`; `CaptureTrigger::from_db_str` now has an
  exhaustive variant guard so a future enum addition forces this parser path to be revisited at
  compile time.
- **Records updated.** `CHANGELOG.md`, `06`, `07`, and `08` record this as a review follow-up with no
  runtime/schema/IPC behavior change.

### Verification
- `cargo fmt --all -- --check` — pass.
- `cargo test -p traits capture_trigger_db_str_round_trips -- --nocapture` — pass.
- `cargo test -p capture -- --nocapture` — pass (`27 passed; 0 failed; 1 ignored` for lib tests; WGC
  smoke remains ignored as a real-desktop check).
- `cargo test -p store migration_v6_widens_capture_trigger_check_without_dropping_children -- --nocapture`
  — pass.
- `git diff --exit-code -- ui\src\bindings` — clean.

### Still Risky
- Live mouse-hook feel/latency and real WGC capture remain manual Windows-desktop checks; this follow-up
  updated the manual acceptance instructions but did not run ignored live tests.

---

## Pass — 2026-06-29 — 0.2.1 PR5: Smart enrichment throttle

**Branch:** `feat/0.2.1-pr5-enrichment-throttle`. The 0.2.1 line. Realizes the roadmap's former PR5
(`docs/0.2.0.md` deferred work; `07` #49): an opt-in, default-OFF throttle that backs enrichment off
under sustained CPU/GPU pressure while capture/OCR/storage never pause.

### Implemented
- **`crates/sysmon` pressure probe (the only new `unsafe`).** `traits::PressureProbe` impl: CPU via
  `GetSystemTimes` (pure busy%-from-FILETIME-delta helper, 6 unit tests); GPU via Windows PDH
  `\GPU Engine(*)\Utilization Percentage` with `PdhAddEnglishCounterW` (locale-proof) summed across
  engines (pure aggregation helper, 4 unit tests). Infallible-by-contract: absent GPU counters latch
  `gpu_monitored=false` / `gpu_pct=None` (truthful CPU-only fallback). 11 `sysmon` tests pass; the live
  `sample_is_well_formed` exercised the real probe on this machine (CPU + a monitored GPU).
- **Pure `kernel/src/throttle.rs` machine + governor loop.** Levels 0/1/2, `exit<enter` hysteresis,
  enter/exit dwell, one level per dwell (9 unit tests incl. flap-suppression and the gpu-unmonitored
  CPU-only path). The governor publishes the level into a shared `Arc<AtomicU8>` the worker pool reads
  live — a level change reaches running workers with **no pool restart** (the headline design choice
  vs the existing concurrency-via-restart mechanism).
- **Worker-pool enforcement.** `claim_kinds` drops `embed_image`+`vision_tag` at level ≥ 1; level 2
  caps concurrent `embed_text` to `throttle.embed_text_floor` (≥1) via an in-flight atomic + RAII guard.
- **Settings / IPC / UI.** Nine clamped `throttle.*` keys; `get_throttle_status` + `throttle_changed`;
  a Settings "Performance throttle" panel (all five view states + honest "GPU not monitored") and a
  StatusRail "Throttling" chip (level ≥ 1 only).
- **Integration proof (`kernel/tests/throttle.rs`, 2 tests).** With a `FakePressureProbe` driving live
  pressure and the *real* worker pool + governor: under sustained pressure `embed_text` drains while
  `embed_image`+`vision_tag` stay pending (done=1, pending=2), then all drain on recovery; with the
  throttle disabled all three drain (gate inert when off).

### Skipped / deferred
- **On-demand vision under throttle is deferred, not bypassed** (default chosen). At level ≥ 1 an
  explicit `enqueue_vision` (priority 10) waits until pressure clears rather than jumping the pause —
  consistent with "back off vision under load." A one-line priority-exemption is the alternative if
  user-initiated tags should bypass the throttle (recorded in `07` #49).
- **Live under-load soak** (toggle on, peg CPU/GPU, watch L1/L2 engage in `npm run tauri dev`) is the
  one acceptance item CI can't cover; the integration test exercises the same paths deterministically.

### Hallucinated / corrected
- First Win32 attempt assumed `GetSystemTimes` lived in `Win32_System_SystemInformation` and PDH handles
  were `isize`; the compiler corrected both — `GetSystemTimes` is in `Win32_System_Threading`, and PDH
  uses `PDH_HQUERY`/`PDH_HCOUNTER` (`*mut c_void` newtypes, not `Send`), so handles are stored as `isize`
  and reconstructed at call sites to keep the probe `Send + Sync`.

### Authorized deviation
- **PDH GPU-Engine counters instead of NVML** (user chose "Universal native (PDH)") — covers any GPU
  vendor + locale-safe + truthful CPU-only fallback. Recorded in `06` #13 and `07` #49.

### Verification (verbatim, all green)
- `cargo fmt --all -- --check` → exit 0 · `cargo clippy --workspace --all-targets -- -D warnings` → exit 0
- `cargo test --workspace` → all suites pass (`sysmon` 11; `kernel` lib 22 incl. 9 throttle; `kernel`
  `throttle.rs` 2; `kernel` `settings.rs` 6; `traits` 52; no failures across the workspace)
- `cd ui && npm run lint` → clean · `npm run build` → built · `git diff --exit-code -- ui/src/bindings`
  → regenerated `PressureSample.ts`/`ThrottleStatus.ts`/`Settings.ts` (committed with the change)

### Still risky
- PDH GPU-Engine counter availability is environment-dependent (headless/VM/old-driver); handled by the
  truthful `gpu_monitored=false` latch, but only the live soak confirms the real GPU reads on a given box.
- `GetSystemTimes` is whole-machine (not per-process), so a heavy *other* app can engage the throttle —
  intended ("is the machine struggling?"), smoothed by the exit dwell + hysteresis, documented honestly.

### Review follow-ups (PR #46, applied 2026-06-29 — all green re-verified)
Three bot findings on the open PR, all accepted as correct improvements (Gemini had none):
- **Codex P3 — StatusRail tooltip now surfaces "GPU not monitored."** The always-visible chip tooltip
  previously showed only CPU% on a box with no GPU counters, silently omitting the GPU state that
  `UI_REFERENCE.md` requires. Now it reads `… · GPU 92%` or `… · GPU not monitored` (the Settings panel
  already handled this) — `ui/src/components/shell/StatusRail.tsx`.
- **`embed_text` floor is now hot-reloaded per tick** (Claude bot). It was baked into `Shared` as a
  plain `usize` at pool spawn, so a floor edit only applied on the next pool restart — inconsistent in
  *kind* with the other `throttle.*` knobs (and a direct DB edit wouldn't apply). Promoted to a shared
  `Arc<AtomicUsize>` (same pattern as `throttle_level`) the governor rewrites from settings each tick;
  the worker pool reads it live. Seamless, no restart. Field doc + the `Settings.ts` binding doc updated
  to match — `kernel/src/{lib,throttle,worker_pool}.rs`, `traits/src/ipc.rs`.
- **`EmbedTextGuard` uses `Relaxed`, not `SeqCst`** (Claude bot). The reader loads the in-flight count
  with `Relaxed` and the floor is a documented soft cap, so the guard's `SeqCst` add/sub was a
  superfluous full fence the reader couldn't observe; lowered to `Relaxed` for consistency.
- **Re-verification:** `cargo fmt --all -- --check` OK · `cargo clippy --workspace --all-targets -D
  warnings` clean · `cargo test --workspace` zero failures (kernel lib 22, throttle integration 2,
  settings 6, traits 52, sysmon 11, all suites green) · `ui` lint + build clean · bindings regenerated
  (`Settings.ts` floor-doc, committed).

## Pass — 2026-06-28 — 0.2.1 PR4 part 2: UIA text + click/scroll-stop triggers

**Branch:** `feat/0.2.1-pr4p2-uia-and-mouse-triggers`. The 0.2.1 line. Two workstreams, one branch,
two commits (B = `c9104a5` event triggers; A = `6b41f5d` UIA), one PR. Both deviate from the roadmap;
both deviations were surfaced to and **explicitly authorized by the user** (see `06` #12 / its 2026-06-28
note and `07` #47/#48).

### Implemented — Workstream A: UIA target-window text, default ON, OCR fallback (`07` #48)
- **New `crates/uia`** (peer of `crates/ocr`, depends on `traits` only — `03 §2`). `UiaTextProvider`
  implements `traits::OcrProvider`, so the kernel capture loop is unchanged. The composite
  `UiaWithOcrFallback` lives in the composition root (`src-tauri/src/lib.rs`, the only place allowed to
  wire impls): per frame, when enabled, try UIA and on any `Err`/timeout/thin-yield fall back to OCR.
  **OCR stays the mandatory floor** — capture still refuses to start only when *OCR* is unavailable.
- **COM / thread-affinity.** One dedicated long-lived **MTA** worker thread owns the `IUIAutomation`
  (UIA is free-threaded; MTA is Microsoft's recommendation), paired `CoInitializeEx`/`CoUninitialize`.
  `recognize()` is async and does **no** COM work on the executor — it `spawn_blocking`s and sends only
  `Send` plain data (`width`, `height`, `target_rect`, `monitor_index`) over an `mpsc` channel; **no
  HWND or COM pointer ever crosses a thread boundary**. The worker calls `GetForegroundWindow` itself
  and scopes UIA via `ElementFromHandle` (capture→recognize are sequential, so the foreground is the
  captured one), with self-window/minimized rechecks.
- **Bounded walk.** Iterative RawView DFS (explicit stack, no recursion, no `.await`), capped by node
  count (4000), depth (40), span count (10000), and a soft per-frame latency deadline checked every
  node. Text via the ladder `TextPattern.DocumentRange().GetText()` → `ValuePattern.CurrentValue` →
  `Name`. The async side also wraps the round-trip in a 2× hard timeout as a wedged-worker net.
- **Geometry (the one genuinely new concern).** UIA `CurrentBoundingRectangle` is virtual-desktop
  screen pixels, so the pure, CI-tested `normalize_screen_rect` **subtracts the captured monitor
  origin** before normalizing to `[0,1]` (unlike OCR's frame-relative normalizer). Spans are emitted
  `source = Uia`, `role = Unknown` (PR3's `textfilter` classifies downstream — `uia` never classifies),
  `mean_confidence = CONFIDENCE_UNKNOWN`, `engine = "uia"`.
- **`primary_source` fix (the only store change).** `records.rs` derived `frames.primary_source` from
  `ocr.engine` (`primary_source_for`: `"uia"`→`uia`, else `ocr`) instead of hardcoding `'ocr'`, so
  `FrameDetail.text_source` reflects reality. Two store tests added.
- **Capability probe + budgets.** `UiaTextProvider::spawn` (MTA init → `CoCreateInstance(CUIAutomation)`)
  is the probe; on failure the composite is OCR-only and logs the reason. Three clamped settings
  (never hardcoded): `capture.uia_text_enabled` (default **ON**, hot per-frame `AtomicBool` in
  `AppState`, set by `set_settings` — no `reload_capture`), `capture.uia_latency_budget_ms` (150,
  20–2000), `capture.uia_min_text_chars` (16, 0–10000, the thin-yield floor). Budget/min-chars bake
  into the provider at startup — they apply on **app restart** (a capture stop/start reuses the
  existing provider; the UI hint and the regenerated binding doc both say "restart" to match).
- **Privacy.** Worker skips `CurrentIsPassword` (never read masked fields) and `CurrentIsOffscreen`
  (preserve OCR's "only what was visible" parity); `target_rect` containment drops out-of-window spans.
  All capture gates already run before `recognize`.
- **UI.** A "Text source" panel in `Settings.tsx` (UIA toggle hot-applies; latency / min-chars apply on
  restart) with mirrored clamps; a `UIA`/`OCR` chip in `MomentDetail.tsx` from `detail.text_source`.

### Implemented — Workstream B: click + scroll-stop triggers (`07` #47 remainder)
- **Triggers (pure, no Win32).** `CaptureTrigger::Click` (`"click"`) / `ScrollStop` (`"scroll_stop"`)
  added to the enum (+ `as_db_str`/`from_db_str`); `trigger.rs` adds `InputEventKind::Click`/`Scroll`
  and emits `Click` on a button press and `ScrollStop` at a scroll burst's trailing edge (reusing the
  existing debounce + min-interval ceiling — a scroll burst collapses to one capture). Three new unit
  tests (click-after-debounce, scroll-burst→one ScrollStop, disabled-kinds-never-emit).
- **`WH_MOUSE_LL` hook (the only new `unsafe`).** `events.rs` installs a global
  `SetWindowsHookExW(WH_MOUSE_LL, mouse_proc, hinstance, 0)` on the existing message-pump thread
  **iff** `on_click || on_scroll_stop`. `mouse_proc` dispatches Click on `WM_*BUTTONDOWN` and Scroll on
  `WM_MOUSEWHEEL`/`WM_MOUSEHWHEEL`, **always `CallNextHookEx`**, **never reads `MSLLHOOKSTRUCT.pt`**,
  and does only a `try_send`. Teardown unhooks the mouse hook first (reverse order). **Roadmap
  deviation** (`06` #12).
- **Migration `schema_version` 5→6.** Widening `frames.capture_trigger`'s `CHECK` requires a parent-
  table rebuild. The migration runner now wraps the migration loop with `PRAGMA foreign_keys=OFF` +
  a post-loop `foreign_key_check` bail + `PRAGMA foreign_keys=ON` (the standard recipe is unavailable
  inside the per-migration transaction otherwise). `MIGRATION_V6` rebuilds `frames` (create-new with
  the widened CHECK incl. `click`/`scroll_stop` → `INSERT … SELECT *` → drop index → drop → rename →
  recreate index). A dedicated populated-DB migration test proves children survive, the new tokens
  insert, a bogus token is rejected, `foreign_key_check` is empty, and cascade still works.
- **Settings/UI.** `capture.event_on_click` / `capture.event_on_scroll_stop` (bool, default off)
  plumbed through `Settings`/load/save/sanitize/`capture_config`; two toggles in the event-driven
  Settings panel; `click`/`scroll stop` added to `MomentDetail`'s trigger labels.

### Hallucinated / corrected
- **Plan claimed a FK-on `DROP TABLE frames` rebuild was safe** — it is **not**: with
  `foreign_keys=ON` the drop cascade-deletes children. Caught before writing the migration; fixed by
  the FK-off wrapper + `foreign_key_check` in the runner, and proven by the populated-DB test (children
  survive). The single heaviest/riskiest piece of either workstream.
- **Wrong UIA control-type IDs** in `classify.rs` first draft (SCROLLBAR/TAB) — corrected to the
  verified windows-rs 0.62 values (SCROLLBAR=50014, TAB=50018) before tests.
- **`normalize_screen_rect` had 8 args** (clippy `too_many_arguments`, limit 7) — refactored to group
  the monitor origin and frame size into tuples (which also reads cleaner: `monitors::monitor_origin`
  already returns a tuple), bringing it to 6 args. Not suppressed with an `#[allow]`.

### Skipped / deferred
- **Smart enrichment throttle** (the other 0.2.1 deferral, `07` #49) — out of scope for this PR.
- **Live UIA app-matrix + click/scroll feel check** are `#[ignore]` local-hardware items
  (`cargo test -p uia -- --ignored`, `cargo test -p capture -- --ignored`), not CI.

### Still risky
- **Default-ON UIA** means the greenfield path runs on every frame from first launch, so OCR-fallback
  correctness (thin-yield + timeout) is load-bearing — covered by the composite's per-frame `Err`→OCR
  path; the real quality gate is the live app matrix (Electron/Chromium/custom Win32).
- **`WH_MOUSE_LL` latency** — mitigated structurally (id-only, `try_send`, immediate `CallNextHookEx`);
  the "is there perceptible lag?" confirmation is a local dev-session item.
- **UIA foreground race** — a foreground change between capture and recognize; mitigated by
  self-window/minimized recheck + `target_rect` containment + thin-yield fallback; residual accepted
  for 0.2.1 (`07` #48).

### Verification
Full suite green: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo build --workspace`, `cargo test --workspace` (uia 10 + 1 ignored; store 49 job-queue + 10 lib
incl. the v6 migration test; traits 50; textfilter 12; settings round-trip), the UI `npm run lint` +
`npm run build`, and the `git diff --exit-code -- ui/src/bindings` guard (the regenerated `Settings.ts`
with the three UIA fields is committed). Raw command output is pasted in the session response.

### Review follow-up — adversarial multi-agent review (3 fixes, 3 refuted)
Ran a whole-branch adversarial review (5 dimension reviewers — UIA COM/lifetime, mouse hook, migration,
composition/fallback, UI/privacy/spec — each finding then independently verified by a skeptic prompted
to refute it). 6 findings raised, **3 confirmed real and fixed**, 3 refuted and accepted as
non-defects:
1. **(Important, `crates/uia/src/worker.rs`)** The bounded DFS re-checked its caps only once per
   `stack.pop()`; the inner sibling-descent loop pushed *every* child of a node with no cap/deadline
   check. A node with a massive sibling fan-out blew the latency/memory bound, and a malformed provider
   returning a **cyclic** sibling chain (`GetNextSiblingElement` never `None`) could spin the MTA
   worker forever (the `recognize()` 2× timeout abandons the *future* but can't cancel the blocking
   closure), permanently wedging the UIA arm for the session (OCR fallback keeps capture alive). Fixed:
   a documented `MAX_STACK` (16 000) bound on pending elements **and** a `deadline` re-check **inside**
   the descent loop, so any tree shape — cyclic included — terminates within the latency budget.
2. **(Minor, `crates/uia/src/geometry.rs`)** `normalize_screen_rect` measured width/height from the
   element's full extent `(r-l)`, so an element straddling the monitor's left/top edge over-reported
   its on-frame width and shifted the center used by the `target_rect` containment filter —
   inconsistent with capture's own `normalize_window_rect`, which clips. Fixed by clipping the
   left/top edge to the monitor (`l.max(mon_left)` / `t.max(mon_top)`) before measuring; TDD
   regression test `left_top_straddling_box_reports_only_on_frame_extent` (RED→GREEN).
3. **(Minor, `crates/traits/src/ipc.rs` → binding + this file)** The two UIA budget doc comments said
   "Applied on next capture start," but the budget bakes into the provider at startup and a capture
   stop/start reuses it — only an app restart re-reads it (the Settings.tsx hint already said
   "restart"). Corrected the doc comments, regenerated `ui/src/bindings/Settings.ts`, and fixed the
   note above to say **app restart**.

**Refuted (accepted as non-defects):** (a) the migration runner's `foreign_key_check` bail leaves
`foreign_keys` OFF before the `ON` restore — but every error path drops the connection by RAII
(`bootstrap_and_migrate` returns the conn only on `Ok`), so no FK-disabled connection is ever observed;
(b) the v6 migration test exercises one of seven child tables — but the FK-off rebuild is a single
uniform mechanism (`frame_text` is representative; no per-table code path), so it isn't tautological
and proves the invariant; (c) the UIA→OCR fallback logs at `debug` (below the default `info` floor) —
but the per-frame source is durably persisted in `frame_text.primary_source`, so over-fallback is
queryable, and no spec mandates a live fallback-rate metric. All gates re-run green after the fixes
(fmt, clippy `-D warnings`, `cargo test --workspace`, UI lint + build, bindings guard).

### Review follow-up — PR #45 automated review (5 comments addressed)
The opened PR's bot reviewers (gemini-code-assist, chatgpt-codex-connector) raised 5 inline comments;
all were confirmed relevant against the actual code and fixed:
1. **(codex P1, `crates/uia/src/worker.rs`) Data corruption on multi-monitor when `target_rect` is
   `None`.** Capture's `normalize_window_rect` returns `Some` only for the monitor whose region holds
   the foreground window's centre; the UIA worker always reads `GetForegroundWindow`, and
   `within_target(None, …)` kept every span — so a *background* monitor's frame was being tagged with
   the foreground window's text (and UIA is default-ON). Fixed: `read_foreground` now bails when
   `req.target_rect` is `None` → OCR fallback for that frame (OCR reads that monitor's real pixels);
   `within_target` now takes a concrete `[f32;4]` (the `None` branch + its test are gone, the live
   test's frame now carries a full-screen rect).
2. **(gemini high, `crates/uia/src/worker.rs`) COM teardown + hung-provider bound.** Replaced the two
   manual `CoUninitialize` calls with an RAII `ComApartment` guard (fires on every exit path incl. a
   panic), and set `IUIAutomation2::SetConnectionTimeout`/`SetTransactionTimeout` to the clamped
   latency budget so a single cross-process call to a hung provider can't wedge the worker (the in-walk
   deadline only fires *between* calls).
3. **(gemini medium + codex P2, `crates/uia/src/lib.rs`) `spawn_blocking` parked a pool thread per
   frame.** `recognize` now sends the request over the (non-blocking) channel and awaits a `tokio`
   **oneshot** reply directly — no `spawn_blocking`. On a hard timeout the receiver drops and the
   worker's later `send` no-ops, so a wedged worker can never grow the blocking pool (codex P2). The
   per-call transaction timeout from (2) keeps the worker itself responsive.
4. **(gemini medium, `crates/store/src/schema.rs`) `SELECT *` in the v6 rebuild.** The `frames`→
   `frames_new` copy now lists all 13 columns explicitly, so the migration stays correct under any
   future column reordering.

All gates green after the fixes: `cargo fmt --all -- --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo build --workspace`, `cargo test --workspace` (uia 9 + 1 ignored,
store 10 + 49, traits 50), UI lint + build, `git diff --exit-code -- ui/src/bindings` clean (no IPC
change this round). Per the maintainer's instruction, the bot threads were addressed in code, not
replied to.

### Review follow-up — PR #45 second review round (2 new P1s addressed)
After the first round's push, codex's re-review raised two further P1s — both real, default-ON
correctness/privacy gaps that the first round's `target_rect` fix did **not** cover:
1. **Foreground race (`crates/uia/src/worker.rs`).** A focus change between the WGC frame and the
   recognize call meant the worker read whatever window was foreground *at recognition time*. The
   `within_target` containment can't catch this when the new window overlaps the old (captured) rect,
   so a different window's text could be stored against the captured screenshot. Fixed by recording
   the foreground window handle at capture time and verifying it: new `CapturedFrame.foreground_hwnd`
   (`Option<i64>`, plain integer to stay `Send`) populated by capture via a new
   `privacy::foreground_hwnd()`, threaded to the worker, and compared against `GetForegroundWindow`
   in `read_foreground` — a mismatch bails → OCR. (`None` ⇒ unrecorded; the rect-containment guard
   remains.)
2. **Offscreen TextPattern reads (`crates/uia/src/worker.rs`).** `DocumentRange().GetText(-1)` returns
   a scrollable provider's **whole** document, including text scrolled off-screen that was never in
   the captured frame — breaking OCR's "only what was visible" parity, and (being a large yield)
   wrongly suppressing the OCR fallback. Fixed by reading only `GetVisibleRanges()` (viewport text),
   concatenating each visible range's text instead of the full document range.
   (The two extra codex threads from the first batch were duplicates of the already-fixed P1/P2.)

`CapturedFrame` gained a field, so its three other construction sites (capture, plus the `ocr`/`kernel`
/`uia` test fixtures) were updated. All gates green: fmt, clippy `-D warnings`, `cargo build
--workspace`, `cargo test --workspace` (uia 9 + 1 ignored, store 10 + 49, traits 50), UI lint + build,
bindings guard clean (no IPC change). Threads addressed in code, not replied to.

---

## Pass — 2026-06-28 — 0.2.1 PR4 part 1: event-driven capture

**Branch:** `feat/0.2.1-pr4p1-event-capture`. The 0.2.1 line; 0.2.0 keeps timer/idle capture.

### Implemented
- **Opt-in event-driven capture (default OFF).** New master setting `capture.event_driven_enabled`
  selects Timer vs Event-driven capture. In event mode the capture source fires on the enabled
  user-activity triggers plus a long fallback timer (a static screen is still sampled), a debounce
  (collapse bursts), and a min-interval rate ceiling (no storms).
- **Four triggers:** **foreground/app-switch** (`SetWinEventHook` `EVENT_SYSTEM_FOREGROUND`,
  `WINEVENT_OUTOFCONTEXT`), **clipboard change** (`AddClipboardFormatListener`, change event only),
  **idle**, and **typing-pause** (both derived from `GetLastInputInfo` timing only).
- **Two new capture modules.** `crates/capture/src/trigger.rs` — a pure, `traits`-only debounce /
  rate-ceiling / idle-edge state machine (no Win32), with 11 unit tests. `crates/capture/src/events.rs`
  — the only new `unsafe`: a dedicated message-pump thread owning a message-only `HWND_MESSAGE`
  window + the foreground hook + clipboard listener, with clean `WM_QUIT` / unhook / destroy / join
  teardown on drop. The event source lives inside `WgcCapture`, which stamps each frame's trigger;
  the kernel capture loop and the `CaptureSource` trait are unchanged.
- **`traits::CaptureTrigger` enum** (`Timer|Idle|ForegroundChange|ClipboardChange|TypingPause|Manual`)
  with `as_db_str`/`from_db_str`, persisted to the new nullable `frames.capture_trigger` column via
  forward-only migration **`schema_version` 4→5**, surfaced in `FrameDetail` and the Moment view's
  "Captured via" row. Legacy frames read back as NULL (unknown).
- **Ten new clamped settings keys** (never hardcoded): `capture.event_driven_enabled` (false),
  `capture.event_on_foreground` (true), `capture.event_on_clipboard` (true), `capture.event_on_idle`
  (false), `capture.event_on_typing_pause` (false), `capture.event_debounce_ms` (500, 100–10000),
  `capture.event_min_interval_ms` (1000, 250–60000), `capture.event_typing_pause_ms` (1500,
  500–10000), `capture.event_idle_threshold_ms` (5000, 1000–60000),
  `capture.event_fallback_interval_ms` (30000, 1000–3600000).
- **Hot-apply with no `src-tauri` change.** `CaptureConfig` `PartialEq` now includes the event
  fields, so the existing `set_settings`→`reload_capture` path restarts a running capture loop when
  any event setting changes.
- **New `windows` features** in `crates/capture/Cargo.toml`: `Win32_UI_Accessibility`,
  `Win32_System_DataExchange`, `Win32_System_LibraryLoader`.

### Skipped / deferred
- **Click + scroll-stop triggers deferred to ≥0.2.2** — both would require a low-level mouse hook
  (`WH_MOUSE_LL`), which the roadmap deliberately steers away from. Recorded in `07` #47.
- No UIA text and no smart enrichment throttle (the other 0.2.1 deferrals) — out of scope for this
  pass; still tracked in `07` #48/#49.
- The attention filter is intentionally left **trigger-agnostic**: the `CaptureTrigger` is a
  provenance label, not a classifier input (`07` #57).

### Privacy posture
- Opt-in, default off. **No keystrokes and no clipboard contents are ever read or stored** — only
  change/idle-timing signals (`GetLastInputInfo` exposes a timestamp only). All existing privacy
  gates still apply (self-exclude own window, excluded-apps, pause-on-lock, diff gate).

### Still risky
- `events.rs` carries the only new `unsafe` (Win32 hook + message pump on a dedicated thread). It has
  a `#[ignore]` hardware lifecycle test that starts/drops the hook source 50× asserting no leak/hang
  (`cargo test -p capture -- --ignored`); the pure trigger logic is covered by the CI unit tests.
- Hook install failure is non-fatal: capture falls back to the event-mode fallback timer + idle
  polling (the machine still runs), logged via `tracing::warn!`.

### Verification
Full suite green: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -D warnings`,
`cargo build --workspace`, `cargo test --workspace` (11 `trigger.rs` unit tests, extended settings
round-trip + sanitize tests), the UI lint + build, and the `git diff --exit-code -- ui/src/bindings`
binding guard. Raw command output is pasted in the session response. (Docs-only pass does not re-run
the gates.)

### Review follow-up — rate ceiling delays rather than drops
An adversarial review of the trigger machine confirmed one real defect: when a discrete event's
debounce window settled but the global `min_interval_ms` rate ceiling blocked the emit, `poll`
cleared `pending` (and set `fired_idle`/`fired_typing_pause`) **before** checking the emit result, so
the trigger was silently dropped until the next event or the fallback timer. Fixed by clearing
`pending` / setting the `fired_*` edge flags **only on a successful `try_emit`**, so the rate ceiling
now **delays** a capture (retried on the next poll) instead of consuming it. Two TDD regression tests
added (`pending_event_retries_after_min_interval_block`, `idle_retries_after_min_interval_block`);
`trigger.rs` now has 11 unit tests, `cargo test -p capture` → 24 passed, fmt + clippy clean.

### Review follow-up — PR #44 automated review (3 fixes)
Three findings from the PR #44 bot reviewers (Claude / Gemini / Codex) were confirmed real and applied
(v5 is unreleased, so the migration could still be corrected without schema drift):
1. **CHECK constraint on `frames.capture_trigger`** (`store/src/schema.rs` `MIGRATION_V5`). It was the
   only closed-set TEXT column without a `CHECK`, unlike `primary_source`/`role`/`suppress_reason`.
   Added `CHECK (capture_trigger IS NULL OR capture_trigger IN (…six tokens…))` so an invalid token
   from a future bug fails loudly at write time instead of silently mapping to `None` (lost
   provenance). SQLite enforces it on new writes only, so existing `NULL` rows need no data migration.
2. **Busy-wait spin on a closed hook channel** (`capture/src/lib.rs` `next_event_trigger`). The old
   comment claimed `recv()` returns `None` *only* when there is no hook source; in fact a `tokio` mpsc
   `recv()` also returns `None` when all senders drop — i.e. the hook thread exits post-startup
   (`GetMessageW` error). That made the `select!` event arm ready every iteration, hot-looping until
   the fallback. Fixed by clearing the local `events` handle (→ `recv_event(None)` is `pending`
   forever) and `tracing::warn!`-ing once, matching the documented "degrade to fallback timer + idle".
3. **Honor disabled event sources before installing them** (`capture/src/events.rs`). The Win32 layer
   installed *both* the foreground hook and the clipboard listener unconditionally. Now `start` takes
   the per-trigger flags and installs only the enabled source(s): a disabled clipboard no longer pushes
   `WM_CLIPBOARDUPDATE` into the 64-slot queue (where churn could crowd out an enabled foreground
   event), and a clipboard-listener setup failure no longer disables the foreground hook. Teardown
   releases only what was registered. The `#[ignore]` lifecycle test passes both flags.

All gates re-run green: fmt, clippy `-D warnings`, `cargo test --workspace` (capture 24, store 49+7),
bindings guard clean, UI lint + build. The events.rs lifecycle leak test (`-- --ignored`, 50× start/
drop with both hooks) passed on real hardware.

### Review follow-up — PR #44 second pass (degraded state now persisted)
The second-pass review confirmed all three fixes above landed and flagged one follow-on: the busy-wait
fix cleared only the **local** `events` handle, so `self.events` still held the dead source — the next
`next_event_trigger` call re-armed the closed channel, costing one extra immediate wake and a repeated
`warn!` every fallback interval (~30 s) forever (log spam, not a correctness bug). Fixed by tracking a
`hook_died` flag through the (now `break`-valued) select loop and retiring the source on `self`
(`self.events = None`) after the loop — dropping it joins the already-exited thread (instant) and makes
later cycles see no source, so the wake + warning fire exactly once. fmt, clippy `-D warnings`, and
`cargo test -p capture` (24 passed) green.

---

## Pass — 2026-06-27 — PR7 audit follow-ups

**Branch:** `codex/pr7-audit-followups`.

### Implemented
- Relabeled Recall Ask source-frame tiles from `Cited frames` to `Frames checked`, matching the
  existing backend semantics: those frame ids are context/provenance supplied to the answer model,
  not model-authored evidence for a positive claim.
- Updated nearby Ask comments in the UI, Tauri command, and inference provider so future readers do
  not reintroduce the PR7 confusion.
- Reconciled PR7 audit docs: the static-chrome search finding is now recorded as resolved by the
  later PR3 self-exclude/backfill fix (`07` #66) with residual rect-None / secondary-monitor risk
  left in `07` #58; the no-evidence Ask finding (`07` #41/#63) is resolved by the relabel approach;
  the PR8 stale-bitmap follow-up is renumbered to `07` #69 to remove the duplicate #66.
- Updated `docs/ARCHITECTURE.md` for the current backend search cap (`1..=2,000`, candidate pool
  capped at 2,000) and updated `docs/TESTING.md` to make PR7 audit artifacts local-only ignored
  evidence.

### Skipped / deferred
- No schema, migration, typed IPC, binding, or prompt/protocol change. True model-authored
  claim-level citations remain deferred until the app has a structured citation protocol; this pass
  only makes the current reviewed-context UI truthful.

### Verification
Automated gates passed on 2026-06-27; raw command output is pasted in the final session response:
- `cd ui && npm ci && npm run lint && npm run build`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --workspace`
- `cargo test --workspace`
- `git diff --exit-code -- ui/src/bindings`

Manual dev-exe verification passed with `npm run tauri dev`, launching
`target/debug/screensearch.exe`; logs are stored under
`.playwright-mcp/pr7-followups-2026-06-27/` and remain ignored local evidence.
- Recall Ask no-evidence query `PR7_NO_EVIDENCE_UNIQUE_TOKEN_20260627_X9Q` rendered an honest
  refusal and displayed retrieved tiles under `FRAMES CHECKED`; the old `Cited frames` label was not
  present.
- Daily report generation progress displayed the existing range-neutral copy:
  `Reports summarize active periods in bounded passes, so larger ranges can take a little longer.`
- Default Recall search for `chrome` returned result rows with the `CONTENT TEXT ONLY` control and
  static-toolbar filter copy visible, including Chrome hits plus backfilled non-Chrome rows, with no
  self-capture/static-chrome regression observed in the sampled dev app state.

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


> Pre-0.2.x (v0.1.0) history archived in specs/archive/05_BUILD_REVIEW.v0.1.0.md.

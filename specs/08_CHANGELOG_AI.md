# 08 — AI Changelog

> Append-only record of what the agent changed during the build, **with reasons**. One entry per
> meaningful change set. Empty until P0 begins. (This tracks build work; the design-phase history
> lives in git.)

## <date> — <short title>
- **Change:** what was added/modified.
- **Why:** the reason, tied to a spec section.
- **Verification:** the command run + verbatim result.

---

## 2026-06-29 — Live proof: recycle valve fires at the committed-RAM ceiling (`fix/vision-sidecar-rss-recycle`)
- **Change:** Runtime verification only (no code change) — confirms the shipped recycle valve
  actually recycles the live vision sidecar when committed RAM crosses the configured ceiling.
- **Why:** Task 7 / DoD for the leak mitigation: unit tests + review proved the logic; this proves
  it end-to-end in the running app, including the no-orphan teardown (`03 §5/§6`).
- **Verification:** Pre-seeded `sidecar.recycle_rss_mb=9000` (ceiling = 9 437 184 000 B = 9000 MiB),
  `sidecar.recycle_enabled=true`, Quality (8B) vision tier; drove a continuous vision backlog and
  sampled `Get-Process llama-server` every 3–5 s. Verbatim:
  - Climb to ceiling, then recycle #1:
    `22:17:15 PID=22256 Priv=8.75 GB → 22:17:18 Priv=9.07 GB (crossed) → 22:17:21 *** RECYCLE: PID 22256 -> 22004  Priv now 4.57 GB ***`
  - Recycle log line (our supervisor path, not a crash):
    `2026-06-29T20:17:18 INFO inference::supervisor: sidecar recycled at committed-RAM ceiling ceiling_bytes=9437184000`
  - No orphan: `llama-server` instance count = **1** immediately after the recycle.
  - Re-accumulation: new PID 22004 climbed 4.57 → 8.83 GB on continued tagging and parked above the
    ceiling when activity paused (the check is `acquire()`-gated by design — it fires on the next
    vision request, not on a timer), confirming the valve bounds host RAM repeatedly.

---

## 2026-06-29 — Sidecar recycle valve + upstream leak docs (`fix/vision-sidecar-rss-recycle`)
- **Change:** Docs-only — records the upstream llama.cpp multimodal host-memory leak and the
  shipped recycle valve mitigation. Files changed: `specs/03_MASTER_PRODUCTION_SPEC.md §8`
  (two new `sidecar.recycle_*` setting rows), `specs/07_KNOWN_GAPS.md` (new row #72),
  `specs/05`/`06`/`08`, and `CHANGELOG.md`.
- **Why:** Confirmed ~149 MB committed host RAM leaked per vision inference on the bundled
  `llama-server` build 9842 (6f4f53f2b) — VRAM flat, not reclaimed while resident, freed on
  exit. The recycle valve (`sidecar.recycle_enabled`/`sidecar.recycle_rss_mb`) was shipped to
  bound the leak; recording it in `§8` + `07` makes the mitigation discoverable and the real fix
  (upstream llama.cpp / newer build) trackable (`06` #15).
- **Verification:** Docs-only; no build step required.

---

## 2026-06-29 — Fix: UIA freezes Chromium/Electron apps — mitigation (`fix/uia-chromium-hang`)
- **Change:** Stop UI Automation text extraction from hanging Chromium-based apps (Chrome, Edge,
  Claude Desktop/Electron) when scrolling large content. Mitigation subset (a cache-request rewrite
  is the planned follow-up):
  - `crates/uia/src/classify.rs`: new pure, CI-tested `trigger_runs_uia(CaptureTrigger) -> bool` —
    `false` for `ScrollStop`/`Click`, `true` otherwise (exhaustive match). `mod classify` →
    `pub mod` so the composition root can call it.
  - `src-tauri/src/lib.rs` `UiaWithOcrFallback::recognize`: gate the UIA branch with
    `uia::classify::trigger_runs_uia(frame.trigger)` — scroll/click frames go straight to OCR.
  - `crates/uia/src/lib.rs`: bounded `sync_channel(1)` + a worker-owned `Arc<AtomicBool> in_flight`;
    `recognize` skips to OCR (rate-limited warn) when a walk is already running, and `try_send`s so
    a queued slot never blocks. Kills the unbounded-backlog amplifier.
  - `crates/uia/src/worker.rs`: `RawViewWalker` → `ControlViewWalker` (collapses Chromium's
    per-text-run node explosion); RAII `in_flight` clear on every walk exit path; per-walk
    `debug!(nodes, spans, elapsed_ms)` + rate-limited `warn!` when a walk hits its latency budget.
  - `CHANGELOG.md`, `05`, `07` updated.
- **Follow-up commits (same branch):**
  - *Configurable policy* — promoted the hardcoded choices to clamped Settings
    (`capture.uia_run_on_interactive` default off, `…_view_control_only` default on,
    `…_max_nodes` 4000 replacing the `MAX_NODES` const, `…_max_textpattern_calls` 64): `traits`
    `Settings` + `kernel` load/save/sanitize + `UiaBudget`/`UiaTriggerPolicy` + `spawn_ocr` wiring
    + Settings UI panel + regenerated `ui/src/bindings/Settings.ts`.
  - *TextPattern gating* — `classify::control_type_wants_textpattern` (pure, CI-tested); the walk
    now probes the live `TextPattern` only on Document/Edit/Text controls and only under
    `max_textpattern_calls`, skipping a cross-process round-trip on every non-text node.
- **Deferred (explicit, needs live verification):** the `FindAllBuildCache` cached-round-trip
  rewrite of the per-node reads (the "real lever" in the plan). Held back because its walk path is
  only exercised by the `#[ignore]`d live-desktop test — a subtle COM/cache bug would silently
  reduce UIA to always-thin-yield (→ OCR) with no CI signal, so it must be verified on a real
  desktop. PR1 + the two follow-ups already remove the hang; the cache rewrite is an efficiency/
  coverage gain. Tracked in `07` #71.
- **Why:** Every capture trigger ran a live, uncached, raw-view walk of the foreground window's
  entire a11y subtree — thousands of synchronous cross-process COM calls the *target* app's UI
  thread had to service, freezing Chromium/Electron on large trees. The latency budget can't
  interrupt an in-flight call and the unbounded queue let every scroll/click pile another walk on, so
  the target never recovered. Scroll/click is the reproducer, so those frames now use OCR; the guard
  + bounded channel + control view cut the per-trigger and total load. (`03 §5` vision-on-demand
  stance; `07` #48 follow-up.) Decisions confirmed with the user: don't run UIA on scroll/click;
  mitigation first, then the cache-request rewrite.
- **Verification (verbatim):**
  - `cargo clippy --workspace --all-targets -- -D warnings` → `Finished \`dev\` profile [...] in 41.45s` (no warnings).
  - `cargo fmt --all -- --check` → clean after `cargo fmt --all`.
  - `cargo test --workspace` → all green; `uia` lib: `running 12 tests` → `test result: ok. 11 passed; 0 failed; 1 ignored` (incl. the 2 new `trigger_runs_uia` tests + the `#[ignore]`d live-desktop walk); no `FAILED`/`error[`/`panicked` anywhere.
  - `git diff --exit-code -- ui/src/bindings` → `BINDINGS CLEAN` (no `Settings` change in this PR).
  - Live acceptance (browser stays responsive while scrolling a large Chromium/Electron page) is the desktop check via `npm run tauri dev`; run before merge.

---

## 2026-06-29 — PR #48 review follow-up + 0.2.1 version bump (`fix/uia-chromium-hang`)
- **Change:** Addressed the actionable PR #48 review findings and bumped the project version.
  - *Redundant lock removed* (Gemini): `crates/uia/src/lib.rs` — `UiaTextProvider.tx` changed from
    `Mutex<mpsc::SyncSender<…>>` to a bare `mpsc::SyncSender<…>`. `SyncSender<T>` is itself `Sync`
    when `T: Send`, and `try_send` takes `&self`, so the provider is still `Sync` (the `OcrProvider`
    trait requirement) with no lock — `recognize` now calls `self.tx.try_send(…)` directly instead
    of `self.tx.lock().expect(…).clone()`. Removes a per-frame lock/clone; behaviour unchanged.
  - *Spec §8 made truthful* (Codex P2): `specs/03_MASTER_PRODUCTION_SPEC.md §8` now documents the
    full 0.2.1 capture key set with defaults + clamp ranges — the event-driven-capture keys (gap
    from #44/#45) and the UIA text-source keys, including the four new hang-fix knobs
    (`uia_run_on_interactive`, `uia_view_control_only`, `uia_max_nodes`, `uia_max_textpattern_calls`).
    Satisfies the repo rule that every setting's default lives in §8.
  - *Version bump to 0.2.1* (user request; tag deferred): `Cargo.toml` `[workspace.package]`
    (inherited by all 9 crates + `src-tauri` via `version.workspace = true`), `src-tauri/tauri.conf.json`,
    `package.json` + `ui/package.json`, and both `package-lock.json` files. `Cargo.lock` regenerated.
  - *Timer-during-scroll gate* (Codex P2, user chose "implement"): closed the residual gap that the
    scroll/click trigger gate leaves in the **default** timer-only capture path (event-driven capture
    off → every frame is `Timer`, so the gate is inert and a tick can land mid-scroll). TDD:
    - `crates/uia/src/classify.rs`: new pure, CI-tested `input_gate_skips_uia(trigger,
      ms_since_last_input, suppress_window_ms, policy) -> bool` — true only for `Timer` within the
      window; `ForegroundChange`/`ClipboardChange`/`Manual` (act *because* of input) and
      `Idle`/`TypingPause` (input already quiesced) are exempt; bypassed by `run_on_interactive` and
      by a `0` window. 3 new tests (RED→GREEN verified).
    - `crates/uia/src/input.rs` (new): `ms_since_last_input() -> Option<u32>` via Win32
      `GetLastInputInfo` + `GetTickCount` (`wrapping_sub` for the tick rollover); one cheap syscall
      pair, no input hooks. Added `Win32_UI_Input_KeyboardAndMouse` + `Win32_System_SystemInformation`
      windows features. `None` → gate stays open (unknown activity never silently disables UIA).
    - New clamped setting `capture.uia_suppress_during_input_ms` (default 500, clamp 0..=10000):
      `traits` `Settings` + default, `kernel` load/save/sanitize (+ round-trip & clamp test),
      Settings UI field + sanitizer, regenerated `ui/src/bindings/Settings.ts`.
    - `src-tauri/src/lib.rs` `UiaWithOcrFallback`: new `suppress_during_input_ms` field + private
      `input_gate_skips` helper; `recognize` runs UIA only when `trigger_runs_uia(..) &&
      !input_gate_skips(..)`. Startup info log gained `suppress_during_input_ms`.
  - *Call-site cross-reference* (claude bot review, classify.rs:83): `trigger_runs_uia`'s `Timer`
    arm now documents the residual and points to `input_gate_skips_uia` + `07` #71, so a future
    reader at the `Timer => true` arm sees that the Timer-during-scroll residual is handled (doc
    comment only — no behaviour change).
- **Why:** Review hygiene (`04 §7`) — the lock was dead weight and `§8` is the settings
  source-of-truth that QA/future work reads; the input-activity gate closes the one real residual the
  trigger gate left (the user picked "implement" over document/defer). Version bump per the user's
  explicit request to move the code onto the 0.2.1 line (the GitHub release tag follows separately).
- **Verification (verbatim):**
  - `cargo fmt --all -- --check` → `=== FMT CLEAN ===` (no diff).
  - `cargo clippy --workspace --all-targets -- -D warnings` → `Finished \`dev\` profile [...]` (no warnings).
  - `cargo test --workspace` → all green; `uia` lib `running 18 tests` → `test result: ok. 16 passed; 0 failed; 2 ignored` (incl. the 3 new `input_gate_skips_uia` tests + the `#[ignore]`d input probe); no `FAILED`/`error[`/`panicked` across the workspace.
  - `cargo build --workspace` → `Finished` with `Cargo.lock` showing `screensearch`/`uia` at `version = "0.2.1"`.
  - `npm run lint` (ui) → `screensearch-ui@0.2.1 lint` clean; `npm run build` → `✓ built in 1.80s`.
  - `git diff --exit-code -- ui/src/bindings` → clean after committing the regenerated `Settings.ts` (new `capture_uia_suppress_during_input_ms` field).

---

## 2026-06-29 — Event-driven capture review follow-up (`codex/event-capture-review-followups`)
- **Change:** Addressed the review findings from the implemented event-driven capture work:
  - `docs/ARCHITECTURE.md` now documents the full landed trigger set, schema v6, and the default-off
    `WH_MOUSE_LL` click/scroll-stop path.
  - `docs/TESTING.md` now includes manual click + scroll-stop acceptance instead of saying those
    triggers are deferred.
  - `crates/traits/src/domain.rs` `capture_trigger_db_str_round_trips` now covers `Click` and
    `ScrollStop`; `from_db_str` now has an exhaustive variant guard so future enum additions force
    this parser path to be revisited at compile time.
  - `CHANGELOG.md`, `05`, `06`, and `07` record the review follow-up.
- **Why:** Keeps the 0.2.1 event-driven capture docs/tests aligned with the shipped implementation
  (`docs/0.2.0.md` deferred-work item; `07` #47) and prevents the `click` / `scroll_stop` DB tokens
  from drifting out of coverage again.
- **PR review hardening:** Addressed the PR #47 bot finding that the original `covered.len() == 8`
  assertion was a tautology. The durable guard is now in `from_db_str` instead of a fixed-array length
  assertion in the test.
- **Verification:** Targeted gates passed: `cargo fmt --all -- --check`;
  `cargo test -p traits capture_trigger_db_str_round_trips -- --nocapture`;
  `cargo test -p capture -- --nocapture`; `cargo test -p store
  migration_v6_widens_capture_trigger_check_without_dropping_children -- --nocapture`;
  `git diff --exit-code -- ui\src\bindings`. Raw output is preserved in the session response.

---

## 2026-06-29 — 0.2.1 PR5: Smart enrichment throttle (`feat/0.2.1-pr5-enrichment-throttle`)
- **Change:** Opt-in, default-OFF CPU/GPU pressure throttle for enrichment (`docs/0.2.0.md` former PR5, `07` #49).
  - **New `crates/sysmon`** (peer of `capture`/`uia`, depends on `traits` only — `03 §2`): implements
    `traits::PressureProbe { sample() -> PressureSample; gpu_monitored() }`. CPU via `GetSystemTimes`
    (busy% = 1 − Δidle/Δ(kernel+user)); GPU via Windows **PDH** `\GPU Engine(*)\Utilization Percentage`
    using `PdhAddEnglishCounterW` (locale-proof), summed across engines. The **only new `unsafe`**;
    pure delta/aggregation helpers are unit-tested without syscalls. Absent GPU counters → latch
    `gpu_monitored=false`, report `gpu_pct=None` (truthful "GPU not monitored", CPU-only throttle).
  - **Pure `kernel/src/throttle.rs` `ThrottleMachine`** (clock injected, no Win32 — mirrors `capture::trigger`):
    levels 0/1/2 with `exit<enter` hysteresis + enter/exit dwell timers, one level per dwell (9 unit tests).
    A kernel `governor_loop` samples the probe each `throttle.sample_interval_ms`, publishes the level into
    a shared `Arc<AtomicU8>` the worker pool reads live (no pool restart on a change), caches `ThrottleStatus`,
    broadcasts `throttle_changed`. Started/stopped to match `throttle.enabled` via `reconfigure_throttle`
    (peer of `reconfigure_enrichment`) and on `ExitRequested`.
  - **Worker pool (`worker_pool::claim_kinds`)**: drops `embed_image`+`vision_tag` at level ≥ 1; at level 2
    an in-flight `AtomicUsize` gate caps concurrent `embed_text` to `throttle.embed_text_floor` (≥1).
    Capture/OCR/storage are structurally outside the pool — never throttle.
  - **IPC + UI**: `get_throttle_status -> ThrottleStatus` command, `throttle_changed` event, new
    `PressureSample`/`ThrottleStatus` ts-rs bindings; a Settings "Performance throttle" panel (master
    toggle + live readout with the honest "GPU not monitored" state + 8 clamped threshold fields, all
    five view states) and a StatusRail "Throttling" chip shown only at level ≥ 1.
  - **Settings**: nine `throttle.*` keys with defaults + clamps (`exit<enter` enforced in `sanitize_settings`),
    wired through `kernel/src/settings.rs` load/save/sanitize, mirrored in the UI sanitizer.
- **Why:** `docs/0.2.0.md` deferred work + `07` #49 — let enrichment yield to the user under load without
  ever pausing capture/OCR/storage; thresholds are settings, never hardcoded (`03 §8`, the PR3 stance).
- **Deviation (user-authorized):** GPU monitored via **PDH GPU-Engine counters (any vendor)** rather than
  the roadmap's NVML-only framing — broader coverage + locale-safe + truthful CPU-only fallback (`06` #13).
- **Verification (all green, verbatim):** `cargo fmt --all -- --check` (exit 0) · `cargo clippy --workspace
  --all-targets -- -D warnings` (exit 0) · `cargo test --workspace` (all suites pass: `sysmon` 11, `kernel`
  lib 22 incl. 9 throttle, `kernel` `throttle.rs` integration 2, `kernel` `settings.rs` 6, `traits` 52) ·
  `cd ui && npm run lint` (clean) · `npm run build` (built) · `git diff --exit-code -- ui/src/bindings`
  (regenerated `PressureSample.ts`/`ThrottleStatus.ts`/`Settings.ts` committed). The live `sysmon` probe
  ran on real hardware (`sample_is_well_formed` sampled CPU + a monitored GPU).

## 2026-06-28 — 0.2.1 PR4 part 2: UIA text + click/scroll-stop triggers (`feat/0.2.1-pr4p2-uia-and-mouse-triggers`)
- **Change:** Two workstreams on one branch (two commits, one PR).
  - **A — UIA target-window text, default ON, OCR fallback (`07` #48).** New `crates/uia`
    (`traits`-only, peer of `crates/ocr`): `UiaTextProvider` implements `traits::OcrProvider`, so the
    kernel capture loop is unchanged. The composite `UiaWithOcrFallback` lives in `src-tauri/src/lib.rs`
    (the only place that wires impls) and per frame tries UIA, falling back to OCR on any
    `Err`/timeout/thin-yield; OCR stays the mandatory floor. A dedicated **MTA** worker thread owns one
    `IUIAutomation`; `recognize()` `spawn_blocking`s and sends only `Send` data over a channel (no
    HWND/COM pointer crosses threads). Bounded iterative RawView DFS (node/depth/span caps + latency
    deadline); text via `TextPattern → ValuePattern → Name`. Pure CI-tested helpers: `normalize_screen_rect`
    (subtracts the captured monitor origin — UIA rects are virtual-desktop pixels), `should_emit`,
    `split_words`. Spans `source=Uia`, `role=Unknown` (PR3 classifies). `crates/store/records.rs`
    derives `frames.primary_source` from `ocr.engine` (`uia`/`ocr`) instead of hardcoding `'ocr'`.
    Settings (clamped, plumbed through `ipc`/`kernel`/UI): `capture.uia_text_enabled` (default ON, hot
    per-frame `AtomicBool` in `AppState`, set by `set_settings`), `capture.uia_latency_budget_ms` (150,
    20–2000), `capture.uia_min_text_chars` (16, 0–10000). UI: a "Text source" Settings panel + a
    `UIA`/`OCR` chip in `MomentDetail`. Privacy: never reads password fields (`CurrentIsPassword`) or
    offscreen/occluded elements (`CurrentIsOffscreen`); `target_rect` containment preserves OCR parity.
  - **B — click + scroll-stop triggers (`07` #47 remainder).** `CaptureTrigger::Click`/`ScrollStop`
    (+ db tokens); `trigger.rs` adds `InputEventKind::Click`/`Scroll` (scroll burst → one trailing-edge
    `ScrollStop`), 3 new unit tests. `events.rs` installs a global `SetWindowsHookExW(WH_MOUSE_LL, …)`
    on the existing message-pump thread **only when** click/scroll-stop is enabled; `mouse_proc` reads
    **only the message id** (never `MSLLHOOKSTRUCT.pt`), `try_send`s, always `CallNextHookEx`. Migration
    `schema_version` 5→6 widens the `frames.capture_trigger` `CHECK` via an FK-safe parent-table rebuild
    (the migration runner now toggles `foreign_keys` OFF around the loop + a post-loop `foreign_key_check`
    bail), proven by a populated-DB migration test. Settings `capture.event_on_click` /
    `capture.event_on_scroll_stop` (default off) + two Settings toggles + `MomentDetail` labels.
- **Why:** Implements the two remaining `docs/0.2.0.md` "Deferred work (0.2.1)" items from the former
  PR4 — UIA text (`07` #48) and the click/scroll-stop trigger remainder (`07` #47). UIA yields more
  structured `content_text` than full-screen OCR while OCR remains a guaranteed fallback; click/scroll
  complete the event-driven trigger set. **Two roadmap deviations, both explicitly authorized by the
  user and recorded:** UIA ships **default ON** (not opt-in) — `07` #48; and click/scroll-stop use
  `WH_MOUSE_LL`, which the roadmap steers away from — `06` #12. Both risks are mitigated structurally
  (UIA: capability probe + per-frame latency budget + thin-yield/timeout → OCR fallback; the mouse
  hook: default-off, installed only when enabled, id-only `try_send` callback). Windows-native APIs
  only; no cross-platform stubs; module crates depend on `traits` only (`03 §2`); no hardcoded
  thresholds.
- **Verification:** Full suite green — `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo build --workspace`, `cargo test --workspace` (uia 9 + 1
  ignored; store 49 + 10 incl. the v6 FK-integrity migration test; traits 50; settings round-trip),
  `cd ui && npm run lint && npm run build`, and `git diff --exit-code -- ui/src/bindings` (regenerated
  `Settings.ts` committed). Live `#[ignore]` items (`cargo test -p uia -- --ignored`,
  `cargo test -p capture -- --ignored`) are local-hardware acceptance gates. Raw output pasted in the
  session response.

## 2026-06-28 — 0.2.1 PR4 part 1: event-driven capture (`feat/0.2.1-pr4p1-event-capture`)
- **Change:** Added opt-in, default-OFF event-driven capture on the 0.2.1 line (0.2.0 keeps
  timer/idle). A new master setting `capture.event_driven_enabled` selects Timer vs Event-driven
  capture; event mode fires on four triggers — **foreground/app-switch** (`SetWinEventHook`
  `EVENT_SYSTEM_FOREGROUND`, `WINEVENT_OUTOFCONTEXT`), **clipboard change** (`AddClipboardFormatListener`,
  change event only), **idle**, and **typing-pause** (both from `GetLastInputInfo` timing) — over a
  long fallback interval (a static screen is still sampled), a debounce (collapse bursts), and a
  min-interval rate ceiling (no storms).
  - **`crates/capture/src/trigger.rs` (new):** a pure, `traits`-only debounce / rate-ceiling /
    idle-edge state machine (no Win32), 9 unit tests.
  - **`crates/capture/src/events.rs` (new):** the only new `unsafe` — a dedicated message-pump thread
    owning a message-only `HWND_MESSAGE` window + the foreground hook + clipboard listener, with clean
    `WM_QUIT` / unhook / destroy / join teardown on drop; plus a `#[ignore]` 50× start/drop lifecycle
    test.
  - **`crates/capture/src/lib.rs`:** the event source lives inside `WgcCapture`, which stamps each
    frame's `CaptureTrigger`; hook-install failure is non-fatal (falls back to the event-mode timer +
    idle polling, logged via `tracing::warn!`). New `windows` features
    (`Win32_UI_Accessibility`, `Win32_System_DataExchange`, `Win32_System_LibraryLoader`).
  - **`crates/traits`:** new `CaptureTrigger` enum
    (`Timer|Idle|ForegroundChange|ClipboardChange|TypingPause|Manual`, ts-rs exported, with
    `as_db_str`/`from_db_str`); `CapturedFrame.trigger`, `NewFrame.capture_trigger`, `FrameDetail.capture_trigger`,
    and the ten `event_*` fields on `CaptureConfig` + `Settings`. `CaptureConfig` `PartialEq` now
    includes the event fields.
  - **`crates/store`:** migration **`schema_version` 4→5** adds the nullable `frames.capture_trigger`
    TEXT column; insert/read write/parse the token (NULL or unknown → `None`).
  - **`crates/kernel`:** `settings.rs` load/save/sanitize the ten clamped `capture.event_*` keys;
    `capture_loop.rs` copies `frame.trigger` onto the stored `NewFrame`.
  - **UI:** the Moment view shows a "Captured via" row; Settings exposes the event-capture toggles +
    thresholds; ts-rs bindings regenerated.
- **Why:** Implements the `docs/0.2.0.md` "Deferred work (0.2.1)" event-driven-capture item and
  resolves `07` #47 — capture should fire on real user activity, not only a fixed timer, while staying
  opt-in and privacy-safe. `SetWinEventHook` + `AddClipboardFormatListener` were chosen over low-level
  keyboard hooks per the spec; **no keystrokes and no clipboard contents are ever read or stored** —
  only change/idle-timing signals (a cross-PR rule). Click + scroll-stop are deferred ≥0.2.2 because
  they need `WH_MOUSE_LL` (`07` #47). The `CaptureTrigger` field now exists but the attention filter
  stays trigger-agnostic by design (`07` #57). The kernel loop + `CaptureSource` trait and `src-tauri`
  are unchanged; hot-apply rides the existing `set_settings`→`reload_capture` path. Windows-only
  honored; no cross-platform stubs.
- **Verification:** Full suite green — `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo build --workspace`, `cargo test --workspace` (9 new
  `trigger.rs` unit tests; extended settings round-trip + sanitize tests), `cd ui && npm ci && npm run
  lint && npm run build`, and `git diff --exit-code -- ui/src/bindings`. The `#[ignore]` events
  lifecycle test runs via `cargo test -p capture -- --ignored` (start/drop the hook source 50×, no
  leak/hang). Raw command output is pasted in the session response.

## 2026-06-27 — Docs token-bloat optimization (archive v0.1.0 history, de-dup, drift fix) (`docs/optimize-doc-token-bloat`)
- **Change:** Split shipped v0.1.0 history out of the append-only logs into `specs/archive/`
  (`05_BUILD_REVIEW`, `06_PATCH_PLAN`, `07_KNOWN_GAPS`, `08_CHANGELOG_AI`) and the human `[0.1.0]`
  releases into `CHANGELOG-ARCHIVE.md`; live logs now hold only the 0.2.x arc. De-duplicated
  `docs/ARCHITECTURE.md` and `docs/0.2.0.md` against `03`/`02`/the audits; relocated `AUDIT_0.2.0_PR*`
  evidence to `docs/audits/` (added to `.gitignore`); fixed README's phantom `AUDIT_V2C` link and the
  stale `CLAUDE.md`/`AGENTS.md` current-state lines; added an archive-on-release convention to
  `CLAUDE.md`/`AGENTS.md`/`04 §7`. ~355 KB trimmed from live docs.
- **Why:** Every LLM session was forced through thousands of lines of shipped history to find current
  status — pure token waste. Archives preserve full history (the contracts 00–04 were already clean).
- **Verification:** Each archive split diffs **empty** against `git HEAD` (05 @ B=95, 08 @ B=451,
  CHANGELOG @ B=403); table rows reconcile (06: 7 live + 4 archived = 11; 07: 31 + 38 = 69, ids
  preserved); `diff <(tail -n +4 CLAUDE.md) <(tail -n +4 AGENTS.md)` empty; no dead markdown links;
  `cargo check --workspace` finished green (`cargo build` blocked only by the running app's exe lock).

## 2026-06-27 — PR7 audit follow-ups: Ask label + docs reconciliation (`codex/pr7-audit-followups`)
- **Change:** Relabeled Ask source-frame tiles from `Cited frames` to `Frames checked`, and updated
  nearby comments to describe the current stream as reviewed context/provenance rather than
  claim-level evidence. Reconciled PR7 audit docs: `07` #41/#63 now resolve via the relabel approach,
  `07` #62 resolves through the later PR3 self-exclude/backfill fix (#66), `07` #65 is narrowed as a
  docs cleanup, and the duplicate PR8 gap id was renumbered to #69. Updated `docs/ARCHITECTURE.md`
  search-limit/status wording, `docs/TESTING.md` PR7 manual expectations, `06`, `05`, and the human
  changelog.
- **Why:** The PR7 audit found that no-evidence Ask answers refused honestly but still rendered
  unrelated retrieved context under `CITED FRAMES`. The user chose the low-risk relabel fix, with no
  schema, migration, typed IPC, or model-output heuristic change. The stale PR7 static-chrome rows
  also needed to reflect the later PR3 audit fix instead of leaving a resolved release blocker open.
- **Verification:** Passed on 2026-06-27. Automated gates run: `cd ui && npm ci && npm run lint &&
  npm run build`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --
  -D warnings`; `cargo build --workspace`; `cargo test --workspace`; and
  `git diff --exit-code -- ui/src/bindings`. Manual dev-exe pass launched `npm run tauri dev`,
  verified the no-evidence Ask token rendered `FRAMES CHECKED` with no `Cited frames` label,
  confirmed Daily report progress kept the range-neutral bounded-pass copy, and spot-checked default
  `chrome` search results with the content/static filter UI visible. Raw command output is preserved
  in the final session response; dev logs are under `.playwright-mcp/pr7-followups-2026-06-27/`.

---

## 2026-06-26 — 0.2.0 PR8 parallel download audit (`codex/0.2.0-pr8-audit`)
- **Change:** Recorded the PR8 audit in tracked release/build-loop docs while keeping the detailed
  audit artifact `docs/AUDIT_0.2.0_PR8_2026-06-26.md` local-only per `.gitignore`. Updated
  `docs/0.2.0.md` to mark PR8 accepted while preserving the separate PR3 release blocker, updated
  `CHANGELOG.md`, and recorded this build-loop entry plus the new narrow PR8 hardening gap in `07`.
  Evidence is preserved under `.playwright-mcp/pr8-2026-06-26/`.
- **Why:** The 0.2.0 release plan required a detailed audit of PR8 after PR1/PR2/PR3/PR6/PR7/PR8
  had merged to `main`, including the follow-up hardening commit. The audit needed static review of
  `crates/inference/src/download.rs`, real dev-exe exercise through `npm run tauri dev`, default
  answer download, Vision Quality 8B GGUF + mmproj download through Moment -> Tag with vision, and
  interrupted resume behavior. The user's note that Vision Beta does not work for tagging was honored
  by using Beta only as a download/resume target.
- **Verification:** Raw command output is preserved verbatim under `.playwright-mcp/pr8-2026-06-26/`.
  The planned gates all passed: `cd ui && npm ci && npm run lint && npm run build`;
  `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo build --workspace`; `cargo test -p inference --lib` (88 passed); `cargo test --workspace`;
  and `git diff --exit-code -- ui/src/bindings`. The live dev-exe pass downloaded the default answer
  model, Vision Quality GGUF + mmproj, and Vision Beta GGUF + mmproj; the Beta interruption captured
  `Chunks: 170`, `Done: 73`, `Pending: 97`, and the restart resumed from those completed chunks
  (about 43% of the GGUF), later showing `86%` at the next sampled UI state before finalizing.

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

## 2026-06-27 — PR #41 review hardening (Codex + Gemini) (`fix/0.2.0-pr3-chrome-backfill`)
- **Change:** Acted on the PR #41 review of the gap-#64/#66 fix (gap #67). Three code fixes:
  1. **Invalidate stale embeddings during backfill (Codex P1).** When `backfill_filter_version`
     rewrites a frame's `content_text` it now `DELETE`s that frame's stale `source='ocr'` row from
     `embeddings` inside the same transaction (the `embeddings_ad` trigger cascades to the vec0
     shadow). Added the `backfill_filter_version_invalidates_stale_text_embedding` store test.
  2. **Catalog cache in backfill (Gemini).** The read-only chrome catalog is loaded once per
     `app_hint` into a `HashMap` cache instead of one `chrome_text_catalog` query per frame (N+1).
  3. **No file orphaning on purge (Gemini).** `purge_self_captures` `continue`s past a transient
     JPEG-delete failure instead of deleting the row (mirrors the retention sweeper); the
     no-progress guard still bounds the loop.
- **Why:** (1) is correctness — without it, dropped chrome keeps surfacing the frame via
  `hybrid_search`'s vector arm (fused even with `include_chrome=false`) until the async re-embed
  runs, or indefinitely if text embedding is off, undermining the PR's whole point. (2) is startup
  perf on large DBs. (3) prevents orphaned JPEGs, consistent with `01 §5`/the existing retention
  pattern. Two findings were accepted as-is by user decision and documented (gap #67, method/CHANGELOG
  comments): the 12→4 `chrome_suppress_min_seen` default reaches new installs only (no settings
  migration — Codex P2), and `reload_capture`'s stop→start is left non-atomic (unreachable UI race —
  Gemini).
- **Verification:** `cargo fmt --all -- --check` (clean), `cargo clippy --workspace --all-targets --
  -D warnings` (clean), `cargo build --workspace` (ok), `cargo test --workspace` (all suites pass;
  `store` integration suite 48→49 with the new P1 test; 0 failures), `git diff --exit-code --
  ui/src/bindings` (clean — no ts-rs type changed).

## 2026-06-27 — PR #41 second review round (Codex + claude[bot]) (`fix/0.2.0-pr3-chrome-backfill`)
- **Change:** Acted on the review of commit `f8f3d83` (gap #68). Two code fixes + one doc fix:
  1. **Backfill releases the store connection between batches (Codex P2).** `backfill_filter_version`
     no longer wraps the whole loop in one `with_conn` — which holds `SqliteStore`'s single
     `Arc<Mutex<Connection>>` (every store method funnels through `with_conn`) for the entire pass. It
     now does one short `with_conn` to check the watermark, list sub-version frames, and snapshot each
     distinct `app_hint`'s catalog; then one `with_conn` per `BACKFILL_BATCH`; then one to advance the
     watermark. The connection is free between batches.
  2. **Self-purge watermark only on full drain (Codex P2).** `purge_self_captures` sets
     `maintenance.self_capture_purged` only when the listing drains to empty after successful deletes;
     a transient failure (now able to leave rows behind after #67's orphan fix) leaves it unset to
     retry next launch.
  3. **`reload_capture` doc comment corrected (claude[bot]).** Tauri 2 async commands are not
     serialized, so the prior "unreachable / serialized command path" rationale was false. The comment
     now states the race is real but accepted (sub-ms window, no UI affordance to fire a save + Stop at
     once); the code is unchanged (user's earlier "leave as-is" decision stands).
- **Why:** (1) keeps the app's DB responsive during the background startup backfill on large upgraded
  DBs; the catalog snapshot is sound because concurrent capture only warms catalogs for *new* frames
  (already at `current`), which the pass doesn't touch. (2) prevents a partial purge from being marked
  permanently complete and leaving own-window chrome searchable. (3) keeps the documentation truthful.
  The 12→4 upgrade-path concern was re-raised with a backfill-clamp variant (`min(stored,4)`); per user
  decision it was declined — the tuned default reaches new installs only (gap #68).
- **Verification:** `cargo fmt --all -- --check` (clean), `cargo clippy --workspace --all-targets --
  -D warnings` (clean), `cargo build --workspace` (ok), `cargo test --workspace` (all suites pass; the
  existing `store` backfill goldens — recleans-against-warm-catalog, the new stale-embedding test, and
  idempotency — pass unchanged over the per-batch-connection rewrite; 0 failures), `git diff
  --exit-code -- ui/src/bindings` (clean).

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
  acceptance/product-semantics gaps at the time: strict static/app-chrome suppression was not fully
  closed on the populated corpus or fresh ScreenSearch UI captures (#62, later resolved by #66 with
  residual #58), and no-evidence Ask refusals still rendered retrieved context under `CITED FRAMES`
  (#63, later relabeled as `Frames checked`).
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


> Pre-0.2.x (v0.1.0) history archived in specs/archive/08_CHANGELOG_AI.v0.1.0.md.

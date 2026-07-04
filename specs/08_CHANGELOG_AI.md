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
> Shipped 0.3.0 history (PR1–PR9 + bridge fixes) → `specs/archive/08_CHANGELOG_AI.v0.3.0.md`.
> Live file holds only the current (post-0.3.0) arc — empty until the next arc begins.

---

## 2026-07-04 — UIA client lifecycle teardown + per-app circuit breaker (hang fix)

- **Change:** Fixed the shipped 0.3.0 defect where UI Automation left Chromium/Electron apps
  hung, persisting after the user disabled capture. (a) `crates/uia/src/worker.rs`: walk reply
  now carries `over_budget`; `worker_main` times the whole walk and owns the rate-limited
  over-budget warn; per-request `cancel` + provider `shutdown` flags checked at both deadline
  checkpoints; worker-exit signal via a dropped `mpsc::Sender`. (b) `crates/uia/src/lib.rs`:
  `UiaTextProvider` gains `shutdown()` / `take_exit_signal()` / `recognize_detailed()`
  (returns a `WalkEnd` for the breaker). (c) New pure `crates/uia/src/breaker.rs`: per-app
  `AppBreaker` (3 consecutive over-budget/timed-out walks → 30-min OCR cooldown; good walk
  resets), fully CI-unit-tested. (d) `crates/traits/src/contracts.rs`: additive
  `OcrProvider::on_capture_stopped()` default-no-op. (e) `crates/kernel/src/lib.rs`:
  `stop_capture` calls it (once, only when a handle existed). (f) `src-tauri/src/lib.rs`:
  `UiaWithOcrFallback` reworked to hold the client in a teardown-able slot with lazy respawn,
  a `UiaRuntimeConfig` snapshot, and the breaker; `apply_settings` swaps config + tears down on
  change (every UIA knob now hot-applies); `spawn_ocr` returns the concrete composite; removed
  the `AppState.uia_enabled` AtomicBool. (g) `ui/src/routes/Settings.tsx`: seven UIA hints
  `APPLY_RESTART`→`APPLY_NOW`; breaker note added to the interactive-walk warning.
- **Why:** `03 §2/§3` (composition-root wiring; provider contracts) and the `07`/#48 UIA hang
  lineage. The 0.3.0 client was never released (held in `Kernel.ocr` for the process lifetime),
  so Chromium kept its accessibility engine on and the hangs outlived "disable capture". User
  decision: breaker-only (no Chromium window-class skip), constants not settings (`07` #92/#93).
- **Verification:** `cargo test -p uia --lib` → `26 passed; 0 failed; 3 ignored`;
  `cargo test -p kernel` green (new `stop_capture_notifies_ocr_provider`); `npm run lint` clean;
  `npm run build` exit 0; `cargo build -p screensearch` → `Finished ... in 20.84s`. Full
  workspace `fmt`/`clippy`/`test` + live `npm run tauri dev` walkthrough recorded on the PR.

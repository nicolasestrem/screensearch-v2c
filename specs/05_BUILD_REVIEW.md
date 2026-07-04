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
> Shipped 0.3.0 history (the whole arc: PR1–PR9 + post-0.2.2 bridge fixes) →
> `specs/archive/05_BUILD_REVIEW.v0.3.0.md`.
> Live file holds only the current (post-0.3.0) arc — empty until the next arc begins.

---

## Pass 1 — 2026-07-04 — Post-0.3.0 defect fix: UIA client lifecycle + per-app circuit breaker

- **Implemented:** Root-caused a shipped 0.3.0 defect where Chromium/Electron apps (Chrome,
  Codex, Claude Desktop) hang/crash while capture runs and **stay hung after the user disables
  capture**. Confirmed via live evidence (Windows AppHang 1002 for chrome.exe fired *inside* a
  capture-off window; `frame_text.primary_source='uia'` rows for chrome/Codex/claude; the log's
  only WARN was the UIA over-budget line) plus a full `crates/uia` + composition-root read. Root
  cause: the `IUIAutomation` client (spawned once, held in `Kernel.ocr` for the process lifetime)
  is never released — neither the `capture.uia_text_enabled` toggle (flag-only) nor `stop_capture`
  (drops only the capture source) disconnects it — so Chromium keeps its accessibility engine on.
  Fix (feature branch `fix/uia-client-lifecycle-breaker`):
  1. **Worker reply plumbing** (`crates/uia/src/worker.rs`): `WalkReply { outcome, over_budget }`;
     `worker_main` times the whole walk and hoists the rate-limited over-budget warn (now also
     covers over-budget *error* walks).
  2. **Cancellation** (`worker.rs` + `lib.rs`): per-request `cancel` + provider `shutdown` flags
     checked at both deadline checkpoints; the caller's 2×budget hard timeout sets `cancel`.
  3. **Per-app circuit breaker** (new pure `crates/uia/src/breaker.rs`, CI-tested): 3 consecutive
     over-budget/timed-out walks per app → 30-min OCR cooldown; a good walk resets. Named
     constants, not settings (user decision — surface-reduction arc).
  4. **Composite lifecycle** (`src-tauri/src/lib.rs` `UiaWithOcrFallback`): the client lives in a
     teardown-able slot; disabling UIA / changing any UIA setting / stopping capture disconnects
     it (`OcrProvider::on_capture_stopped` default-no-op trait method, called by `stop_capture`);
     the next eligible frame lazily respawns. Side benefit: **all UIA knobs now hot-apply** (the
     APPLY_RESTART trap is gone).
  - Verification (verbatim):
    - `cargo test -p uia --lib` → `test result: ok. 26 passed; 0 failed; 3 ignored`.
    - `cargo test -p kernel` (incl. new `stop_capture_notifies_ocr_provider`) → all green.
    - `cd ui && npm run lint` → clean; `npm run build` → exit 0; `cargo build -p screensearch`
      → `Finished dev profile ... in 20.84s`.
- **Skipped / deferred:** A Chromium window-class *skip* (former "Fix A") — the user chose
  breaker-only, so Chromium apps still get UIA (and the first walk still flips them into a11y
  mode) but are backed off on repeated struggle. Recorded in `07` (two new rows).
- **Hallucinated / corrected:** One breaker unit test had an off-by-one (expected a 4th bad walk
  to open the breaker; the 3rd does). Caught by `cargo test`; test fixed, impl was correct.
- **Still risky:** The live-desktop UIA lifecycle test (`uia_worker_exits_on_shutdown`) and the
  hang-doesn't-recur behavior are `#[ignore]`d in CI (need a real session) — must be run locally
  (`cargo test -p uia -- --ignored`) + a `npm run tauri dev` walkthrough before merge.
- **Review follow-up (PR #79, 2026-07-04):** addressed the one substantive review finding (a
  `claude`-bot inline comment; gemini/codex left no actionable comments). `get_or_spawn_uia`
  baked the *caller's* stale `cfg.budget` snapshot: a settings save landing while the slot was
  still empty (capture-start → first-spawn window) swaps `config` + tears down a **no-op empty
  slot**, then the racing frame spawned the client with the pre-save budget — persisting until
  the next save, contradicting "every UIA setting hot-applies." Fix: drop the `cfg` param and
  read `self.config.lock().budget` fresh under the `respawn_gate` (the spawn now owns reading
  the live config); log lines use the fresh `budget` too. Residual two-save micro-race recorded
  in `07` #95 (not human-reachable, self-heals). No signature/IPC change → bindings stay clean.

---

## Pass 2 — 2026-07-04 — Post-0.3.0: Flow overlay default hotkey `Ctrl+Alt+Z` + one-shot remap

- **Implemented:** Changed the Flow overlay default summon chord from `Ctrl+Alt+Space` (collided
  with Claude Desktop's global quick-entry shortcut) to `Ctrl+Alt+Z` in all three sources of
  truth (`crates/traits/src/ipc.rs` default, `src-tauri/src/overlay.rs` `OVERLAY_DEFAULT_CHORD`,
  `ui/src/components/domain/HotkeyField.tsx` `DEFAULT_OVERLAY_HOTKEY`), plus a one-shot load-path
  remap (`kernel::settings::load_overlay_hotkey`, mirroring the `load_tier` beta→quality
  precedent) that rewrites a persisted `Ctrl+Alt+Space` to the new default exactly once and
  leaves any custom chord untouched. RegisterHotKey failure was already surfaced in Settings
  (D6 prior art, `overlay.rs` `failed_status`+`emit_hotkey_warning`) — reused, not rebuilt.
  - Verification (verbatim): `cargo test -p kernel --test settings` →
    `test result: ok. 13 passed; 0 failed` (incl. new `overlay_hotkey_legacy_default_remaps_once`
    + `overlay_hotkey_custom_value_survives`; updated the persisted-value assertion in
    `overlay_hotkey_empty_string_resets_to_default` to the new default). `npm run lint` clean.
- **Skipped / deferred:** TODO-3 (a Settings-level cross-chord conflict *check* between the two
  hotkeys) stays open — deferred by decision; this change doesn't implement it.
- **Hallucinated / corrected:** none.
- **Still risky:** the AZERTY/AltGr caveat (AltGr reported as Ctrl+Alt) is unchanged; `Ctrl+Alt+Z`
  on AZERTY is produced by AltGr+Z where Z is a letter, but the overlay registers the chord, not
  a character, so no typing conflict — confirm on a live AZERTY session in the walkthrough.
- **Review follow-up (PR #80, 2026-07-04):** addressed the inline review comments across two
  rounds (all bot-authored; evaluated on merits, not replied to). Round 1: (1) **codex P2 — the
  remap wasn't truly one-shot.** It was value-only, so a user who deliberately set `Ctrl+Alt+Space`
  back had it re-remapped to `Ctrl+Alt+Z` on the next `load_settings`, breaking the reversible
  escape hatch the CHANGELOG / `07` #94 promised. Fixed by gating the remap behind a persisted
  marker (`overlay.hotkey_migrated`), latched on the first load regardless of stored value, so it
  fires at most once per install and the stored chord is honored verbatim afterward; new test
  `overlay_hotkey_deliberate_legacy_survives_after_migration` proves it. (2) **gemini — hint
  hardcoded the chord:** `Settings.tsx` now interpolates the imported `DEFAULT_OVERLAY_HOTKEY`.
  (3) **gemini — test hardcoded the default JSON:** the two persisted-value assertions now derive
  the expected string from `serde_json::to_string(&Settings::default().overlay_hotkey)`. Round 2
  (on the marker fix itself): (4) **codex/claude P2 — don't latch a failed remap.** The marker was
  written unconditionally, so a *failed* `overlay.hotkey` rewrite followed by a *successful* marker
  write latched the migration anyway — the stale `Ctrl+Alt+Space` would then be honored forever and
  the promised retry never happened. Fixed: the marker is now written **only after** the remap
  rewrite succeeds (a custom value / fresh install still latches immediately, since there is no
  rewrite to fail); a failed rewrite leaves the migration un-run so the next load retries; test
  `overlay_hotkey_failed_remap_is_retried_not_latched`. (5) **codex/claude — source-of-truth docs
  still said `Ctrl+Alt+Space`:** updated every *living* reference to `Ctrl+Alt+Z` (`specs/03`,
  `specs/02`, `specs/UI_REFERENCE.md`, `README.md`, `docs/TESTING.md`, `docs/ARCHITECTURE.md`);
  `docs/0.3.0.md` (the shipped-arc design record) and the build-loop logs are left citing
  `Ctrl+Alt+Space` on purpose — 0.3.0 genuinely shipped that default and this is a post-0.3.0 fix.
  CHANGELOG + `07` #94 corrected (the reversibility claim now actually holds). No IPC shape change
  → bindings clean.

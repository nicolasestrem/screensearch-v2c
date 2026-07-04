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
- **Review follow-up (PR #80, 2026-07-04):** addressed all three inline review comments (all
  bot-authored; evaluated on merits, not replied to). (1) **codex P2 — the remap wasn't truly
  one-shot.** It was value-only, so a user who deliberately set `Ctrl+Alt+Space` back had it
  re-remapped to `Ctrl+Alt+Z` on the next `load_settings`, breaking the reversible escape hatch
  the CHANGELOG / `07` #94 promised. Fixed by gating the remap behind a persisted marker
  (`overlay.hotkey_migrated`), latched on the first load regardless of stored value, so it fires
  at most once per install and the stored chord is honored verbatim afterward; new test
  `overlay_hotkey_deliberate_legacy_survives_after_migration` proves it. (2) **gemini — hint
  hardcoded the chord:** `Settings.tsx` now interpolates the imported `DEFAULT_OVERLAY_HOTKEY`.
  (3) **gemini — test hardcoded the default JSON:** the two persisted-value assertions now derive
  the expected string from `serde_json::to_string(&Settings::default().overlay_hotkey)`. CHANGELOG
  + `07` #94 corrected (the reversibility claim now actually holds). No IPC shape change → bindings
  clean.

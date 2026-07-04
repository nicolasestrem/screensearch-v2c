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

## 2026-07-04 — Flow overlay default hotkey → Ctrl+Alt+Z (+ one-shot remap)

- **Change:** Default Flow overlay chord changed `Ctrl+Alt+Space` → `Ctrl+Alt+Z` in the three
  sources of truth (`crates/traits/src/ipc.rs`, `src-tauri/src/overlay.rs`,
  `ui/src/components/domain/HotkeyField.tsx`), Settings hint text updated
  (`ui/src/routes/Settings.tsx`), and a load-path one-shot migration
  `kernel::settings::load_overlay_hotkey` that remaps a stored exact `Ctrl+Alt+Space` to the new
  default (persists it, logs once), leaving custom chords untouched.
- **Why:** The old default collided with Claude Desktop's global quick-entry shortcut (`03 §8`
  hotkey config). The remap lives in the load path, not the startup sweep, for the same reason as
  `load_tier` (the composition root registers the chord straight from `load_settings`' output).
- **Verification:** `cargo test -p kernel --test settings` → `13 passed; 0 failed` (two new remap
  tests + updated persisted-value assertion); `npm run lint` clean. Full `fmt`/`clippy`/`build`/
  `test` + a live hotkey walkthrough recorded on the PR.

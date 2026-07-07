# 08 — AI Changelog — archived v0.3.3 history (browser-freeze hotfix)

> Archived on the v0.3.3 release sweep (2026-07-07, folded into 0.4.0 PR1); entries preserved
> verbatim from the live `specs/08_CHANGELOG_AI.md`. Contents: the single 0.3.3 hotfix entry —
> UIA skips Chromium/Electron windows.
> Earlier history → the v0.1.0 / v0.2.x / v0.3.0 / v0.3.1 / v0.3.2 archives in this folder.

---

## 2026-07-07 — 0.3.3 hotfix: UIA skips Chromium/Electron windows

- **Change:** UIA no longer walks Chromium/Electron/CEF windows (`Chrome_WidgetWin_*`); they are
  routed to OCR before any COM call. Added `classify::is_chromium_window_class`, a new
  `crates/uia/src/window.rs` (`class_name_of` via `GetClassNameW`; public `hwnd_is_chromium`),
  a composite fast-path in `UiaWithOcrFallback::recognize` (`src-tauri/src/lib.rs`, with a
  first-seen-per-app info log), and a worker backstop in `read_foreground` before
  `ElementFromHandle`.
- **Why:** A UIA tree walk of a Chromium window is a synchronous cross-process COM call onto the
  target's UI thread whose first-touch a11y-tree build cannot be aborted mid-call; on 2026-07-07 it
  hard-froze Chrome (survived stopping capture and killing our process). Reverses the `07` #93
  "keep UIA on browsers" decision (`06` #25) now its revisit condition fired. Browsers stay
  captured via OCR (skip = OCR, not ignore); native apps keep UIA; breaker retained as native
  backstop.
- **Verification:** `cargo test -p uia` → 26 passed (new classifier test); full CI parity green
  (`fmt`/`clippy -D warnings`/`cargo test --workspace` **524 passed**/bindings clean); live
  detection against real Chrome/Edge/Notepad handles; live end-to-end `npm run tauri dev` showing
  `UIA disabled for Chromium/Electron app; using OCR app="chrome"` with Chrome `Responding=True`,
  and `uia walk complete nodes=46` on Notepad. Full evidence in `05` Pass 1.

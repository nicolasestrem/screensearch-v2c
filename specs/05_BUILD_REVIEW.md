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
> Shipped 0.3.1 history (the P7.1 triage patch: post-0.3.0 bridge fixes PR #79/#80 + PR1–PR4) →
> `specs/archive/05_BUILD_REVIEW.v0.3.1.md`.
> Shipped 0.3.2 history (the P7.2 product-shell mini-arc: PR1–PR6) →
> `specs/archive/05_BUILD_REVIEW.v0.3.2.md`.
> Live file holds only the current arc.

---

## Pass 1 — 2026-07-07 — 0.3.3 hotfix: UIA skips Chromium/Electron windows (`07` #93, `06` #25)

- **Implemented:** UIA now never walks a `Chrome_WidgetWin_*` (Chromium/Electron/CEF) foreground
  window; such frames are routed to OCR before any COM touch. New pure classifier
  `classify::is_chromium_window_class` (prefix `Chrome_WidgetWin_`), a Win32 helper module
  `crates/uia/src/window.rs` (`class_name_of` via `GetClassNameW`; public `hwnd_is_chromium`),
  a composition-root fast-path in `UiaWithOcrFallback::recognize` (logs `UIA disabled for
  Chromium/Electron app; using OCR` once per app), and a worker backstop in `read_foreground`
  before `ElementFromHandle`. Browsers stay captured via OCR; native apps keep UIA; the breaker
  is retained as a native-app backstop.
- **Verification (verbatim):**
  - `cargo test -p uia`: `test result: ok. 26 passed; 0 failed; 3 ignored` (incl. new
    `chromium_window_classes_are_detected_others_left_alone`).
  - `cargo fmt --all -- --check` clean; UI `npm run lint`/`build` clean;
    `cargo clippy --workspace --all-targets -- -D warnings` clean;
    `cargo test --workspace`: **524 passed, 0 failed**; `git diff --exit-code -- ui/src/bindings` clean.
  - **Live detection** against real handles (`UIA_PROBE_HWND=… cargo test -p uia -- --ignored
    live_hwnd_classification`): Chrome `class="Chrome_WidgetWin_1" is_chromium=true`; Edge
    `Chrome_WidgetWin_1 true`; taskbar `Shell_TrayWnd false`; Notepad `Notepad false`.
  - **Live end-to-end** (`npm run tauri dev`, RUST_LOG=info,uia=debug; capture on): Chrome
    (the 2026-07-07 repro window) foreground →
    `INFO … UIA disabled for Chromium/Electron app; using OCR (07 #93) app="chrome"`, capture kept
    encoding (OCR), **no** `circuit breaker opened for app="chrome"`, **no** `still finishing a
    walk`, Chrome `Responding=True` throughout and after the dev-app teardown. Notepad foreground →
    `DEBUG uia::worker: uia walk complete nodes=46 spans=50 elapsed_ms=93` (UIA still walks native).
- **Skipped / deferred:** no change to the breaker (kept as native-app backstop, `07` #92); no new
  setting (unconditional, per surface-reduction ethos); the long-term high-fidelity browser path
  (extension/CDP DOM reader) remains a future 0.4.x option, not built here.
- **Hallucinated / corrected:** none. Composite `START CAPTURE` accessible name is upper-cased by
  CSS (matched via UIA name for the live test).
- **Still risky:** UIA remains active for native apps, where a pathological tree could still be
  heavy; the unchanged breaker backstops that, exactly as before. First-touch browser wedge is now
  eliminated by construction (no Chromium tree is ever touched).

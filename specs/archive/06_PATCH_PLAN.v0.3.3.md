# 06 — Patch Plan — archived v0.3.3 rows

> Archived on the v0.3.3 release sweep (2026-07-07, folded into 0.4.0 PR1); original ids preserved.
> One row shipped in the 0.3.3 hotfix: #25 (reversed prior product decision — UIA must skip
> Chromium/Electron windows). The standing cross-arc rows #15 (upstream llama.cpp leak) and
> #23 (PDH blind to Vulkan) remain in the live `specs/06_PATCH_PLAN.md`.
> Earlier rows → the v0.1.0 / v0.2.x / v0.3.0 / v0.3.1 / v0.3.2 archives in this folder.

| # | Priority | Issue | Source (spec §, file:line) | Fix | Status |
|---|---|---|---|---|---|
| 25 | P1 | **Reversed prior product decision: UIA must skip Chromium/Electron windows (was: keep UIA on them).** `07` #93 (0.3.0/0.3.1) recorded the maintainer *declining* a Chromium window-class skip in favor of UIA fidelity, with the revisit condition "only if the first-walk cost proves too disruptive." On 2026-07-07 a live UIA walk hard-froze Chrome ("Not Responding") — a wedge that survived stopping capture and killing our own process — so the condition fired and the maintainer reversed the decision this session (chose "OCR browsers now (safe)"). | `07` #92/#93; `crates/uia/src/{classify,window,worker}.rs`; `src-tauri/src/lib.rs` (`UiaWithOcrFallback`) | **Resolved (0.3.3).** Skip `Chrome_WidgetWin_*` windows → OCR before any COM touch (fast-path in the composite + worker backstop). Browsers stay captured via OCR; native apps keep UIA; breaker retained as native-app backstop. Live-verified end-to-end (see `05` Pass + `07` #93). | ✅ resolved 0.3.3 |

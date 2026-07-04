# 06 — Patch Plan

> **Ordered fixes required before shipping**, plus any **spec contradictions** found during the
> build (`04 §5`). Empty until the build surfaces something.

| # | Priority | Issue | Source (spec §, file:line) | Fix | Status |
|---|---|---|---|---|---|
| 15 | P5 | **Upstream llama.cpp multimodal host-memory leak (build 9842).** ~149 MB committed host RAM leaked per vision inference; VRAM flat. Mitigated locally by the `sidecar.recycle_*` valve (recycle at a committed-RAM ceiling). Real fix requires a newer bundled `llama-server` build. | `07` #72; `specs/03_MASTER_PRODUCTION_SPEC.md §8`; `crates/inference` sidecar lifecycle | **Open (upstream).** Local mitigation shipped (`sidecar.recycle_enabled` default true, `sidecar.recycle_rss_mb` default 0=auto). Upgrade the bundled llama-server build when a fix lands upstream; re-run the 60-frame isolated test to confirm RSS stays flat. | open (upstream) |
| 21 | P1 | **UIA text source hangs Chromium/Electron apps, persisting after capture is disabled (post-0.3.0 defect, not a spec contradiction).** The `IUIAutomation` client is created once and never released; toggling UIA off is flag-only and `stop_capture` drops only the capture source, so Chrome/Codex/Claude Desktop stay in accessibility mode and keep hanging after "disable capture". No per-app back-off, and a wedged walk is never cancelled. | `crates/uia/src/{lib,worker}.rs`; `src-tauri/src/lib.rs` (`spawn_ocr`, `UiaWithOcrFallback`, `set_settings`); `crates/kernel/src/lib.rs` (`stop_capture`) | **Fixed** on `fix/uia-client-lifecycle-breaker`: client teardown on disable / settings-change / capture-stop (releases the client → apps leave a11y mode); per-app circuit breaker (3 bad walks → 30-min OCR cooldown, `crates/uia/src/breaker.rs`); hard-timeout cancellation; all UIA knobs now hot-apply. Additive `OcrProvider::on_capture_stopped` default-no-op. Not a spec silence/contradiction — a defect in the shipped 0.3.0 lifecycle. | fixed (PR open, awaiting review) |

> Resolved v0.1.0-era rows (#1–#4) → `specs/archive/06_PATCH_PLAN.v0.1.0.md` (ids preserved).
> Shipped 0.2.x rows (#5–#14 + the 0.2.1 build notes) → `specs/archive/06_PATCH_PLAN.v0.2.x.md` (ids preserved).
> Resolved 0.3.0-arc rows (#16–#20) → `specs/archive/06_PATCH_PLAN.v0.3.0.md` (ids preserved). Only the open upstream-leak row #15 stays live.

When the spec contradicts itself, stop, ask the user, and log the resolution here before coding.

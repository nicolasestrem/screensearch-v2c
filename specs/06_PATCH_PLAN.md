# 06 — Patch Plan

> **Ordered fixes required before shipping**, plus any **spec contradictions** found during the
> build (`04 §5`). Empty until the build surfaces something.

| # | Priority | Issue | Source (spec §, file:line) | Fix | Status |
|---|---|---|---|---|---|
| 15 | P5 | **Upstream llama.cpp multimodal host-memory leak (build 9842).** ~149 MB committed host RAM leaked per vision inference; VRAM flat. Mitigated locally by the `sidecar.recycle_*` valve (recycle at a committed-RAM ceiling). Real fix requires a newer bundled `llama-server` build. | `07` #72; `specs/03_MASTER_PRODUCTION_SPEC.md §8`; `crates/inference` sidecar lifecycle | **Open (upstream).** Local mitigation shipped (`sidecar.recycle_enabled` default true, `sidecar.recycle_rss_mb` default 0=auto). Upgrade the bundled llama-server build when a fix lands upstream; re-run the 60-frame isolated test to confirm RSS stays flat. | open (upstream) |
| 16 | P5 | **Spec demanded an Inno Setup installer + portable ZIP, but Tauri 2 ships NSIS/MSI natively; v0.1.0 shipped NSIS only** — a standing spec-vs-reality contradiction (`07` #26). | `00 §G`, `01`, `02 §5`, `03 §11`+§13.9, `docs/ARCHITECTURE.md §12`, `.github/workflows/ci.yml`, `README.md` | **✅ Resolved (user-approved full sweep, `chore/cancel-inno-and-a11y-matrix`):** rewrote all 9 live refs to **NSIS**; Inno/MSI/portable ZIP **dropped**; DoD §13.9 re-scoped to NSIS and **met**; `07` #26 closed. Code-signing stays the lone open packaging item (`07` "Manual steps"). | ✅ resolved |

> Resolved v0.1.0-era rows (#1–#4) → `specs/archive/06_PATCH_PLAN.v0.1.0.md` (ids preserved).
> Shipped 0.2.x rows (#5–#14 + the 0.2.1 build notes) → `specs/archive/06_PATCH_PLAN.v0.2.x.md` (ids preserved). Only the open upstream-leak row #15 stays live.

When the spec contradicts itself, stop, ask the user, and log the resolution here before coding.

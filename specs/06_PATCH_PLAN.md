# 06 — Patch Plan

> **Ordered fixes required before shipping**, plus any **spec contradictions** found during the
> build (`04 §5`). Empty until the build surfaces something.

| # | Priority | Issue | Source (spec §, file:line) | Fix | Status |
|---|---|---|---|---|---|
| 1 | — | **Vision scheduling model changed (user decision 2026-06-21).** `03 §8` modelled vision triggering as a single `enrich.vision_mode` enum (`on_demand`/`timer`/`idle`). Per the user, timed and idle enrichment must each be an **independent opt-in toggle (off by default) + a user-set threshold**; on-demand is always available. | `03 §8`; `crates/traits/src/ipc.rs` (`Settings`) | Replaced `enrich_vision_mode: VisionMode` with `enrich_vision_timer_enabled`/`_interval_ms` + `enrich_vision_idle_enabled`/`_idle_secs`; removed the `VisionMode` enum. **`03 §8` updated to match.** | ✅ applied |

When the spec contradicts itself, stop, ask the user, and log the resolution here before coding.

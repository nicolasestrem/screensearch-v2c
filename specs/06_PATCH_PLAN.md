# 06 — Patch Plan

> **Ordered fixes required before shipping**, plus any **spec contradictions** found during the
> build (`04 §5`). Empty until the build surfaces something.

| # | Priority | Issue | Source (spec §, file:line) | Fix | Status |
|---|---|---|---|---|---|
| 1 | — | **Vision scheduling model changed (user decision 2026-06-21).** `03 §8` modelled vision triggering as a single `enrich.vision_mode` enum (`on_demand`/`timer`/`idle`). Per the user, timed and idle enrichment must each be an **independent opt-in toggle (off by default) + a user-set threshold**; on-demand is always available. | `03 §8`; `crates/traits/src/ipc.rs` (`Settings`) | Replaced `enrich_vision_mode: VisionMode` with `enrich_vision_timer_enabled`/`_interval_ms` + `enrich_vision_idle_enabled`/`_idle_secs`; removed the `VisionMode` enum. **`03 §8` updated to match.** | ✅ applied |
| 2 | — | **OCR confidence not available from the chosen engine (P2).** `03 §3` `OcrResult.mean_confidence: f32` (and `ocr_text.mean_confidence REAL NOT NULL`) assume a per-word/line confidence, but the spec-mandated WinRT `Media.Ocr` exposes **none** — `OcrWord` has only `Text` + `BoundingRect` (verified against Microsoft Learn). | `03 §3/§4`; `crates/ocr/src/lib.rs` | **Resolved (user, 2026-06-21):** record the sentinel **`-1.0` = "unknown"** (`ocr::CONFIDENCE_UNKNOWN`) rather than fabricate a score; no schema/trait change. The P5 UI renders negative as "n/a". (See `07`.) | ✅ applied |
| 3 | P2 | **PR #21 audit-doc review follow-up.** Reviewers found the audit artifact had not mirrored the live follow-up work into `05_BUILD_REVIEW.md` / `07_KNOWN_GAPS.md`, and the report overlabelled local Node 26 frontend checks as CI-equivalent despite CI using Node 22. | PR #21 review threads; `AGENTS.md`; `04 §7`; `.github/workflows/ci.yml` | Mirrored the audit follow-up into `05`, added open gaps #40–#45 to `07`, clarified the audit as CI-order local smoke rather than exact CI-runtime evidence, and noted that the obsolete-term search was captured before the audit artifact existed. | ✅ applied |

When the spec contradicts itself, stop, ask the user, and log the resolution here before coding.

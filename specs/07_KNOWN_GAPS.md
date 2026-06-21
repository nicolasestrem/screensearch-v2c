# 07 — Known Gaps

> Where the **spec is silent** and a decision was needed, plus **manual interventions** still
> required (with owner + deadline). The agent appends here instead of guessing (`04 §5`). Empty
> until the build hits a gap.

| # | Date | Gap (spec was silent on…) | Resolution / decision | Owner | Needed by |
|---|---|---|---|---|---|
| 1 | 2026-06-21 | `03 §8` listed `enrich.vision_timer_interval_ms` / `enrich.vision_idle_secs` **without defaults**, and modelled mode as a single enum. | **Resolved (user):** timed = **60 min** (`3_600_000` ms), idle = **5 min** (`300` s); both are **opt-in toggles, off by default** (see `06` patch #1). `03 §8` updated. | user | ✅ done |
| 2 | 2026-06-21 | Intake/spec don't fix a **bundle identifier** for installers. | **Resolved (user):** `app.screensearch.desktop` was taken → use **`app.screensearchv2c.desktop`** (`tauri.conf.json`). | user | ✅ done |
| 3 | 2026-06-21 | `03 §7` `get_readiness` returns a `Readiness` but `03` didn't define the per-component status enum. | **Resolved:** defined `ComponentStatus { Unknown, Disabled, Initializing, Ready, Unavailable, Error }` + `ComponentReadiness { status, detail? }`; `Readiness` now holds one per subsystem so the UI can show *why*. `03 §7` updated. | agent | ✅ done |
| 4 | 2026-06-21 | `02 §5` "WebView2/Vulkan/llama smoke-check" doesn't specify the form. | **Resolved:** `crates/doctor` is now a **library + thin CLI** (`cargo run -p doctor [-- --json]`) returning a structured `Report` — reusable by CI and (later) the app's readiness panel. Diagnostic only (never fails CI). | agent | ✅ done |

Resolved engineering decisions (spec silent on *how*, recorded for traceability):
- **ts-rs 64-bit ints → TS `number`** via per-field `#[ts(type = "number")]` (Tauri JSON wire);
  `TS_RS_LARGE_INT` env is ignored by ts-rs 10.1's macro. Guarded by `no_bigint_in_ipc_types`.
- **ts-rs `export_to`** anchors at the source-file dir here → `../../../ui/src/bindings/`.
- **CSP** set to `null` in `tauri.conf.json` for P0 dev convenience — **harden in P5**.
- **§9 file logging** (daily rotation) deferred to P1 (needs the resolved app data dir).

Manual steps still required (e.g. signing certs, first-run model download, CI secrets):
- **First-run model download** (vision/answer GGUF + mmproj, embedding models) — P3/P4, per
  `MODEL_REGISTRY §4`. Not bundled.
- **Code-signing certificate** for the installer — P5 packaging. Recommended path for this MIT OSS
  project (cheapest → most turnkey): **SignPath Foundation** (free Authenticode signing for
  qualifying OSS) → **Azure Trusted Signing** (~US$10/mo, Microsoft-run, builds SmartScreen
  reputation) → **Certum Open Source Code Signing** (cheap annual cloud cert for OSS devs). Note:
  since 2023 all OV certs require a hardware token or cloud HSM, so a plain file-based cert is no
  longer available; budget for that. Decide owner before P5.
- ~~esbuild `allow-scripts` postinstall~~ — **resolved** locally via `npm approve-scripts --all`.

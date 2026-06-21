# 07 — Known Gaps

> Where the **spec is silent** and a decision was needed, plus **manual interventions** still
> required (with owner + deadline). The agent appends here instead of guessing (`04 §5`). Empty
> until the build hits a gap.

| # | Date | Gap (spec was silent on…) | Resolution / decision | Owner | Needed by |
|---|---|---|---|---|---|
| 1 | 2026-06-21 | `03 §8` lists `enrich.vision_timer_interval_ms` and `enrich.vision_idle_secs` **without default values**. | **Provisional** defaults set in `Settings::default()`: timer = `300_000` ms (5 min), idle = `300` s (5 min). Not used in the default `on_demand` mode, so non-blocking for P0. **Confirm before P3/P4** vision scheduling. | user | P3 |
| 2 | 2026-06-21 | Intake/spec don't fix a **bundle identifier** (reverse-DNS) for installers. | Chose `app.screensearch.desktop` (site is screensearch.app). Easy to change pre-release. Confirm. | user | P5 |
| 3 | 2026-06-21 | `03 §7` `get_readiness` returns a `Readiness` but `03` doesn't define the per-component status enum. | Defined `ComponentStatus { Unknown, Initializing, Ready, Unavailable, Error }`. | agent | — |
| 4 | 2026-06-21 | `02 §5` "WebView2/Vulkan/llama smoke-check" doesn't specify the form. | Implemented as `crates/doctor` (`cargo run -p doctor`): registry WebView2 lookup, `vulkan-1.dll` load probe, `llama-server.exe` PATH probe. Diagnostic only (never fails CI). | agent | — |

Resolved engineering decisions (spec silent on *how*, recorded for traceability):
- **ts-rs 64-bit ints → TS `number`** via per-field `#[ts(type = "number")]` (Tauri JSON wire);
  `TS_RS_LARGE_INT` env is ignored by ts-rs 10.1's macro. Guarded by `no_bigint_in_ipc_types`.
- **ts-rs `export_to`** anchors at the source-file dir here → `../../../ui/src/bindings/`.
- **CSP** set to `null` in `tauri.conf.json` for P0 dev convenience — **harden in P5**.
- **§9 file logging** (daily rotation) deferred to P1 (needs the resolved app data dir).

Manual steps still required (e.g. signing certs, first-run model download, CI secrets):
- **First-run model download** (vision/answer GGUF + mmproj, embedding models) — P3/P4, per
  `MODEL_REGISTRY §4`. Not bundled.
- **Code-signing certificate** for the installer — P5 packaging.
- Local dev note: an npm `allow-scripts` policy blocked esbuild's postinstall; build still
  succeeded. On a locked-down machine you may need `npm approve-scripts`.

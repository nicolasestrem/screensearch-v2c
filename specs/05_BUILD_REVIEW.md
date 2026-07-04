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
> Live file holds only the current (post-0.3.0) arc — empty until the next arc begins.

---

## Pass 1 — 2026-07-04 — Post-0.3.0 defect fix: UIA client lifecycle + per-app circuit breaker

- **Implemented:** Root-caused a shipped 0.3.0 defect where Chromium/Electron apps (Chrome,
  Codex, Claude Desktop) hang/crash while capture runs and **stay hung after the user disables
  capture**. Confirmed via live evidence (Windows AppHang 1002 for chrome.exe fired *inside* a
  capture-off window; `frame_text.primary_source='uia'` rows for chrome/Codex/claude; the log's
  only WARN was the UIA over-budget line) plus a full `crates/uia` + composition-root read. Root
  cause: the `IUIAutomation` client (spawned once, held in `Kernel.ocr` for the process lifetime)
  is never released — neither the `capture.uia_text_enabled` toggle (flag-only) nor `stop_capture`
  (drops only the capture source) disconnects it — so Chromium keeps its accessibility engine on.
  Fix (feature branch `fix/uia-client-lifecycle-breaker`):
  1. **Worker reply plumbing** (`crates/uia/src/worker.rs`): `WalkReply { outcome, over_budget }`;
     `worker_main` times the whole walk and hoists the rate-limited over-budget warn (now also
     covers over-budget *error* walks).
  2. **Cancellation** (`worker.rs` + `lib.rs`): per-request `cancel` + provider `shutdown` flags
     checked at both deadline checkpoints; the caller's 2×budget hard timeout sets `cancel`.
  3. **Per-app circuit breaker** (new pure `crates/uia/src/breaker.rs`, CI-tested): 3 consecutive
     over-budget/timed-out walks per app → 30-min OCR cooldown; a good walk resets. Named
     constants, not settings (user decision — surface-reduction arc).
  4. **Composite lifecycle** (`src-tauri/src/lib.rs` `UiaWithOcrFallback`): the client lives in a
     teardown-able slot; disabling UIA / changing any UIA setting / stopping capture disconnects
     it (`OcrProvider::on_capture_stopped` default-no-op trait method, called by `stop_capture`);
     the next eligible frame lazily respawns. Side benefit: **all UIA knobs now hot-apply** (the
     APPLY_RESTART trap is gone).
  - Verification (verbatim):
    - `cargo test -p uia --lib` → `test result: ok. 26 passed; 0 failed; 3 ignored`.
    - `cargo test -p kernel` (incl. new `stop_capture_notifies_ocr_provider`) → all green.
    - `cd ui && npm run lint` → clean; `npm run build` → exit 0; `cargo build -p screensearch`
      → `Finished dev profile ... in 20.84s`.
- **Skipped / deferred:** A Chromium window-class *skip* (former "Fix A") — the user chose
  breaker-only, so Chromium apps still get UIA (and the first walk still flips them into a11y
  mode) but are backed off on repeated struggle. Recorded in `07` (two new rows).
- **Hallucinated / corrected:** One breaker unit test had an off-by-one (expected a 4th bad walk
  to open the breaker; the 3rd does). Caught by `cargo test`; test fixed, impl was correct.
- **Still risky:** The live-desktop UIA lifecycle test (`uia_worker_exits_on_shutdown`) and the
  hang-doesn't-recur behavior are `#[ignore]`d in CI (need a real session) — must be run locally
  (`cargo test -p uia -- --ignored`) + a `npm run tauri dev` walkthrough before merge.
- **Review follow-up (PR #79, 2026-07-04):** addressed the one substantive review finding (a
  `claude`-bot inline comment; gemini/codex left no actionable comments). `get_or_spawn_uia`
  baked the *caller's* stale `cfg.budget` snapshot: a settings save landing while the slot was
  still empty (capture-start → first-spawn window) swaps `config` + tears down a **no-op empty
  slot**, then the racing frame spawned the client with the pre-save budget — persisting until
  the next save, contradicting "every UIA setting hot-applies." Fix: drop the `cfg` param and
  read `self.config.lock().budget` fresh under the `respawn_gate` (the spawn now owns reading
  the live config); log lines use the fresh `budget` too. Residual two-save micro-race recorded
  in `07` #95 (not human-reachable, self-heals). No signature/IPC change → bindings stay clean.

---

## Pass 2 — 2026-07-04 — Post-0.3.0: Flow overlay default hotkey `Ctrl+Alt+Z` + one-shot remap

- **Implemented:** Changed the Flow overlay default summon chord from `Ctrl+Alt+Space` (collided
  with Claude Desktop's global quick-entry shortcut) to `Ctrl+Alt+Z` in all three sources of
  truth (`crates/traits/src/ipc.rs` default, `src-tauri/src/overlay.rs` `OVERLAY_DEFAULT_CHORD`,
  `ui/src/components/domain/HotkeyField.tsx` `DEFAULT_OVERLAY_HOTKEY`), plus a one-shot load-path
  remap (`kernel::settings::load_overlay_hotkey`, mirroring the `load_tier` beta→quality
  precedent) that rewrites a persisted `Ctrl+Alt+Space` to the new default exactly once and
  leaves any custom chord untouched. RegisterHotKey failure was already surfaced in Settings
  (D6 prior art, `overlay.rs` `failed_status`+`emit_hotkey_warning`) — reused, not rebuilt.
  - Verification (verbatim): `cargo test -p kernel --test settings` →
    `test result: ok. 13 passed; 0 failed` (incl. new `overlay_hotkey_legacy_default_remaps_once`
    + `overlay_hotkey_custom_value_survives`; updated the persisted-value assertion in
    `overlay_hotkey_empty_string_resets_to_default` to the new default). `npm run lint` clean.
- **Skipped / deferred:** TODO-3 (a Settings-level cross-chord conflict *check* between the two
  hotkeys) stays open — deferred by decision; this change doesn't implement it.
- **Hallucinated / corrected:** none.
- **Still risky:** the AZERTY/AltGr caveat (AltGr reported as Ctrl+Alt) is unchanged; `Ctrl+Alt+Z`
  on AZERTY is produced by AltGr+Z where Z is a letter, but the overlay registers the chord, not
  a character, so no typing conflict — confirm on a live AZERTY session in the walkthrough.
- **Review follow-up (PR #80, 2026-07-04):** addressed the inline review comments across two
  rounds (all bot-authored; evaluated on merits, not replied to). Round 1: (1) **codex P2 — the
  remap wasn't truly one-shot.** It was value-only, so a user who deliberately set `Ctrl+Alt+Space`
  back had it re-remapped to `Ctrl+Alt+Z` on the next `load_settings`, breaking the reversible
  escape hatch the CHANGELOG / `07` #94 promised. Fixed by gating the remap behind a persisted
  marker (`overlay.hotkey_migrated`), latched on the first load regardless of stored value, so it
  fires at most once per install and the stored chord is honored verbatim afterward; new test
  `overlay_hotkey_deliberate_legacy_survives_after_migration` proves it. (2) **gemini — hint
  hardcoded the chord:** `Settings.tsx` now interpolates the imported `DEFAULT_OVERLAY_HOTKEY`.
  (3) **gemini — test hardcoded the default JSON:** the two persisted-value assertions now derive
  the expected string from `serde_json::to_string(&Settings::default().overlay_hotkey)`. Round 2
  (on the marker fix itself): (4) **codex/claude P2 — don't latch a failed remap.** The marker was
  written unconditionally, so a *failed* `overlay.hotkey` rewrite followed by a *successful* marker
  write latched the migration anyway — the stale `Ctrl+Alt+Space` would then be honored forever and
  the promised retry never happened. Fixed: the marker is now written **only after** the remap
  rewrite succeeds (a custom value / fresh install still latches immediately, since there is no
  rewrite to fail); a failed rewrite leaves the migration un-run so the next load retries; test
  `overlay_hotkey_failed_remap_is_retried_not_latched`. (5) **codex/claude — source-of-truth docs
  still said `Ctrl+Alt+Space`:** updated every *living* reference to `Ctrl+Alt+Z` (`specs/03`,
  `specs/02`, `specs/UI_REFERENCE.md`, `README.md`, `docs/TESTING.md`, `docs/ARCHITECTURE.md`);
  `docs/0.3.0.md` (the shipped-arc design record) and the build-loop logs are left citing
  `Ctrl+Alt+Space` on purpose — 0.3.0 genuinely shipped that default and this is a post-0.3.0 fix.
  CHANGELOG + `07` #94 corrected (the reversibility claim now actually holds). No IPC shape change
  → bindings clean.

---

## Pass 2 — 2026-07-04 — 0.3.1 PR1 (specs contract; specs-only, no code)

- **Implemented:** The 0.3.1 patch contract ("P7.1 — post-0.3.0 triage", `docs/0.3.1.md`)
  normalized into the specs: `04` (reading order + source-of-truth row + PR1→PR4 build order
  with D5/D8/D9 encoded), `07` (rows #96–#99: the #69/#56/#57/#54 deferrals + the resolved
  quick-menu silence), `UI_REFERENCE` (D1 Moment inline text; D2/D3 report filename + footer;
  D4 NavRail version footer), `CLAUDE.md`/`AGENTS.md` current-state, `CHANGELOG.md` +
  `08` entries. Verification = the diff itself: `git diff --name-only main` shows only `.md`
  files (verbatim output on the PR).
- **Skipped / deferred:** Everything with a runtime surface — deliberately. PR2 (#64) and PR3
  (#59/#65/#57-partial) implement this contract; the GitHub issue hygiene (close #54, label
  #56/#69 `deferred-0.3.2`, comment on #57) runs right after the PR opens (user-approved).
- **Hallucinated / corrected:** The roadmap's "quick menu" (D4) does not exist as a named
  surface anywhere in the specs or the UI — assumed candidates (CommandPalette vs. Settings vs.
  NavRail) were put to the user instead of guessed; resolution (NavRail footer) recorded in
  `07` #99. Nothing else assumed.
- **Broke / regressed:** Nothing — no code touched.
- **Still risky:** The D2 filename contract assumes the report download keeps its `.md`
  extension (true today, `ui/src/routes/Recall.tsx`); if PR3 finds the download path differs,
  the spec wording ("extension unchanged") still holds by construction. `06` stays empty — no
  spec contradiction surfaced while normalizing (verified: `03` carries no report-filename/
  footer contract; `UI_REFERENCE` had no nested-scroll text to replace).

---

## Pass 3 — 2026-07-04 — 0.3.1 PR2 Phase A profile-first baseline (#64)

- **Scope / protocol (before fix code):** Profiled the current WebP tree on
  `fix/0.3.1-pr2-vision-throughput` against the last pre-WebP tag `v0.2.1`
  (`c22625c`) in a separate worktree `..\ss-v021-pr2-profile`. Per the user's runtime
  correction, both live app runs used `npm run dev` so the Tauri WebView had a live
  Vite server. Baseline worktree used a temporary local-only bundle identifier
  `app.screensearchv2c.pr2baseline` (uncommitted) and junctioned `models`/`sidecar`
  to the temp dev profile so both runs used the same downloaded quality vision model:
  `Qwen3VL-8B-Instruct-Q4_K_M.gguf`. Workload: one warmup `vision_tag`, then 30
  repeated `vision_tag` jobs against the same captured screen content; current run
  used the stored native WebP frame
  `frames/day-20638/1783174081669-0.webp` (3440x1440, 3,200,026 bytes), baseline
  used a JPEG copy of that frame under the v0.2.1 storage shape
  `frames/day-20638/1783174081669-0.jpg` (1280x536, 141,733 bytes, q80-equivalent
  ffmpeg `-q:v 5`). Throttle and embedding lanes were disabled for both runs;
  `enrich.worker_concurrency=2`.
- **Throughput numbers (Phase A baseline):**
  - Current WebP tree (`npm run dev`, schema v10, jobs 46-75): `done|30`;
    `min(created_at)=1783175422214`, `max(updated_at)=1783175489000`,
    frames/min = **26.95**. Per-minute grouping: `2026-07-04T16:30|16`,
    `2026-07-04T16:31|14`.
  - Pre-WebP baseline `v0.2.1` (`npm run dev`, schema v6, jobs 2-31): `done|30`;
    `min(created_at)=1783175777815`, `max(updated_at)=1783175807000`,
    frames/min = **61.68**. Per-minute grouping: `2026-07-04T16:36|30`.
  - Regression size: current WebP throughput is **43.7%** of the pre-WebP baseline
    (56.3% slower), far outside the PR2 acceptance band.
- **GPU utilization shape (`Get-Counter '\GPU Engine(*)\Utilization Percentage'`,
  1s samples; summed all engines + max single engine):**
  - Current WebP: 55 samples, sum avg **42.35%**, median **37.85%**, p95 **93.64%**,
    max **97.05%**, min **3.50%**. Sample excerpt shows repeated idle valleys between
    spikes: `3.516`, `9.062`, `81.336`, `28.425`, `58.674`, `3.613`, `50.216`,
    `69.636`, `3.836`, `38.461`.
  - Pre-WebP JPEG baseline: 25 samples, sum avg **59.53%**, median **59.71%**,
    p95 **91.33%**, max **94.05%**, min **3.53%**. It still has job-bound variation,
    but holds a substantially higher median/average during the batch and finishes in
    less than half the wall time.
- **Decode probe (app-equivalent `image::open(...).to_rgba8()`, release-mode local
  probe under `target/pr2-profile/decode-probe`, not repo code):**
  - Native WebP source: `dims=3440x1440`, 30 iterations, avg **44.73 ms**,
    p50 **44.57 ms**, p95 **46.09 ms**.
  - Pre-WebP JPEG source: `dims=1280x536`, 30 iterations, avg **3.09 ms**,
    p50 **3.07 ms**, p95 **3.20 ms**.
- **Call-site answer — does WebP block the next inference dispatch?** Yes on the
  vision dispatch path, through synchronous decode rather than only encode:
  `crates/kernel/src/worker_pool.rs::vision_tag_outcome` loads the stored capture with
  `load_rgba(abs.clone()).await` before it calls `vision.analyze(&image).await`
  (current lines 494 and 516). That means the sidecar/GPU request for each job cannot
  be dispatched until CPU/file decode completes. The storage encode call site is
  `crates/kernel/src/capture_loop.rs::process_frame`, which awaits `write_webp(...)`
  before the frame row/text/jobs are stored (current line 109; encoder at line 213).
  This blocks the capture cycle for new frames and competes for blocking-pool/CPU work,
  but the measured regression is decode-dominant for already queued `vision_tag` jobs.
- **Decision gate:** The slowdown is attributable to the WebP format path (native
  WebP decode before VLM dispatch, with capture-side lossless WebP encode as secondary
  contention), not to degrade-to-text or model-side behavior. PR2 may proceed to Phase B
  under D5. No schema change and no new user-facing setting are indicated.

---

## Pass 4 — 2026-07-04 — 0.3.1 PR2 Phase B fix + acceptance profile (#64)

- **Implemented:** `crates/kernel/src/vision_proxy.rs` adds an internal, bounded JPEG vision
  proxy beside stored WebP captures (`<frame>.vision.jpg`, max edge 1280 px, q80). The capture
  loop owns a bounded writer queue (capacity 8) and flushes it on capture-loop shutdown; if the
  queue is full or an older WebP has no proxy yet, the worker pool lazily creates the same proxy
  once, then dispatches vision jobs from the cheap JPEG decode path. The stored `frames.image_path`
  remains the WebP; no schema, UI, setting, API, or storage-format change. Retention/self-capture
  purge now delete the sidecar proxy when deleting a WebP.
- **Why:** Phase A showed `worker_pool::vision_tag_outcome` synchronously decoded native WebP
  before `vision.analyze`, producing 26.95 jobs/min vs. 61.68 jobs/min on the last pre-WebP
  baseline. The proxy restores the pre-WebP 1280 px vision workload without reverting storage
  away from WebP and follows D5's first preference: remove WebP work from the vision hot path
  before trying encoder knobs or a format escape hatch.
- **Acceptance profile (`npm run dev`, same temp dev DB/model/settings/workload as Pass 3):**
  - Warmup: job `76|done|0||1783176370886|1783176380000`; proxy created at
    `frames/day-20638/1783174081669-0.vision.jpg`, 147,174 bytes.
  - Fixed current tree (`npm run dev`, jobs 77-106): `done|30`;
    `min(created_at)=1783176405822`, `max(updated_at)=1783176435000`,
    frames/min = **61.69**. Per-minute grouping: `2026-07-04T14:46|12`,
    `2026-07-04T14:47|18`.
  - Acceptance math: fixed current = **100.0%** of the pre-WebP v0.2.1 baseline
    (61.69 vs. 61.68 jobs/min), comfortably inside the required within-10% band.
- **GPU utilization shape after fix (`target/pr2-profile/gpu-current-fixed-proxy-dev-getcounter.csv`):**
  21 samples; summed engines avg **61.18%**, median **54.95%**, p95 **91.55%**, max
  **94.09%**, min **28.48%**; max single engine avg **53.03%**, median **46.83%**, p95
  **87.80%**, max **90.21%**, min **19.60%**. The repeated near-idle 3-4% valleys seen in
  Phase A's WebP run disappeared from the measured batch.
- **Decode probe after fix (same app-equivalent release-mode probe):**
  - WebP source still costs `dims=3440x1440`, avg **46.83 ms**, p50 **46.46 ms**,
    p95 **51.18 ms**.
  - Vision proxy costs `dims=1280x536`, avg **3.08 ms**, p50 **3.06 ms**,
    p95 **3.24 ms**.
- **Verification (verbatim):** `cargo test -p kernel` →
  `test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`;
  integration suites also passed: `9 passed`, `12 passed`, `15 passed`, `2 passed`.
  Full CI-order pass:
  `npm --prefix ui ci` → `added 347 packages, and audited 348 packages in 5s` /
  `found 0 vulnerabilities`;
  `npm --prefix ui run lint` → `> eslint .`;
  `npm --prefix ui run build` → `✓ built in 1.76s`;
  `node scripts/stage-mcp.mjs` → `[stage-mcp] up to date: ...screensearch-mcp-x86_64-pc-windows-msvc.exe`;
  `cargo fmt --all -- --check` → exit 0;
  `cargo clippy --workspace --all-targets -- -D warnings` →
  `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 2.82s`;
  `cargo build --workspace` →
  `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 8.08s`;
  `cargo test --workspace` →
  `Finished \`test\` profile [unoptimized + debuginfo] target(s) in 9.82s` with all non-ignored
  suites passing; `git diff --exit-code -- ui/src/bindings` → exit 0.

---

## Pass 5 — 2026-07-04 — 0.3.1 PR2 review follow-ups (#64)

- **Addressed PR #83 review threads:** (1) removed synchronous `proxy_path.exists()` from
  `VisionProxyWriter::try_enqueue`; the blocking writer still does the idempotent existing-file
  check before writing. (2) Replaced async-path synchronous metadata/remove calls with
  `tokio::fs` in capture encode logging, vision proxy lookup/rebuild cleanup, worker-pool
  image-existence classification, and retention/self-capture proxy cleanup. (3) Made
  `remove_frame_image_and_proxy` async and return proxy-delete failures (except NotFound), so
  retention/self-capture cleanup leaves the DB row retryable instead of orphaning a proxy when
  Windows AV/indexer sharing blocks deletion. (4) Capped queued capture-side proxy generation by
  `min(storage.max_width, 1280)` when a storage cap exists; lazy proxies still derive from the
  already-stored WebP. No bot replies were posted (user requested no bot replies).
- **Tests added:** `queued_proxy_respects_storage_max_width_below_proxy_cap`,
  `remove_frame_image_and_proxy_deletes_webp_and_proxy`, and
  `remove_frame_image_and_proxy_propagates_proxy_delete_failure`.
- **Verification (verbatim):** focused checks:
  `cargo test -p kernel vision_proxy -- --nocapture` →
  `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 43 filtered out`;
  `cargo test -p screensearch remove_frame_image_and_proxy -- --nocapture` →
  `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 12 filtered out`.
  Full CI-order pass:
  `npm --prefix ui ci` → `added 347 packages, and audited 348 packages in 5s` /
  `found 0 vulnerabilities`;
  `npm --prefix ui run lint` → `> eslint .`;
  `npm --prefix ui run build` → `✓ built in 1.82s`;
  `node scripts/stage-mcp.mjs` → `[stage-mcp] up to date: ...screensearch-mcp-x86_64-pc-windows-msvc.exe`;
  `cargo fmt --all -- --check` → exit 0;
  `cargo clippy --workspace --all-targets -- -D warnings` →
  `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 3.42s`;
  `cargo build --workspace` →
  `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 22.44s`;
  `cargo test --workspace` →
  `Finished \`test\` profile [unoptimized + debuginfo] target(s) in 25.53s` with all non-ignored
  suites passing; `git diff --exit-code -- ui/src/bindings` → exit 0.

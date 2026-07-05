# 05 — Build Review — archived v0.3.1 history (P7.1 triage patch)

> Archived on the v0.3.1 release sweep (2026-07-05, 0.3.1 PR4); entries preserved verbatim from
> the live `specs/05_BUILD_REVIEW.md`. Contents: the two post-0.3.0 bridge fixes (PR #79 UIA
> client lifecycle + circuit breaker; PR #80 overlay default hotkey Ctrl+Alt+Z) and the 0.3.1
> patch passes (PR1 specs contract; PR2 #64 profiling → stop condition → user-decided fix;
> PR3 polish bundle; PR4 audit + release). Earlier history → the v0.1.0 / v0.2.x / v0.3.0
> archives in this folder.

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

## Pass 2a — 2026-07-04 — Post-0.3.0: Flow overlay default hotkey `Ctrl+Alt+Z` + one-shot remap

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

## Pass 2b — 2026-07-04 — 0.3.1 PR1 (specs contract; specs-only, no code)
<!-- Renamed from a duplicate "Pass 2" heading on the v0.3.1 archive fold so every "05 Pass N"
     citation in 06/07/08/CHANGELOG resolves uniquely; Pass 3/4/5 numbering is unchanged. -->

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

## Pass 3 — 2026-07-04 — 0.3.1 PR2 Phase A (#64 profiling) — **STOP CONDITION TRIGGERED**

Profile-first per D9 (`04 §3`), release builds only (the closed PR #83 profiled under
`npm run dev`; debug skews CPU-bound image work 10–50× and produced misleading numbers —
its "1.1–1.5 s WebP encodes" measure **26 ms** in release). Branch
`fix/0.3.1-pr2-vision-throughput-r2`, instrumentation-only commit `dbb1789` (decode/analyze
split in `worker_pool`, acquire/prep/complete split in `inference::vision`, encode timing +
bytes in `capture_loop`; same patch applied locally-uncommitted to the baseline worktree).

- **Method (all disclosures):**
  - Current tree: release exe (`cargo build --release -p screensearch --features
    tauri/custom-protocol` — without the feature flag a raw cargo release build loads the
    devUrl and is not the shipped app). Isolated data dir via an **uncommitted**
    `tauri.conf.json` identifier edit → `app.screensearchv2c.pr2current`.
  - Baseline: `v0.2.1` (= c22625c, the last pre-WebP tag) in worktree `..\ss-v021-pr2-profile`,
    identifier edited (uncommitted) → `app.screensearchv2c.pr2baseline`, same release build.
  - Both data dirs junction `models` + `sidecar` to the live install → **same
    `Qwen3VL-8B-Instruct-Q4_K_M.gguf` (Quality tier), same mmproj, same `llama-server`
    build** on both sides. Same machine, same 3440×1440 monitor, same animated workload
    page (1 Hz clock/text page kept visible), capture timer 1000 ms, vision timer 60 s /
    batch 500, `enrich.worker_concurrency=2` (default) on both sides. v0.2.1's
    `enrich.image_embeddings` was forced **off** (parity: the lane doesn't exist post-0.3.0).
  - Saturation: the vision queue was kept non-empty throughout the measured windows by
    re-enqueueing `vision_tag` jobs for already-tagged frames (`insert_vision` is an upsert;
    the per-job code path — claim → decode → VLM → upsert — is identical). Rates below are
    from minutes where pending > 0.
  - GPU shape via `nvidia-smi -l 1` (ground truth). The PDH `\GPU Engine(*)\Utilization
    Percentage` counters the roadmap suggested (and that `sysmon` uses) read **~1–2 % while
    nvidia-smi showed 80–100 %** — PDH does not attribute this Vulkan compute workload.
    Recorded in `06`/`07`.
- **Numbers (steady-state, saturated queue, ≥15 min & ≥500 jobs per side):**

  | metric | v0.2.1 (pre-WebP) | current tree (0.3.1 base) | ratio |
  |---|---|---|---|
  | stored frame | 1280×536 JPEG q80 (~113 KB) | 3440×1440 lossless WebP (~950 KB) | 8.4× bytes |
  | VLM request | 1280×536 (stored as-is) | **1568×656** (native → `VISION_MAX_EDGE` cap) | 1.50× pixels |
  | vision jobs done/min | **~89–95** (12-min window: 72,101,95,96,97,94,95,62,89,98,91,83) | **~31–35** (29,35,32,30,32,34 · 31,25,37,41,35,39,38) | **~0.36×** |
  | per-job total_ms (median/avg) | 1234 / 1406 | 3061 / 3185 | 2.48× |
  | decode_ms (median) | 3 | 23 | +20 ms |
  | prep_ms (median: VLM downscale+JPEG+base64) | 12 | 47 | +35 ms |
  | **complete_ms (median: sidecar round-trip)** | **1217** | **2984** | **+1767 ms (≈97 % of the delta)** |
  | capture encode_ms (median) | 42 (resize→1280 + JPEG) | 26 (native lossless WebP) | encode got *faster* |
  | GPU shape (nvidia-smi, saturated) | median 94 %, dips <10 % in 13 % of samples | median 90 %, dips <10 % in 18 % of samples | same character |

- **Finding (the decision gate):** the throughput regression is real (~2.5–2.9×) and is
  attributable to the #58 storage change as a *bundle* — but **~97 % of the per-job delta is
  sidecar compute on the 1.5×-pixel VLM request** (native-res storage means
  `downscale_for_vlm` now caps at 1568 px instead of receiving a 1280 px stored image; the
  Qwen3-VL vision encoder + prefill scale super-linearly with patch count). The encode step
  measures 26 ms and **got cheaper than v0.2.1's** 42 ms resize+JPEG; decode adds 20 ms.
  The D5 fix ladder (async encode → cheaper encoder settings → format revert) addresses
  ≤55 ms of a ~1830 ms per-job regression and **cannot reach the ±10 % acceptance**, and the
  roadmap's premise ("the signature of the encode step landing synchronously on the vision
  hot path and serializing work") is contradicted by the release measurements: under load
  both trees hold the GPU at 80–100 % with the same brief per-job dips. Per the PR2 stop
  condition (`04 §3`, `docs/0.3.1.md`): **STOP — reported to the user with options instead
  of fixing a different regression under this PR.** No fix code written.
- **Skipped / deferred:** Phase B not entered (stop condition). The instrumentation commit
  stands on its own (privacy-safe timing logs + stale JPEG-era doc-comment fixes).
- **Hallucinated / corrected:** the initial profiling runs captured zero frames — the
  profiling DB had persisted `capture.event_driven_enabled=true` (and the diff gate blocks a
  static screen; user pointed at both). Fixed by seeding timer mode + keeping the animated
  page visible. Also `models.vision_tier` must be seeded as the JSON string `"quality"`, not
  the bare word (parse falls back to Default and logs `unparsable tier`).
- **Still risky:** single-machine, single-GPU (NVIDIA/Vulkan) evidence; the 2.45× complete_ms
  ratio vs the 1.50× pixel ratio is consistent with attention-dominated vision encoding but
  was not decomposed further (would need llama-server-side timing).

### Pass 3 addendum — Phase B (fix) + Phase C (verification), same day

- **User decision on `06` #22: option (a)** — cap the VLM tag request at 1280 px.
  Implemented as `VISION_MAX_EDGE` 1568 → 1280 (`crates/inference/src/vision.rs`, commit
  `13d619e`), pinned dimension tests deliberately updated (3440×1440 → 1280×536; portrait
  536×1280), stale 1568 references in `models.rs` docs updated. Stored captures untouched
  (still native-res lossless WebP); zero schema change; no new setting; no UI.
- **Phase C re-measure (fixed tree, release, same protocol):** three windows.
  1. *Same frame population as the pre-fix A2 run* (native-WebP re-tags, busy screen content,
     capture + embed jobs live): **33 → ~72 done/min** (2.2×); per-job total median
     3061 → 1683 ms; complete_ms 2984 → 1624 ms at 1280×536.
  2. *Like-for-like vs the v0.2.1 baseline* — the baseline's own 72 stored 1280×536 JPEG
     frames were imported into the fixed tree's data dir (same files, same content) and
     drained with capture off: **89,91,88,93,96,94,96,87,89 done/min (avg 91.4) vs the
     baseline's 89.4 — 102 % of the pre-WebP baseline; acceptance (±10 %) met.** Per-job
     total median **1173 ms vs 1234 ms** (fixed tree marginally faster on identical jobs);
     complete_ms median 1158 vs 1217.
  3. *GPU shape* (nvidia-smi, 1 s): fixed tree median 93 %, dips <10 % in 9–12 % of samples
     (capture-on window included) — the same steady-under-load character as the v0.2.1
     baseline (median 94 %, 13.2 % dips). No per-frame spike/idle sawtooth.
  - The residual gap in window 1 (72 vs 89/min) is **content + pool competition, not the
    request size**: on identical frames (window 2) the trees are at parity; the busier A2-era
    frame population produces longer model generations (complete_ms 1624 vs 1158 at identical
    request dims), and 172 `embed_text` jobs shared the 2-worker pool during window 1 while
    the baseline ran nearly embed-free.
- **Verification:** full suite green post-fix (verbatim in `08`); `cargo test -p inference`
  105 passed with the updated pins; like-for-like numbers quoted in the PR description.

---

## Pass 4 — 2026-07-04 — 0.3.1 PR3: polish bundle (#59 + #65 + #57 version link)

- **Implemented:** the three PR3 polish items (`docs/0.3.1.md` PR3; D1–D4), branch
  `feat/0.3.1-pr3-polish-bundle` (worktree `ss-v031-pr3-polish`). No schema change, no new
  settings surface, no ts-rs binding change.
  - **#59 (D1):** removed `max-h-80 overflow-auto` from both `<pre>` blocks in
    `MomentDetail.tsx` — recognized + raw text grow inline; one page scroll context.
  - **#65 (D2):** `save_report_markdown` Tauri command (Downloads, `.partial`-rename write,
    deterministic `-2`/`-3` collision suffix via `unique_markdown_path`; stem sanitized by
    `sanitize_report_stem`) + UI stem `screensearch-report-YYYY-MM-DD-HHmm` (`reportFileStem`).
    Two pure helpers unit-tested against a temp dir.
  - **#65 (D3):** `buildReportFooter` (single source) — app version · model · covered dates ·
    filters · counts — rendered on screen AND appended to the copied/saved markdown.
    `useReport` retains the submitted `ReportRequest` for the footer's span/filters.
  - **#57 partial (D4):** `useAppVersion()` + NavRail `v{version}` link.
  - **Verbatim gates:** `npm run lint` no errors/warnings; `npm run build` `✓ built`;
    `cargo fmt --all -- --check` exit 0; `cargo clippy --workspace --all-targets -- -D
    warnings` `Finished … in 31.07s`; `cargo build --workspace` `Finished … in 48.56s`;
    `cargo test --workspace` all green (`screensearch_lib` `14 passed`, incl. the 2 new
    helper tests); `git diff --exit-code -- ui/src/bindings` clean.
- **Hallucinated / corrected:**
  - The roadmap's `06 §5` "0.3.1 PR3" text referenced `local_api.rs:432–435` as the
    `download_dir()` precedent; the file is `src-tauri/src/local_api.rs` (not the `crates/api`
    path some notes imply) and the precedent is `export_data`. Followed the actual code.
  - D4 open-mechanism resolved by the roadmap's own **live-test-first** procedure: a plain
    `<a target="_blank">` was live-tested in `npm run tauri dev` and **did not open the OS
    browser** (maintainer-confirmed, 2026-07-04). Added `tauri-plugin-opener` and routed all
    external opens (version link + the two pre-existing broken report/answer markdown links)
    through `openUrl()`. Capability scope broadened from repo-only to `http://*` + `https://*`
    because the markdown links open arbitrary model-cited URLs (logged in `08` + `06` #24 is
    the separate tokens-count fix).
  - Spec contradiction (D3): `UI_REFERENCE §4/§5` "tokens count" → "counts (passes · periods
    covered · frames summarized)"; logged `06` #24.
- **Skipped / deferred:** everything else in #57 (load/unload-model, start/stop-vision quick
  actions) stays deferred to 0.3.2 (`07` #97), unchanged by this PR.
- **Still risky:** the opener capability allows any `http(s)` URL from the main + overlay
  windows. These are user-initiated clicks on rendered links that open in the **external OS
  browser** (never the app WebView), which is the intended, sandboxed behavior — but worth a
  glance at PR4 audit time.
- **Live verification:** post-fix `npm run tauri dev` re-check of the version link (opener
  plugin) + Moment scroll + report save/footer — see the PR description / manual acceptance in
  `docs/TESTING.md` (0.3.1 PR3).

---

## Pass 5 — 2026-07-05 — 0.3.1 PR4 (audit + tag `v0.3.1`)

The 0.3.1 closing audit per `docs/0.3.1.md` PR4, same shape as 0.3.0's PR9: full mandatory
re-read (`04 §1`), D1–D9 landing verification, `07` deferral coverage, release doc sweep,
version bump, CHANGELOG cut + archive fold, release notes. Branch
`chore/0.3.1-pr4-audit-release`.

- **Implemented — D1–D9 audit, all PASS.** Each decision verified evidence-first against `main`
  (66e0b45 = merged PR #86), then independently re-checked by an adversarial second pass
  explicitly prompted to refute it; **no refutation stood**.
  - **D1 (#59):** neither Moment text region nor any ancestor (`MomentDetail.tsx`, the `details`
    disclosure, `Panel`, the `Moment.tsx` route wrapper) carries `max-h-*`/`overflow-*` classes
    or equivalent inline styles; the page is one scroll context. Contract present in
    `UI_REFERENCE §4/§5`.
  - **D2 (#65):** `reportFileStem` (`ui/src/lib/time.ts`) builds
    `screensearch-report-YYYY-MM-DD-HHmm` from **local-time** accessors (`getFullYear`/…, no UTC
    methods); `sanitize_report_stem` + `unique_markdown_path` (`src-tauri/src/lib.rs`) append
    `-2`/`-3` on same-minute collision and write via `.partial` rename; `ReportView` routes the
    Download button through the command (Blob fallback outside Tauri); both pure helpers
    unit-tested.
  - **D3 (#65):** `buildReportFooter` (`ui/src/lib/reportFooter.ts`) emits app version · model
    id · time span (exclusive-end corrected) · filters (kind + optional Custom focus) · the
    coverage counts; rendered on screen **and** appended to Copy + saved markdown; no footer on
    no-evidence reports; no new settings key.
  - **D4 (#57-partial):** NavRail `v{version}` → exact repo URL (matches `git remote`), a native
    anchor (default tab stop; Enter fires click; the roving `tabIndex` applies only to the five
    nav items); hidden outside Tauri (`useAppVersion` → null); opens via
    `tauri-plugin-opener::openUrl` — external OS browser, never the WebView. No quick-action UI
    landed anywhere in the arc's additions.
  - **D5/D9 (#64):** `05` Pass 3's numbers table + explicit STOP record precede the Phase B/C
    fix addendum structurally; `06` #22 records the roadmap contradiction + the user decision
    (option a) and #23 the PDH-blind-to-Vulkan finding; PR #85's description quotes the
    before/after (89.4 → 91.4 done/min, 102 % of baseline); `VISION_MAX_EDGE = 1280` with the
    rationale doc-comment and pinned 1280×536 dimension tests.
  - **D6/D7 (§2 disposition coverage):** `07` #96–#99 all present with owner + date — #96
    carries the hard "auto-update MUST land before 0.4.0 ships" sequencing note; #97 cites D6
    (pull-based, non-shaming) as binding on 0.3.2 design; #98 records the #54 fold-forward
    (D7); #99 the resolved quick-menu silence. GitHub hygiene verified via `gh`: #54 CLOSED
    with the fold-forward comment; #56/#69 OPEN + `deferred-0.3.2`; #57 OPEN with the split
    comment. No §2 deferral is missing from `07`.
  - **D8 (hard constraint):** `git diff v0.3.0..HEAD -- crates/store` is empty;
    `LATEST_SCHEMA_VERSION = 10` on both sides; no migration/`schema_version` change anywhere
    in the release diff; the three patch PRs (#82 specs-only / #85 / #86) added **no** settings
    key, **no** crate, **no** UI settings surface. (Of the two post-0.3.0 defect fixes riding
    this release: #79 added none; #80 added `overlay.hotkey_migrated` — an internal one-shot
    load-path migration latch, not user-facing settings surface.)
- **Opener capability glance (Pass 4's "still risky" item) — reviewed, accepted.**
  `src-tauri/capabilities/{default,overlay}.json` grant exactly `opener:allow-open-url` scoped
  `http://*` + `https://*` (plus `core:default`); there is **no** `open-path` and no shell
  scope, so the surface is user-initiated link clicks opening in the sandboxed OS browser. The
  breadth (any http(s) host, not repo-only) is deliberate and recorded (`08` PR3 entry): report/
  answer markdown links open arbitrary model-cited URLs.
- **Riding-fix risk closure (corrected per the audit's own completeness pass):** PR #79's
  live-desktop `#[ignore]`d UIA tests **are** recorded on the PR (`cargo test -p uia --
  --ignored` → 3 passed, incl. `uia_worker_exits_on_shutdown`, and the merge commit repeats
  it), but the **full `tauri dev` hang-recovery walkthrough that Pass 1 declared merge-gating
  is not recorded anywhere** — carried below as an accepted residual, not asserted. PR #80's
  PR body records the full verification suite only; the "live hotkey walkthrough" its `08`
  entry claims is **not evidenced on the PR** — the live register/summon check remains
  unrecorded (low risk: the chord registers a key, not a character, and the D6 failure path
  is loud). The AZERTY/AltGr confirmation (Pass 2a) is **removed from the checklists by user
  decision (2026-07-05, PR4 review): it cannot be tested on this setup**; the low-risk
  rationale above stands as the record.
- **Verification suite (verbatim, release tree, run this session):** UI `npm ci` → `found 0
  vulnerabilities`; `npm run lint` → clean (no output); `npm run build` → `✓ built in 1.76s`;
  `node scripts/stage-mcp.mjs` → `up to date`; `cargo fmt --all -- --check` → exit 0;
  `cargo clippy --workspace --all-targets -- -D warnings` → ``Finished `dev` profile … in 31.94s``
  (no warnings); `cargo build --workspace` → `Finished … in 42.07s`; `cargo test --workspace` →
  every suite `test result: ok`, **0 failed** (8 ignored = the live-session/GPU-gated ones);
  `git diff --exit-code -- ui/src/bindings` → clean (exit 0).
- **Installer:** `npm run build` (tauri build, release) → NSIS bundle
  `ScreenSearch_0.3.1_x64-setup.exe` built from the bumped tree (verbatim tail in `08`).
- **Skipped / deferred:** the `v0.3.1` tag + GitHub release are **prepared but not executed** —
  they follow the maintainer's merge + explicit approval (no-merge/no-tag-without-approval).
  Release notes (PR body) state plainly that auto-update (#69) is not in this release — one
  more manual download; #59/#65 close with this PR.
- **Hallucinated / corrected:** the first draft of this pass asserted "PR #80 recorded the
  full suite + live hotkey walkthrough" (repeating the `08` entry's claim) — the audit's own
  completeness pass checked the PR body and found **no recorded walkthrough**; corrected above
  rather than shipped. Evidence-taxonomy note on D3/D4: the headline PR3 flows (no nested
  scrollbar; dated filename + `-2` collision + footer on screen and in the saved file; version
  link opens the OS browser) were live-verified pre-merge on PR #86; three sub-items —
  Copy carries the footer, a no-evidence range renders no footer, keyboard Enter-activation of
  the version link — are **code-verified only** (static analysis of `ReportView.tsx` /
  `NavRail.tsx`), not observed live in this audit.
- **New issue surfaced during the audit, then fixed in this PR:** **#84 "Bug when quiting the
  app"** (maintainer-filed 2026-07-04, *after* the `docs/0.3.1.md §2` disposition table
  froze): quit doesn't stop capture before the worker/sidecar drain, so an in-flight VLM
  call/download can keep persisting screenshots during shutdown. Two PR4-review Codex findings
  pinned it: a P1 (the `RunEvent::ExitRequested` handler in `src-tauri/src/lib.rs` never called
  `stop_capture()`), then a P2 (the first fix placed `stop_capture()` *after* the local-API
  graceful shutdown, which can wait up to 3 s draining an open `/v1/ask` SSE, leaving capture
  live during that window). **Fixed in PR4 (2026-07-05, maintainer decision to fix rather than
  defer):** the handler now calls `kernel.stop_capture().await` **first of all**, before the
  local-API graceful shutdown and before the throttle/vision-scheduler/worker drain, so no new
  frames are captured or persisted once quit begins in any config (API on or off). The change
  is the sole code edit in this otherwise docs-only closing PR; it touches no schema, no
  settings key, no crate, no UI surface, so **D8 still holds**. Full verification suite re-run
  green with the fix in (below). Recorded as `07` #101. The separate "unbounded worker-shutdown
  wait" facet is unchanged (bounded by the Job-Object sidecar kill + startup stale-job requeue,
  `03 §6`), tracked for the 0.3.2 lifecycle arc. Closing the GitHub issue is the maintainer's
  follow-up.
- **Still risky:** (a) the PR #79 `tauri dev` hang-recovery walkthrough (Pass 1's merge gate)
  and the PR #80 live hotkey register/summon check are **not recorded** — accepted residuals,
  above (the AZERTY confirmation is waived by user decision, not carried);
  (b) standing rows stay honest: `06` #15/#23 (upstream leak; PDH blind to Vulkan), `07` #91
  (monitor hot-unplug), #58 (multi-DPI live check), and the `07` manual code-signing step
  (unsigned installer; SmartScreen warns). Release-asset provenance: the audited installer was
  built from this branch's tree — after merge + tag approval, rebuild at the tagged commit (or
  show `git diff <tag> <build-commit>` empty) and re-run the 7z `screensearch-mcp` inclusion
  check before attaching.

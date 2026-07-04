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

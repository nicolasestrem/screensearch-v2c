# 08 — AI Changelog — archived v0.3.1 history (P7.1 triage patch)

> Archived on the v0.3.1 release sweep (2026-07-05, 0.3.1 PR4); entries preserved verbatim from
> the live `specs/08_CHANGELOG_AI.md` (newest first): PR4 audit + release sweep; PR3 polish
> bundle; PR2 Phase A/B/C + review round; the post-0.3.0 bridge fixes (UIA lifecycle, overlay
> hotkey); PR1 specs contract. Earlier history → the v0.1.0 / v0.2.x / v0.3.0 archives.

---

## 2026-07-05 — 0.3.1 PR4: audit + release sweep (v0.3.1)

- **Change:** the 0.3.1 closing PR on `chore/0.3.1-pr4-audit-release` (docs + version bump only;
  no code, no schema, no settings). (a) **Audit:** D1–D9 verified landed against `main`
  (evidence-first, each check adversarially re-verified; all PASS — full record in `05` Pass 5);
  `07` §2-deferral coverage confirmed (#96–#99); the Pass 4 opener-capability "still risky" item
  reviewed + accepted; GitHub hygiene confirmed via `gh` (#54 closed w/ fold-forward, #56/#69
  `deferred-0.3.2`, #57 split comment). (b) **Doc sweep:** README status → v0.3.1;
  `docs/ARCHITECTURE.md` header → 0.3.1; `CLAUDE.md`/`AGENTS.md` current-state → patch complete,
  0.3.2/0.4.0 next-up; `02 §8` status → 0.3.0 + 0.3.1 shipped; `docs/API.md` `/v1/health`
  example → 0.3.1 (the endpoint returns `CARGO_PKG_VERSION`). (c) **Version bump:** 0.3.0 →
  0.3.1 in `Cargo.toml` (workspace), `package.json`, `ui/package.json`,
  `src-tauri/tauri.conf.json` + both npm locks + `Cargo.lock`. (d) **CHANGELOG:** `[Unreleased]`
  → `[0.3.1] — 2026-07-05` (lead: the #64 fix is the reason the patch exists; the auto-update
  manual-download caveat; zero schema changes) + an Audited section; then archived per `04 §7`.
  (e) **Archive fold:** shipped 0.3.1 entries of `05`/`06`/`07`/`08` →
  `specs/archive/*.v0.3.1.md` (ids preserved); `CHANGELOG.md` `[0.3.1]` → `CHANGELOG-ARCHIVE.md`;
  live logs keep only open rows (`06` #15/#23; `07` unchanged-open rows + a new #100 standing
  row for the TODO-3 cross-chord check that #94's archival would otherwise bury, + a new #101
  row for issue #84 — the quit-path bug filed after the §2 disposition table froze; **resolved
  by the maintainer at PR4 review, 2026-07-05**, and archived with #94/#99). `06` #21 status
  refreshed (PR #79 merged; ships in v0.3.1).
  Pass 5's evidence taxonomy was corrected by the audit's own completeness pass before
  shipping: the PR #79 `tauri dev` hang-recovery walkthrough and the PR #80 live hotkey
  register/summon check are **not recorded** on their PRs and are carried as accepted
  residuals instead of asserted; the AZERTY/AltGr confirmation was **removed from the
  checklists by user decision** (2026-07-05 — not testable on this setup); the duplicate
  "Pass 2" headings were renamed Pass 2a/2b at archive time so "05 Pass N" citations resolve
  uniquely.
- **Why:** `docs/0.3.1.md` §3 PR4 — same shape as 0.3.0's PR9 (`04 §7` archive-on-release). The
  tag `v0.3.1` + GitHub release are prepared but await maintainer merge + explicit approval.
- **Verification (verbatim, this session, on the release tree):**
  - UI: `npm ci` → `found 0 vulnerabilities` · `npm run lint` → clean · `npm run build` →
    `✓ built in 1.76s`.
  - `node scripts/stage-mcp.mjs` → `up to date`.
  - `cargo fmt --all -- --check` → exit 0 · `cargo clippy --workspace --all-targets -- -D
    warnings` → `Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.94s` (no
    warnings) · `cargo build --workspace` → `Finished … in 42.07s` · `cargo test --workspace` →
    every suite `test result: ok`, 0 failed (8 ignored: live-session/GPU-gated) ·
    `git diff --exit-code -- ui/src/bindings` → clean (exit 0).
  - Installer: `npm run build` (tauri build) → exit 0, `Finished 1 bundle at:
    …\target\release\bundle\nsis\ScreenSearch_0.3.1_x64-setup.exe`; `7z l` on the installer
    lists `screensearch.exe` **and** `screensearch-mcp.exe` (both rebuilt at 0.3.1).

---

## 2026-07-04 — 0.3.1 PR3: polish bundle (#59 nested scroll · #65 report filename+footer · #57 version link)

- **Change:** three user-visible polish items on `feat/0.3.1-pr3-polish-bundle`, no schema
  change, no new settings surface (`docs/0.3.1.md` PR3; D1/D2/D3/D4).
  - **#59 (D1) — Moment nested scrollbar removed.** Dropped `max-h-80 overflow-auto` from
    both the recognized-text and raw-text `<pre>` blocks in
    `ui/src/components/domain/MomentDetail.tsx`; the text now grows inline and the page's
    single scroll context (`AppShell` `<main>`) owns scrolling.
  - **#65 (D2) — date-stamped, collision-safe report filename.** New Tauri command
    `save_report_markdown(stem, markdown) -> path` in `src-tauri/src/lib.rs`: resolves
    `download_dir()` (same path as `export_data`), sanitizes the stem to a safe leaf, picks
    the first free `<stem>.md` / `<stem>-2.md` / `<stem>-3.md` (`unique_markdown_path`), and
    writes through a `.partial` rename. Two pure helpers (`sanitize_report_stem`,
    `unique_markdown_path`) with unit tests (temp dir). Plain `String` args → **no ts-rs
    binding churn**. UI (`ReportView.tsx`) builds the local-time stem
    `screensearch-report-YYYY-MM-DD-HHmm` (`reportFileStem` in `lib/time.ts`), invokes the
    command, toasts the returned path; keeps a Blob download as the browser-dev fallback
    (guarded by `isTauri()`).
  - **#65 (D3) — self-describing report footer.** `buildReportFooter(report, request,
    appVersion)` (`ui/src/lib/reportFooter.ts`) emits one plain-text block: app version ·
    model · covered date(s) · filters (kind + optional Custom focus) · counts (passes ·
    periods · frames summarized) · truncated note. Single source: rendered on screen AND
    appended (after a `---`) to the copied/saved markdown, so the exported file carries its
    own provenance (previously it had none). `useReport` now retains the submitted
    `ReportRequest` in state (the footer's time-span + filter source); `ReportResponse` is
    unchanged (bindings stay clean).
  - **#57 partial (D4) — NavRail version link.** `useAppVersion()` (`getVersion()` from
    `@tauri-apps/api/app`, null + hidden UI outside Tauri) feeds a quiet `v{version}` footer
    link in the NavRail. **Open-mechanism decision (per the roadmap's live-test procedure):**
    a plain `<a target="_blank">` was live-tested in `npm run tauri dev` and **did not open
    the OS browser** (confirmed by the maintainer, 2026-07-04) — Tauri v2 ignores such
    anchors. So this PR adds **`tauri-plugin-opener`** (`+ @tauri-apps/plugin-opener`,
    `.plugin(tauri_plugin_opener::init())`, and an `opener:allow-open-url` capability on both
    the `main` and `overlay` windows) and routes external opens through `openUrl()` via a
    shared `openExternal()` helper. Per the roadmap, the two **pre-existing broken** markdown
    links (`ReportView.tsx`, `AnswerStream.tsx` — model-output links that also relied on
    `target="_blank"`) were switched to the same helper.
  - **Review follow-up (link interception hardened).** The first cut of the two markdown
    renderers called `e.preventDefault()` + `openExternal()` **unconditionally** — which
    killed links in browser-dev (no Tauri runtime → silent no-op) and any non-http(s) link
    (`mailto:`, in-page anchors) the `opener` scope would block. Added
    `handleExternalLinkClick(e, href)` to `openExternal.ts`: it intercepts **only** http(s)
    links **only** when `isTauri()`; every other case falls through to the restored native
    `target="_blank" rel="noopener noreferrer"` so those links still work. NavRail is
    untouched — its link is a fixed https URL rendered only when `version` is truthy (Tauri
    only), so the guard is moot there.
- **Why:** `docs/0.3.1.md` PR3 + D1–D4; `04 §5` (spec silence/contradiction → stop + log).
  - **Capability scope note (deviation, logged):** the roadmap suggested scoping
    `opener:allow-open-url` to the repo URL. Because the report/answer markdown links open
    **arbitrary model-cited URLs**, a repo-only scope would leave them broken (permission
    denied) — defeating the roadmap's own "switch the two links too" instruction. Scope is
    therefore `http://*` + `https://*` (the repo URL is a subset). External opens land in the
    OS browser (sandboxed), never the app WebView; the URLs are user-initiated clicks on
    rendered links. This is a mechanism detail, not a product decision.
  - **Spec contradiction resolved (D3):** `UI_REFERENCE §4/§5` said the footer keeps "the
    existing tokens count"; the footer has never had a token count. Corrected both
    occurrences to "the existing counts (passes · periods covered · frames summarized)" and
    logged as `06` #24.
- **Verification (verbatim, worktree `ss-v031-pr3-polish`):**
  - UI: `npm ci` clean; `npm run lint` → **no errors/warnings**; `npm run build` → `✓ built`.
  - `node scripts/stage-mcp.mjs` → staged.
  - `cargo fmt --all -- --check` → exit 0.
  - `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile … in
    31.07s` (no warnings).
  - `cargo build --workspace` → `Finished` in 48.56s.
  - `cargo test --workspace` → all green; `screensearch_lib` unit suite `14 passed` incl.
    the new `sanitize_report_stem_produces_a_safe_leaf_name` +
    `unique_markdown_path_appends_2_3_on_collision`.
  - `git diff --exit-code -- ui/src/bindings` → clean (exit 0).
  - Live (`npm run tauri dev`): version link live-tested (see D4 above — plain anchor failed,
    driving the opener-plugin decision); post-fix live re-check recorded in `05`.

---

## 2026-07-04 — 0.3.1 PR2 Phase A: #64 profiling instrumentation + stop-condition report

- **Change:** (a) Instrumentation-only commit on `fix/0.3.1-pr2-vision-throughput-r2`
  (`dbb1789`): per-job `decode_ms`/`analyze_ms` + source dimensions in
  `crates/kernel/src/worker_pool.rs::vision_tag_outcome`; `acquire_ms`/`prep_ms`/`complete_ms`
  + request dimensions in `crates/inference/src/vision.rs::analyze` (new `vlm_request_dims`
  shared with `downscale_for_vlm` + pinning test); `encode_ms` + stored bytes in
  `crates/kernel/src/capture_loop.rs::process_frame` (`write_webp` now returns the file size);
  stale JPEG-era doc comments fixed. All privacy-safe (durations/dimensions/bytes/paths only).
  (b) Phase A profiling record in `05` Pass 3; contradiction + PDH finding as `06` #22/#23.
  **No fix code** — the PR2 stop condition triggered.
- **Why:** `docs/0.3.1.md` PR2 Phase A + D9 (`04 §3`): numbers before fix code. Release-build
  measurement (current tree vs the v0.2.1 pre-WebP tag, same Qwen3-VL-8B + llama-server)
  attributes ~97 % of the ~2.5× per-job regression to the sidecar processing the 1.5×-pixel
  VLM request (native-res storage → 1568 px cap vs the old 1280 px stored width); the encode
  step (D5's target) measures 26 ms and got *cheaper* than v0.2.1's 42 ms. Stop condition:
  the slowdown is not on the encode path → reported with options instead of fixing under
  this PR (`06` #22).
- **Verification:** `cargo fmt --all -- --check` clean; `cargo clippy -p kernel -p inference
  --all-targets -- -D warnings` → `Finished dev profile ... in 20.11s` (no warnings);
  `cargo test -p kernel -p inference` → all green (`105 passed` inference incl. new
  `vlm_request_dims_match_encoded_output`; kernel suites all `ok`). Profiling evidence:
  `05` Pass 3 tables (1263 baseline jobs / 509 current-tree jobs parsed from the timing logs).

---

## 2026-07-04 — 0.3.1 PR2 Phase B/C: VISION_MAX_EDGE 1568→1280 (#64 fix) + verification

- **Change:** `crates/inference/src/vision.rs`: `VISION_MAX_EDGE` 1568 → 1280 (user decision
  on `06` #22, option a) with the rationale in the const doc; pinned dimension tests updated
  deliberately (`downscales_oversized_frame_to_max_edge` → 1280×536,
  `vlm_request_dims_match_encoded_output` incl. portrait). `crates/inference/src/models.rs`:
  stale 1568 doc reference updated. `CHANGELOG.md` `### Fixed` entry. `05` Pass 3 addendum
  (Phase B/C record), `06` #22 → resolved.
- **Why:** `06` #22 / `05` Pass 3 — restore VLM-request parity with the pre-WebP baseline;
  keeps WebP + native-res storage, zero schema (D8), no settings/UI surface.
- **Verification (verbatim):** like-for-like drain of the baseline's own 1280×536 JPEG frames
  on the fixed tree: per-minute `89,91,88,93,96,94,96,87,89` (avg **91.4/min**) vs the v0.2.1
  baseline **89.4/min** — within the ±10 % acceptance; per-job `total_ms` median **1173** vs
  baseline **1234**; GPU (nvidia-smi) median 93 % both trees, no sawtooth. Full suite:
  `cargo fmt --all -- --check` → exit 0 · `cargo clippy --workspace --all-targets -- -D
  warnings` → `Finished dev profile ... in 6.49s` (clean) · `cargo build --workspace` →
  `Finished dev profile ... in 22.65s` · `cargo test --workspace` → every suite
  `test result: ok` (0 failed; incl. `inference` 105 passed) · `ui npm run lint` → clean ·
  `npm run build` → `✓ built in 2.17s` · `git diff --exit-code -- ui/src/bindings` → exit 0.

---

## 2026-07-04 — 0.3.1 PR2 review round: comment hygiene + zero-dim guard (PR #85)

- **Change:** (a) The three timing-log code comments no longer narrate the task
  (`#64` / `0.3.1 PR2` / `D9` references dropped from `vision.rs` analyze timing,
  `capture_loop.rs` encode timing, `worker_pool.rs` decode/analyze timing); the substantive
  content (what is measured + the privacy contract) is kept. The `VISION_MAX_EDGE` const
  doc and pinned-test rationale keep their spec citations — they record the *decision*, per
  the codebase convention. (b) `vlm_request_dims` gains a zero-width/zero-height
  pass-through guard (a scaled zero-area source would hand `image::imageops::resize` an
  empty buffer, which can panic) + two test assertions. Unreachable from real decoders
  (WebP/JPEG/PNG can't be 0-dim), so purely defensive — no behavior change for real frames.
- **Why:** PR #85 review comments — the comment-hygiene rule (comments must not reference
  the current task; it rots) and a defensive-input finding on the new pure helper.
- **Round 2 (same day):** (c) `capture_loop.rs::write_webp`: the post-write
  `fs::metadata` byte-count read is now non-fatal (`unwrap_or(0)`) — the count is
  observational (timing log only), and a transient AV/ACL failure after a successful write
  must not propagate out of `process_frame` and silently drop an on-disk frame from the
  DB. (d) Remaining task-ID references trimmed (`VISION_MAX_EDGE` const doc, `models.rs`
  `default_ctx_for` doc, two test comments): GitHub-issue/arc labels dropped; the
  regression rationale + the durable `06` #22 spec-row citation kept (codebase convention).
- **Verification (verbatim):** `cargo fmt --all -- --check` → clean; `cargo clippy -p
  inference -p kernel --all-targets -- -D warnings` → `Finished dev profile [unoptimized +
  debuginfo] target(s)` (no warnings, both rounds); `cargo test -p inference -p kernel` →
  every suite `test result: ok` (0 failed; `inference` `105 passed` incl. the extended
  `vlm_request_dims_match_encoded_output`).

---

## 2026-07-04 — UIA client lifecycle teardown + per-app circuit breaker (hang fix)

- **Change:** Fixed the shipped 0.3.0 defect where UI Automation left Chromium/Electron apps
  hung, persisting after the user disabled capture. (a) `crates/uia/src/worker.rs`: walk reply
  now carries `over_budget`; `worker_main` times the whole walk and owns the rate-limited
  over-budget warn; per-request `cancel` + provider `shutdown` flags checked at both deadline
  checkpoints; worker-exit signal via a dropped `mpsc::Sender`. (b) `crates/uia/src/lib.rs`:
  `UiaTextProvider` gains `shutdown()` / `take_exit_signal()` / `recognize_detailed()`
  (returns a `WalkEnd` for the breaker). (c) New pure `crates/uia/src/breaker.rs`: per-app
  `AppBreaker` (3 consecutive over-budget/timed-out walks → 30-min OCR cooldown; good walk
  resets), fully CI-unit-tested. (d) `crates/traits/src/contracts.rs`: additive
  `OcrProvider::on_capture_stopped()` default-no-op. (e) `crates/kernel/src/lib.rs`:
  `stop_capture` calls it (once, only when a handle existed). (f) `src-tauri/src/lib.rs`:
  `UiaWithOcrFallback` reworked to hold the client in a teardown-able slot with lazy respawn,
  a `UiaRuntimeConfig` snapshot, and the breaker; `apply_settings` swaps config + tears down on
  change (every UIA knob now hot-applies); `spawn_ocr` returns the concrete composite; removed
  the `AppState.uia_enabled` AtomicBool. (g) `ui/src/routes/Settings.tsx`: seven UIA hints
  `APPLY_RESTART`→`APPLY_NOW`; breaker note added to the interactive-walk warning.
- **Why:** `03 §2/§3` (composition-root wiring; provider contracts) and the `07`/#48 UIA hang
  lineage. The 0.3.0 client was never released (held in `Kernel.ocr` for the process lifetime),
  so Chromium kept its accessibility engine on and the hangs outlived "disable capture". User
  decision: breaker-only (no Chromium window-class skip), constants not settings (`07` #92/#93).
- **Verification:** `cargo test -p uia --lib` → `26 passed; 0 failed; 3 ignored`;
  `cargo test -p kernel` green (new `stop_capture_notifies_ocr_provider`); `npm run lint` clean;
  `npm run build` exit 0; `cargo build -p screensearch` → `Finished ... in 20.84s`. Full
  workspace `fmt`/`clippy`/`test` + live `npm run tauri dev` walkthrough recorded on the PR.

---

## 2026-07-04 — Flow overlay default hotkey → Ctrl+Alt+Z (+ one-shot remap)

- **Change:** Default Flow overlay chord changed `Ctrl+Alt+Space` → `Ctrl+Alt+Z` in the three
  sources of truth (`crates/traits/src/ipc.rs`, `src-tauri/src/overlay.rs`,
  `ui/src/components/domain/HotkeyField.tsx`), Settings hint text updated
  (`ui/src/routes/Settings.tsx`), and a load-path one-shot migration
  `kernel::settings::load_overlay_hotkey` that remaps a stored exact `Ctrl+Alt+Space` to the new
  default, leaving custom chords untouched. The migration is gated by a persisted marker
  (`overlay.hotkey_migrated`) so it runs at most once per install — a later deliberate
  `Ctrl+Alt+Space` is then honored verbatim — and the marker is latched **only after** the remap
  rewrite succeeds, so a failed rewrite is retried on the next load instead of being abandoned.
  Living source-of-truth docs updated to the new default (`specs/03`, `specs/02`,
  `specs/UI_REFERENCE.md`, `README.md`, `docs/TESTING.md`, `docs/ARCHITECTURE.md`); the shipped
  `docs/0.3.0.md` arc record keeps `Ctrl+Alt+Space` (0.3.0 shipped it; this is a post-0.3.0 fix).
- **Why:** The old default collided with Claude Desktop's global quick-entry shortcut (`03 §8`
  hotkey config). The remap lives in the load path, not the startup sweep, for the same reason as
  `load_tier` (the composition root registers the chord straight from `load_settings`' output).
  Marker gating + latch-on-success come from PR #80 review (codex/claude P2): a value-only remap
  re-fired every load (breaking the reversible escape hatch), and an unconditional marker latched
  a failed rewrite forever (`07` #94).
- **Verification:** `cargo test -p kernel --test settings` (new remap + durability + failed-retry
  tests; updated persisted-value assertions derive the expected from `Settings::default()`);
  `npm run lint`/`build` clean; full `fmt`/`clippy`/`build`/`test` + a live hotkey walkthrough
  recorded on the PR.

---

## 2026-07-04 — 0.3.1 PR1: specs contract (P7.1 triage patch; specs-only)

- **Change:** Normalized the 0.3.1 roadmap (`docs/0.3.1.md`, decisions D1–D9) into the specs —
  no code, no schema, no UI. (a) `specs/04`: `docs/0.3.1.md` added to the §1 mandatory reading
  order (with the hard patch constraint: no new subsystems / schema migrations / settings
  surface, D8); a 0.3.1 row in the §2 source-of-truth table; a §3 build-order bullet for
  PR1→PR4 encoding PR2's profile-first mandate + stop condition (D9), the fix preference order
  (D5), and zero-schema-changes (D8). (b) `specs/07`: rows #96 (#69 auto-update → 0.3.2,
  **hard-sequenced before 0.4.0 ships**), #97 (#56 systray + #57 quick actions → the 0.3.2
  lifecycle mini-arc, bound by D6 pull-based/non-shaming), #98 (#54 closed → folded into the
  0.4.0 sessions arc, D7), and #99 (resolved spec silence: the "quick menu" of D4 = the
  **NavRail footer**, user decision 2026-07-04). (c) `specs/UI_REFERENCE.md`: §3 NavRail
  version-footer prose block + tree line (D4); §4 Moment row — recognized-text and raw-text
  grow inline, no nested scrollbar, one scroll context (D1); §4 Recall-reports row + §5
  `ReportView` — dated download filename `screensearch-report-YYYY-MM-DD-HHmm.md` local time
  with `-2`/`-3` collision suffixes (D2) and the footer contract app version · model id · time
  span · filters (D3); §5 `NavRail`/`MomentDetail` notes. (d) `CLAUDE.md`/`AGENTS.md`: the
  "Current state" paragraph names the active 0.3.1 arc. (e) `CHANGELOG.md`: Docs entry under
  `[Unreleased]`.
- **Why:** `docs/0.3.1.md` §3 PR1 — every later PR must be implementable from the specs alone
  without reopening the roadmap, per the arc's established operating model (`04 §1/§2`). The
  quick-menu surface was a genuine spec silence (no spec/component uses issue #57's term), so it
  was resolved with the user per `04 §5` and recorded in `07` #99.
- **Verification:** `git diff --name-only main` → only `.md` files (verbatim list on the PR);
  D1–D9 landing checklist grep output pasted on the PR. No build/test impact possible (docs
  only); CI runs the full suite on the PR regardless.

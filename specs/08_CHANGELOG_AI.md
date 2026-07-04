# 08 — AI Changelog

> Append-only record of what the agent changed during the build, **with reasons**. One entry per
> meaningful change set. Empty until P0 begins. (This tracks build work; the design-phase history
> lives in git.)

## <date> — <short title>
- **Change:** what was added/modified.
- **Why:** the reason, tied to a spec section.
- **Verification:** the command run + verbatim result.

---

> Pre-0.2.x (v0.1.0) history → `specs/archive/08_CHANGELOG_AI.v0.1.0.md`.
> Shipped 0.2.x history (0.2.0–0.2.2) → `specs/archive/08_CHANGELOG_AI.v0.2.x.md`.
> Shipped 0.3.0 history (PR1–PR9 + bridge fixes) → `specs/archive/08_CHANGELOG_AI.v0.3.0.md`.
> Live file holds only the current (post-0.3.0) arc — empty until the next arc begins.

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

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

---

## 2026-07-04 — 0.3.1 PR2: vision throughput regression fixed (#64)

- **Change:** Added an internal vision proxy path for WebP captures. `crates/kernel/src/vision_proxy.rs`
  creates `<frame>.vision.jpg` (max edge 1280 px, q80) beside the stored lossless WebP; the capture
  loop enqueues proxy writes through a bounded worker and flushes it on shutdown; the worker pool
  dispatches vision from the proxy and lazily creates one for older WebPs. Retention/self-capture
  purge now remove the proxy when deleting the WebP. WebP remains the DB/API/storage image format;
  no schema, settings, or UI surface changed.
- **Why:** Phase A measured the #64 regression on the same quality Qwen3VL model: current WebP tree
  26.95 vision frames/min vs. pre-WebP `v0.2.1` 61.68 frames/min. The blocking point was
  `vision_tag_outcome` decoding native WebP before every `vision.analyze` call. The proxy removes
  that CPU/file decode from the repeated inference dispatch path and restores the pre-WebP 1280 px
  vision workload without using D5's lower-preference encoder-settings or format-revert escapes.
- **Verification:** `cargo test -p kernel` passed; live `npm run dev` acceptance profile after the
  fix: jobs 77-106 → `done|30`, `1783176405822|1783176435000|30|61.69`; GPU counter summary
  avg/median summed engines **61.18%/54.95%** with no repeated 3-4% idle valleys; decode probe:
  WebP source avg **46.83 ms**, proxy JPEG avg **3.08 ms**. Full CI-order verification passed:
  `npm --prefix ui ci`, `npm --prefix ui run lint`, `npm --prefix ui run build`,
  `node scripts/stage-mcp.mjs`, `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace`,
  `cargo test --workspace`, and `git diff --exit-code -- ui/src/bindings`.

---

## 2026-07-04 — 0.3.1 PR2 review follow-ups

- **Change:** Addressed all actionable PR #83 review comments without posting bot replies. The
  capture/vision async paths now avoid synchronous metadata/remove checks by using `tokio::fs`;
  retention/self-capture cleanup now propagates proxy-delete failures so screenshot proxies cannot
  be orphaned while the DB row is marked purged/deleted; and queued capture-side proxy generation
  respects `storage.max_width` when that cap is below 1280 px. Added focused regression tests for
  proxy max-width and proxy-delete failure propagation.
- **Why:** Reviewers correctly identified two privacy/correctness risks: `.vision.jpg` could exceed
  a user-configured storage width cap for newly captured frames, and a failed proxy deletion could
  leave a screenshot derivative after the app considered the frame purged. The async I/O changes keep
  the new cleanup/proxy checks from blocking executor threads.
- **Verification:** `cargo test -p kernel vision_proxy -- --nocapture`, `cargo test -p screensearch
  remove_frame_image_and_proxy -- --nocapture`, and the full CI-order sequence (`npm --prefix ui ci`,
  `npm --prefix ui run lint`, `npm --prefix ui run build`, `node scripts/stage-mcp.mjs`,
  `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo build --workspace`, `cargo test --workspace`, `git diff --exit-code -- ui/src/bindings`)
  all passed.

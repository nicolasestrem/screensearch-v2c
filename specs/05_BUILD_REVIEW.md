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

## Pass 1 — 2026-07-05 — 0.3.2 PR1 (specs contract; specs-only, no code)

- **Implemented:** The 0.3.2 mini-arc contract ("P7.2 — product shell", `docs/0.3.2.md`, D1–D12)
  normalized into the specs: `04` §1/§2/§3/§4, `03` §7d + §8 + §11b, `UI_REFERENCE` §3/§4/§5/§8/§10,
  `07` (rows #96/#97/#100/#83 updated; #102–#104 added; updater-key manual step),
  `CLAUDE.md`/`AGENTS.md` current-state + no-cloud rule, `CHANGELOG.md` + `08` entries.
  Verification = the diff itself: `git diff --name-only main` shows only `.md` files (verbatim
  output on the PR).
- **Skipped / deferred:** Everything with a runtime surface — deliberately (PR2–PR5 implement this
  contract). GitHub hygiene (label #88 `deferred-0.4.0` + fold-forward comment) done right after the
  PR opened.
- **Hallucinated / corrected:** The two settings key names (`app.close_to_tray`,
  `app.run_at_startup`) are a PR1 naming proposal — no `app.*` namespace existed before — and are
  flagged as such in `03 §8` so PR3 owns the final call. Nothing else assumed.
- **Broke / regressed:** Nothing — no code touched.
- **Still risky:** `03 §7d` codifies single-instance behavior that previously lived only in code
  (`src-tauri/src/lib.rs` — show → unminimize → focus); the spec documents shipped behavior, so if
  PR3 observes a different order the spec follows the code. `06` stays empty — no spec contradiction
  surfaced while normalizing; the one near-conflict (the `04 §4` "localhost + model downloads only"
  network line vs. D2's update check) was resolved inside this PR by extending the guardrail line
  itself, so PR2 won't hit a false stop.
- **Review response (same PR, second commit):** five automated-review findings addressed.
  (1) `07` row #97 used 0.3.1's "D6" for the pull-based principle, which collides with 0.3.2's
  D6 = two-tier IA → relabelled to **D4** (0.3.2's no-push decision) with a renumbering note, closing
  a real "spec contradictory → STOP" trap for a PR3 agent. (2) **`02` brought into scope** (now 10
  files, not 9): `02` is the scope/phase authority read before `04`, and its §8 Status still said
  "No active arc" and named only the lifecycle half — a fresh-session contradiction. Fixed §8 Status
  (arc active; lifecycle **and** interface; zero-schema) + annotated the §5 "Later" auto-update
  mention. No new `§5d` (would be scope beyond a consistency fix). (3) `UI_REFERENCE §8` shell matrix
  said "five routes"; the router has six → now names all six content routes incl. **Moment**, the
  origin of the one-scroll-context rule. (4) `03 §7d` run-at-startup made its own `(D3).` clause. (5)
  `03 §8` lifecycle-keys parenthetical split into sentences. All specs-only; still no code.

---

## Pass 2 — 2026-07-05 — 0.3.2 PR2 (auto-update, #69; Rust lane)

- **Implemented:** `tauri-plugin-updater` wired against a minisign-signed GitHub-Releases
  `latest.json`, with the passive pull-based UX (D1) and the release pipeline that feeds it (`03 §11b`).
  Rust: `UpdateStatus` (traits) + `src-tauri/src/update.rs` (manager: single-flight check → background
  download → hold verified bytes → install on user restart) + `lib.rs` wiring (plugin, managed state,
  release-only launch check, three commands, shared `graceful_shutdown`) + `main.rs` `--version`.
  Config: `createUpdaterArtifacts` + `plugins.updater` (real pubkey `27E1C773C0BDF81E`, endpoint,
  passive installMode); no CSP/capability change (Rust-driven). UI: full ipc layer (command/query/
  mutation/event/live-event) + `UpdateIndicator` (NavRail footer presence dot + manual check) +
  `AppPanel` (Settings · App). Pipeline: `scripts/make-latest-json.mjs` + `.github/workflows/release.yml`.
  **Verification (verbatim on the PR):** UI lint clean; UI build clean; `cargo fmt --check` clean;
  `cargo clippy --workspace --all-targets -D warnings` clean; `cargo build --workspace` ok;
  `cargo test --workspace` all green (0 failed); `git diff --exit-code -- ui/src/bindings` clean.
  Live E2E (real signed installer): detect → background download → signature-verify → install-on-restart
  (before/after `--version`) + tampered-manifest rejection, per the `docs/TESTING.md` runbook.
- **Skipped / deferred (in-scope-for-the-arc-but-not-this-PR):** the tray "Check for updates" menu
  item is PR3 (it reuses `update::check_for_updates`); run-at-startup / close-to-tray settings in the
  App section are PR3; the App section's final Essentials-tier placement is PR5. Windows code signing
  (Authenticode/SignPath) is explicitly **not** this PR — the minisign updater signature is not an
  installer certificate; the `07` code-signing row stays open.
- **Hallucinated / corrected:** none load-bearing. Confirmed against the tree/plugin: the manual check
  belongs in the NavRail **footer** now (roadmap/mission wording + gap #99) — `UI_REFERENCE §3`/`§5`
  updated so the spec matches; the two version sources (`CARGO_PKG_VERSION` for `--version` vs. the
  `tauri.conf.json` version the updater compares) agree in production but need both set for a test build
  (documented in the runbook).
- **Broke / regressed:** nothing. The `RunEvent::ExitRequested` block was refactored into a shared
  `graceful_shutdown` helper with identical ordering/semantics (idempotent, so the double-invoke on
  install-then-exit is harmless).
- **Still risky:** (1) the endpoint `releases/latest/download/latest.json` resolves only for a
  **published, non-prerelease** release — every historical release was a prerelease, so v0.3.2+ must be
  published as a full release or the updater is inert (recorded in the runbook + PR). (2) Downloaded
  installer bytes are held in RAM until restart (~tens of MB) — commented; temp-file spill is the future
  escape hatch. (3) Key custody is a release blocker (D2): losing the private key strands every install.
- **Review response (PR #91, second commit):** four automated-review findings addressed in `update.rs`
  + the two check controls, all legitimate. (a) `pending` was a `TokioMutex` but never locked across an
  `.await` → switched to `StdMutex` (removes async-lock overhead). (b) The single-flight `in_flight`
  flag was reset manually (leaked on a panic) → replaced with an RAII `InFlightGuard` that clears it on
  drop. (c) The **manual `check_for_updates` actually blocked through the whole ~13 MB download**
  (contradicting its own doc + the "background download" contract): the check and download are now split,
  the download runs in a spawned task that owns the guard, and the command returns the post-check
  snapshot — matching D1. (d) `Available` was a phantom (set back-to-back with `Downloading`, no yield):
  it is now set in `run_check` before the download task spawns, so it is genuinely observable before
  `Downloading`. The two check buttons now disable while a background download is in flight. Re-verified:
  full suite green; live log-based positive (detect → background download → signature-verified) + negative
  (tampered → `Invalid encoding in minisign data`) re-run on the refactored build.
- **Review response (PR #91, third commit):** three follow-up findings on the refactored build,
  all legitimate — two share one root (a recheck can strand a staged update). (a) `download_and_stage`
  set `Error` on a failed download **without clearing `pending`** → status `Error` while a valid
  installer stayed staged, an inconsistent state with no UI path to apply it (the restart button
  renders only for `Ready`): the `Err` branch now clears `pending` first (consistency over reusing
  the earlier bytes). (b) The **NavRail** "Check for updates" button was still enabled in `ready`, so
  a click while offline could clobber the staged `Ready` → `Error` and strand the verified installer;
  it is now hidden in `ready` (matching `AppPanel`, which already hides it) — the action then is
  Restart, via the presence dot's link to Settings. (c) `scripts/make-latest-json.mjs`'s version guard
  always read the committed `tauri.conf.json`, so the documented overlay-based E2E flow (which stamps
  the version via `--config`, not the committed file) failed the guard; added a **test-only
  `--expected-version`** override (the release workflow never passes it, so the strict conf-drift guard
  is intact for releases) and updated the `docs/TESTING.md` runbook to use it instead of editing the
  committed config. Re-verified: full suite green; log-based positive + negative re-run.

---

## Pass 3 — 2026-07-05 — 0.3.2 PR3 (system tray + quick actions, #56/#57; Rust lane)

- **Implemented:** The native Tauri tray (`src-tauri/src/tray.rs`, `tray-icon` feature) with a live
  passive state icon + the exact `03 §7d` six-item menu, close-to-tray (default on) with a one-time
  first-restore toast, run-at-startup (`tauri-plugin-autostart`, default off, register-before-persist),
  and the Load/Unload-answer-model + Start/Stop-vision quick actions in both the tray and the in-app
  quick menu + command palette (#57 complete). Backing changes: `JobStats` per-kind vision split +
  `cancel_vision` command + `cancel_pending_vision_jobs` store method (DELETE on existing rows, no
  schema change) + `JobProgress` emission on enqueue/cancel so labels track live. UI: `QuickActions`
  (NavRail footer), four palette entries, two `AppPanel` toggles.
  **Verification (verbatim on the PR):** UI lint clean; UI build clean; `node scripts/stage-mcp.mjs`;
  `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean;
  `cargo build --workspace` ok; `cargo test --workspace` all green (0 failed — incl. the new
  `tray::tests` mapping/icon/label, store `job_stats_splits_out_vision_pending_and_running` +
  `cancel_pending_vision_jobs_removes_only_pending_vision`, kernel settings round-trip);
  `git diff --exit-code -- ui/src/bindings` reflects the regenerated `JobStats.ts` + `Settings.ts`
  (committed). **Live run** (`npm run tauri dev`): clean boot (schema_version=10, no migration; no
  "tray init failed"; no autostart-reconcile error); the new NavRail QuickActions render in the real
  app with correct IPC-driven labels ("Load answer model" / "Start vision tagging") — PrintWindow
  screenshot on file; close-to-tray verified (WM_CLOSE → process alive + window hidden);
  single-instance restore verified (second launch exited + restored the window); one-time toast
  verified (`app.tray_toast_done=true` persisted after first restore → never repeats); no orphaned
  processes after exit.
- **Skipped / deferred (in-scope-for-the-arc-but-not-this-PR):** the App section's final Essentials-tier
  placement is **PR5** (its toggles land here with provisional placement, per D3/D6); the gap-#100
  cross-chord conflict warning + the #83/JPEG dead-setting removals are **PR5**; the D9 shell-layout
  hardening is **PR4**. No new global hotkeys (`03 §7d`).
- **Hallucinated / corrected:** Two design assumptions were corrected against the tree during the plan
  pass: (1) the jobs status column is `state`, not `status` (cancel SQL fixed); (2) `JobStats` is
  aggregate-only, so it was **extended** with `vision_pending`/`vision_running` and the on-demand
  enqueue/cancel paths were made to **emit `JobProgress`** — without that the vision label would stay
  stale until a worker completed a job. The sister-app review (D5) found V2c already ships two of its
  four hardening patterns (success-gated hotkey persistence, scoped shortcut replacement in
  `overlay.rs`) and is single-process (so the sister app's 3 s health-poll + textual-only icon + OS
  notifications were all rejected in favor of the event-bus feed + per-state glyphs + no push, D4).
- **Broke / regressed:** nothing. The `CloseRequested` handler for `main` changed from unconditional
  quit to a close-to-tray branch (quit preserved when the setting is off or the tray failed to build —
  `close_to_tray_enabled` returns false without a tray, so a build failure can never trap the window);
  `graceful_shutdown` and the single-instance show/unminimize/focus sequence are reused unchanged.
- **Still risky:** (1) The *native* tray menu-item clicks + the run-at-startup registry write are not
  automatable in this environment, so they were not click-tested here (recorded in `08`); they reuse the
  exact command paths the verified UI quick actions call, and those paths carry unit/integration coverage.
  (2) `cancel_vision` cannot stop an already-running vision job (no lease) — by design the label honestly
  stays "Stop vision tagging" until `vision_running` drains; not a bug. (3) The tray icon is composed at
  runtime from `32x32.png` + a status dot; at the ~16 px Windows tray render size the dot is legible in
  the base capture but the tooltip carries the authoritative state.

---

> Pre-0.2.x (v0.1.0) history → `specs/archive/05_BUILD_REVIEW.v0.1.0.md`.
> Shipped 0.2.x history (0.2.0–0.2.2) → `specs/archive/05_BUILD_REVIEW.v0.2.x.md`.
> Shipped 0.3.0 history (the whole arc: PR1–PR9 + post-0.2.2 bridge fixes) →
> `specs/archive/05_BUILD_REVIEW.v0.3.0.md`.
> Shipped 0.3.1 history (the P7.1 triage patch: post-0.3.0 bridge fixes PR #79/#80 + PR1–PR4) →
> `specs/archive/05_BUILD_REVIEW.v0.3.1.md`.
> Live file holds only the current arc — empty until the next arc begins.

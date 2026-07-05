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

> Pre-0.2.x (v0.1.0) history → `specs/archive/05_BUILD_REVIEW.v0.1.0.md`.
> Shipped 0.2.x history (0.2.0–0.2.2) → `specs/archive/05_BUILD_REVIEW.v0.2.x.md`.
> Shipped 0.3.0 history (the whole arc: PR1–PR9 + post-0.2.2 bridge fixes) →
> `specs/archive/05_BUILD_REVIEW.v0.3.0.md`.
> Shipped 0.3.1 history (the P7.1 triage patch: post-0.3.0 bridge fixes PR #79/#80 + PR1–PR4) →
> `specs/archive/05_BUILD_REVIEW.v0.3.1.md`.
> Live file holds only the current arc — empty until the next arc begins.

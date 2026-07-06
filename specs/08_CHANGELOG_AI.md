# 08 — AI Changelog

> Append-only record of what the agent changed during the build, **with reasons**. One entry per
> meaningful change set. Empty until P0 begins. (This tracks build work; the design-phase history
> lives in git.)

## <date> — <short title>
- **Change:** what was added/modified.
- **Why:** the reason, tied to a spec section.
- **Verification:** the command run + verbatim result.

---

## 2026-07-06 — 0.3.2 PR5 review follow-up (PR #94, second commit; docs only)

- **Change:** Corrected the human-facing record after the PR #94 automated review. Codex's one P2
  (on `crates/uia/src/classify.rs`) is factually right that `capture.uia_run_on_interactive = true`
  was not fully inert: it also bypassed the `input_gate_skips_uia` Timer suppress gate, so an
  opted-in install now gets OCR for mid-input timer frames. **No code change** — `03 §8` pre-decided
  the retirement including that bypass ("the suppress window now always applies"; tolerated +
  ignored on load, no migration — D8), and `capture.uia_suppress_during_input_ms = 0` is the
  documented opt-out for such installs. What needed fixing was overclaimed deadness in the record:
  `CHANGELOG.md` no longer says the setting "could never fire" (it now names the retired side
  effect and the remedy), `07` #83 carries a review note, and `05` Pass 5 records the disposition.
- **Why:** accuracy of the shipped record (`04 §7`); the behavior change itself is contract-settled
  (`03 §8`, D8), so honoring the review means documenting it truthfully, not migrating it.
- **Verification:** docs-only diff (`git diff --stat` on the follow-up commit: `CHANGELOG.md`,
  `specs/05`, `specs/07`, `specs/08`); no code touched, so the Pass 5 build/test evidence stands.

---

## 2026-07-06 — 0.3.2 PR5: Settings two-tier IA (D6; UI lane, after PR3 + PR4)

- **Change:** The flat Settings wall (16 panels, ~60 fields) became the settled two-tier IA.
  - **UI.** `ui/src/routes/Settings.tsx` reordered into **Essentials** (Capture — interval,
    monitors, event-driven master toggle · Hotkeys · Privacy · Models — tier pickers, thinking,
    plus `ModelPanel` folded in for the D6 load/unload · Storage · App (`AppPanel`) · Data
    (`ApiPanel` + export)) and **Advanced** — seven collapsed groups behind a new `Expander`
    primitive (`primitives/Expander.tsx`: header = disclosure button with
    `aria-expanded`/`aria-controls`, labelled-region body, intro visible while collapsed) with
    per-session open state in `useUiStore.settingsExpanded` (`state/uiStore.ts`). One
    plain-language intro sentence per section (§9 voice), incl. `ApiPanel`/`AppPanel`.
    The Hotkeys section gained the **gap-#100 inline cross-chord conflict warning**
    (`chordsConflict`: case/modifier-order-insensitive, computed on the live draft; `role="status"`
    warn line — UI-side only per `03 §7d`).
  - **Rust (D8 removals).** `storage_jpeg_quality` + `capture_uia_run_on_interactive` left the
    `Settings` struct (`crates/traits/src/ipc.rs`), the per-key load/save/clamp paths
    (`crates/kernel/src/settings.rs` — both keys appended to `RETIRED_SETTINGS_KEYS`, the shipped
    tolerate-and-drop mechanism), the capture-loop config (`capture_loop.rs`, `kernel/src/lib.rs`),
    and the UIA policy layer (`crates/uia/src/classify.rs`: `UiaTriggerPolicy` deleted;
    `ScrollStop|Click` never walk; the input-suppress gate always applies when non-zero) +
    `src-tauri/src/lib.rs` wiring. Tests updated; the generic retired-keys tests now cover the two
    new keys (the "config with removed keys loads without error" acceptance). `Settings.ts`
    regenerated (exactly the two fields) and committed.
  - **Docs.** `UI_REFERENCE §5` (+`Expander`); `07` #83 → ✅ / #100 → ✅ (built);
    `docs/ARCHITECTURE.md`; `CLAUDE.md`/`AGENTS.md` current-state; `CHANGELOG.md`; `05` Pass 5.
- **Why:** `docs/0.3.2.md` §3 PR5 + D6/D7/D8 — the interface half's closing PR: the wall becomes
  two honest tiers on PR4's hardened shell, PR3's keys keep their D6 home (App), and the two
  provably-inert knobs stop lying to users (`07` #83; JPEG quality's own hint said "has no effect
  today"). Presentation-first (D7): zero key renames, zero semantic changes; zero DB migrations
  (D10); tokens only (D12).
- **Verification:** `cd ui && npm run lint` (clean) `&& npm run build` (`✓ built in 1.61s`, tsc
  clean); `node scripts/stage-mcp.mjs`; `cargo fmt --all -- --check` (clean) · `cargo clippy
  --workspace --all-targets -- -D warnings` (clean) · `cargo build --workspace` (ok) ·
  `cargo test --workspace` (**47 suites, 523 passed, 0 failed**) · bindings diff = the two removed
  fields only, committed. **Live** (WebView2 CDP method, `05` Pass 4/5): real-DB startup logged
  `settings: dropped retired keys keys=["storage.jpeg_quality", "capture.uia_run_on_interactive"]`;
  D6 tier order verified in the DOM; expanders collapsed by default, Enter/Space keyboard toggling,
  state surviving route round-trips; the #100 warning fired live on a recorded collision and
  cleared on revert; 0 nested scrollers / no horizontal scrollbar collapsed **and** expanded (D9
  holds); screenshot evidence captured.

---

## 2026-07-06 — 0.3.2 PR4: shell layout hardening (D9; UI lane)

- **Change:** Enforced the D9 shell layout contract app-wide (structural CSS only, tokens only). Phase A
  (reproduce-first, recorded in `05` Pass 4 + `07` #106): the NavRail ghost/duplication glitch is a WebView2
  GPU-compositor stale-surface artifact — the DOM and CDP renderer screenshots stay clean under
  route/scroll/reload stress; the only rail-moving mechanism is a banner mount/unmount shifting the nav row
  (measured +40 px), so the artifact is upstream-class and not observable in-app. Phase B:
  - **Shell:** `AppShell` `<main>` → `relative [scrollbar-gutter:stable] [contain:paint]` + a new
    `ScrollContainerContext`; NavRail → `relative z-rail isolate` (ghost-rail compositor mitigation);
    StatusRail chips get stable floor widths + 5-skeleton loading parity + `overflow-x-clip` / eyebrow hidden
    below `lg`; `Panel` header `min-h-12`.
  - **Recall:** the virtualized search list now scrolls the shell `<main>` (window-scroller pattern with a
    measured `scrollMargin`), the mode/query header is `sticky`, and degraded chips sit in a reserved
    `min-h-8` status slot — removing the nested virtualizer scroller and the push-on-resolve CLS.
  - **Nested scrollers removed:** AnswerStream thinking `<pre>` grows inline (autoscroll retargets the
    nearest scrollable ancestor); AnswerStream/ReportView citation rows and the Moment filmstrip wrap
    instead of scrolling horizontally.
  - **Skeletons:** Deck/Insights/Settings loading skeletons now mirror their populated layouts (Insights
    renders its real header during load); the Deck Today panel reserves the minimap band.
- **Why:** `docs/0.3.2.md` PR4 + `UI_REFERENCE §8` (D9 acceptance-grade shell layout contract): one scroll
  context per route, no nested/horizontal scrollbars 1280×720→ultrawide at 100–150 % DPI, no CLS on load,
  fixed rails. D10 (no schema changes) and D12 (structural only) honored; overlay surfaces exempt (recorded
  in `UI_REFERENCE §8`); ghost rail dispositioned per the stop condition (`07` #106).
- **Verification:** `npm run lint` EXIT 0 · `npm run build` `✓ built in 1.64s` · `cargo fmt --all -- --check`
  EXIT 0 · `cargo clippy --workspace --all-targets -- -D warnings` EXIT 0 · `cargo build --workspace`
  Finished · `cargo test --workspace` **524 passed, 0 failed** · `git diff --exit-code -- ui/src/bindings`
  clean. Live WebView2 (CDP) 36-cell size/DPI matrix: every cell `docHScroll = 0`, sole scroller is the
  shell `<main>`, rail stable (local evidence under `docs/audits/shots-0.3.2-pr4/`, gitignored per
  policy). Not automatable here (manual
  live-acceptance): the AnswerStream thinking-trace inline growth needs a reasoning answer model that emits
  `<think>` output — unavailable this session; the change matches the shipped MomentDetail #59 fix.

---

## 2026-07-06 — 0.3.2 PR3 review follow-up: four correctness fixes (PR #92)

- **Change:** Addressed the four valid findings from PR #92's automated review (all bot reviewers;
  no human review comments). No API/schema/binding change.
  - **`ui/src/components/shell/CommandPalette.tsx`.** The palette listed both halves of each new
    lifecycle toggle unconditionally (Load *and* Unload, Start *and* Stop vision). Now it reads
    `useSidecarStatus` + `useJobStats` (exactly as `QuickActions` does) and renders only the
    contextually valid entry, so it can never re-`preload()` an already-loaded model or fire an
    Unload/Stop that errors or no-ops.
  - **`ui/src/routes/Settings.tsx`.** The route seeds its editable `draft` once and bulk-saves the
    whole draft, but the two `app_*` toggles are owned by `AppPanel`'s self-contained round-trip and
    never re-entered the draft — so a later bulk Save of any other field reverted them. The reconcile
    effect now mirrors `app_run_at_startup` / `app_close_to_tray` from the live query into the draft
    (only those two fields; in-progress edits preserved; unchanged draft keeps its reference so no
    re-render loop), so the diff never flags them and Save never reverts them.
  - **`src-tauri/src/tray.rs` + `src/lib.rs`.** The tray seeded `vision_active` to `false` at init;
    a restart with pending/running `vision_tag` jobs left over from a prior session therefore showed
    "Start vision tagging" (and no `JobProgress` is emitted until a worker settles), so the backlog
    couldn't be stopped from the tray. `tray::init`/`build` now take a `vision_active` seed; setup
    computes it from `store.job_stats()` (`vision_pending + vision_running > 0`) alongside the
    readiness snapshot and seeds both the atomic and the menu label.
  - **`src-tauri/src/lib.rs` (`set_settings`).** Register-before-persist changes the OS autostart
    registration before saving; if the save then failed, the OS state stayed flipped while the UI
    rolled back. On a `save_settings` error, the autostart registration is now restored to the prior
    value (the boot `reconcile_autostart` remains the backstop).
- **Why:** PR review correctness; keeps the two lifecycle surfaces (tray, palette, quick menu)
  consistent and the persisted settings honest (`03 §7d`, `docs/0.3.2.md` D3/D4).
- **Verification:** `npm run lint` (clean) · `npm run build` (built in 2.17s) · `node
  scripts/stage-mcp.mjs` (up to date) · `cargo fmt --all -- --check` (clean) · `cargo clippy
  --workspace --all-targets -- -D warnings` (clean) · `cargo build --workspace` (Finished in 25.87s)
  · `cargo test --workspace` (**524 passed, 0 failed**) · `git diff --exit-code -- ui/src/bindings`
  (clean) · live `npm run tauri dev`: clean boot (`schema_version=10`, no migration; **no "tray init
  failed"**, no panic, no autostart warning; subsystems up through embeddings/vision scheduler),
  clean shutdown, no orphaned `screensearch.exe`/`llama-server.exe`.

---

## 2026-07-05 — 0.3.2 PR3: system tray + quick actions (#56/#57; Rust lane)

- **Change:** Implemented issue #56 (systray) + the remainder of #57 (quick actions) per
  `03 §7d` / `docs/0.3.2.md` §3 PR3 (D3/D4/D5).
  - **Traits.** `JobStats` gains `vision_pending` + `vision_running` (subsets of the aggregate
    counts, ts-rs → `ui/src/bindings/JobStats.ts`) so the "Start/Stop vision tagging" label tracks the
    queue. `Settings` gains `app_close_to_tray` (default **true**) + `app_run_at_startup` (default
    **false**) → `Settings.ts`. `Store` trait gains `cancel_pending_vision_jobs()` (default `Ok(0)`).
  - **Store.** `job_stats()` now `GROUP BY state, kind` (one pass fills the vision split);
    `cancel_pending_vision_jobs()` = `DELETE FROM jobs WHERE kind='vision_tag' AND state='pending'`
    (a running job is left to finish — no schema change, D10).
  - **Kernel.** `cancel_vision()` (store cancel → emit `JobProgress` → count) + `enqueue_vision` now
    also emits `JobProgress` (via a new `emit_job_progress` helper) so the tray/quick-menu label flips
    immediately instead of waiting for a worker tick. `settings.rs` loads/saves the two `app.*` keys.
  - **src-tauri.** New `src/tray.rs`: a native Tauri `TrayIconBuilder` + `MenuBuilder` (feature
    `tray-icon`) built in `.setup()` after `AppState` is managed. `TrayState` holds the `TrayIcon` +
    three mutable `MenuItem` handles (updated with `set_text`, **never** a menu rebuild) + three
    runtime-composed icon variants (a status dot — `--ok`/`--ink-muted`/`--danger` — overlaid on the
    bundled `32x32.png`, via the `image` crate promoted from dev-dep to dep) + atomics. State is fed
    (no poller) from `forward_events` — `ReadinessChanged` → icon/tooltip/pause label + authoritative
    capture re-sync; `SidecarStatus` → Load/Unload answer-model label (loaded = resident answer lane);
    `JobProgress`/`JobCompleted` → vision label. Menu actions reuse the existing command paths
    (`start/stop_capture`, `load_model`/`unload_model`, `enqueue_vision`/`cancel_vision`,
    `update::run_check` now `pub(crate)`; Quit = `app.exit(0)` → the shared `graceful_shutdown`).
    Pause/Resume uses the sister-app **atomic toggle** (`fetch_xor` derives the target, rollback on
    error). `lib.rs`: registers `tauri-plugin-autostart` (Rust-driven, no JS capability), the
    single-instance callback + tray Open + left-click all call `tray::restore_main_window`; the
    main-window `CloseRequested` branch now hides to tray when `app.close_to_tray` (else the existing
    clean quit); `set_settings` applies run-at-startup **register-before-persist** (autostart enable/
    disable before `save_settings`; failure aborts the save so a save never claims a launch-at-login
    that didn't take) and refreshes the tray's cached close-to-tray flag; a boot autostart reconcile
    syncs the OS registration to the persisted value. New `cancel_vision` command. The one-time
    close-to-tray toast fires on the **first restore** after a hide (not at hide — a toast at hide is
    never seen), via the existing `toast` event, guarded by an internal `app.tray_toast_done` settings
    key (the `api.token` precedent, outside the `Settings` struct) so it never repeats.
  - **UI (tokens-only, functional tones, a11y).** `commands.ts`/`mutations.ts` gain
    `cancelVision`/`useCancelVision`. New `components/shell/QuickActions.tsx` (NavRail footer:
    Load/Unload answer model from `useSidecarStatus`+`useLoadModel`/`useUnloadModel`; Start/Stop vision
    tagging from `useJobStats`+`useEnqueueVision`/`useCancelVision`). `CommandPalette.tsx` gains the
    same four actions. `AppPanel.tsx` gains the two lifecycle toggles (`Toggle`, self-contained
    `useSettings`/`useSetSettings` round-trip; a run-at-startup failure rolls the optimistic toggle
    back and explains inline — `UI_REFERENCE §4` Settings·App row).
  - **Specs.** `03 §8` naming-proposal hedge removed (names are now contract, D7) + `app.tray_toast_done`
    documented as an internal marker; `03 §7d` + `UI_REFERENCE §3` toast sentence corrected to
    first-restore; `07` row #97 marked built; new gap row recording the two user decisions.
- **Why:** `docs/0.3.2.md` §3 PR3 + `03 §7d` — the tray is the app's passive lifecycle surface (#56)
  and the quick actions complete #57. Two user decisions (2026-07-05): vision Start/Stop = backlog
  run + cancel (one new `cancel_vision` command, no scheduler stomping); the one-time toast shows on
  first restore, not at hide.
- **Verification:** `cd ui && npm run lint` (clean) `&& npm run build` (clean); `node scripts/stage-mcp.mjs`;
  `cargo fmt --all -- --check` (clean) · `cargo clippy --workspace --all-targets -- -D warnings` (clean) ·
  `cargo build --workspace` (ok) · `cargo test --workspace` (all green, 0 failed — incl. new tray
  mapping/icon/label unit tests, store `job_stats_splits_out_vision_*` + `cancel_pending_vision_jobs_*`,
  kernel settings round-trip) · `git diff --exit-code -- ui/src/bindings` (regenerated `JobStats.ts` +
  `Settings.ts` committed). **Live run** (`npm run tauri dev`): app boots clean (schema_version=10, no
  migration; no "tray init failed"; autostart reconcile clean); the new NavRail QuickActions render
  with correct IPC-driven labels ("Load answer model", "Start vision tagging") — screenshot on file;
  close-to-tray verified (WM_CLOSE kept the process alive + hid the window); single-instance restore
  verified (second launch exited + restored the hidden window); one-time toast verified
  (`app.tray_toast_done=true` persisted on first restore → never repeats); no orphaned processes after exit.
- **Not automatable here (manual live-acceptance for the maintainer):** clicking the *native* tray
  menu items (Pause/Resume, Load/Unload, Start/Stop vision, Check for updates, Quit) and the
  run-at-startup registry write need real OS-tray/Settings interaction — but they call the exact same
  command paths the verified UI quick actions use, and the reused capture/model/vision/update paths
  carry unit + integration coverage.

---

## 2026-07-05 — 0.3.2 PR2: auto-update (#69; Rust lane)

- **Change:** Implemented issue #69 (auto-update) per `03 §11b` / `docs/0.3.2.md` §3 PR2 (D1/D2).
  - **Rust core.** New `crates/traits` type `UpdateStatus` (tagged enum `idle`/`checking`/
    `available`/`downloading`/`ready`/`error`, exported via ts-rs → `ui/src/bindings/UpdateStatus.ts`).
    New `src-tauri/src/update.rs`: an updater manager holding its own `UpdaterState` (status +
    downloaded `PendingUpdate` + a single-flight `AtomicBool`), three typed commands
    (`get_update_status`, `check_for_updates`, `restart_to_apply_update`), and the
    `update_status_changed` event. `src-tauri/src/lib.rs`: registers `tauri-plugin-updater`, manages
    `UpdaterState`, spawns a **release-build-only** launch check, registers the three commands, and
    **factors the `RunEvent::ExitRequested` shutdown into a shared `graceful_shutdown` helper** reused
    by install-on-restart (so quit + update-install can't drift). `src-tauri/src/main.rs`: a
    `--version`/`-V` early-return before the Builder (single-instance-safe; redirection-capturable) for
    the acceptance before/after evidence.
  - **Config.** `tauri.conf.json`: `bundle.createUpdaterArtifacts = true` + a `plugins.updater` block
    (the real minisign **public** key `27E1C773C0BDF81E`, the GitHub-Releases `latest.json` endpoint,
    Windows `installMode: "passive"`). No CSP change (the fetch is Rust-side reqwest, not the webview)
    and **no updater capability** (the flow is driven by our own commands, not the plugin's JS surface —
    keeps typed-IPC-only and gives PR3's tray the same Rust entry point). `.gitignore` gains `*.key` /
    `*.key.pub` as a belt-and-braces guard (the private key lives outside the repo).
  - **UI (all quiet + tokens-only, five states per `UI_REFERENCE §4`).** `commands.ts` / `queryKeys.ts`
    / `queries.ts` (`useUpdateStatus`) / `mutations.ts` (`useCheckForUpdates`, `useRestartToApplyUpdate`,
    no toasts) / `events.ts` + `useLiveEvents.ts` (`update_status_changed` → cache mirror, no toast).
    New `components/shell/UpdateIndicator.tsx` (NavRail footer: a presence dot **only** while an update
    exists — never a count — plus the quiet manual "Check for updates" control) and
    `components/domain/AppPanel.tsx` (Settings · **App** section: update status line + check/restart +
    version/repo link; self-contained like `ApiPanel`, PR3 adds run-at-startup/close-to-tray, PR5 owns
    final placement). `UI_REFERENCE §3`/`§5` touched to name the footer manual-check control (the
    "footer button now" decision, gap #99).
  - **Release pipeline.** New `scripts/make-latest-json.mjs` (emits the signed `latest.json`; hard-fails
    on a tag/version mismatch or a missing `.sig`, so an unsigned build can never yield a manifest) and
    `.github/workflows/release.yml` (tag `v*` → windows-latest build + sign with
    `TAURI_SIGNING_PRIVATE_KEY` → manifest → **draft** release with installer + `.sig` + `latest.json`;
    `workflow_dispatch` dry-run uploads artifacts). Maintainer writes notes + publishes (repo culture).
  - **Docs.** `docs/TESTING.md` auto-update runbook (positive + negative signature test + the
    publish-as-full-release reminder); `CHANGELOG.md` `[Unreleased]`; `specs/07` row #96 (built) + the
    updater-key custody record.
- **Why:** `docs/0.3.2.md` §3 PR2 + `03 §11b` — auto-update must land before 0.4.0 so the sessions
  release reaches installed copies. Driving it Rust-side keeps the UI on typed IPC only and lets PR3's
  tray reuse the same commands.
- **Verification:** `cd ui && npm run lint` (clean) `&& npm run build` (clean); `node scripts/stage-mcp.mjs`;
  `cargo fmt --all -- --check` (clean) · `cargo clippy --workspace --all-targets -- -D warnings` (clean) ·
  `cargo build --workspace` (ok) · `cargo test --workspace` (all green, 0 failed) ·
  `git diff --exit-code -- ui/src/bindings` (clean; new `UpdateStatus.ts` committed). Live E2E
  (signed installer detect → background download → signature-verify → install-on-restart, plus the
  tampered-manifest rejection) per the `docs/TESTING.md` runbook — evidence quoted on the PR.
- **Manual gate (D2 — RELEASE BLOCKER):** the production minisign keypair was generated
  (fingerprint `27E1C773C0BDF81E`, public key in `tauri.conf.json`); the maintainer must set the CI
  secrets `TAURI_SIGNING_PRIVATE_KEY` (+ `_PASSWORD`) and make the **offline backup** before tagging
  `v0.3.2` (`specs/07`). Losing the private key strands every installed copy on manual downloads.
  Windows code signing (Authenticode) is **not** this PR — that `07` row stays open.

---

## 2026-07-05 — 0.3.2 PR1: specs contract (P7.2 product-shell mini-arc; specs-only)

- **Change:** Normalized the 0.3.2 roadmap (`docs/0.3.2.md`, decisions D1–D12) into the specs —
  no code, no schema, no UI. (a) `specs/04`: `docs/0.3.2.md` in the §1 mandatory reading order
  (hard constraints: zero DB schema migrations D10, new settings only where a PR names them,
  presentation-first D7, structural-only D12); a 0.3.2 row in §2; a §3 build-order bullet for
  PR1→PR6 (Rust lane PR2→PR3 sequential ∥ UI lane PR4; PR5 after both; PR4 reproduce-first + the
  WebView2 stop condition; D5 reviewed-import for PR3); the §4 network guardrail extended to admit
  the signed GitHub-Releases update check. (b) `specs/03`: new **§7d** (tray icon + menu,
  close-to-tray, single-instance codified, clean quit via §6 — D3/D4, no new chords) and **§11b**
  (updater: plugin, minisign manifest, signature-rejection negative requirement, passive D1 UX,
  key custody D2, genesis note minisign ≠ Authenticode); §8 **0.3.2 lifecycle keys**
  (`app.close_to_tray` true / `app.run_at_startup` false — names flagged as a PR1 proposal) + the
  two dead-setting retirements with load tolerance (D8) + the `uia_suppress_during_input_ms`
  cross-ref fix. (c) `specs/UI_REFERENCE.md`: §3 two-tier Settings IA (D6/D7), App section, tray +
  updater blocks (+ tree lines); §4 rows Tray / Updater indicator / Settings·App; §5
  `UpdateIndicator` + `TrayMenu`; §8 **shell layout contract** (D9, acceptance-grade, binds all
  future UI work); §10 item 9. (d) `specs/07`: rows #96/#97/#100/#83 pointed at PR2/PR3/PR5/PR5;
  new #102 (#88 fold-forward, D11) / #103 (settings search deferred, D6) / #104 (visual-refresh
  possibility, D12); the **updater-key custody manual step** (D2, release blocker).
  (e) `CLAUDE.md`/`AGENTS.md`: current-state names the active 0.3.2 arc; the no-cloud hard rule
  admits the signed update check. (f) `CHANGELOG.md`: Docs entry under `[Unreleased]`.
- **Why:** `docs/0.3.2.md` §3 PR1 — every later PR must be implementable from the specs alone
  without reopening the roadmap (`04 §1`/`§2`); the guardrail edits prevent a false
  spec-contradiction stop when PR2 adds the updater's outbound HTTPS check.
- **Verification:** `git diff --name-only main` → only `.md` files (verbatim list on the PR); the
  D1–D12 → spec-location traceability table pasted on the PR. No build/test impact possible (docs
  only); CI runs the full suite on the PR regardless.
- **Review-response addendum (second commit on the PR):** five automated-review findings applied,
  all specs-only. `07` #97's legacy "D6" (0.3.1's pull-based numbering) relabelled to **D4** to match
  0.3.2's namespace (D6 is now two-tier IA), removing a real "spec contradictory → STOP" trap. `02`
  brought into scope (**10 files now, not 9**): §8 Status said "No active arc" and named only the
  lifecycle half — fixed to name the active arc with lifecycle **and** interface + zero-schema, and
  annotated the §5 "Later" auto-update mention (no new `§5d`). `UI_REFERENCE §8` shell matrix widened
  from "five routes" to all six content routes (naming **Moment**). Two `03` formatting nits (§7d
  run-at-startup gets its own `(D3).` clause; §8 lifecycle-keys parenthetical split into sentences).

---

> Pre-0.2.x (v0.1.0) history → `specs/archive/08_CHANGELOG_AI.v0.1.0.md`.
> Shipped 0.2.x history (0.2.0–0.2.2) → `specs/archive/08_CHANGELOG_AI.v0.2.x.md`.
> Shipped 0.3.0 history (PR1–PR9 + bridge fixes) → `specs/archive/08_CHANGELOG_AI.v0.3.0.md`.
> Shipped 0.3.1 history (post-0.3.0 bridge fixes + PR1–PR4) →
> `specs/archive/08_CHANGELOG_AI.v0.3.1.md`.
> Live file holds only the current arc — empty until the next arc begins.

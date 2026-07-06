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

### PR3 review follow-up (PR #92, 2026-07-06)
- **Reviewers:** all automated (Claude review action, ChatGPT/Codex connector, Gemini). No human review
  comments. Four findings were valid; all fixed (Gemini reported none; bot acknowledgement comments were
  not replied to, per the maintainer's "no need to answer bots").
- **Fixed:** (1) **CommandPalette dual actions** — the palette listed Load *and* Unload / Start *and*
  Stop vision unconditionally while `QuickActions` showed only the valid half; the palette now reads the
  same `useSidecarStatus`/`useJobStats` state and renders one contextual entry per pair (no re-`preload()`
  of a loaded model, no Unload/Stop that errors or no-ops). (2) **Settings-draft revert** — `Settings.tsx`
  seeds its `draft` once and bulk-saves it, but the `app_*` toggles live in `AppPanel`'s own round-trip;
  a later bulk Save reverted them. The reconcile effect now mirrors the two `app_*` fields from the live
  query into the draft (those fields only), so they never flag dirty and Save never reverts them.
  (3) **Tray vision seed** — `tray::init` seeded `vision_active=false`; a restart with a leftover
  `vision_tag` backlog then showed "Start" and couldn't stop it. Setup now computes the seed from
  `store.job_stats()` and passes it into `init`/`build`. (4) **Autostart rollback** — `set_settings`
  now restores the prior OS autostart registration if `save_settings` fails after register-before-persist
  changed it (boot `reconcile_autostart` remains the backstop).
- **Verification:** UI lint + build clean; `cargo fmt`/`clippy`/`build` clean; `cargo test --workspace`
  **524 passed, 0 failed**; bindings diff clean; live boot clean (no tray-init failure / panic / autostart
  warning), clean shutdown, no orphaned processes. Full verbatim record in `08` (2026-07-06 entry).

---

## Pass 4 — 2026-07-06 — 0.3.2 PR4 (shell layout hardening, D9; UI lane)

Reproduce-first per `docs/0.3.2.md` PR4 / `04 §3`: Phase A (repro + inventory) recorded here **before any
fix code**. Method: the real app (`npm run tauri dev`) driven over the WebView2 DevTools protocol —
launched with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`, a dependency-free
Node CDP client (`Runtime.evaluate` audits + `Emulation.setDeviceMetricsOverride` for the size/DPI matrix
+ `Page.captureScreenshot`). This attaches to the **live WebView2** (Edge 149), lifting the historical
"Playwright can't attach to the Tauri WebView" limitation for populated states.

### Phase A.1 — NavRail ghost/duplication: root cause = WebView2 compositor stale-surface (upstream class)

- **Reproduce protocol run:** (a) route-cycle stress — Deck→Recall→Timeline→Insights→Settings→Deck, DOM
  rail snapshot + screenshot each transition; (b) Moment scroll stress; (c) startup probe — `Page.reload`
  then sample rail geometry every 200 ms for 8 s; (d) banner-shift measurement.
- **Findings (evidence):**
  - **DOM is invariably clean.** Rail anchor count is constant at **6** (5 nav links + the version-footer
    link) across every route transition, scroll, and reload — **0 count-mismatch anomalies**; item `top`
    positions are deterministic. The rail is not virtualized, not remounted per route, and has no
    transform/sticky/portal. This rules out a React/DOM/virtualized-re-render cause.
  - **Renderer screenshots are clean.** Every `Page.captureScreenshot` (route-cycle + all 36 matrix cells)
    shows a single correct rail — no duplicated/offset items in the renderer paint tree.
  - **The only app-level mechanism that moves the rail is a banner mount/unmount.** AppShell stacks
    `StatusRail / ReadinessBanner (conditional) / [NavRail + main]` vertically; the banner is a flex sibling
    **above** the nav row, so mounting it pushes the entire rail down. Measured empirically: injecting a
    40 px banner-sized sibling above the nav row shifts the NavRail top **48 px → 88 px (exactly +40 px)**
    and it snaps back on removal. On a warm reload the readiness banner did not appear (kernel stayed ready),
    so no shift occurred — consistent with the glitch being **intermittent** and tied to slow-subsystem-init
    (or any future banner) transitions.
- **Root cause / disposition:** the observed "ghost / duplicated nav items at wrong vertical offsets" is a
  **GPU-compositor stale-surface artifact in WebView2** — the rail's pre-shift painted surface persisting on
  the physical display after the row shifts vertically — **not** a DOM/React/renderer defect (both are
  provably clean above) and, by construction, **not observable through CDP screenshots** (which read the
  renderer, not the composited display surface). This matches known WebView2/Chromium reports of ghost
  renders with a clean DOM (e.g. WebView2Feedback #2421 graphics corruption; VS Code #113188 "ghost renders
  … no duplicate HTML"). Per the PR4 **stop condition** + maintainer decision (2026-07-06), this is the
  upstream class: STOP on directly "fixing" it, apply the cleanest CSS-level mitigation, and record a `07`
  gap row that stays open pending field recurrence. See `07` #106.
- **Mitigation applied in Phase B (preventive hardening):** give the NavRail its own stacking context /
  compositing boundary so it owns an isolated surface that repaints atomically on the shift
  (`relative` + `z-rail` [finally consuming the previously-unused `--z-rail` token] + `isolation: isolate`),
  and give `main` a paint-containment boundary (`contain: paint`) so the scrolling content layer cannot
  share/bleed a surface with the rail. `contain: paint` only (never `layout`) — layout containment would
  perturb the Recall `scrollMargin` `offsetTop` measurement (A.2). Risk-checked: the fixed overlays
  (`CommandPalette`, `ToastViewport`, `DevStateBadge`) are siblings of `main`, and there are zero
  `position: fixed` elements inside route content, so paint-containing `main` reparents nothing.

### Phase A.2 — Per-route scroll-context + CLS inventory (empirical, this app's live DB)

- **Single scroll spine:** `AppShell` `<main class="flex-1 min-w-0 overflow-y-auto">` (`AppShell.tsx:59`)
  is the only intended scroller; `html/body/#root` are locked to 100% (no page scroll); no `100vh` anywhere.
- **Confirmed nested-scroll violations (live audit):**
  - **Recall (`Recall.tsx:255`)** — the `@tanstack/react-virtual` pane `div.min-h-0.flex-1.overflow-auto`
    becomes a **nested vertical scroller inside `main`** as soon as results populate (the route container is
    `h-full`, so `main` doesn't scroll and the inner pane owns the scrollbar mid-pane). Idle Recall shows no
    scroller; driving a real search ("the" → 14 hits) surfaced it at both default and 853×480 CSS.
  - **Moment (`Moment.tsx:171`)** — the "Around this moment" filmstrip `div.flex.gap-3.overflow-x-auto`
    is a **nested horizontal scroller in all six matrix cells** (Windows draws a permanent 10 px h-scrollbar;
    no overlay scrollbars).
  - **AnswerStream (`AnswerStream.tsx:105`)** — the thinking-trace `<pre class="max-h-64 overflow-auto">`
    nests inside `main` (route) and inside the FlowOverlay window; citation rows `overflow-x-auto`
    (`AnswerStream.tsx:152`, `ReportView.tsx:136`) are horizontal nested scrollers.
- **Exempt (recorded interpretation, ratified with maintainer 2026-07-06):** the contract's unit is the
  **route**. Modal/overlay surfaces are not route scroll contexts — the CommandPalette listbox
  (`CommandPalette.tsx:298`, `max-h-80 overflow-y-auto`, a `fixed` combobox popup) and the separate
  FlowOverlay window (`FlowOverlay.tsx:263`, a fixed-size always-on-top window) keep their bounded scroll.
  This interpretation is added to `UI_REFERENCE §8` as a clarifying line.
- **Horizontal-scrollbar baseline (good, must preserve):** the size/DPI matrix audit shows **`docHScroll = 0`
  in every cell** (5 routes idle + Moment) down to 853×480 CSS (1280×720 @150 %). StatusRail at 853×480
  renders brand + 5 chips with headroom (visual check); the only horizontal-overflow risk is the rare
  worst case where the two **conditional** chips (throttling + downloading) appear simultaneously — covered
  defensively in Phase B (`overflow-x-clip` + hide the "Command Deck" sub-eyebrow below `lg`), not a
  steady-state break.
- **CLS sources on load:** no `scrollbar-gutter` anywhere (`globals.css:48-60`) → the `main` scrollbar
  appearing/disappearing reflows all `mx-auto`-centered content by ±10 px, and drives a
  `useAdaptiveBucketCount` width-oscillation class on Timeline/Insights; StatusRail loading renders **3**
  skeletons vs **5** populated content-width chips; `Panel` headers grow 41→48 px when late `action` chips
  arrive (Deck/Timeline/Insights); Deck/Settings/Insights skeletons under-reserve below the fold. Fonts are
  the Windows system stack (no `@font-face` → no font-swap CLS); toasts overlay (`fixed z-toast`); images in
  MomentDetail carry intrinsic dims. Fixes in Phase B Steps 1 & 4.
- **Before-fix evidence:** the 36-cell before/after matrix (`audit.json` + screenshots) is saved under
  `docs/audits/shots-0.3.2-pr4/` as **local audit evidence** (that path is gitignored by policy, like
  `.playwright-mcp/`); the machine-readable results are tabulated in this Pass and in the PR description.

### Phase B — D9 enforced app-wide (structural CSS only, tokens only, D12)

- **Implemented (Step 1 — shell hardening):** `AppShell` `<main>` is now `relative
  [scrollbar-gutter:stable] [contain:paint]` and provides a `ScrollContainerContext` (new
  `ui/src/components/shell/ScrollContainerContext.ts`, the arc's only new file) exposing its ref; the
  reserved gutter kills the ±10 px reflow (and the bucket-oscillation class), `contain: paint` is the
  ghost-rail compositor boundary. NavRail is `relative z-rail isolate` (own stacking/compositing surface;
  consumes the `--z-rail` token). StatusRail: five persistent chips get floor widths
  (`min-w-24/20/16/28/20 justify-center`; `font-mono` already `tabular-nums`), loading renders **5**
  matching skeletons (was 3), the "Command Deck" eyebrow is `hidden lg:inline`, the header is
  `overflow-x-clip` + chip group `min-w-0` (clips the leftmost transient chip first, never a scrollbar).
  `Panel` header is `min-h-12` (kills the 41→48 px late-action-chip growth app-wide). Verified live: computed
  `main` = `contain:paint` + `scrollbar-gutter:stable` + `position:relative`, `nav` = `z-index:10` +
  `isolation:isolate`; 853×480 screenshot shows brand + 5 even chips, no eyebrow, no overflow.
- **Implemented (Step 2 — Recall one scroll context):** dropped the route `h-full`; the mode toggle +
  query input are a `sticky top-0 z-rail bg-base pb-4` header (opaque, butts flush against content — no
  peek-through); the degraded-mode chips moved into a permanently-reserved `min-h-8` `role="status"` slot
  (appears in place, never pushes — the maintainer-ratified reading of "only the readiness banner may
  reserve"); the `@tanstack/react-virtual` virtualizer now scrolls the shell `<main>`
  (`getScrollElement: () => mainRef.current`, `scrollMargin = listWrap.offsetTop` re-measured by
  ResizeObserver on mode/width change, row transform `translateY(row.start - scrollMargin)`). The nested
  `overflow-auto` pane is gone. Verified live: post-search the only scroller is `main` (default + 853×480);
  row 0 aligns exactly to the list top (no blank band); sticky header pins at `top:48` while scrolled;
  Ask/Reports modes clean.
- **Implemented (Step 3 — nested-scroller removals):** AnswerStream thinking `<pre>` grows inline (dropped
  `max-h-64 overflow-auto`, the #59 pattern) and its stream-follow retargets the nearest scrollable
  ancestor (`main` in the route, the overlay pane in the Flow overlay) with the 48 px near-bottom guard
  kept; AnswerStream + ReportView citation rows and the Moment "Around this moment" filmstrip switch
  `overflow-x-auto` → `flex-wrap`. Verified live: Moment audit shows only `main` (was `main` +
  `overflow-x-auto`), filmstrip wraps into a grid; a real Ask streamed with the citations wrapped into a
  grid (7 tiles), `docHScroll = 0` and no nested scrollers throughout streaming.
- **Implemented (Step 4 — skeleton parity):** DeckSkeleton mirrors the populated stack (hero `h-28` /
  Today+Queue grid `h-48` / WhereWasI+Intentions grid `h-40` / recents `h-64`) and the Today panel reserves
  the minimap band (`h-7`) while the density query resolves + `min-h-7` on the top-apps chip row; Insights
  loading renders the **real** header (synchronously known — the interactive range control is live during
  load) + matching chips/trend/grid skeletons (removed the blind `InsightsSkeleton`); SettingsSkeleton fills
  the fold (header + 5 panels). Verified live via `?__devState=loading`.
- **Skipped / deferred:** the ghost-rail glitch itself is **not** fixed in app code — dispositioned upstream
  with the mitigation above (`07` #106, Phase A stop condition). Settings skeleton is a pragmatic fold-fill
  only (PR5 restructures the whole route). No token/palette/type changes (D12); no schema changes (D10);
  no new settings; overlay surfaces (command palette, toast stack, FlowOverlay window) left as bounded
  scrollers (recorded exemption).
- **Hallucinated / corrected:** the `SearchBody` `virtualizer` prop was typed `Virtualizer<HTMLDivElement>`;
  moving the scroll element to the shell `<main>` (`HTMLElement`) broke `tsc` — retyped to
  `Virtualizer<HTMLElement, Element>`. Caught by `npm run build` before commit.
- **Still risky:** the ghost-rail mitigation is not CI-observable (compositor artifact invisible to CDP);
  re-check on the physical display and WebView2 runtime updates (`07` #106). The AnswerStream **thinking
  trace** inline-growth could not be exercised live — the answer model available in this session emits no
  `<think>` reasoning output (thinking was already on; trace stayed empty), so the `<pre>` never rendered;
  the CSS change is byte-identical to the shipped-and-verified MomentDetail #59 fix and the autoscroll
  retarget is lint-clean + guarded, but a reasoning-model Ask is a manual-acceptance follow-up.
- **Verification (verbatim, on the PR):** `npm run lint` EXIT 0 · `npm run build` `✓ built in 1.64s`
  (tsc clean) · `node scripts/stage-mcp.mjs` up to date · `cargo fmt --all -- --check` EXIT 0 ·
  `cargo clippy --workspace --all-targets -- -D warnings` `Finished` EXIT 0 · `cargo build --workspace`
  `Finished` EXIT 0 · `cargo test --workspace` **524 passed, 0 failed** · `git diff --exit-code --
  ui/src/bindings` clean. **Live run** (`npm run tauri dev`, WebView2 CDP): 36-cell size/DPI matrix — every
  cell `docHScroll = 0`, the only scroller is the shell `<main>`, rail stable at 6 anchors (local
  evidence under `docs/audits/shots-0.3.2-pr4/`, gitignored).

### Phase B — PR #93 automated-review follow-up (2026-07-06)

Bot review only (gemini-code-assist, chatgpt-codex, claude). Triaged on merit; no bot replies (maintainer directive). Two applied, one class declined:

- **Applied (gemini — `nearestScrollable` layout thrashing):** the stream-follow ancestor walk read
  `n.scrollHeight > n.clientHeight` per token, forcing a synchronous layout on every ancestor on each
  streamed token. Dropped the overflow check — the walk now returns the first `overflow-y: auto/scroll`
  ancestor structurally (`AnswerStream.tsx:21`). Post-PR there is a single overflow candidate per host
  (`main` in the route, the overlay pane in Flow), so the returned element is unchanged; the caller already
  guards with `nearBottom` (a no-op when the container isn't overflowing), so behaviour is identical minus
  the reflow.
- **Applied (chatgpt-codex — long tokens re-introduce h-scroll):** the inline-grown thinking `<pre>` had
  `whitespace-pre-wrap` only, which wraps at whitespace but does not break an unbroken token wider than the
  pane — a horizontal-scroll path back into the contract this PR closes. Added `break-words` to the thinking
  `<pre>` (`AnswerStream.tsx:123`, reasoning prose — wrapping reads fine) and the ReportView footer
  (`ReportView.tsx:152`). Gap in the original audit: "Moment pre blocks already #59-clean" covered nested
  scroll, not long-token horizontal overflow.
- **Declined (claude — 7× "multi-line comment block violates CLAUDE.md"):** the quoted rule ("one short
  line max") exists nowhere in this repo's `CLAUDE.md`/`AGENTS.md`/`specs/04`; the codebase convention is
  multi-line block comments (e.g. AnswerStream's own pre-existing module header). Acting on a fabricated
  rule would make the new comments inconsistent with the surrounding code. Not applied.

### Phase B — Moment grid-blowout + recognized-text regression (2026-07-06, maintainer-reported)

Maintainer screenshot (default window, a Moment whose recognized text is a very wide UIA-captured markdown
table): the `CONTEXT`/`VISION` right rail is shoved off the right edge and the route overflows horizontally.

- **Root cause:** `MomentDetail`'s two-column grid (`lg:grid-cols-[1.6fr_1fr]`) blows out. Grid items
  default to `min-width:auto`, so the `1.6fr` track expanded to the recognized-text `<pre>`'s large
  min-content width and pushed the `1fr` context column past the viewport (a route-level horizontal
  overflow — the exact D9 violation this PR targets). **Pre-existing, latent** until a frame with very wide
  text appeared; PR4's Moment audit cells had no wide preformatted capture. `break-words` from the earlier
  follow-up neither caused nor fixed it — `overflow-wrap:break-word` by spec does **not** reduce a box's
  min-content size, so the track stayed wide.
- **Fix (structural, D9):** `min-w-0` on both `MomentDetail` grid columns (`MomentDetail.tsx:78,136`) so the
  tracks honour their `fr` share and the block wraps/scrolls inside instead of forcing the route wider.
- **Recognized-text treatment (maintainer decision 2026-07-06):** wrapping a wide table mangles its columns
  into an unreadable stack. Chosen behaviour = **horizontal-scroll code block**: the `content_text`/`raw_text`
  `<pre>` are now `overflow-x-auto whitespace-pre` (`MomentDetail.tsx:111,127`) — columns preserved, the
  block scrolls sideways on its own, vertical page scroll stays single. Recorded as a **scoped content
  exemption** in `UI_REFERENCE §8` (distinct from tile/thumbnail strips, which still wrap). Reverts the
  earlier `break-words`/wrap on these two blocks.
- **Verification (verbatim):** `npm run lint` EXIT 0 · `npm run build` `✓ built in 1.54s` (tsc clean). No
  Rust/binding surface touched (UI className + comments only) — the PR's `cargo` suite (524 passed) and
  clean `ui/src/bindings` diff still hold. Live visual confirm on the maintainer's running session (the
  wide-table Moment that surfaced the blowout).

### Phase B — AnswerStream stream-follow regression (2026-07-06, chatgpt-codex PR #93 finding)

- **Finding (valid):** the auto-follow effect only depended on `thinking`, so once a reasoning model
  finished the trace and switched to answer `token` deltas, the effect stopped firing and the shell
  `<main>` no longer followed the growing answer — it streamed below the fold until the final
  focus-on-done. A **regression this PR introduced**: removing the thinking `<pre>`'s capped inner scroller
  (`max-h-64`, #59) lets a long trace pin the user at its end while the answer scrolls past the viewport.
- **Fix:** replaced the `thinking`-only, `thinkingRef`-anchored follow with a single effect anchored to a
  bottom **sentinel** (`streamEndRef`, an `aria-hidden` div after the citations), keyed on `[thinking,
  answer, streaming]`, so it follows **both** phases; kept the 48 px near-bottom guard (scrolling up to
  re-read is not yanked) and `nearestScrollable` (shell `<main>` in the route, overlay pane in Flow).
  Dropped the now-irrelevant `thinkingOpen` gate (the sentinel sits below a collapsed trace too).
- **Declined (claude bot, 3 more "multi-line comment block violates CLAUDE.md" on the follow-up commits):**
  same fabricated rule (absent from this repo); this file's established style is multi-line explanatory
  comments (module header, the Markdown-link and focus-on-done blocks are all 5–8 lines). Not applied.
- **Verification (verbatim):** `npm run lint` EXIT 0 · `npm run build` `✓ built in 1.54s` (tsc clean).
  UI-only. Stream-follow across the thinking→answer transition is a manual-acceptance follow-up — the
  session's answer model still emits no `<think>` output (see the manual-acceptance note above).

---

> Pre-0.2.x (v0.1.0) history → `specs/archive/05_BUILD_REVIEW.v0.1.0.md`.
> Shipped 0.2.x history (0.2.0–0.2.2) → `specs/archive/05_BUILD_REVIEW.v0.2.x.md`.
> Shipped 0.3.0 history (the whole arc: PR1–PR9 + post-0.2.2 bridge fixes) →
> `specs/archive/05_BUILD_REVIEW.v0.3.0.md`.
> Shipped 0.3.1 history (the P7.1 triage patch: post-0.3.0 bridge fixes PR #79/#80 + PR1–PR4) →
> `specs/archive/05_BUILD_REVIEW.v0.3.1.md`.
> Live file holds only the current arc — empty until the next arc begins.

# 05 — Build Review — archived v0.3.2 history (P7.2 product-shell mini-arc)

> Archived on the v0.3.2 release sweep (2026-07-06, 0.3.2 PR6); entries preserved verbatim from
> the live `specs/05_BUILD_REVIEW.md`. Contents: the six 0.3.2 passes — PR1 specs contract;
> PR2 auto-update (#69) + review follow-ups; PR3 systray + quick actions (#56/#57) + review
> follow-up; PR4 shell-layout hardening (Phase A repro → #106 stop condition → Phase B) +
> follow-ups; PR5 Settings two-tier IA (D6/D8) + review response; PR6 audit + release sweep
> (D1–D12 all PASS; #89 fixed in-PR; key custody satisfied; release-pipeline dry-run).
> Earlier history → the v0.1.0 / v0.2.x / v0.3.0 / v0.3.1 archives in this folder.

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
- **Follow-up (gemini PR #93, HIGH — valid):** the sentinel effect fires per streamed token and
  `nearestScrollable` walks ancestors with `getComputedStyle`, so resolving it every token thrashes during
  a stream. Cached the resolved scroller in `scrollerRef` (resolved once when a stream starts, cleared when
  `!streaming`) — the scrollable ancestor is stable for a stream's duration, so behaviour is unchanged.
  `npm run lint` EXIT 0 · `npm run build` `✓ built in 1.58s`.

---

## Pass 5 — 2026-07-06 — 0.3.2 PR5 (Settings two-tier IA, D6; UI lane, after PR3 + PR4)

- **Implemented:**
  - **Two-tier IA (D6, tier membership exactly as settled).** `ui/src/routes/Settings.tsx`
    restructured: **Essentials** always visible in the D6 order — Capture (interval, monitors,
    event-driven master toggle moved in), Hotkeys (overlay/marks chords, overlay results, dwell),
    Privacy, Models (tier pickers + thinking + `ModelPanel` folded in: the D6 "load/unload"),
    Storage (max width + retention), App (`AppPanel`, unchanged self-contained surface), Data
    (`ApiPanel` + the export panel, adjacent — export stays its own panel per `UI_REFERENCE §5`).
    **Advanced** = seven collapsed-by-default groups, one expander each: Capture tuning (change
    threshold + the event-driven sub-knobs), Text source / UIA, Enrichment & scheduling,
    Performance throttle (+ live readout), Text filtering (+ suppression readout), Reports &
    retrieval, Inference engine (the former "Sidecar (advanced)" fields). Every section opens with
    one plain-language sentence (§9 voice); `ApiPanel`/`AppPanel` got intros too.
  - **New `Expander` primitive** (`ui/src/components/primitives/Expander.tsx`, in the §5
    inventory): Panel-shaped disclosure — the header row is the button (`aria-expanded` +
    `aria-controls`, ~61 px hit), the body a labelled `role="region"`; the intro stays visible
    while collapsed. Open state per-session in `useUiStore.settingsExpanded` (Zustand ephemeral
    state per `UI_REFERENCE §6`; no new persistence machinery).
  - **Gap #100 conflict warning:** `chordsConflict` (case-insensitive, modifier-order-insensitive)
    compared live against the **draft** chords; a `role="status"` warn-tone line under the two
    `HotkeyField`s fires while they match and clears when they differ. UI-side only (`03 §7d`);
    the D6 registration-failure warning stays the save-time safety net.
  - **Dead-setting removals (D8):** `storage_jpeg_quality` + `capture_uia_run_on_interactive`
    removed from `Settings` (`crates/traits/src/ipc.rs`), the load/save/clamp paths
    (`crates/kernel/src/settings.rs`), the capture-loop config (`capture_loop.rs`/`lib.rs`), and
    the UI; both keys joined `RETIRED_SETTINGS_KEYS` (tolerate **+ drop** on startup — the shipped
    0.3.0 unknown-key mechanism `03 §8` points at). `crates/uia/src/classify.rs`: the
    `UiaTriggerPolicy` struct is gone — `trigger_runs_uia(ScrollStop|Click)` is now
    unconditionally `false` (those triggers can't fire on new frames since the 0.3.0 trim; legacy
    frames stay readable) and the `input_gate_skips_uia` bypass fell away (the suppress window
    always applies when non-zero, exactly as `03 §8` documents). ts-rs `Settings.ts` regenerated
    (exactly the two fields) and committed. `07` #83 → ✅, #100 → ✅ (built).
  - **Docs:** `UI_REFERENCE §5` (+`Expander`), `docs/ARCHITECTURE.md` jpeg line, `CLAUDE.md` /
    `AGENTS.md` current-state, `CHANGELOG.md` entry, this file + `08`.
  - **Verification (verbatim):** `npm run lint` (clean, exit 0) · `npm run build` `✓ built in
    1.61s` (tsc clean) · `node scripts/stage-mcp.mjs` ok · `cargo fmt --all -- --check` clean ·
    `cargo clippy --workspace --all-targets -- -D warnings` `Finished dev profile ... in 7.38s`
    (clean) · `cargo build --workspace` ok · `cargo test --workspace` → **47 suites, 523 passed,
    0 failed** · bindings diff = exactly the two removed `Settings.ts` fields (committed).
    **Live acceptance** (`npm run tauri dev` + WebView2 CDP, the PR4 method): startup on the real
    dev DB (which carried both retired keys) logged `WARN kernel::settings: settings: dropped
    retired keys keys=["storage.jpeg_quality", "capture.uia_run_on_interactive"]` and loaded
    clean; Essentials render in D6 order; all 7 expanders `aria-expanded=false` by default,
    Enter collapses / Space expands on the focused header, open state survives a Deck→Settings
    round-trip (per-session store); recording the overlay chord into the marks field raised the
    #100 warning live ("Both shortcuts are set to Ctrl+Alt+Z …") and the form Reset cleared it
    (draft left clean, nothing persisted); nested-scroller audit = **0** nested scrollers and no
    horizontal scrollbar with all groups collapsed **and** fully expanded (D9 holds); screenshot
    set captured (Essentials top, Advanced collapsed, expanded, conflict state — local evidence,
    `docs/audits` stays untracked).
- **Skipped / deferred:** settings search box (deferred by D6 — `07` #103); any visual/token
  change (D12 fence); tray/quick-menu/palette surfaces untouched (PR3's, already final).
- **Hallucinated / corrected:** none load-bearing. One planning-stage correction: the roadmap's
  "tolerated-and-ignored" wording vs. the shipped tolerate-and-**drop** mechanism — `03 §8`'s
  annotation points at the existing unknown-key rule, whose implementation (`RETIRED_SETTINGS_KEYS`
  + startup sweep, "grows per arc") both tolerates and purges; reusing it inherits the generic
  regression tests (`load_drops_retired_event_keys_without_error`,
  `save_settings_never_writes_retired_keys`) and satisfies the "loads without error" acceptance.
- **Broke / regressed:** nothing observed. The event-driven sub-knobs moved to Advanced while
  their master toggle stays in Essentials·Capture — the collapsed group shows an honest pointer
  line when the master is off (same conditional semantics as before, D7).
- **Still risky:** the Advanced groups' per-session state lives in a module store, so a very long
  session accumulates open groups (by design — "persists sensibly"); the conflict warning
  normalizes chords lexically (modifier-set + key) and would not flag two *semantically* equal but
  differently-tokenized chords beyond case/order (none are producible by `HotkeyField`, which
  emits canonical chords).
- **Review response (PR #94, second commit — docs only):** Codex raised one P2 on
  `crates/uia/src/classify.rs`: for an install that had opted into `capture.uia_run_on_interactive
  = true`, the key was not fully inert — the old `policy.run_on_interactive || suppress_window_ms
  == 0` head of `input_gate_skips_uia` also bypassed the Timer input-suppress gate, so removal
  changes those installs to OCR for mid-input timer frames. **Factually confirmed against main;
  disposition: as designed, no code change.** `03 §8` pre-decided exactly this in PR1 ("the
  suppress window now always applies, since the former `uia_run_on_interactive` bypass retired
  with the knob"; tolerated + ignored on load, **no migration** — D8), and the traits doc on
  `capture_uia_suppress_during_input_ms` already documents the `0` opt-out, which is the remedy
  for such installs. What the finding *did* expose is overclaimed deadness in the human-facing
  record — `CHANGELOG.md` said the setting "could never fire" — so the CHANGELOG wording now
  names the retired side effect + the `Suppress during input = 0` remedy, and `07` #83 carries a
  review note. The claude-code review found no issues; gemini errored (no content to address).

---

## Pass 6 — 2026-07-06 — 0.3.2 PR6 (audit + tag `v0.3.2`; release sweep)

- **Method (the `docs/0.3.2.md` PR6 shape, same as 0.3.1's PR4):** full mandatory re-read
  (`04 §1`: `01`/`02`/`03 §7d`/`§11b`/`§13b`, `docs/0.3.2.md`, `UI_REFERENCE §3`/`§8`, live
  `05`–`08` + CHANGELOG); a **10-agent adversarial audit** against `main` `d8bc5d2` — 7 parallel
  evidence agents (D1+D2 · D3–D5 · D6–D8 · D9+D12 · D10+release-infra · D11+§2-coverage+hygiene ·
  stale-reference sweep), then 3 independent refuters re-attacking every PASS claim with
  file:line counterevidence; a live smoke on the bumped tree; and a release-pipeline dry-run.
- **Implemented — D1–D12 audit, all PASS (no refutation stood):**
  - **D1 (pull-based update UX):** `update.rs` single-flight check → background download →
    install only via `restart_to_apply_update` (the sole `update.install` call site, after
    `graceful_shutdown`); launch check release-builds-only (`lib.rs:1126`); manual check in tray +
    App section + NavRail footer; the presence dot renders only when an update exists; refuters
    confirmed no other install/relaunch path, no updater dialog wiring, no check timer. Live: the
    manual check quiet-failed exactly per contract (endpoint 404 until the first full release) —
    inline App-section line "Couldn't check for updates … Try again", **zero dialogs**, plugin
    `ERROR` + wrapper `WARN` in the log only.
  - **D2 (updater infra):** `createUpdaterArtifacts: true`, pubkey fingerprint `27E1C773C0BDF81E`
    **cryptographically verified** by decoding the baked key (embedded key ID matches, not just
    the comment), endpoint `releases/latest/download/latest.json`, `installMode: passive`;
    `release.yml` signs from CI secrets and drafts a non-prerelease release;
    `make-latest-json.mjs` refuses to emit without a `.sig` or on tag/version drift. **Key
    custody satisfied 2026-07-06** (manual-steps entry in `07`): CI secrets set (delegated,
    verified via `gh secret list`) + offline backup user-attested.
  - **D3 (tray scope):** exactly the six `03 §7d` menu items (`tray.rs:174-196`, no seventh);
    three state icons fed by the kernel event bus (no poller); close-to-tray default **true** /
    run-at-startup default **false** (`ipc.rs:754-755`); one-shot first-restore toast persisted
    as `app.tray_toast_done`; autostart register-before-persist with rollback; single-instance
    restores; both quit paths route through `graceful_shutdown`.
  - **D4 (no push):** no notification plugin/API anywhere in `src-tauri`; tray toggles are silent
    (`tracing::warn` on failure only); no badge counts (`UpdateIndicator` is presence-only).
    **Refuter correction accepted (record accuracy, not a violation):** the arc did add toast
    call sites beyond the one-shot restore toast — `QuickActions.tsx` (load/unload + vision
    feedback), `UpdateIndicator.tsx:47` / `AppPanel.tsx:91,99` (check/install failure) — every
    one fires only as synchronous feedback to an explicit user click; nothing is push-shaped,
    scheduled, or counting. D4 holds.
  - **D5 (reviewed import):** Pass 3 records the sister-app review verbatim (2 patterns already
    present, 2 adopted; the 3 s health-poll, textual icon, and OS notifications rejected per D4);
    `tray.rs` is built on this repo's `traits::`/`AppState`/event-bus types, not ported code.
  - **D6 (two-tier Settings IA):** Essentials membership exactly as settled (every D6-named field
    in its named tier, none extra, none missing — refuter-checked field-by-field); seven Advanced
    expanders, collapsed by default, per-session store; intros everywhere; the #100 conflict
    warning normalizes case + modifier order, no false positive on empty chords, `role="status"`.
    Live: all 7 expanders `aria-expanded=false` on the bumped tree.
  - **D7 (presentation-first):** the arc's whole `Settings`-struct delta =
    `storage_jpeg_quality` + `capture_uia_run_on_interactive` removed, `app_close_to_tray` +
    `app_run_at_startup` added — zero renames, zero retained-key semantic changes (the
    `capture_uia_suppress_during_input_ms` shift is D8's pre-decided consequence, `03 §8`/#83).
  - **D8 (dead-setting mechanics):** both keys in `RETIRED_SETTINGS_KEYS`; the load path is
    tolerant by construction (named-key reads — an orphaned key is simply never read, so even a
    failed drop cannot error); save never re-emits; `UiaTriggerPolicy` gone; regression tests
    pinned; zero references to `jpeg_quality` in `crates/store`.
  - **D9 (shell layout contract):** `UI_REFERENCE §8` acceptance-grade contract present;
    `AppShell` main = the single route scroll context (`scrollbar-gutter:stable` +
    `contain:paint`); NavRail `relative z-rail isolate`; Recall window-scroller; the refuters'
    independent overflow sweep found only the authorized scrollers (palette listbox, Flow
    overlay window, the two Moment `<pre>` exemptions) — 0 nested-scroll violations, StatusRail
    skeleton/chip width parity 1:1.
  - **D10 (zero DB schema migrations):** `schema.rs` byte-identical to `v0.3.1`
    (`LATEST_SCHEMA_VERSION = 10` both sides, `MIGRATIONS` ends at 10); the arc's only added SQL
    is a `GROUP BY` change + a `DELETE` on the existing `jobs` table; zero DDL in the 79-file arc
    diff. Live boot on the real dev DB: `store opened … schema_version=10`, no migration.
  - **D11 (#88 fold-forward):** `07` #102 sits next to #98; issue #88 OPEN + `deferred-0.4.0` +
    the rationale comment (role separation = a sessions-schema requirement).
  - **D12 (no visual redesign):** `git diff v0.3.1..HEAD -- ui/src/styles/` **empty**
    (tokens/globals byte-identical); tailwind/postcss configs unchanged; no hardcoded style
    values added anywhere in the arc diff (`z-rail` consumption is usage of a pre-existing
    token); `07` #104 present.
  - **§2 disposition coverage:** every `docs/0.3.2.md` §2 row verified carried — #96/#97
    (built, close at tag), #100 ✅, #83 ✅, JPEG quality (dispositioned via CHANGELOG + `03 §8`
    per the roadmap's own routing), #102/#98, #91, #75 — plus #103 (D6 search deferral), #104
    (D12), #105 (PR3 decisions), #106 (ghost rail: open, upstream class, mitigation shipped),
    the updater-key manual step (now ✅), and the Authenticode step (open **by design**).
  - **GitHub hygiene:** open issues reconciled — #69 (closes at publish, runbook), #88
    (dispositioned), **#89 surfaced undispositioned** (filed 2026-07-05, after the roadmap froze
    — the exact #84-precedent shape) → **maintainer decision: fix in PR6.** Fixed in this PR's
    first commit: `reportFileStem` (`ui/src/lib/time.ts`) gains the report kind for
    daily/weekly (`screensearch-report-daily-YYYY-MM-DD-HHmm`), custom keeps the bare 0.3.1 D2
    shape; `ReportView` passes `request.kind`; the backend `sanitize_report_stem` passes the new
    stem through untouched; `UI_REFERENCE` naming contract amended. No open PRs; no 0.3.2
    milestone exists (conditional in PR1 — none created, no action).
- **Verification (verbatim, release tree at the bump commit):** `npm ci` ok · `npm run lint`
  (eslint, exit 0) · `npm run build` `✓ built in 1.71s` · `node scripts/stage-mcp.mjs` "up to
  date" · `cargo fmt --all -- --check` clean · `cargo clippy --workspace --all-targets --
  -D warnings` `Finished dev profile … in 8.76s` (clean) · `cargo build --workspace` `Finished
  dev profile … in 27.11s` · `cargo test --workspace` → **47 suites, 523 passed, 0 failed** ·
  `git diff --exit-code -- ui/src/bindings` clean.
- **Release-pipeline dry-run (`release.yml` `workflow_dispatch` off this branch, secrets live):**
  run `28808288616` → **success** (every step green; the version gate + Draft-Release steps
  correctly skipped on a non-tag ref). The `updater-bundle` artifact contains exactly the release
  triple: `ScreenSearch_0.3.2_x64-setup.exe` (13,430,977 B) + `.exe.sig` (424 B, "signature from
  tauri secret key") + `latest.json` (`version: "0.3.2"`, URL
  `releases/download/v0.3.2/ScreenSearch_0.3.2_x64-setup.exe`, non-empty signature). 7z listing
  of the CI installer shows `screensearch.exe` **and** `screensearch-mcp.exe` bundled (the 0.3.1
  provenance-residual check). This is the pre-tag proof of "release with a signed updater
  manifest" — the tag build repeats the identical pipeline with the Draft-Release step live.
- **Live smoke (bumped tree, `npm run tauri dev` + WebView2 CDP):** clean boot
  (`schema_version=10`, OCR/UIA/sysmon/throttle/workers up, sidecar lazy); Settings two tiers +
  7 collapsed expanders live; quick actions present in the NavRail footer; manual update check →
  quiet inline failure, zero dialogs (expected 404 — the endpoint resolves only after the first
  published full release); answer model loaded via the Settings control; a real daily report
  generated and downloaded → toast `Report saved → …\Downloads\`
  **`screensearch-report-daily-2026-07-06-1901.md`** (the #89 fix observed working); app + sidecar
  exited to 0 processes (no orphan).
- **Skipped / deferred:** the weekly filename variant was not separately live-run (same one-line
  kind segment as the live-verified daily); the full PR4 screenshot matrix was not re-run (PR4's
  gate, static contract re-verified instead); close-to-tray/single-instance native interactions
  rest on Pass 3's live record (not re-executed; statically re-verified by two agents).
- **Hallucinated / corrected:** the evidence pass initially claimed the one-shot restore toast
  was the arc's only added toast — refuted (see D4 above) and corrected here. Two cosmetic claim
  fixes: no `rust-toolchain.toml` exists (toolchain parity comes from `@stable` in both
  workflows); the D6 conflict-warning message shows one chord form when the colliding chords are
  typed in different orders (message clarity only; detection is order-insensitive).
- **Broke / regressed:** nothing observed; the bump + #89 fix are the arc's only code deltas in
  this PR.
- **Still risky:** the v0.3.2 installs are the first updater-capable population — a first-release
  updater bug is unfixable *by* the updater (mitigated by the PR2 E2E + this dry-run; worst case
  is another manual download, today's status quo). The release **must be published as a full
  (non-prerelease) release** or `releases/latest/download/latest.json` never resolves (every
  historical release was a prerelease — this is the one new failure mode at publish time).
  Ghost-rail (#106) mitigation remains not CI-verifiable; re-check on WebView2 updates.

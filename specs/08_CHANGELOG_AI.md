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
> Shipped 0.3.1 history (post-0.3.0 bridge fixes + PR1–PR4) →
> `specs/archive/08_CHANGELOG_AI.v0.3.1.md`.
> Shipped 0.3.2 history (PR1–PR6) → `specs/archive/08_CHANGELOG_AI.v0.3.2.md`.
> Shipped 0.3.3 history (the browser-freeze hotfix) → `specs/archive/08_CHANGELOG_AI.v0.3.3.md`.
> Live file holds only the current arc.

---

## 2026-07-11 — 0.4.0 PR5 Pass 14: post-review final clean integrated suite

- **Verification:** Ran the color-disabled UI-first sequence after the PR #104 fixes: `npm ci` →
  `npm run test` → lint → UI build → MCP staging → fmt → workspace clippy/build/test → generated-
  binding guard. All ten commands exited 0; the npm allow-scripts warning was non-failing.
- **Evidence:** `specs/05_BUILD_REVIEW.md` Pass 14 preserves each captured log verbatim, including
  all 1,091 workspace-test lines and the empty fmt/binding-guard outputs. An exact source-vs-fence
  comparison passed for all ten blocks.
- **Scope:** verification/docs only; no code, schema/migration, API/MCP, frame behavior, or generated
  binding changed in this pass.

## 2026-07-11 — 0.4.0 PR5 PR #104 review follow-up

- **Change:** Addressed all four unresolved inline review threads in code `c360d7e`, clustered into
  three fixes because the two store threads were duplicates. The session-content preflight is now an
  indexed, early-stopping SQLite `EXISTS` query with an explicit character set proven identical to
  Rust `str::trim` Unicode whitespace. Timeline error and empty states reserve the same four session
  lanes as loading/populated. Scheduler reconciliation compares session/artifact state, writes only
  assignment/artifact deltas, and suppresses `sessions_changed` for a semantic no-op while retaining
  conservative invalidation if a multi-write pass fails after a possible partial commit.
- **Why:** The review findings were applicable correctness/performance issues inside the settled
  PR5 session UI contract: avoid loading every content row, keep D9 geometry state-stable, and keep
  the pull-based refresh event truthful rather than firing once per scheduler cadence.
- **Verification:** Focused tests went RED then GREEN for the SQL VM early stop plus exact Unicode
  whitespace, Timeline error/empty fixed lanes, and unchanged-pass event suppression. Full store and
  kernel focused suites, UI test/typecheck/lint/build, fmt, and focused clippy passed; exact evidence
  is preserved in `05` Pass 13. Independent post-fix review found no critical, important, or minor
  findings and returned Ready: yes after its scheduler 13/13, store sessions 10/10, and UI 2/2
  focused reruns. The post-review full workspace suite is intentionally a later pass.
- **Review handling:** No bot replies or review-thread state changes were made, per maintainer
  instruction. No schema/migration, API/MCP, audio, notification, nudge, score, NavRail, frame-level
  behavior, or generated binding changed.

## 2026-07-11 — 0.4.0 PR5 Pass 12: final clean integrated suite

- **Verification:** Ran the color-disabled UI-first sequence at tip `8629e0c`: `npm ci` → additional
  `npm run test` regression gate → lint → UI build → MCP staging → fmt → workspace clippy/build/test
  → generated-binding guard. All commands exited 0; the npm allow-scripts warning was non-failing.
- **Evidence:** `specs/05_BUILD_REVIEW.md` Pass 12 preserves every command's raw output verbatim,
  including the 1,084-line `cargo test --workspace` log and empty fmt/binding-guard blocks.
- **Scope:** verification/docs only; no code, schema, API/MCP, frame behavior, or generated binding
  was changed by this record.

## 2026-07-11 — 0.4.0 PR5 Pass 11: final review-fix native acceptance

- **Change:** Recorded the real `npm run dev` Tauri/WebView2 acceptance of code `02e5cad` against docs
  `67b76ce`: typed-null `sessions_changed` fired once after startup scheduler work without a toast;
  loading, empty, 7-day populated, and dense 30-day all retained the fixed 140/192 grid/outer
  geometry; dense overflow rendered the neutral range-narrowing control.
- **Keyboard/layout evidence:** the dense view showed 12 visible bands plus
  `9 more sessions — narrow the range`, zero overlaps, and 1280 document/viewport parity. Native CDP
  Enter moved focus from the keyboard-focusable overflow button to `TODAY` inside `aria-label="Time
  range"`. Loading used `32px 32px 32px 32px` and five skeleton elements; the 7-day view showed 21
  visible bands with identical geometry.
- **Why:** closes the final live acceptance for the explicit fixed-four-lane D9 decision and quiet
  scheduler invalidation protocol (`05` Pass 11; `docs/TESTING.md`).
- **Scope:** documentation evidence only. No schema, API/MCP, frame, code, or generated-binding change.

## 2026-07-11 — 0.4.0 PR5 final review: fixed session geometry + live refresh

- **Change:** Recorded code commit `02e5cad` (`fix(sessions): stabilize live session bands`): Timeline
  reserves exactly four band lanes in initial skeleton/loading/error/empty/populated; collisions that
  need lane 5+ aggregate into a neutral keyboard control focusing the existing range presets. A
  typed-null `sessions_changed` event now invalidates mounted session queries after successful
  scheduler work, without any notification/toast surface.
- **Why:** the five-simultaneous-session RED exposed a genuine D9 ambiguity: unbounded normal-flow
  lane growth changed route geometry. The user explicitly selected fixed-four-lane option 1; no-CLS
  remains binding (`UI_REFERENCE §3`/`§8`, `03 §7`, `06` #31).
- **Verification:** layout RED `0 !== 4` (0 pass / 1 fail); GREEN `npm run test` 1 pass / 0 fail,
  controller rerun 65.3531 ms; typecheck/lint clean; build 438 modules in 1.59 s. Refresh RED E0425
  (missing `run_scheduler_pass`) + E0599 (missing `KernelEvent::SessionsChanged`); GREEN scheduler
  event test 1 passed / 0 failed / 57 filtered; focused kernel+screensearch all-target clippy clean.
- **Scope:** documentation normalization only in this follow-up. Code/schema version, API/MCP, audio,
  notifications, nudges, scores, NavRail, and frame-level behavior remain unchanged.

## 2026-07-10 — 0.4.0 PR5 Task 3: native runbook and integrated verification record

- **Change:** Added the PR5 native/WebView2 acceptance runbook to `docs/TESTING.md`; reconciled the
  final review behavior (measured token-aware session-band packing, normal-flow lane expansion,
  absolute band labels, Recap backend cancellation, and separate lazy-summary failure) across the
  human changelog and live build-loop records; and recorded the integrated UI-first verification in
  `05` with raw command output.
- **Why:** `03 §13c-5`, `UI_REFERENCE §7`/`§8`, and the PR5 Task 3 brief require native acceptance to
  be reproducible and automated evidence to remain distinct from real WebView2 observation.
- **Contract status:** no schema change or migration (schema remains 11), no new contradiction, and
  no new silent-spec gap. API/MCP remains PR6-owned.
- **Verification:** full UI-first command output is preserved verbatim in `specs/05_BUILD_REVIEW.md`;
  the subsequent real `npm run dev` Tauri/WebView2 acceptance is recorded in `05` Pass 9 and
  `docs/TESTING.md`. It passed the live band/drill-in/Moment round trip, exact ownership of all 39
  Recap citations, Recap cancellation, keyboard entry, layout/DPI, reduced-motion, low-confidence,
  no-exchange, and Settings checks. The current 21-session dataset had no open row, so that variant
  is transparently unavailable rather than claimed. No app runtime errors were observed.

## 2026-07-10 — 0.4.0 PR5 Task 2: session recall surfaces

- **Change:** Added generated-type IPC/TanStack Query integration for session lists, base/lazy detail,
  and Recap; deterministic Timeline bands whose configured ribbon height is a minimum and whose
  token-height normal-flow lanes expand as needed, with no hidden lanes or nested or horizontal
  scrollbar; the code-split session drill-in with inline
  summary/exchanges, wrapped representative frames, existing report rendering and citations; safe
  Session↔Moment route-state round-trips; optional Moment/Deck session context; and the collapsed
  Advanced Sessions settings group. Capture and retention now invalidate session caches with frames.
- **Why:** `03 §7`/`§7e`/`§8b`, `UI_REFERENCE §3`–`§8`, and the PR5 Task 2 brief require sessions to
  become visible inside existing routes without a NavRail addition, new report machinery, duplicate
  server state, nested scrollbars, or frame-level behavior change.
- **Verification:** `npm run lint`, `npm run typecheck`, and `npm run build` pass. The production build
  emits a distinct `Session-*.js` chunk. Focused state review covers invalid/missing/loading/error/
  partial/populated drill-in, session-band loading/error/empty/populated, open boundaries, absent
  exchanges/session references, summary/Recap retries, and direct-link versus route-state navigation.
- **Remaining acceptance:** native real-data/sidecar/DPI walkthrough under `03 §13c-5`; no browser-only
  mock was introduced to make that integration appear exercised.

## 2026-07-10 - 0.4.0 PR4: production sessions engine + recognition

- **Change:** added `crates/sessions` as the production home for the pure concurrent segmenter,
  tuned v3 taxonomy, deterministic confidence, and conservative exchange extraction. The concrete
  provider is wired by `src-tauri`; `kernel` depends only on the new `SessionSegmenter` trait. Added
  schema-11 session persistence methods with unfrozen SQL guards and frozen-frame ownership guards;
  the kernel now reconciles the mutable 24 h tail, freezes closed rows, refreshes AI exchanges, and
  advances one throttle-aware historical chunk per 60 s tick through the raw
  `sessions.backfill_done_until` checkpoint. Added lazy in-app title/summary generation with one
  model call and cached `summary_model` provenance; no IPC/API command is added in this PR. Added the
  two final typed settings/clamps and the `--algo shipped` harness adapter + synthetic parity gate.
- **Implementer's calls:** (1) the crate tax is worthwhile: `crates/sessions` keeps the algorithm
  pure and independently testable while the provider seam preserves `03 §2`; (2) only
  `gap_close`/`min_len` come from settings — merge/absorb/meeting/focus/density/qualification and
  W are named constants at the D9-frozen values; (3) exchange artifacts are delete-then-inserted on
  every mutable AI reconciliation (therefore also on close) and once during frozen backfill,
  idempotently; (4) confidence bands are anchored `0.70..=0.95` (absorbed-time penalty),
  focus-with-stem `0.45..=0.65` (density margin), bare focus `0.30`, keeping every anchorless result
  below every anchored result; (5) the single raw checkpoint key stores `{cursor_ms,target_ms}` JSON
  so the initial one-shot horizon is resumable without adding another typed setting; empty scanned
  ranges advance only to the scanned chunk edge, never to an unscanned future target; (6) gap #117
  is resolved by mapping app stem `chatgpt` to `codex` while excluding a title containing
  `ChatGPT Classic`; the canonical taxonomy moved to `crates/sessions/taxonomy.toml`, and both
  production and referee parsers apply that negative evidence.
- **Why:** `docs/0.4.0.md` PR4 / `03 §7e` / `06` #26–#28. This productionizes the already-approved
  concurrent ground-truth model without schema or frame-level changes (D4/D10), keeps generation
  lazy and in-app-only (D3/D12), and preserves honest no-match exchange behavior (D8).
- **Verification so far:** focused red/green suites cover 20 sessions-core tests, seven store
  persistence tests, scheduler reconciliation/frozen guards/chunk cuts, settings clamps, lazy
  intelligence, and shipped↔harness parity. The binding D9 re-run on 07-07/08 + held-out 07-09
  **met with no retuning**: shipped pooled partitioned F1 `0.400/0.489` at ±120/180 s, tool
  `9/11 = 0.818`, daily counts `8/7/6` vs labels `11/5/8`; micro `0.077/0.086`, grouped
  `0.391/0.435`. Full CI ladder + live backup/backfill checks are recorded in `05` before PR open.
- **Live-check amendment:** real Codex UI text exposed a false-positive in the first marker set: the
  standalone `Codex` navigation label started an oversized agent block. Tightened desktop extraction
  to observed, bounded Codex structures (`Q … File` prompt; `Working/Worked for <duration>` response)
  and required colons for generic desktop/browser role headings. A live-shaped regression is green
  (sessions crate now 3 unit + 18 integration tests). The clean live recomputation is paused mid-
  checkpoint for the user-requested break; exact DB/process/resume disposition is in `05`.
- **Live-check amendment 2:** resumed capture exposed Explorer breadcrumb rows beginning with ASCII
  `>` inside Claude-Code-attributed sessions; the real Claude prompt glyph is `❯`. Removed the ASCII
  alias and made an empty `❯` inert so it cannot absorb the terminal status bar. Two red/green live-
  shaped regressions pin both cases. Final artifact recomputation: no breadcrumb matches, no invalid
  roles, no exchanges on non-AI sessions; genuine inline Claude prompts + `●` outputs remain. The
  full workspace/UI ladder was rerun after these changes (verbatim excerpts in `05`).
- **Open-PR review follow-up:** evaluated all six unresolved inline threads plus Claude's top-level
  review. Added red/green regressions and fixed four applicable areas: inclusive equality at the
  frozen last-frame boundary; exact reuse and completion of an unfrozen crash-interrupted historical
  row (assigning only its still-unowned frames before freezing); inclusion of both endpoints in lazy
  summary sampling; and one-time taxonomy normalization during parse. The fragmented-interrupter
  suggestion was rejected because the frozen harness deliberately counts a fragmented same-key
  presence span and D9 requires shipped parity; the release-assert suggestion was rejected because a
  panic would violate D10's no-sessions degradation contract and no duplicate-producing path was
  demonstrated. PR review automation is now recorded in `AGENTS.md`/`CLAUDE.md`: no routine bot
  mentions, ritual merge warnings, or bot replies. The post-review D9 rerun remained unchanged:
  shipped F1 `0.400/0.489`, tool `0.818`, ahead of micro `0.077/0.086` and grouped `0.391/0.435`.
  A later automatic Claude pass then found that reconciliation counted a zero-length endpoint touch
  as overlap; a red test reproduced stable-id reuse across adjacent windows, and the matcher now
  requires strictly positive overlap.

---

## 2026-07-10 - 0.4.0 PR3: sessions schema + migration (10 -> 11)

- **Change:** appended `MIGRATION_V11` (`crates/store/src/schema.rs`) and bumped
  `LATEST_SCHEMA_VERSION` 10 -> 11 — the sessions arc's only schema change (D4). Creates the
  `sessions` and `session_artifacts` tables, the `frames.session_id` column, and the four indexes
  (`idx_sessions_time`, `idx_frames_session`, `idx_artifacts_session`, `idx_artifacts_frame`),
  transcribed verbatim from the authoritative DDL in `03 §4`. Structure only, no backfill, no table
  rebuild (PR4's segmenter assigns history later). Updated the three post-migration-latest tripwire
  asserts to 11 and extended the fresh-vs-migrated parity test to span v11. Added five inline
  `migration_tests` (structure-only / no-backfill, sessions CHECKs, the compound artifact role CHECK
  incl. the load-bearing NULL-role-on-exchange rejection, FK SET-NULL/CASCADE behaviors + the
  `idx_artifacts_frame` query-plan assertion, and the D10 additivity proof: seven store surfaces
  identical pre/post on a populated fixture) plus an env-gated `#[ignore]` Gate 0 live-copy test
  (`crates/store/tests/store.rs`). Re-normalized the `03 §4` `context_key` column comment to the
  `06` #27/#28 closed grammar; logged the `03`-vs-`0.4.0.md` DDL divergence as `06` #29 (03 wins,
  by-design PR1 normalization). **Post-review amendment (`06` #30, PR #102):** added
  `idx_artifacts_frame ON session_artifacts(frame_id)` to both `MIGRATION_V11` and `03 §4` in lockstep
  after the review flagged the `frame_id` FK (`ON DELETE SET NULL`) as the one unindexed v11 FK
  delete-path — a frame-retention delete would otherwise full-scan `session_artifacts`. Additive, no
  behavior change; the DDL stays a verbatim transcription.
- **Why:** `docs/0.4.0.md` §3 PR3 / `03 §4` "0.4.0 migration" / `03 §13c.3` (D1/D4/D8/D10). Additive
  (D10) — every frame-level feature is proven unchanged; structure-only +1 forward-only migration (D4).
- **Verification:** `cargo test -p store --lib migration` -> **11 passed** (5 new v11 tests + the
  parity test now spanning v11). Full ladder green (`05` Pass 4): UI lint + `built in 2.02s`;
  `stage-mcp` up to date; `cargo fmt --check` clean; `clippy --workspace --all-targets -D warnings`
  no warnings; `cargo build --workspace` ok; `cargo test --workspace` all suites ok (store lib 38,
  store.rs 63 + 1 ignored Gate 0); `git diff --exit-code -- ui/src/bindings` clean (no ts-rs changes).
  Gate 0 (D5) on a throwaway copy of the real live DB: `Gate 0: migrated 3036 frames 10 -> 11 in
  145.678ms (fk clean, sessions empty)` — the live DB itself was never opened by this build.

---

## 2026-07-10 - 0.4.0 PR2b: PR review fixes (3 harness correctness findings)

- **Change:** addressed three automated-review findings on the open PR, all dev-only harness code.
  (1) `main.rs` `overlaps_any` (the replay `~` concurrency marker) compared spans by pointer, but
  the detailed session list and the `spans_for` pass are separate allocations, so the self-copy was
  never excluded and every overlapping session was marked concurrent; now excluded by value (frame
  range). (2) `group.rs` `group_concurrent` re-sorts after `enforce_non_overlap_per_track`, whose
  same-track start-forward clamp could push a start past a different-track session and trip the
  global-sort `debug_assert!`. (3) `score.rs` `tool_accuracy` now restricts the max-overlap candidate
  set to predicted `Kind::Ai` spans, so a labeled AI session that overlaps a longer meeting/focus span
  is no longer stolen by it (which would underreport the D9 primary tool-accuracy gate).
- **Verification:** `cargo test -p harness` **119 passed** (+3 regression tests); fmt + clippy
  `-D warnings` clean. Re-scored at frozen params (`merge_gap 2700`/`absorb_max 1800`, 120 s): the
  recorded evidence is unchanged — held-out **07-09** still partitioned F1 **0.29**, tool **4/4 =
  1.000**; the fixes prevent underreporting on shapes the scored days did not happen to hit.

---

## 2026-07-10 - 0.4.0 PR2b: concurrent segmenter built + validated; D9 gate SET

- **Change:** built the concurrent per-identity-track segmenter (`06` #28 / `07` #114) in the
  dev-only harness and set the binding D9 gate (`06` #26). `labels.rs` v2 (per-identity non-overlap);
  `group.rs` `group_concurrent` + `build_bands_concurrent` + `OpenTrack` + `flush_pending` (18 new
  tests); `score.rs` identity-partitioned typed-boundary referee + `Algo` enum + `score_days` +
  algo-aware sweep/stability (partitioned-boundary drift); `main.rs` 3-way `--algo
  micro|grouped|concurrent` (concurrent now the default), partitioned + posF1 output, overlap-marked
  replay, dual micro+grouped sweep baselines, non-zero exit on unknown subcommand. The serial `06`
  #27 `group()`/`segment_grouped()` path is **byte-untouched** (the `--algo grouped` A/B baseline);
  the 13 pinned `segment()` tests are unchanged.
- **Why:** `07` #114 resolved concurrent — real usage runs parallel recognized tools, which the
  serial model collapses. Sessions of different identities may overlap in wall-clock time while a
  frame stays owned by exactly one session (so PR3's schema 11 is unchanged). The gate is
  **recognition-primary with a boundary-F1 floor** because boundary F1 is structurally capped by
  foreground-only capture (`07` #117) — recognition is the arc's payoff.
- **Verification:** `cargo test -p harness` **116 passed**; fmt + clippy `-D warnings` + binding
  guard clean. Evidence (harness binary, `--algo concurrent`, identity-partitioned, 120/180 s, frozen
  `merge_gap 2700 s`/`absorb_max 1800 s`): tuning pooled F1 **0.452/0.581** (tool 0.714); **held-out
  07-09** F1 **0.286**, tool **1.000**, beating grouped **0.167** and micro **0.068**; stability
  6 h-stable (W = 24 h). Full numbers + gate → `05` Pass 3 final, `06` #26/#27/#28. Read-only export
  verified (live DB byte-identical); `harness-data/` git-ignored (labels never committed).

---

## 2026-07-10 - 0.4.0 PR2b: concurrent session model (`07` #114 resolved) - specs gate

- **Change:** recorded the concurrency resolution in the spec channel **before any code** (the #27
  procedure). `07` #114 → **resolved: concurrent** (option b), with the finding that **exclusive
  frame ownership** keeps overlapping sessions inside **schema 11 with zero DDL change** (PR3
  unaffected). `06` #28 added — the per-identity-track amendment layered on #27 (track map replaces
  the single open group; anchor selection + `HOST_PRECEDENCE` inert on the shipped path, kept in the
  `--algo grouped` serial baseline; anchor **qualification** survives via `IDENTITY_QUALIFY_MS`;
  `SustainedForeignIdentity` close removed; per-track None-budget absorption into the last-touched AI
  track; meeting bands no longer barriers; `labels.toml` v2 per-identity non-overlap;
  identity-partitioned referee metric). `06` #26 (D9 gate) flipped DEFERRED → **unblocked, lands at
  PR2b Phase C** on the partitioned metric with the held-out fresh-day gate. `07` #116 added (accepted
  concurrency limitations: same-tool instances and `browser-ai` fold to one track; thin overlap
  evidence → re-verify per new labeled day). `05` Pass 3 opened.
- **Why:** the serial `§7e`/#27 model collapses parallel recognized tools into one band; PR2's
  fresh-day labeling proved real usage is concurrent. The contract must change (through `06`/`07`)
  before the harness code that depends on it (stop-at-ambiguity, `04 §5`). Keeping the serial
  redesign as the A/B baseline preserves the validated 0.50/0.57 anchor.
- **Verification:** `git diff --name-only` shows specs only (`05`/`06`/`07`/`08`); no code touched in
  this commit. Baseline `cargo test -p harness` = 91 passed on the branch before edits.

---

## 2026-07-10 - 0.4.0 PR2: task-level grouping redesign landed + validated; D9 deferred (concurrency)

- **Change:** implemented the `§7e` task-level grouping redesign in the harness (taxonomy v3 spinner
  rule; `segment_micro` unfloored pass 1; new `group.rs` two-pass engine = meeting bands + an
  identity-anchored accretion walk; score.rs sweep/stability rewired to the grouped pipeline with
  label snapping + Stage-A/B sweeps + `IDENTITY_QUALIFY` threading; main.rs `--algo grouped|micro`,
  dual 120/180 s scoring, honesty columns, close-reason replay). `segment()` + its 13 pinned tests
  are unchanged as the A/B baseline. Recorded the fresh-day findings `07` #114 (session concurrency)
  and #115 (recall-based labels unreliable + non-work fullscreen activity uncapturable).
- **Why:** `docs/0.4.0.md` §3 PR2 - validate the segmenter before PR3 freezes schema 11. The redesign
  fixes the #110 over-segmentation (grouped F1 0.50/0.57 vs 0.14/0.16 baseline through the referee;
  16 vs 129 predicted sessions, in-sample, NOT the D9 gate). But labeling the fresh day surfaced that
  real usage can be **concurrent** (more than one recognized tool active at once), which the serial
  model cannot represent, and that ground truth must come from the capture, not recall. So **D9
  thresholds (`06` #26) are deferred** until the
  serial-vs-concurrent session-model decision (`07` #114) is made - an acceptance gate cannot be fixed
  against a target of unsettled shape.
- **Verification:** `cargo test -p harness` 87 passed / 0 failed; fmt + clippy `-D warnings` + binding
  guard clean; full CI ladder run at PR time (verbatim in the PR body). Score/sweep/stability run
  offline against the git-ignored exports; only aggregate numbers recorded.
- **Status:** PR opened for review of the built serial redesign + harness; **not** claiming final
  acceptance numbers, **not** merging. PR3 must not freeze schema 11 until #114 is decided.

## 2026-07-10 - 0.4.0 PR2: Phase B redesign kickoff - the #110 task-level grouping (specs gate)

- **Change (`.md`-only this commit):** recorded the `§7e` **task-level grouping amendment** as
  `06` **#27** (open decision row) after the maintainer confirmed the over-segmentation is
  representative of every real day ("no calm days"), settling the redesign direction. Sessions become
  **task-level anchored groups** (two-pass: micro app-runs -> macro identity-anchored groups):
  `sessions.context_key` stores the closed grammar `ai:<tool> | meeting:<id> | focus:<stem>`; a
  session closes on `merge_gap_secs` inactivity, a foreign recognized identity sustained beyond
  `absorb_max_secs`, or a meeting-band edge; foreign runs up to `absorb_max` are absorbed; anchorless
  focus sessions require `focus_min_len_secs` + the `focus_min_density_fph` gate. Reserved `06` **#26**
  for the binding D9 gate. Added `07` **#112** (accepted limitations: the invisible back-to-back tool
  handoff = one merged AI band; the one-data-point `HOST_PRECEDENCE`/`IDENTITY_QUALIFY_MS` constants;
  spinner specificity) and **#113** (density-gate honesty: ON at 90 fph, AI/meeting exempt, residual
  leisure FP accepted via D11 confidence, fresh-day validation required). `#110` marked
  redesign-active; `#111` resolving via taxonomy v3.
- **Why:** `docs/0.4.0.md` §3 PR2 - the segmenter must be validated on real labeled data before PR3
  freezes schema 11, and the evidence (pooled boundary F1 0.128, parameter-independent across the whole
  sweep; 34-351 app-runs per labeled session) shows the app-context key structurally cannot yield
  task-level sessions. The amendment goes through the `06`/`07` spec channel **before** the code that
  depends on it (stop-at-contradiction), resolved at Phase C with the harness rerun numbers + commit.
  **Zero DDL change** - macro rows satisfy every schema-11 CHECK; PR3 re-normalizes only the `§4`
  context_key comment via the sanctioned escape hatch (`03 §4`).
- **Engineering calls recorded:** the redesign came from a read-only design panel (analyze -> 3
  independent designs -> 2 judges -> synthesis) run entirely over the git-ignored exports; the winning
  "anchored two-pass grouping" chassis keeps `segment()` + its 13 pinned tests as the A/B baseline and
  adds a `group.rs` macro pass, per the harness seams. Eleven product calls were taken from the
  maintainer (unlabeled = no-session; 19:00-class = one merged band; density gate ON at 90 fph;
  label-snapping ON; meeting-band floor 10 min; sweep decides merge_gap).
- **Verification:** `git diff --name-only` = specs `.md` only (no code touched this commit). Full CI
  ladder + harness rerun numbers land with the code commits and Phase C.

## 2026-07-09 - 0.4.0 PR2: Phase B interim (over-segmentation finding + codex/vscode fix)

- **Change:** kicked off Phase B on 2 hand-labeled ground-truth days (07-07, 07-08). Verified the
  maintainer's rough timings against the **shipped live local API** (`GET /v1/export`), wrote the two
  `labels.toml` (git-ignored), and scored the candidate segmenter. Fixed a recognition bug: app-hint
  matching was substring, so `codex`.contains(`code`) mis-tagged the Codex desktop app as `vscode`;
  app-hint matching is now **exact-stem** (`.exe`-stripped), title stays substring; the `codex` entry
  is corrected to a desktop app and the unused `vscode`/`cursor` entries dropped (`taxonomy.toml`
  version 1 -> 2). Recorded the headline finding in `07` #110 and the claude-code title limitation in
  `07` #111; expanded `05` Pass 2.
- **Why:** `docs/0.4.0.md` §3 PR2 - the segmenter must be validated on real labeled data before PR3
  freezes schema 11. The evidence shows the `§7e` app-context key over-segments real fragmented days
  ~10-40x (pooled boundary F1 0.13; the whole param sweep 0.09-0.13; `replay` = one session per
  app-run, 22/44 vs the labeled 5/2), and no parameterization fixes it. Maintainer decision
  (2026-07-09): **flag-and-gather** - fix the bug, do NOT freeze PR3, accumulate more days before the
  redesign call.
- **Verification:** `cargo test -p harness` 58 passed / 0 failed; `cargo fmt --all -- --check` clean;
  `cargo clippy -p harness --all-targets -- -D warnings` clean; `git diff --exit-code -- ui/src/bindings`
  clean. Tool recognition on the 2 labeled days rose 0.20 -> 0.40 after the fix; boundary F1 unchanged
  (structural). Score/sweep/replay run offline against the git-ignored exports; only aggregate numbers
  are recorded (no personal titles).
- **Status:** PR2 **paused short of the D9 thresholds** (and the `06` #26 gate) pending more days + the
  #110 redesign decision. Branch pushed to `origin` as a backup; not a PR.

## 2026-07-07 — 0.4.0 PR2: segmentation validation harness (dev-only; Phase A paused for data)

- **Change:** added `crates/harness` — a dev-only, read-only segmentation validation harness (the
  candidate segmenter = `resume.rs` generalized per `03 §7e`, the D7 seed `taxonomy.toml` + matcher,
  read-only export + digest + labels template, the D5 `VACUUM INTO` backup, and typed DP-optimal
  boundary/tool scoring + parameter sweep + freeze-lookback stability). Standalone workspace crate
  (no internal-crate deps, never bundled, no `ts-rs`); `harness-data/` git-ignored for the personal
  exports + labels. Added `toml` to `[workspace.dependencies]`. `docs/TESTING.md` gains a harness
  section. Ran the D5 pre-migration backup of the live DB.
- **Why:** `docs/0.4.0.md` §3 PR2 — no segmenter ships untested against the maintainer's real data;
  the harness is the referee that produces the binding D9 thresholds (recorded in `05`/`06`) before
  PR3 freezes the schema. Read-only + dev-only + git-ignored personal data per the PR2 constraints.
- **Verification:** 57 harness lib tests; `cargo test --workspace` 581 passed / 0 failed; clippy
  `--workspace --all-targets -D warnings`, `fmt --check`, UI `lint`/`build`, and the ts-rs binding
  guard all clean (verbatim in `05` Pass 2); harness end-to-end on a synthetic fixture; the D5 backup
  printed `integrity_check = ok` + 558/558 frame parity.
- **Status:** **Phase A paused** — the live DB holds one day (558 frames, 0 marks), well short of the
  5–10 representative days the evidence phase needs. Maintainer chose to accumulate multi-day usage,
  then resume labeling → scoring → D9 thresholds. Branch local/unpushed, no PR, until the evidence
  exists. Early finding: the seed terminal patterns do not recognize this machine's Claude Code /
  Codex titles (to be tuned on the accumulated captures).

## 2026-07-07 — 0.4.0 PR1: specs contract (sessions arc)

- **Change:** Normalized the 0.4.0 sessions-arc contract (`docs/0.4.0.md`, "P8 — frames → sessions
  reframe", decisions D1–D16) into the specs, no code touched. New `02` §5d (arc section) and its
  §7/§8 updates; `03` gains the sessions + `session_artifacts` DDL and the 0.4.0-migration prose
  (schema 10 → 11) in §4, three proposal IPC rows in §7, the `/v1/sessions*` + `session_id`-ask
  API/MCP deltas in §7c, the new **§7e** sessions contract (segmentation context key, gap/close/dwell
  + freeze rule, taxonomy TOML file + D7 seed set, recognition outputs, best-effort exchange
  extraction, lazy title/summary + recap-is-the-report-engine), two `sessions.*` settings keys in §8,
  the D16 gate in §11b, and the new **§13c** 0.4.0 DoD; `04` reading-order + source-of-truth + build
  order; `UI_REFERENCE` session bands + drill-in spec; `07` rows #98/#102/#91 + new #107/#108 + two
  manual-steps bullets (D5 backup gate, D15 usage-signals procedure); `docs/MCP.md`, `CLAUDE.md`/
  `AGENTS.md`, `CHANGELOG.md`. This pass also completed the overdue **v0.3.3 archival sweep**
  (05/06/07/08 + `CHANGELOG.md` → the `*.v0.3.3.md` archives).
- **Why:** `docs/0.4.0.md` §3 PR1 requires the contract in the specs so a fresh agent session can
  implement any of PR2–PR6 from the specs alone, without reopening the roadmap (the standing 0.3.x
  PR1 shape). Proposal-level naming is flagged where a later PR owns the final call (the 0.3.2
  `app.*` precedent). The schema, migration mechanics, and DoD follow `03 §4`/§13b and the
  D15-of-0.3.0 forward-only-bump-by-one rule.
- **Verification:** `git diff --name-only main` → only `.md` files (verbatim on the PR); full CI
  parity run for a no-regression check (`cargo fmt --all -- --check`; `cargo clippy --workspace
  --all-targets -- -D warnings`; `cargo build --workspace`; `cargo test --workspace`; UI `npm run
  lint`/`build`; `git diff --exit-code -- ui/src/bindings` clean). Evidence in `05` Pass 1.
- **Review follow-up (2026-07-07):** added CHECK constraints tying `sessions.tool` to `kind='ai'`
  (D7) and `session_artifacts.role` to `kind='exchange'` (D8) in the `03` §4 DDL, per the automated
  reviewers — the DDL now enforces the two conditional rules its comments already stated. Still
  `.md`-only.
- **Review follow-up, round 2 (2026-07-07):** three more Codex notes on `03`, all applied: exchange
  artifacts now **require** a role (`CHECK ((kind='exchange' AND role IN ('user','agent')) OR
  (kind IN ('transcript','note') AND role IS NULL))`, matching §7e's "roles never invented"); the
  `§7c` `GET /v1/sessions/{id}` is now strictly read-only — `include_summary` returns cached-or-`null`
  and never triggers generation (D12; generation lives on the in-app IPC path); and the freeze
  lookback window is now a **named** parameter in §7e (proposed default 24 h, PR2-confirmed) instead
  of an unspecified constant. Still `.md`-only.
- **Review follow-up, round 3 (2026-07-07):** two notes applied. Fixed a SQLite three-valued-logic
  bug in the round-2 `role` CHECK (added `role IS NOT NULL` to the exchange branch — `NULL IN (…)` is
  NULL and SQLite passes a NULL CHECK, so exchange+NULL-role was still admitted). Re-keyed the
  `browser-ai` taxonomy seed off the dormant `browser_url` field (production capture sets it `None`)
  onto stored `app_hint` + window-title metadata, domain match a later refinement; recorded the
  `browser_url`-capture enhancement as `07` #109. Still `.md`-only.

- **Review follow-up, round 4 (2026-07-07):** defined `GET /v1/sessions?from=&to=` time filtering as
  an overlap query (`started_at < to AND COALESCE(ended_at, now) > from`, with half-open single-bound
  variants) so Timeline/MCP callers do not drop long or open sessions that started before the
  requested window. Still `.md`-only.
- **Adversarial-review follow-up:** fixed complete ownership of pass-1-consumed excursion frames;
  strict frozen-tail boundary trimming; a resumable fixed-target backfill stall; and delayed overlap
  against already-frozen incremental rows. Backfill now scans later frozen horizons for a safe gap,
  persists/reopens cleanly, trims historical prefixes before immutable frozen tails, verifies frame
  assignment before freezing new rows, and remains idempotent. Expanded shipped parity across None,
  meetings, gap equality, density, qualification, and open projection without changing the frozen
  harness baseline. The post-fix D9 rerun remained F1 0.400/tool 0.818. Active run instructions now
  use only `npm run dev`; never a direct executable.

## 2026-07-10 — 0.4.0 PR5 Task 1: typed session UI commands and exact Recap

- **Change:** Added `ts-rs` exports for the schema-11 session domain and four new IPC models; session
  references on frame/resume payloads; exact overlap-query normalization; bounded session detail;
  lazy cached title/summary generation; and a cancellable session Recap that reuses the existing
  report engine over only the requested session's frame ownership.
- **Why:** `03 §7`/`§7e`, `§8b`, `UI_REFERENCE §3`/`§6`, and the PR5 Task 1 brief require a typed Rust
  boundary for the Timeline drill-in, Moment/Deck links, and a Recap with no cross-track leakage or
  new summarization machinery.
- **Verification:** TDD RED→GREEN focused tests covered exports/no-bigint, store sample totals and
  endpoints, no cross-session leakage, missing/deleted references, query normalization, detail
  filtering, cached intelligence, scoped Recap citations, and no-evidence zero calls. The combined
  `cargo test -p traits -p store -p kernel -p screensearch` and task-relevant clippy passed.
- **Review follow-up:** Split the session-bearing frame/resume shapes into Tauri-only generated DTOs
  so the existing API/MCP JSON remains unchanged; made the preflight use Rust whitespace semantics;
  and clamped the store sampler itself to 24. Focused RED→GREEN tests, UI lint/build, fmt, and focused
  clippy passed; broad integration and native acceptance are now recorded in `05` Passes 8–9.

## 2026-07-11 — Docs cleanup + repository-exposure scrub (maintenance, no app changes)

- **Change:** Reduced over-exposure in a public repo and refreshed stale docs. Removed four demo
  screenshots that had been captured from a live install (real name/username/personal folders,
  messaging-app usage, real browsing OCR) — `screenshots/timeline.png` kept, the rest pending
  re-shoot on synthetic data. Replaced ~90 absolute `C:\Users\<name>\…` paths in the build-loop
  logs (`05` live + `05` v0.1.0 archive) and `01_PROJECT_CONTEXT.md` with a `<repo>` / `<backup-dir>`
  placeholder; genericized timestamped personal-activity prose in `05` (messaging-app leisure blocks,
  evening-session wording) to honor the no-personal-life-in-repo rule; sanitized the
  `crates/sessions/tests/engine.rs` fixtures to synthetic `user` / `DEVBOX`; switched the crate author
  email to a `users.noreply.github.com` address. Refreshed `docs/ARCHITECTURE.md` (15 crates, schema
  v11 sessions migration, `sessions`/`harness` crates), `README.md` (status → v0.3.3 + 0.4.0 arc,
  crate layout, the mandatory `node scripts/stage-mcp.mjs` build step, screenshots note), the shared
  15-crate line + code map in `CLAUDE.md`/`AGENTS.md`, and the `docs/API.md` health version. Moved
  the pre-0.4.0 manual acceptance checklists to `specs/archive/TESTING.pre-0.4.0.md`; deleted the
  three obsolete `docs/superpowers/` design notes.
- **Why:** Maintainer request to cut over-exposure and bring key docs current; enforces the
  no-personal-life-in-repo rule and the archive-on-release convention (`04 §7`).
- **Verification:** `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets --
  -D warnings` finished with no warnings; `cargo build --workspace` clean; `cargo test --workspace`
  all suites `ok` / 0 failed (sessions engine 21/21 with the sanitized fixtures);
  `git diff --exit-code -- ui/src/bindings` clean; `git grep -inE 'users.nicol|nicol@|estrem@gmail'`
  returns no hits; `git ls-files -- screenshots/` lists only `timeline.png`.

## 2026-07-11 — Synthetic-data README screenshots + dev-only seeder
- **Change:** Restored the five README hero screenshots (Deck, Timeline with session bands, Recall,
  Insights, Moment), all rendered against **synthetic seed data** with no personal content. Added
  `crates/store/tests/seed_demo.rs` — a CI-ignored (`#[ignore]`) integration test that seeds an
  isolated schema-11 store entirely through the public `store` API: ~120 frames across a plausible day,
  ten **frozen** overlapping sessions (focus / meeting / concurrent Claude Code), marks, and AI
  exchanges. Frames are future-dated so the sessions scheduler (past-only, never deletes a frozen row)
  leaves them intact. Screenshots were captured from the real WebView2 app (capture off, no model)
  pointed at an isolated app-data dir via a temporary, uncommitted `identifier` swap, driven over the
  WebView2 remote-debugging port. Added `docs/SCREENSHOTS.md` (the regeneration how-to), updated the
  README Screenshots section into a five-image gallery with a synthetic-data note, and logged both in
  `CHANGELOG.md`.
- **Why:** Maintainer request to re-shoot the screenshots removed in the exposure scrub, on synthetic
  data — restoring the visual preview with zero personal content and a documented, repeatable path.
- **Verification:** `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets --
  -D warnings` no warnings; `cargo build --workspace` clean; `cargo test --workspace` all suites `ok`
  (the seeder test stays ignored); `git diff --exit-code -- ui/src/bindings` clean. The demo data was
  seeded live (122 frames / 10 frozen sessions verified in the DB) and every route captured populated.

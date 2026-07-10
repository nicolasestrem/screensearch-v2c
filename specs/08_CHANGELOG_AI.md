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

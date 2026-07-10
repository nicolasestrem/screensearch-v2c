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

> Pre-0.2.x (v0.1.0) history → `specs/archive/05_BUILD_REVIEW.v0.1.0.md`.
> Shipped 0.2.x history (0.2.0–0.2.2) → `specs/archive/05_BUILD_REVIEW.v0.2.x.md`.
> Shipped 0.3.0 history (the whole arc: PR1–PR9 + post-0.2.2 bridge fixes) →
> `specs/archive/05_BUILD_REVIEW.v0.3.0.md`.
> Shipped 0.3.1 history (the P7.1 triage patch: post-0.3.0 bridge fixes PR #79/#80 + PR1–PR4) →
> `specs/archive/05_BUILD_REVIEW.v0.3.1.md`.
> Shipped 0.3.2 history (the P7.2 product-shell mini-arc: PR1–PR6) →
> `specs/archive/05_BUILD_REVIEW.v0.3.2.md`.
> Shipped 0.3.3 history (the browser-freeze hotfix: UIA skips Chromium/Electron windows) →
> `specs/archive/05_BUILD_REVIEW.v0.3.3.md`.
> Live file holds only the current arc.

---

## Pass 1 — 2026-07-07 — 0.4.0 PR1 (specs contract; specs-only, no code)

- **Implemented:** The 0.4.0 sessions-arc contract ("P8 — frames → sessions reframe",
  `docs/0.4.0.md`, decisions D1–D16) normalized into the specs: `02` §5d (new arc section) + §7
  (non-goals: audio pointer → §5d; D15 permanent no-telemetry commitment) + §8 (status: 0.3.3
  shipped, 0.4.0 active); `03` §4 (sessions/`session_artifacts` DDL + the 0.4.0-migration prose,
  10 → 11) + §7 (three proposal IPC rows) + §7b (forward pointer) + §7c (`/v1/sessions*` endpoints,
  `session_id` ask scope, three MCP tools) + new §7e (segmentation / taxonomy / recognition /
  exchanges / lazy-intelligence contract) + §8 (two `sessions.*` keys) + §11b (D16 gate) + new §13c
  (0.4.0 DoD); `04` §1/§2/§3 (reading-order + source-of-truth row + build-order bullet); `UI_REFERENCE`
  §3 (drill-in route + sessions bold-prose block) + §4 (state-matrix rows) + §5 (components) + §7
  (a11y); `07` (rows #98/#102/#91 updated; #107/#108 added; two manual-steps bullets — the D5 backup
  gate and the D15 usage-signals procedure); `docs/MCP.md` (PR6 tool-growth note); `CLAUDE.md`/
  `AGENTS.md` current-state; `CHANGELOG.md` + `08`. This pass also folded forward the overdue **v0.3.3
  archival sweep** (05/06/07/08 + `CHANGELOG.md` → `specs/archive/*.v0.3.3.md` + `CHANGELOG-ARCHIVE.md`),
  per the standing archive-on-release rule. Verification = the diff itself: `git diff --name-only main`
  shows only `.md` files (paste verbatim on the PR).
- **Skipped / deferred:** everything with a runtime surface — deliberately (PR2–PR6 implement this
  contract). The D15 convenience script (`scripts/`) is **not** built here: PR1 acceptance is "no
  code changes", so the documented reading procedure carries copy-pasteable `gh api` commands
  instead; the optional script is a later follow-up (runs on the maintainer's machine, never in the
  app). GitHub hygiene (0.4.0 milestone + `in-progress-0.4.0` label + relabel/milestone #88) is done
  right after the PR opens (0.3.2-PR1 precedent).
- **Hallucinated / corrected:** naming left explicitly **proposal-level** where a later PR owns the
  final call (the 0.3.2 `app.*` precedent): the three IPC command names + the `FrameDetail` session
  reference (PR5), the two settings keys' clamp ranges + Settings-UI home (PR4/PR5), the drill-in
  route shape `/timeline/session/:id` + component names (PR5), open-session summary semantics (PR6),
  the taxonomy file path/crate home (PR4). Recorded drift: issue **#88 carries `deferred-0.3.4`**, not
  the `deferred-0.4.0` that `docs/0.4.0.md` line 124 and `07` #102 assert — corrected in `07` #102 and
  by the relabel; the roadmap text is left as-is (settled doc). Post-0.3.3 issue triage
  (`docs/0.4.0.md` §2 lead-in): **no-op** — no issues filed since 2026-07-07; #88 is the only open
  issue.
- **Broke / regressed:** nothing — no code touched (`cargo`/UI suites run for parity only).
- **Still risky:** `03` §4's sessions DDL is a contract-with-a-PR2-escape (an inline caveat lets PR3
  re-normalize column details — e.g. `context_key` structure — if PR2's evidence demands); `06` stays
  empty unless a contradiction surfaces while implementing PR2–PR7.
- **Review response (2026-07-07):** the two automated reviewers (Gemini, Codex) both flagged that the
  `03` §4 DDL documented `sessions.tool` "NULL unless kind='ai'" (D7) and `session_artifacts.role`
  "NULL unless kind='exchange'" (D8) in comments but did not enforce them, while sibling columns
  (`kind`/`host`/`frozen`) carry CHECK constraints. Applied both — `tool CHECK (tool IS NULL OR kind =
  'ai')` and `role CHECK (role IS NULL OR (role IN ('user','agent') AND kind = 'exchange'))` — so the
  DDL now rejects the invalid `kind='transcript', role='user'` row the Codex note called out. Still
  `.md`-only. Bot-authored notes not otherwise replied to (maintainer directive).
- **Review response, round 2 (2026-07-07):** three further Codex P2 notes on `03`, all applicable:
  (1) **exchanges must require a role** — the round-1 CHECK still allowed `kind='exchange', role=NULL`,
  which contradicts §7e ("roles never invented" ⇒ no role, no exchange); tightened to
  `CHECK ((kind='exchange' AND role IN ('user','agent')) OR (kind IN ('transcript','note') AND role IS
  NULL))`. (2) **read-only GET must not mutate** — `include_summary=1` triggering lazy generation +
  caching is a hidden write under D12; rewrote `§7c` so the API/MCP surface returns cached-or-`null`
  and **never** generates, with generation moved to the in-app IPC path (`§7e` clarified to match; an
  API-triggered generation would be an explicit `POST`, out of arc). (3) **freeze lookback undefined**
  — the freeze rule referenced a lookback window that no §8 key or §7e line pinned, forcing PR4 to
  invent a hidden constant; defined it as a named parameter (proposed default 24 h, PR2-confirmed
  against the harness, recorded in `05`/`06`), deliberately **not** a user setting so the `§8` two-key
  surface holds. Still `.md`-only.
- **Review response, round 3 (2026-07-07):** two more notes, both valid and applied. (1) **Claude bot
  caught a real bug in the round-2 CHECK** — SQLite three-valued logic means
  `(kind='exchange' AND role IN ('user','agent'))` evaluates to **NULL** (not FALSE) when `role IS
  NULL`, and SQLite *passes* a CHECK that is NULL, so exchange+NULL-role still slipped through; added
  the load-bearing `role IS NOT NULL` guard to the exchange branch so it short-circuits to FALSE. (2)
  **Codex flagged the `browser-ai` seed keyed on a dormant field** — production capture hard-codes
  `browser_url: None` (`capture_loop.rs:130`, dormant per `resume.rs`/`§7b`), so domain matching can
  never fire on real frames; re-keyed browser-AI recognition in `§7e` onto stored metadata (`app_hint`
  + window-title patterns) with the domain match as a refinement that activates only if `browser_url`
  capture lands, and recorded that capture enhancement as `07` **#109** (not sessions work). The `§7e`
  context-key browser-domain term is likewise flagged dormant (matching the shipped `§7b` posture).
  Still `.md`-only.

- **Review response, round 4 (2026-07-07):** addressed the inline P2 on the sessions list range
  contract: `GET /v1/sessions?from=&to=` now explicitly uses **overlap** semantics, not
  `started_at BETWEEN` semantics. The predicate is recorded as `started_at < to AND
  COALESCE(ended_at, now) > from`, with half-open forms for single-bound requests and request-time
  `now` for open sessions, so long/still-active bands remain visible to Timeline/MCP range queries.
  Still `.md`-only.

## Pass 2 — 2026-07-07 — 0.4.0 PR2 (ground-truth + validation harness; code complete, Phase A paused on data)

- **Implemented:** the dev-only, read-only **segmentation validation harness** (`crates/harness`,
  lib+bin, standalone workspace member — no internal-crate deps, never bundled by NSIS, no `ts-rs`
  so the binding guard stays clean). Modules: `model` (export/label types; `Kind`/`Host` mirror the
  `sessions` CHECK sets; `SessionSpan` = the D9 referee unit), `labels` (per-day TOML parse +
  validation, touching allowed, `HH:MM`→epoch), `taxonomy` (D7 seed `taxonomy.toml` + substring
  matcher, app_ok AND title_ok, first-match-wins), `export` (read-only `SQLITE_OPEN_READ_ONLY` +
  `query_only`, local-day bounds via SQLite tz math with a 24h **DST guard**, `suggest-days`, the
  **D5 `VACUUM INTO` backup** with integrity + row-count attestation), `digest` (human context-run
  timeline + labels template), `segmenter` (pure; `resume.rs` generalized per `§7e` — context key
  `app ⊕ domain ⊕ tool`, close on gap ≥ `gap_close` OR sustained switch, keyless > `gap_close`
  splits), `score` (**typed DP-optimal** boundary P/R/F1 + edge exclusion, tool accuracy, sweep,
  freeze-lookback stability), `data` (loaders). **Verification (verbatim):** 57 harness lib tests;
  full workspace `cargo test --workspace` **581 passed / 0 failed**; `cargo clippy --workspace
  --all-targets -- -D warnings` clean; `cargo fmt --all -- --check` clean; UI `npm run lint` +
  `npm run build` clean; `git diff --exit-code -- ui/src/bindings` clean. End-to-end on a **synthetic
  fixture** DB (F1 1.000, tool 4/4). **D5 backup run against the live DB** (the first live-DB
  command): `screensearch-2026-07-07.db` outside the repo + app-data dir, `PRAGMA integrity_check`
  = ok, 558/558 frames, 0/0 marks matched.
- **Skipped / deferred (Phase A blocked on data):** the ground-truth labeling, scoring, and D9
  threshold proposal. `suggest-days` on the live DB shows **a single day** (2026-07-07, 558 frames,
  0 marks) — the DB was recently reset. That is far short of the 5–10 representative days + the
  contiguous 2–3-day stretch the evidence phase and the freeze-lookback stability check require.
  **Maintainer decision (2026-07-07): accumulate real multi-day usage, then resume Phase A** (the
  harness is done and waiting). D9 thresholds, the `06` **#26** binding-gate row, and the
  freeze-lookback confirmation are therefore **pending real data** (they are the resume deliverable).
- **Hallucinated / corrected — early recognition finding:** on the one available day the seed
  taxonomy recognized `claude-desktop` and `vscode`, but the machine's `windowsterminal` runs did
  **not** match `claude-code`/`codex` (the seed requires "claude"/"codex" in the window title; the
  real terminal titles do not carry them). This confirms the `§7e` "patterns are tuned in PR2 against
  real captures, not guessed" clause: the terminal title patterns will need the maintainer's real
  Claude Code / Codex title shapes, which is exactly what the accumulated data provides. The
  `vscode`/`cursor` `kind` product call (ai vs focus, seeded ai) stays open for the labeling handoff.
- **Broke / regressed:** nothing — additive dev-only crate; the full workspace suite is green.
- **Still risky:** the binding D9 thresholds must not be set on one thin, non-representative day
  (the arc's headline risk — "looks fine in design, fails on real days"). Held per the
  accumulate-first decision; the branch (`feat/0.4.0-pr2-validation-harness`) stays local/unpushed,
  no PR, until the evidence exists.

### Pass 2 continued - 2026-07-09 - Phase B interim (2 labeled days; flag-and-gather)

Data accumulated (07-07..07-09). The maintainer flagged day-kinds and gave rough session timings,
which I **verified against the shipped live local API** (`GET /v1/export` on `127.0.0.1:43210`;
bearer token read read-only from the D5 backup, never the live DB) rather than trusting memory. Took
a fresh **D5 backup** first (`screensearch-2026-07-09.db`, `integrity_check` ok, 1780/1780 frames).
Wrote ground-truth `harness-data/*/labels.toml` for the two substantive days: 07-07 = Google Meet
standup 09:58-10:30 + admin/focus + Claude desktop + two short Claude Code terminal stints (a mixed
day, not Claude-Code-heavy, correcting the maintainer's guess); 07-08 = Codex desktop 16:59-19:00 +
one Claude Code evening session 19:00-22:54 (the maintainer's call over steady Codex activity
underneath).

- **Headline finding (redesign checkpoint; `07` #110):** the `§7e` app-context key **over-segments**
  these real days ~10-40x. Baseline (seed taxonomy, defaults) pooled boundary **F1 0.13** (P 0.07,
  R 0.75); the full 6x3 sweep is **F1 0.09-0.13, parameter-independent**; `replay` = one session per
  sustained app-run (22 and 44 vs the labeled 5 and 2). The maintainer's sessions are task-level; the
  key is app-level. **Decision (2026-07-09): flag-and-gather** - record it, fix the recognition bug,
  keep PR3 / schema 11 untouched, accumulate more (incl. calmer) days before deciding the redesign.
- **Fixed in-pass (codex->vscode recognition bug, commit `6c2d746`):** app-hint matching was
  substring, so `codex`.contains(`code`) tagged the Codex desktop app as `vscode`. Now **exact-stem**
  (`.exe`-stripped) for app_hint, substring for title; `codex` corrected to a desktop app; unused
  `vscode`/`cursor` dropped (`taxonomy.toml` v1 -> v2). Tool recognition on the 2 days **0.20 -> 0.40**;
  boundary F1 unchanged (the over-segmentation is structural). Verification (verbatim): `cargo test
  -p harness` **58 passed / 0 failed**; `cargo fmt --all -- --check` clean; `cargo clippy -p harness
  --all-targets -- -D warnings` clean; `git diff --exit-code -- ui/src/bindings` clean.
- **Known limitation (`07` #111):** Claude-Code-in-terminal titles are task-titled with a `✳`/braille
  spinner prefix, so the `claude` pattern misses most of them (the spinner marker flagged 61/77 and
  22/24 frames); a spinner-aware title match is deferred to tune with the #110 redesign.
- **Branch state:** now **pushed** to `origin/feat/0.4.0-pr2-validation-harness` as a backup (unstable
  system; not a PR, no merge without approval). Personal labels + `reports/` stay git-ignored
  (aggregate numbers only here). Still no D9 thresholds and no `06` #26 gate: those wait for the #110
  redesign call plus more days.

### Pass 2 continued - 2026-07-10 - Phase B redesign (the #110 task-level grouping; specs gate)

The maintainer confirmed **every real day is heavily fragmented** ("no calm days"), which voids the
gather-to-decide pause: the over-segmentation is representative, not a two-day fluke, so the redesign
direction is settled. Decision (`docs/0.4.0.md` §3): **redesign now, one PR2** — implement the
task-level grouping in the harness, validate, propose the D9 thresholds, then open the PR. A design
panel (analyze → 3 independent designs → 2 judges → synthesis, all read-only over the git-ignored
exports) produced the **anchored two-pass grouping** algorithm; its constituent designs reconstructed
to pooled boundary F1 **0.73–0.79** vs today's **0.128** (estimates only — the recorded numbers come
from the harness referee at Phase C). Eleven product calls were taken from the maintainer and recorded
(`06` #27, `07` #112/#113): unlabeled time = affirmatively no-session; the invisible 19:00-class tool
handoff = one merged AI band with exact outer edges (interior split out of heuristic reach); density
gate ON at 90 fph for anchorless focus only (AI/meeting exempt); label-snapping ON for labels inside
no-frame gaps; meeting-band floor 10 min; the sweep decides merge_gap.

- **Specs gate (this commit, `.md`-only):** `06` **#27** records the `§7e` amendment (two-level key;
  merge_gap / absorb_max / meeting-band close rule; identity-generalized excursion absorption;
  anchorless floors + density gate; kind/tool/context_key freeze with boundaries) as an **open
  decision row** resolved at Phase C with the rerun numbers + commit; **zero DDL change** (macro rows
  satisfy every schema-11 CHECK; PR3 re-normalizes only the `§4` context_key comment via the escape
  hatch). `06` **#26** reserved for the binding D9 gate. `07` **#112** (accepted limitations),
  **#113** (density-gate honesty). `#110` notes the redesign is active under #27 (flips to
  resolved-by-#27 at Phase C); `#111` resolves via taxonomy v3 (the spinner-prefix matcher).
- **Amendment procedure honored:** the contract change goes through `06`/`07` **before** the code that
  depends on it (stop-at-ambiguity); the model.rs `SessionSpan.context_key` doc comment is updated in
  the same code commit as the two-level key, so code-doc and spec never silently diverge.
- **Next:** taxonomy v3 (spinner rule) → `segment_micro` extraction (13 pinned tests unchanged as the
  A/B baseline) → `group.rs` two-pass + referee wiring (both `score.rs` call sites switch to
  `segment_grouped` in the same commit, or the recorded numbers would measure the ungrouped algorithm)
  → Phase B evidence → Phase C D9 thresholds (STOP for approval). Verification for this `.md`-only
  commit: `git diff --name-only` shows specs only; no code touched.

### Pass 2 continued - 2026-07-10 - redesign landed + validated; concurrency fork; D9 DEFERRED

The task-level grouping redesign is **implemented, tested, and pushed** (commits `8b037aa` taxonomy v3
spinner rule, `f976ee8` `segment_micro` + `GroupParams`, `a76466a` `group.rs` two-pass + referee
wiring + CLI, `3e92f14` docs). New `crates/harness/src/group.rs` = pass 2 (meeting bands + the
identity-anchored accretion walk); `segment_micro` = the unfloored pass 1; `segment()` and its **13
pinned tests unchanged** as the A/B baseline.

- **Implemented + verified (verbatim):** `cargo test -p harness` **87 passed / 0 failed** (13 pinned
  segmenter + 20 new `group.rs` + taxonomy v3 + score/main rewire); `cargo fmt --all -- --check`
  clean; `cargo clippy -p harness --all-targets -- -D warnings` clean; binding guard clean. **A/B
  through the referee on the 2 labeled days (keep_interior ON, defaults, PRELIMINARY / in-sample -
  NOT the D9 evidence):** grouped pooled typed boundary **F1 0.500 @120s / 0.571 @180s** vs the
  ungrouped baseline **0.142 / 0.156**; predicted sessions **16 vs 129**. Sweep best in-sample cell
  0.70 (absorb_max 1800); `meeting_gap` 1-D response FLAT (-> propose as a named constant).
  **Stability re-proven through the grouped pipeline: 6 h-stable on 3 days (the 24 h default holds
  with margin).**
- **Corrected on the fresh day:** attempting to label a fresh day surfaced two findings bigger than
  #110 (recorded `07` #114/#115): **(1) session concurrency** - more than one recognized tool can be
  active in the same time window, which the **serial** `§7e` model + non-overlapping segmenter +
  `labels.toml` cannot represent; **(2) recall-based labels are unreliable** (a rough recollection of
  the day did not match the capture), so ground truth must be reconstructed from the digest, not
  recalled. Personal specifics stay out of the repo (recorded only in untracked agent memory).
- **D9 DEFERRED (maintainer, 2026-07-10):** the binding thresholds (`06` #26) are NOT set this PR.
  Rationale: an acceptance gate cannot be fixed while the session **model** (serial vs concurrent,
  `07` #114) is undecided - the referee would score a serial segmenter against a target of unsettled
  shape. The built serial redesign stands as the baseline regardless. The model decision is the
  post-holiday conversation; the harness re-validates whichever model is chosen.
- **Broke / regressed:** nothing (additive dev-only crate; full CI ladder run at PR time).
- **Still risky:** the serial-vs-concurrent decision (`#114`) is the real gate now; do NOT freeze PR3
  / schema 11 until it is made. PR opened for review of the built work, **not** to claim final
  acceptance numbers.

## Pass 3 — 2026-07-10 — PR2b: concurrency resolved, specs gate first

The serial-vs-concurrent decision (`07` #114) is made: **concurrent** (maintainer, 2026-07-10, via
the resume-PR2 kickoff). Real usage runs parallel recognized tools, so the serial model of `06` #27
collapses ground truth into single precedence-attributed bands. PR2b builds the concurrent model in
the harness, keeps the serial redesign as the `--algo grouped` A/B baseline, and sets the D9 gate on
the concurrent metric against the held-out fresh days. Branch `feat/0.4.0-pr2b-concurrent-sessions`
(off `main` @ `6530288`, PR #100 merged); baseline `cargo test -p harness` **91 passed** before any
change.

- **Specs gate (this commit, `.md`-only, the #27 procedure repeated):** `07` **#114 → resolved
  (concurrent)** with the load-bearing finding recorded — **exclusive frame ownership** (one
  `app_hint` → one micro-span → one track → one session) means overlapping sessions fit **schema 11
  with zero DDL change** (verified against `03 §4`: no non-overlap constraint, overlap-agnostic time
  index, single nullable `frames.session_id` FK), so **PR3 is not affected** by concurrency. `06`
  **#28** records the per-identity-track amendment layered on #27 (track map replacing the single
  open group; anchor selection + `HOST_PRECEDENCE` inert on the shipped path but kept in the serial
  baseline; anchor **qualification** survives via `IDENTITY_QUALIFY_MS`; `SustainedForeignIdentity`
  close removed; per-track None-budget absorption into the last-touched AI track; meeting bands no
  longer barriers; `labels.toml` v2 per-identity non-overlap; identity-partitioned referee). `06`
  **#26** (D9 gate) flips from DEFERRED to **unblocked → lands at PR2b Phase C** on the partitioned
  metric with the fresh-day gate. `07` **#116** records the accepted concurrency limitations
  (same-tool instances and `browser-ai` fold into one track; thin overlap evidence → re-verify on
  every new labeled day). Concurrency **structurally removes** the #112(b) host-precedence pathology
  from the shipped path.
- **Amendment procedure honored:** the contract change goes through `06`/`07` **before** the code
  that depends on it (stop-at-ambiguity). No code touched in this commit.
- **Next:** `labels.rs` v2 (per-identity non-overlap, TDD) + export 07-10 + digest-first label drafts
  → **STOP 1** (maintainer sanity-checks the four label files) → `group.rs` concurrent per-track walk
  + `score.rs` partitioned referee (serial path byte-untouched; the 13 pinned `segment()` tests
  unchanged) → tune on 07-07/08 only and freeze params → fresh-day eval on 07-09/10 + stability +
  draft D9 (`06` #26) → **STOP 2** (maintainer approves the numbers; never tune-until-green). Every
  recorded number comes from the harness binary; design-doc estimates are never transcribed.
  Verification for this `.md`-only commit: `git diff --name-only` shows specs only; no code touched.

### Pass 3 continued - 2026-07-10 - labels.toml v2 + digest-first drafts + STOP 1 resolved

- **`labels.toml` v2 (code, TDD):** `resolve_day` relaxed from one global non-overlap chain to
  **per-identity-track** non-overlap (`ai` by tool, `focus`/`other` pooled per kind, `meeting`
  exempt), keeping a global sort-by-start requirement; serial files stay valid. New `track_key`
  helper. `cargo test -p harness labels::` = **14 passed** (4 new: cross-identity-overlap accepted,
  same-tool/focus overlap rejected, serial files still valid, out-of-start-order rejected); fmt +
  clippy clean.
- **Fresh data:** 07-10 exported read-only (DB size+mtime byte-identical before/after, verbatim);
  `harness-data/` confirmed git-ignored. 07-10 is a **partial** overnight capture (00:22-06:49).
- **Digest-first label drafts (gap #115) + STOP 1 (maintainer, 2026-07-10):** four concurrent label
  files drafted from the digests and reviewed. Tuning days 07-07/08 **revised for concurrency and
  approved**: 07-07 splits the old single 15:15 session into concurrent `claude-code ∥ claude-desktop`
  (the serial label already noted the desktop app "open alongside"); 07-08 is the concurrency source -
  `codex` runs continuously 16:59-22:53 **under** two `claude-code` terminal stints (17:02-17:11,
  20:13-21:39), correcting the recalled serial `claude-code 19:00-22:54`. Held-out 07-09 (primary
  fresh day): the leisure-heavy 19:01-20:23 block **unlabeled** per maintainer (Telegram chatting,
  not focus); the 12:05-13:24 analytics block kept as focus. Parser-validated (per-identity
  non-overlap holds on all four); the serial `--algo grouped` scores the new concurrent 07-07/08
  labels at pooled F1 0.44/0.50 - lower than the old serial-label 0.50/0.57 precisely because the
  serial algo cannot represent the concurrency the labels now encode (the gap the concurrent
  segmenter must close).
- **Two held-out findings recorded (`07` #117):** the 07-10 4-agent day exposed that (a) capture is
  **foreground-only + diff-gated**, so a background agent (codex-desktop) is nearly uncaptured; (b)
  spinner ambiguity (`#112c`) makes codex-CLI look like `claude-code`; (c) OpenAI **renamed the Codex
  desktop app to "ChatGPT"** ~4 h before the capture, so it lands under `app_hint "chatgpt"` and the
  `codex` entry misses it (scored days 07-07/08/09 predate the rename, so the taxonomy is left
  unchanged). Net: 07-10's recoverable tool-accuracy/recall are structurally capped by capture +
  taxonomy, so **07-10 is the capture-limit demonstrator, not a D9 scoring day** (07-09 is the primary
  held-out day) - maintainer decision.
- **Next:** freeze params on 07-07/08 only, then build `group.rs` concurrent per-track walk +
  `score.rs` partitioned referee (serial path byte-untouched; the 13 pinned `segment()` tests
  unchanged), evaluate 07-09 (+ 07-10 as demonstrator), draft the D9 gate, **STOP 2** for approval.

### Pass 3 final - 2026-07-10 - concurrent segmenter built + validated; D9 gate SET (STOP 2 approved)

The concurrent per-identity-track model is **built, tested, and validated**; the D9 gate is **set
with maintainer approval** (`06` #26). Commits on `feat/0.4.0-pr2b-concurrent-sessions`: specs gate,
`labels.rs` v2, `group.rs` `group_concurrent` (18 tests), `score.rs` identity-partitioned referee +
3-way `--algo` CLI, records. `cargo test -p harness` **116 passed**; fmt + clippy `-D warnings` +
binding guard clean; the 13 pinned `segment()` tests + 20 serial `group.rs` tests **unchanged** (the
serial path is byte-untouched, kept as the `--algo grouped` A/B baseline).

- **Every number below is from the harness binary** (`--algo concurrent`, identity-partitioned metric,
  labels snapped, keep_interior ON, 120 s + 180 s). Design-doc estimates are never transcribed.
- **Freeze (tuning days 07-07/08 ONLY, pre-stated pick rule = max pooled partitioned F1 @120 s, then
  @180 s, then fewer sessions):** `merge_gap = 2700 s`, `absorb_max = 1800 s`, all other knobs at
  defaults. `merge_gap` is a **clear local F1 peak** (2400→0.400, **2700→0.452**, 3000→0.387;
  monotone-drop past 2700); `absorb_max` plateaus at ≥1800. Stage-B verdicts at the frozen base:
  `meeting_gap` + `focus_min_density` FLAT → named constants (density stays 90 per `07` #113 for the
  fresh sparse-session check); `focus_min_len`/`gap_close`/`min_len`/`IDENTITY_QUALIFY` are sensitive
  but their 1-D optima diverge from defaults — **kept at defaults** (greedy combination on n=2 = the
  tune-until-green D9 forbids). **A key finding:** the concurrent model's `merge_gap` governs
  **per-tool idle tolerance** (a background agent stays one session across foreground gaps), so it is
  necessarily larger than the serial `merge_gap` (which governed overall-activity idle).
- **Evidence (partitioned typed boundary F1 @120 s / @180 s; posF1 = pooled position-only, the
  0.128/0.50 comparability line):**

  | day | role | F1 @120/180 | posF1 | tool acc | pred/lab sessions |
  |---|---|---|---|---|---|
  | 07-07 | tune | 0.74 / 0.74 | 0.84 | 3/4 | 5 / 6 |
  | 07-08 | tune | 0.00 / 0.33 | 0.17 / 0.33 | 2/3 | 4 / 3 |
  | **tuning pooled** | | **0.452 / 0.581** | 0.581 / 0.645 | **0.714** | |
  | **07-09** | **held-out (gate)** | **0.286 / 0.286** | 0.286 | **1.000** | 4 / 5 |
  | 07-10 | demonstrator (NOT gated) | 0.00 / 0.00 | 0.20 | 0.50 | 3 / 4 |

  **A/B on the held-out 07-09:** concurrent **0.286** vs serial-grouped **0.167** vs micro **0.068** —
  the concurrent model wins **2–4×** with **perfect tool recognition (4/4)**. Stability re-proven
  through the concurrent pipeline: **6 h-stable on the tuning days**, so the **24 h** freeze window
  holds with wide margin (`W = 24 h`).
- **D9 gate SET (`06` #26; recognition-primary, maintainer-approved 2026-07-10):** PRIMARY — tool
  accuracy ≥ 0.65 AND predicted session count within 2× of labeled AND beats both baselines; FLOOR —
  partitioned F1 ≥ 0.20 @120 s; W = 24 h. Boundary F1 is a **floor, not the target**, because it is
  structurally capped by foreground-only capture (`07` #117): on 07-08 the labeled claude-code
  20:13-21:39 is largely unrecoverable (thin foreground presence under dominant codex fails
  `IDENTITY_QUALIFY`); the absorb rule over-merges fragmented unrecognized work into the last-touched
  AI track (07-09's analytics into claude-code); browser-ai over-recognizes (title-only, `#109`). The
  **recognition** signal (the arc's payoff) is strong; the **boundary** signal is capture-capped.
- **Method disclosures:** labels are digest-first (`07` #115), concurrent v2 (`06` #28); labeled
  boundaries snapped to the nearest captured frame; 07-08's tuning labels were revised from the
  earlier serial recollection to the digest-grounded concurrent shape; 07-10 is a partial (overnight)
  capture and the capture-limit demonstrator (`07` #117), excluded from the gate.
- **Broke / regressed:** nothing (additive dev-only crate; serial + micro paths unchanged). PR3 is
  **unaffected** — exclusive frame ownership keeps concurrency inside schema 11 with zero DDL change.
- **Resolved:** `06` #26 (gate SET), #27 (serial baseline), #28 (concurrent, shipped); `07` #110
  (over-segmentation), #114 (concurrency). Open follow-ups: `07` #116 (identity-granularity limits),
  #117 (capture/taxonomy ceiling → PR4 taxonomy re-tune + a later capture change).

## Pass 4 — 2026-07-10 — 0.4.0 PR3 (sessions schema + migration 10 → 11)

- **Implemented:** `MIGRATION_V11` (`crates/store/src/schema.rs`) — the sessions arc's **only**
  schema change (D4). Creates `sessions`, `session_artifacts`, `frames.session_id`, and the three
  indexes (`idx_sessions_time`, `idx_frames_session`, `idx_artifacts_session`), transcribed verbatim
  from the authoritative DDL in `03 §4:328–363` (both hardened CHECKs included:
  `sessions.tool CHECK (tool IS NULL OR kind = 'ai')` and the compound `session_artifacts.role`
  CHECK with the load-bearing `role IS NOT NULL` guard). **Structure only, no backfill** — plain
  `CREATE TABLE` + `ALTER TABLE ... ADD COLUMN` + `CREATE INDEX`, no table rebuild, so the runner's
  FK-off recipe is untouched and the migration is fast. `LATEST_SCHEMA_VERSION` 10 → 11 (the runner
  confirms +1 against the constant via its `debug_assert_eq!`, never hardcoded). The 03 §4
  `context_key` column comment was re-normalized to the `06` #27/#28 closed grammar (the sanctioned
  escape hatch). §7e prose deltas [a]–[f] were **not** touched — they are PR4's obligation (`06` #27
  scopes PR3 to the §4 comment).
- **Tests (all green):** five new inline `migration_tests` in `crates/store/src/lib.rs` on a
  populated schema-10 fixture (`seed_v10_fixture`: three frames with
  `app_hint`/`window_title`/`browser_url`/`capture_trigger` incl. one image-purged, `frame_text` +
  FTS mirrors, `text_spans`, two marks open+resolved, two text embeddings with vec0 vectors, a mixed
  jobs queue): `migration_v11_adds_sessions_structure_only` (version +1, five new objects,
  sessions/artifacts empty, every frame `session_id IS NULL`, all pre-existing rows survive incl. FTS
  match, fk clean); `migration_v11_sessions_check_constraints`; `migration_v11_artifact_role_kind_coupling`
  (incl. the NULL-role-on-exchange rejection — the load-bearing guard case);
  `migration_v11_fk_set_null_and_cascade`; and the **D10 additivity proof**
  `migration_v11_preserves_frame_surfaces` — seven store surfaces (`hybrid_search` FTS+vector arms,
  `ocr_texts`, `list_marks`, `recent_frame_contexts`, `timeline_buckets`, `sample_frames_in_range`,
  `insights_summary`) are `assert_eq!`-identical before and after the migration on the same fixture,
  with vacuity guards so no surface is silently empty. The existing
  `fresh_and_migrated_schemas_agree_at_latest` parity acceptance now spans v11 unchanged. Verbatim:

  ```
  $ cargo test -p store --lib migration
  running 11 tests
  test migration_tests::migration_v8_indexes_image_retention_sweep ... ok
  test migration_tests::migration_v11_artifact_role_kind_coupling ... ok
  test migration_tests::migration_v10_adds_marks_with_cascade ... ok
  test migration_tests::migration_v7_adds_image_purged_present_by_default ... ok
  test migration_tests::migration_v6_widens_capture_trigger_check_without_dropping_children ... ok
  test migration_tests::migration_v11_sessions_check_constraints ... ok
  test migration_tests::migration_v9_drops_image_lane_and_embed_image_jobs ... ok
  test migration_tests::migration_v11_fk_set_null_and_cascade ... ok
  test migration_tests::migration_v11_adds_sessions_structure_only ... ok
  test migration_tests::migration_v11_preserves_frame_surfaces ... ok
  test migration_tests::fresh_and_migrated_schemas_agree_at_latest ... ok
  test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 27 filtered out; finished in 0.09s
  ```

- **Gate 0 / D5 backup record (release-blocker-class manual step, `07` manual-steps):** the maintainer
  confirmed a dated pristine copy of the live `screensearch.db` exists **outside** the app data dir
  (attested in-session 2026-07-10; the live DB doubles as the PR2 ground-truth dataset). The live DB
  was **never opened by this branch's build** — its mtime is unchanged (`07/10/2026 10:55:06`,
  183,693,312 bytes) throughout PR3; the app was not running. A separate **throwaway** copy (the live
  `.db` + `-wal` + `-shm`, copied to an isolated scratchpad) was migrated 10 → 11 by the env-gated
  `live_db_copy_migrates_to_v11_fast_and_clean` integration test and then deleted. Verbatim:

  ```
  $ SCREENSEARCH_MIGRATION_CHECK_DB=<throwaway copy>  cargo test -p store --test store live_db_copy -- --ignored --nocapture
  running 1 test
  Gate 0: migrated 3036 frames 10 -> 11 in 145.678ms (fk clean, sessions empty)
  test live_db_copy_migrates_to_v11_fast_and_clean ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 63 filtered out; finished in 0.36s
  ```

  3036 frames migrated in **~146 ms** (well under the test's 30 s bound), fk clean, sessions +
  artifacts empty, zero backfilled frames — structure-only confirmed on real, live-shaped data.
  **`npm run tauri dev` was NOT run on this branch** (the dev build shares the live Roaming data dir;
  a v11-migrated live DB would brick the installed v0.3.3, which rejects newer schema versions).
- **Full verification ladder (verbatim, all green):**

  ```
  $ cd ui && npm run lint          → > eslint .            (no errors)
  $ cd ui && npm run build         → ✓ built in 2.02s
  $ node scripts/stage-mcp.mjs     → [stage-mcp] up to date: ...screensearch-mcp-x86_64-pc-windows-msvc.exe
  $ cargo fmt --all -- --check     → (clean, exit 0)
  $ cargo clippy --workspace --all-targets -- -D warnings → Finished `dev` in 6.68s (no warnings)
  $ cargo build --workspace        → Finished `dev` in 16.56s
  $ cargo test --workspace         → all suites ok; store lib 38 passed; store.rs 63 passed + 1 ignored (Gate 0)
  $ git diff --exit-code -- ui/src/bindings → clean (PR3 adds no ts-rs types)
  ```

- **Skipped / deferred:** the segmenter + the historical backfill job (PR4); the IPC/UI/API/MCP session
  surfaces (PR5/PR6). PR3 adds **no** ts-rs-exported types, so `ui/src/bindings` is byte-identical
  (the guard is clean above). No new settings, no new NavRail route (D13).
- **Hallucinated / corrected:** none. The `docs/0.4.0.md` §3 PR3 proposal DDL differs from `03 §4` in
  two CHECK constraints; this was already an intended PR1 normalization (the proposal block is labeled
  "Proposal-level DDL — PR1 normalizes the final form into `03 §4`"), logged for the record as `06`
  #29 with disposition **03 wins**. The migration transcribes `03 §4` verbatim; both hardened CHECKs
  are exercised by the tests.
- **Broke / regressed:** nothing. Additive migration; every pre-existing frame-level feature is proven
  identical pre/post on the fixture (D10) and the full workspace suite is green.
- **Still risky:** the `sessions.host` CHECK admits NULL by design (an un-guarded `host IN (…)`
  evaluates to NULL and SQLite passes a NULL CHECK) — intentional (host is optional), and covered by
  the constraint test's focus-session-with-NULL-host case. The `session_artifacts.role` CHECK relies
  on the `role IS NOT NULL` guard for the same NULL-CHECK reason; the guard case is tested explicitly.

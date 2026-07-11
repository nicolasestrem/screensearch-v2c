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
  schema change (D4). Creates `sessions`, `session_artifacts`, `frames.session_id`, and the four
  indexes (`idx_sessions_time`, `idx_frames_session`, `idx_artifacts_session`, `idx_artifacts_frame`
  — the last added by the review amendment `06` #30, see below), transcribed verbatim
  from the authoritative DDL in `03 §4:328–368` (both hardened CHECKs included:
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
  jobs queue): `migration_v11_adds_sessions_structure_only` (version +1, six new objects,
  sessions/artifacts empty, every frame `session_id IS NULL`, all pre-existing rows survive incl. FTS
  match, fk clean); `migration_v11_sessions_check_constraints`; `migration_v11_artifact_role_kind_coupling`
  (incl. the NULL-role-on-exchange rejection — the load-bearing guard case);
  `migration_v11_fk_set_null_and_cascade` (adds an `EXPLAIN QUERY PLAN` assertion that the frame-delete
  FK lookup rides `idx_artifacts_frame`); and the **D10 additivity proof**
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
  **`npm run dev` was NOT run on this branch** (the dev build shares the live Roaming data dir;
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
- **Review amendment (2026-07-10, PR #102, `06` #30):** the PR review (gemini-code-assist) flagged that
  `session_artifacts.frame_id` (an `ON DELETE SET NULL` FK) had no covering index, so a routine
  frame-retention delete (`03 §5`) would full-scan `session_artifacts` to null matching rows — the one
  FK delete-path in v11 that was unindexed (session→frames and session→artifacts were already covered).
  With maintainer approval, `CREATE INDEX idx_artifacts_frame ON session_artifacts(frame_id)` was added
  to **both** `MIGRATION_V11` and the authoritative `03 §4` DDL in lockstep, preserving the verbatim
  transcription (the DDL block is `03 §4:328–368`; the claude-review "character-for-character identical"
  property holds). Pure performance, additive, no behavior change (D10). Tests updated: the structure
  test asserts **six** new objects (2 tables + 4 indexes); the FK test adds an `EXPLAIN QUERY PLAN`
  assertion that the frame-delete lookup uses the new index. Full ladder re-run green after the change.

## Pass 5 — 2026-07-10 — 0.4.0 PR4 (segmentation engine + recognition; pre-live checkpoint)

- **Implemented:** the production `crates/sessions` concurrent engine and v3 taxonomy; session-domain
  provider/store contracts; schema-11 SQL persistence with frozen guards; incremental reconciliation,
  freeze, exchange refresh, and resumable six-hour historical chunks in `kernel`; lazy cached in-app
  title/summary generation; the two final settings keys/clamps; composition-root lifecycle wiring;
  and the harness `--algo shipped` adapter/parity test. `03 §7e` now carries the `06` #27/#28
  concurrent amendments, and `07` #117 records the ChatGPT→Codex/Classic-exclusion decision.
- **D9 binding gate:** **MET, no retuning.** The input was an isolated copy containing only labeled
  07-07, 07-08, and held-out 07-09; 07-10 was excluded. Verbatim referee output:

  ```text
  $ cargo run -p harness -- score --algo shipped --data <three-day-dir>
  Scoring 3 labeled day(s), algo=shipped, merge_gap 2700s, absorb_max 1800s, focus_min_len 600s, density 90fph. Labels snapped to the nearest frame.
  PRIMARY metric = identity-PARTITIONED typed boundary F1 (`07` #114); the pooled position-only F1 (the 0.128/0.50 history) is shown beside it as `posF1`.

  -- tolerance 120s --
  day            pred   lab match      P     R    F1   posF1       tool
  2026-07-07        8    11     7   0.88  0.64  0.74    0.84     3/4
  2026-07-08        7     5     0   0.00  0.00  0.00    0.17     2/3
  2026-07-09        6     8     2   0.33  0.25  0.29    0.29     4/4
  POOLED(part) P=0.429 R=0.375 F1=0.400 (matched 9/21 pred, 9/24 lab); posF1=0.489; tool 9/11 = 0.818

  -- tolerance 180s --
  day            pred   lab match      P     R    F1   posF1       tool
  2026-07-07        8    11     7   0.88  0.64  0.74    0.84     3/4
  2026-07-08        7     5     2   0.29  0.40  0.33    0.33     2/3
  2026-07-09        6     8     2   0.33  0.25  0.29    0.29     4/4
  POOLED(part) P=0.524 R=0.458 F1=0.489 (matched 11/21 pred, 11/24 lab); posF1=0.533; tool 9/11 = 0.818

  $ cargo run -p harness -- score --algo micro --data <three-day-dir>
  -- tolerance 120s --
  POOLED(part) P=0.043 R=0.375 F1=0.077 (matched 9/209 pred, 9/24 lab); posF1=0.120; tool 8/11 = 0.727
  -- tolerance 180s --
  POOLED(part) P=0.048 R=0.417 F1=0.086 (matched 10/209 pred, 10/24 lab); posF1=0.146; tool 8/11 = 0.727

  $ cargo run -p harness -- score --algo grouped --data <three-day-dir>
  -- tolerance 120s --
  POOLED(part) P=0.409 R=0.375 F1=0.391 (matched 9/22 pred, 9/24 lab); posF1=0.435; tool 10/11 = 0.909
  -- tolerance 180s --
  POOLED(part) P=0.455 R=0.417 F1=0.435 (matched 10/22 pred, 10/24 lab); posF1=0.478; tool 10/11 = 0.909
  ```

  Gate check: tool `0.818 >= 0.65`; daily predicted/labeled `8/11`, `7/5`, `6/8` are each within
  2×; shipped beats micro and grouped at both tolerances; ±120 s F1 `0.400 >= 0.20`.
- **Focused verification (verbatim terminal summaries):**

  ```text
  $ cargo fmt --all -- --check
  (no output; exit 0)
  $ cargo clippy -p kernel -p harness -p sessions -p store --all-targets -- -D warnings
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.39s
  $ cargo test -p harness
  test result: ok. 119 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  $ cargo test -p kernel sessions_scheduler_contract_tests
  test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 43 filtered out; finished in 0.01s
  $ cargo test -p kernel --test sessions_intel
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
  $ cargo test -p store --test sessions
  test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
  $ cargo test -p sessions
  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  ```

- **Full CI-parity ladder (UI first; raw output):** the required non-quiet workspace test emitted
  1060 green lines; the compact rerun below preserves representative raw suite summaries without
  paraphrasing their pass/fail counts.

  ```text
  $ cd ui && npm ci
  added 348 packages, and audited 349 packages in 4s

  151 packages are looking for funding
    run `npm fund` for details

  found 0 vulnerabilities
  npm warn allow-scripts 1 package has install scripts not yet covered by allowScripts:
  npm warn allow-scripts   esbuild@0.25.12 (postinstall: node install.js)
  npm warn allow-scripts
  npm warn allow-scripts Run `npm approve-scripts --allow-scripts-pending` to review, or `npm approve-scripts <pkg>` to allow.

  $ npm run lint
  > screensearch-ui@0.3.3 lint
  > eslint .

  $ npm run build
  > screensearch-ui@0.3.3 build
  > tsc --noEmit && vite build

  vite v6.4.3 building for production...
  transforming...
  ✓ 434 modules transformed.
  rendering chunks...
  computing gzip size...
  dist/index.html                                  0.80 kB │ gzip:  0.36 kB
  dist/overlay.html                                0.99 kB │ gzip:  0.42 kB
  dist/assets/globals-oPR43bCE.css                 31.54 kB │ gzip:  6.69 kB
  dist/assets/timeRanges-BJgzkTNX.js                0.29 kB │ gzip:  0.19 kB
  dist/assets/openExternal-cOuZy8U8.js              0.31 kB │ gzip:  0.23 kB
  dist/assets/useAdaptiveBucketCount-pnT9Dvsn.js    0.35 kB │ gzip:  0.26 kB
  dist/assets/NotFound-GT5xmkd7.js                  0.44 kB │ gzip:  0.32 kB
  dist/assets/EmptyState-CIH9-Lyf.js                0.52 kB │ gzip:  0.31 kB
  dist/assets/Panel-C7dEyHdc.js                     0.68 kB │ gzip:  0.44 kB
  dist/assets/timelineDraw-B37WQvuk.js              0.75 kB │ gzip:  0.44 kB
  dist/assets/FrameTile-CvZpniId.js                 0.95 kB │ gzip:  0.53 kB
  dist/assets/time-cX9c3v95.js                      1.01 kB │ gzip:  0.46 kB
  dist/assets/FrameImage-PCfV_Ty8.js                1.62 kB │ gzip:  0.85 kB
  dist/assets/HighlightedSnippet-DKdy48F9.js        1.65 kB │ gzip:  0.83 kB
  dist/assets/HotkeyField-C0RHDXNK.js               2.54 kB │ gzip:  1.32 kB
  dist/assets/path-B-7-bRzz.js                      3.83 kB │ gzip:  0.96 kB
  dist/assets/Insights-DsVkGRfs.js                  5.46 kB │ gzip:  2.11 kB
  dist/assets/Timeline-DpYfWqP9.js                  6.50 kB │ gzip:  2.97 kB
  dist/assets/Moment-lfN7bZwI.js                    7.33 kB │ gzip:  2.74 kB
  dist/assets/Deck-CkSQWV7a.js                     10.66 kB │ gzip:  3.61 kB
  dist/assets/overlay-CRRPoGHe.js                  10.93 kB │ gzip:  3.85 kB
  dist/assets/globals-Dyr3sT_-.js                  15.89 kB │ gzip:  5.84 kB
  dist/assets/main-D3G4FMmK.js                     22.61 kB │ gzip:  7.32 kB
  dist/assets/query-u_0r_xiX.js                    35.77 kB │ gzip: 10.59 kB
  dist/assets/Recall-D0S9Iphb.js                   40.42 kB │ gzip: 13.20 kB
  dist/assets/Settings-C3mGMQhp.js                 46.83 kB │ gzip: 13.28 kB
  dist/assets/router-CL_afuJ-.js                   64.86 kB │ gzip: 22.21 kB
  dist/assets/react-vendor-DTiTYlFD.js            143.42 kB │ gzip: 46.01 kB
  dist/assets/AnswerStream-D4T9ftQq.js            160.09 kB │ gzip: 48.97 kB
  ✓ built in 1.84s

  $ node scripts/stage-mcp.mjs
  [stage-mcp] building screensearch-mcp (release)...
  [stage-mcp] up to date: C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\src-tauri\binaries\screensearch-mcp-x86_64-pc-windows-msvc.exe
      Finished `release` profile [optimized] target(s) in 0.29s

  $ cargo fmt --all -- --check
  (no output; exit 0)

  $ cargo clippy --workspace --all-targets -- -D warnings
      Checking traits v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\traits)
     Compiling screensearch v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\src-tauri)
      Checking mcp v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\mcp)
      Checking doctor v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\doctor)
      Checking textfilter v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\textfilter)
      Checking sessions v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\sessions)
      Checking inference v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\inference)
      Checking kernel v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\kernel)
      Checking api v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\api)
      Checking capture v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\capture)
      Checking uia v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\uia)
      Checking ocr v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\ocr)
      Checking sysmon v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\sysmon)
      Checking embeddings v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\embeddings)
      Checking store v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\store)
      Checking harness v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\harness)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.62s

  $ cargo build --workspace
     Compiling traits v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\traits)
     Compiling mcp v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\mcp)
     Compiling sessions v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\sessions)
     Compiling inference v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\inference)
     Compiling textfilter v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\textfilter)
     Compiling embeddings v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\embeddings)
     Compiling uia v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\uia)
     Compiling ocr v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\ocr)
     Compiling capture v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\capture)
     Compiling api v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\api)
     Compiling sysmon v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\sysmon)
     Compiling kernel v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\kernel)
     Compiling store v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\store)
     Compiling harness v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\crates\harness)
     Compiling screensearch v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\src-tauri)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.96s

  $ cargo test --workspace
      Finished `test` profile [unoptimized + debuginfo] target(s) in 29.42s

  $ cargo test --workspace --quiet   # compact verbatim rerun of the same workspace suites
  running 119 tests
  ....................................................................................... 87/119
  ................................
  test result: ok. 119 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
  running 105 tests
  ....................................................................................... 87/105
  ..................
  test result: ok. 105 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.04s
  running 49 tests
  .................................................
  test result: ok. 49 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.10s
  running 38 tests
  ......................................
  test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
  running 64 tests
  .........................i......................................
  test result: ok. 63 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.56s
  running 66 tests
  ..................................................................
  test result: ok. 66 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
  running 30 tests
  ........................iiii..
  test result: ok. 26 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.00s
  $ git diff --exit-code -- ui/src/bindings
  (no output; exit 0)
  ```

- **Skipped / deferred at this checkpoint:** PR5 IPC/UI and PR6 API/MCP; no command or NavRail route
  is added. Full workspace/UI/binding verification and the D5-backed live checks remain before PR
  open and will be appended here verbatim.
- **Hallucinated / corrected:** the first backfill cut helper treated an empty scanned chunk as proof
  that the entire future target was empty; a red regression test exposed the skip, and the helper now
  advances only to the scanned desired boundary. The initial shipped score banner printed harness
  model defaults even though the shipped arm correctly used 2700/1800; the banner now reads the
  production params. The canonical taxonomy's new Classic exclusion initially was ignored by the
  harness parser; the parity fixture now covers both renamed Codex and Classic, and both parsers
  apply the exclusion.
- **Broke / regressed:** none observed in focused suites; full/additive and live checks pending.
- **Still risky:** the live historical pass and real marker extraction remain qualitative gates;
  backup is mandatory before launching the branch against the live DB.

### Pass 5 pause disposition — 2026-07-10

- **D5/live evidence obtained:** backup command output was:

  ```text
  D5 backup written: \\?\C:\Users\nicol\ScreenSearch Backups\screensearch-2026-07-10.db
  PRAGMA integrity_check: ok
  row counts — source: 3114 frames / 0 marks; copy: 3114 frames / 0 marks (match: true)

  FullName      : C:\Users\nicol\ScreenSearch Backups\screensearch-2026-07-10.db
  Length        : 185266176
  LastWriteTime : 7/10/2026 2:19:32 PM
  BACKUP_OUTSIDE_APP_DATA=True
  BACKUP_OUTSIDE_REPO=True
  ```

  The dev build opened the live DB at schema 11, started the sessions scheduler, and advanced the
  checkpoint once per minute while inference/vision work continued. The first pass reached its
  target and produced 20 sessions / 1,614 assigned frames, including real `claude-code`, `codex`,
  `claude-desktop`, and `browser-ai` recognition.
- **Live D8 finding + fix:** the first Codex extraction treated a bare `Codex` navigation label as an
  agent marker and consumed unrelated screen chrome. That fails the high-precision acceptance. The
  parser now requires the observed Codex desktop nav signature plus bounded `Q … File` prompt and
  `Working/Worked for <duration>` response structures; generic desktop/browser roles require an
  explicit colon. A new live-shaped regression proves navigation is not emitted. Raw focused result:

  ```text
  running 3 tests
  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  running 18 tests
  test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  ```

- **Derived-data reset correction:** to re-run the live backfill through the corrected extractor,
  only `frames.session_id`, `sessions`, `session_artifacts`, and the internal checkpoint were reset;
  frames/text/embeddings/marks stayed `3114/3114/2910/0`. The standalone sqlite CLI connection had
  foreign keys off by default, so deleting sessions initially left 355 derived orphan artifacts;
  these were explicitly deleted immediately, `PRAGMA foreign_key_check` returned no rows, and the
  corrected app began recomputation from source frames. This is recorded as a tooling correction,
  not hidden as a successful cascade test (the Store-path cascade is separately automated).
- **Exact pause state:** app + `llama-server` + dev cargo processes are stopped (no orphans). Live DB
  is schema 11 with 3,114 source frames; corrected recomputation is safely resumable at
  `{"cursor_ms":1783473465286,"target_ms":1783600094929}` with 14 sessions, 5 frozen, 800 assigned
  frames, and 116 artifacts. On resume, launching `npm run dev` continues the checkpoint.
- **Remaining before PR:** (1) let corrected backfill reach cursor=target, then re-check exchange
  samples; (2) manually start capture (it was paused before launch; max frame stayed 3114) and prove
  frame growth during a pass; (3) maintainer foregrounds a meeting-titled window (browser AI already
  exists in history) and judges real exchange output; (4) spot-check search/Ask/Timeline/marks/
  where-was-i; (5) rerun the full CI ladder because extraction code changed after the prior full run;
  (6) adversarial review, final commit/push, open PR. No PR exists and nothing is merged.

### Pass 5 resumed live/final-verification evidence — 2026-07-10

- **Historical pass + capture:** corrected recomputation reached
  `{"cursor_ms":1783609157370,"target_ms":1783609157370}`. While it advanced, live capture grew
  from frame 3114 to 3154; final derived history held 20 sessions / 1,614 assigned frames and
  `PRAGMA foreign_key_check` returned no rows. Recognition rows included one `browser-ai`/browser,
  nine `claude-code`/terminal, five `claude-desktop`/desktop, four `codex`/desktop, and one focus
  session. Real Meet-titled frames were captured, but their longest chained band was ~8m25s, below
  the frozen 10-minute meeting floor, so the live meeting-session row remains a maintainer/manual
  gate rather than being misreported as passing.
- **D8 live correction:** real output traced two false-positive sources to the exact input lines:
  Windows Explorer emits `> This pc` / `> Network`, while genuine Claude Code uses `❯`; and an empty
  standalone `❯` is followed by the terminal status bar. Removed only the ambiguous ASCII alias and
  required inline content after `❯`. Both regressions were red before the fix and green after it.
  A final artifact-only recomputation produced genuine prompt/agent samples, zero `This pc`/`Network`
  artifacts, zero invalid exchange roles, and zero exchange rows on non-AI sessions. Codex/browser
  sessions with no strong marker correctly emitted no exchanges. Sessions test output:

  ```text
  running 3 tests
  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  running 20 tests
  test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  ```

- **Fresh post-fix CI ladder:** the first `npm ci` attempt failed with `EPERM unlink esbuild.exe`
  because the required Tauri/Vite dev launch still held the binary. After stopping only that
  worktree's dev processes (and verifying no orphaned `llama-server`), the exact ladder passed:

  ```text
  $ cd ui && npm ci
  added 348 packages, and audited 349 packages in 3s
  found 0 vulnerabilities
  $ npm run lint
  > eslint .
  $ npm run build
  ✓ 434 modules transformed.
  ✓ built in 1.76s
  $ node scripts/stage-mcp.mjs
  [stage-mcp] up to date: C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\src-tauri\binaries\screensearch-mcp-x86_64-pc-windows-msvc.exe
      Finished `release` profile [optimized] target(s) in 0.28s
  $ cargo fmt --all -- --check
  (no output; exit 0)
  $ cargo clippy --workspace --all-targets -- -D warnings
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.84s
  $ cargo build --workspace
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.09s
  $ cargo test --workspace --quiet
  test result: ok. 119 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
  test result: ok. 105 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.05s
  test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
  test result: ok. 63 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.56s
  test result: ok. 66 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
  test result: ok. 26 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.00s
  $ git diff --exit-code -- ui/src/bindings
  (no output; exit 0)
  ```

- **Launch-surface correction:** a direct `target/debug/screensearch.exe` launch was attempted for a
  read-only API spot-check; the maintainer corrected that this repository must always launch through
  `npm run dev`. The direct process was stopped immediately, its unauthorized API output is not
  counted as evidence, and subsequent live work uses only the required npm/Tauri launch surface.
- **Still manual before PR:** maintainer confirmation of search/Ask/Timeline/marks/where-was-i in the
  UI, plus a qualifying ten-minute meeting-title band if the meeting row is held as a hard PR gate.
  Automated/D9/backup/backfill/capture/recognition/exchange/full-suite gates are otherwise evidenced.

### Pass 5 adversarial-review corrections — 2026-07-10

- **Review verdict and fixes:** the reserved read-only adversarial pass found two critical and three
  important correctness gaps. Publication paused. Red regressions proved that pass-1 short excursions
  were marked consumed without transferring their frame ids; a frozen-boundary frame at exactly
  `ended_at` was retained even though `ended_at` is the last owned-frame time; and a checkpoint whose
  fixed historical target cut a continuous track could never scan far enough on later ticks to see
  the closing gap. Production now transfers every consumed id exactly once, trims incremental tails
  strictly after frozen `ended_at` using true min/max timestamps, and preserves the fixed checkpoint
  target while allowing later scans through the current frozen horizon.
- **Delayed/restarted backfill hardening:** a persisted on-disk checkpoint test now closes a track
  beyond the initial target after reopening the store and proves the completed retry is idempotent.
  A second real-store test seeds a frozen incremental overlap: backfill keeps only the historical
  prefix, assigns every pre-gap frame exactly once, freezes only after successful assignment, and
  emits no same-track overlap. Exact frozen retries count already-owned ids before validating the
  assignment result; partial new rows remain deletable until assignment succeeds.
- **Parity expansion:** the frozen harness implementation and scorer remain unchanged. The shipped
  parity suite grew from one fixture to a table covering interleaved identities, None absorption,
  meetings, merge-gap equality, density, qualification failure, and open projection. All referee
  fields match; only the deliberate production ownership extension may have a larger frame count for
  pass-1-consumed excursions because the harness stored counts but not owned ids.
- **Post-fix D9 gate rerun (no retuning):** output remained exactly at the approved evidence line:

  ```text
  -- tolerance 120s --
  2026-07-07        8    11     7   0.88  0.64  0.74    0.84     3/4
  2026-07-08        7     5     0   0.00  0.00  0.00    0.17     2/3
  2026-07-09        6     8     2   0.33  0.25  0.29    0.29     4/4
  POOLED(part) P=0.429 R=0.375 F1=0.400 (matched 9/21 pred, 9/24 lab); posF1=0.489; tool 9/11 = 0.818
  -- tolerance 180s --
  POOLED(part) P=0.524 R=0.458 F1=0.489 (matched 11/21 pred, 11/24 lab); posF1=0.533; tool 9/11 = 0.818
  ```

- **Focused raw verification:** `sessions` 21/21, kernel scheduler-contract 8/8, shipped parity 3/3,
  and clippy for `sessions`/`kernel`/`harness` passed with warnings denied.
- **Fresh full post-review ladder (verbatim compact output):** the exact non-quiet
  `cargo test --workspace` also exited 0 before the compact evidence rerun.

  ```text
  $ cd ui && npm ci
  added 348 packages, and audited 349 packages in 4s
  found 0 vulnerabilities
  $ npm run lint
  > eslint .
  $ npm run build
  ✓ 434 modules transformed.
  ✓ built in 1.63s
  $ node scripts/stage-mcp.mjs
  [stage-mcp] up to date: C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\src-tauri\binaries\screensearch-mcp-x86_64-pc-windows-msvc.exe
      Finished `release` profile [optimized] target(s) in 0.26s
  $ cargo fmt --all -- --check
  (no output; exit 0)
  $ cargo clippy --workspace --all-targets -- -D warnings
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.39s
  $ cargo build --workspace
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.09s
  $ cargo test --workspace --quiet
  test result: ok. 119 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  test result: ok. 105 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.05s
  test result: ok. 51 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.12s
  test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
  test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
  test result: ok. 63 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.57s
  test result: ok. 66 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
  test result: ok. 26 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.00s
  $ git diff --exit-code -- ui/src/bindings
  (no output; exit 0)
  ```
- **Meeting live note:** the maintainer held the meeting for eleven wall-clock minutes, but the DB
  contains only a 17:12:28–17:14:05 title band (six frames, 1.62 minutes); the prior band ended at
  16:54:21. The 18-minute gap exceeds `meeting_gap`, so no qualifying session row is expected. This
  remains honest capture-limited evidence. All future app launches use only `npm run dev`; direct
  executable launch is forbidden.

### Pass 6 open-PR review follow-up — 2026-07-10

- **Thread audit:** the thread-aware PR read found six unresolved inline threads and one substantive
  top-level review. Four findings were applicable and reproduced with red tests: exact frozen-end
  equality retained the boundary frame; a crash-interrupted unfrozen historical row was ignored on
  retry and duplicated; even summary sampling omitted the final frame; taxonomy patterns were
  normalized repeatedly in the per-frame matcher. Production now treats stored endpoints as
  inclusive, reuses and completes the stable partial row id while assigning only unowned frames,
  samples first+last, and normalizes taxonomy entries once at startup.
- **Late automatic review:** after the first follow-up push, Claude added a seventh inline finding:
  `overlap_ms >= 0` treated adjacent same-key sessions as stable-id matches. The red regression
  returned `[Some(7)]` where `[None]` was required; reconciliation now requires strictly positive
  overlap, while the existing positive-overlap stable-id test remains green.
- **Not applied:** the micro-interrupter proposal conflicts with the frozen harness contract, whose
  documented and tested behavior treats fragmented presence of the same key as sustained; changing
  only production would break D9 parity. The `debug_assert!` ownership note was defense-in-depth, not
  a reproduced defect; promoting it to a release panic would violate D10 (failure degrades to no
  sessions, never a dead scheduler). Existing exactly-once ownership tests remain the enforcement.
- **Focused red evidence:** the new tests failed before production edits with `Some(96)` vs
  `Some(100)`, unnormalized `"  Example.EXE  "`, retained start `100` vs expected `200`, and two
  sessions vs one after partial-row restart. After the fixes:

  ```text
  $ cargo test -p sessions --lib -- --nocapture
  running 4 tests
  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

  $ cargo test -p kernel --lib -- --nocapture
  running 55 tests
  test sessions_intel::tests::even_sampling_includes_both_session_endpoints ... ok
  test sessions_scheduler_contract_tests::frozen_guard_treats_equal_last_frame_timestamp_as_overlap ... ok
  test sessions_scheduler_contract_tests::historical_backfill_reuses_an_exact_unfrozen_partial_row ... ok
  test sessions_scheduler_contract_tests::overlap_matching_does_not_reuse_a_merely_touching_session ... ok
  test result: ok. 55 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.14s
  ```
- **Workflow disposition:** routine GitHub review is automatic. `AGENTS.md`/`CLAUDE.md` now forbid
  routine bot mentions and ritual merge warnings; actionable feedback is addressed in code without
  bot-thread replies. The PR remains open under the standing maintainer-approval rule.
- **Post-review D9 rerun (no retuning):** unchanged and still above every gate: shipped predicted
  `8/7/6` vs labeled `11/5/8`, tool `9/11 = 0.818`, pooled partitioned F1 `0.400/0.489` at
  ±120/180 s. The untouched baselines remain micro `0.077/0.086` and grouped `0.391/0.435`.
- **Fresh post-comment full ladder:** the exact non-quiet `cargo test --workspace` completed with
  exit 0 (1,063 output lines); the compact rerun below records the changed and gate-bearing suites.

  ```text
  $ cd ui && npm ci
  added 348 packages, and audited 349 packages in 4s
  found 0 vulnerabilities
  $ npm run lint
  > eslint .
  $ npm run build
  ✓ 434 modules transformed.
  ✓ built in 1.75s
  $ node scripts/stage-mcp.mjs
  [stage-mcp] up to date: C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr4-segmentation-engine\src-tauri\binaries\screensearch-mcp-x86_64-pc-windows-msvc.exe
      Finished `release` profile [optimized] target(s) in 0.21s
  $ cargo fmt --all -- --check
  (no output; exit 0)
  $ cargo clippy --workspace --all-targets -- -D warnings
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.62s
  $ cargo build --workspace
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 15.82s
  $ cargo test --workspace --quiet
  test result: ok. 119 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  test result: ok. 105 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.05s
  test result: ok. 55 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.16s
  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
  test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
  test result: ok. 63 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.56s
  test result: ok. 66 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
  test result: ok. 26 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.00s
  $ git diff --exit-code -- ui/src/bindings
  (no output; exit 0)
  $ git diff --check
  (no output; exit 0)
  ```

## Pass 6 — 2026-07-10 — 0.4.0 PR5 Task 1 (typed session IPC + exact Recap)

- **Implemented:** Exported the existing session domain rows/enums and the new `SessionQuery`,
  `SessionReference`, `SessionDetail`, and `SessionRecapRequest` IPC models through `ts-rs`, with
  explicit JavaScript `number` mappings for every 64-bit field. Added `list_sessions`, `get_session`,
  and `session_recap` to the Tauri invoke surface. `FrameDetail` and `ResumeContext` now carry an
  optional session reference without changing their no-session behavior.
- **Persistence:** Added a truthful session-frame total and an at-most-24 chronological/even sample
  that includes the first and last frames and filters by `frames.session_id`. Session detail returns
  only `exchange` artifacts. A deleted derived session is omitted from frame/resume payloads while
  the frame survives.
- **Report reuse:** Refactored `kernel::reports` behind a scoped internal source. Existing reports
  continue through the time-range source unchanged; session Recap feeds the same depth planning,
  context budgeting, map/reduce, progress, cancellation, citation, and truncation code from an exact
  `frames.session_id` source. Overlapping session spans cannot leak frames. Missing filtered evidence
  returns the existing honest empty response before the shell acquires an answer provider.
- **TDD evidence:** RED failures were captured for missing IPC exports, store sampling/reference
  support, resume hydration, session command helpers, scoped Recap generation, and the shell evidence
  preflight. Each then passed focused GREEN tests. The combined task-relevant suite passed with zero
  failures, and task-relevant clippy completed under `-D warnings`.
- **Skipped / deferred:** Frontend components/routes are the next PR5 task. API/MCP session endpoints
  remain PR6-owned. No schema, migration, taxonomy, segmentation, settings, audio, notification, or
  NavRail changes were made.
- **Hallucinated / corrected:** None. The only intermediate defect was a derive edit landing on the
  adjacent `AnswerDelta`; the compiler caught it before GREEN and the intended `ResumeContext` derive
  was corrected.
- **Still risky:** Live Tauri/UI round-trip and Recap sidecar behavior require the PR5 integrated UI
  and real-app acceptance pass. Rust coverage proves exact frame ownership and no-model empty evidence.
- **Review follow-up:** Kept HTTP/API/MCP response DTOs byte-shape compatible by moving session
  hydration into flattened Tauri-only `UiFrameDetail`/`UiResumeContext` responses; aligned the Recap
  evidence preflight with Rust `str::trim()` so tab/newline-only rows never acquire the provider; and
  enforced the 24-frame cap inside the store regardless of caller input. Each finding was reproduced
  RED and passed focused GREEN tests; UI lint/build and focused clippy/fmt are clean. The final
  integrated broad suite is recorded in Pass 8 and native acceptance in Pass 9.

## Pass 7 — 2026-07-10 — 0.4.0 PR5 Task 2 (sessions UI surfaces)

- **Implemented:** Added typed command/TanStack Query wrappers and stable list/base-detail/
  summary-detail/Recap keys. Capture ticks and retention cleanup invalidate session lists and
  details alongside their frame sources. Timeline now overlays deterministic interval-packed native
  session buttons without replacing the density ribbon. Its configured height is a minimum;
  token-height normal-flow lanes expand it as needed, with no hidden lanes or nested or horizontal
  scrollbar. Focus/AI/meeting/other use only the existing neutral/ok/warn token vocabulary. Loading,
  error, and no-session outcomes stay partial.
- **Drill-in:** Added the lazy `/timeline/session/:id` route with invalid/missing/loading/error/
  partial/populated states. The inference-free detail and lazy title/summary calls start together;
  the view shows neutral fallback title, exact absolute span, kind/tool/host, numeric confidence,
  truthful open boundaries, wrapped representative frames with total count, honest empty exchanges,
  and inline-growing summary/exchange content. Recap is a TanStack mutation over the existing report
  response/progress stream and reuses `ReportView`, wrapped citation tiles, bare custom filename, and
  footer behavior with `session id: <stable-id>` as the active filter.
- **Round-trip/settings:** Session frame/citation/exchange links carry a validated route-state return
  path, so drill-in → Moment → Session survives while direct Moment/session deep links remain valid.
  Moment conditionally links `Part of session`; Deck preserves its existing Moment selection while
  adding the optional session span. Advanced Settings gains a collapsed Sessions expander immediately
  after Enrichment & scheduling with only the two already-generated settings and final clamps/copy.
- **React/performance:** The route remains a separate Vite chunk; independent base/summary requests
  avoid a waterfall; TanStack Query is the sole session server-state owner; only interval lane packing
  is memoized (on primitive range dependencies); no global listener, dependency, handwritten wire
  type, nested vertical scroller, or horizontal strip was added.
- **Skipped / deferred:** No Rust/core/API/MCP/schema/segmentation/taxonomy changes. No frontend test
  runner or dependency was added, per the task brief. A native live-app walkthrough with real session
  data/sidecar remains the integrated PR5 acceptance pass (`03 §13c-5`), not something browser-only
  Vite can truthfully emulate.
- **Hallucinated / corrected:** The first Timeline loading treatment covered too much density; review
  narrowed it to two fixed overlay bars so the frame ribbon stays visible. The session-band slider was
  split into sibling ARIA slider/button layers so interactive buttons are never nested inside a slider.
- **Broke / regressed:** Nothing observed in lint, TypeScript, or Vite production build.
- **Still risky:** Dense real-world overlaps can produce many deterministic lanes. The configured
  ribbon height is a minimum; token-height normal-flow lanes expand it as needed, with no hidden
  lanes or nested or horizontal scrollbar. Native screenshot/DPI review remains important in the
  integrated pass. Recap runtime behavior still depends on a ready local answer model, and is
  covered by its inline retry.
- **Verification:** Fresh `npm ci` → `npm run lint` → `npm run build` completed at exit 0 (348
  packages, 0 vulnerabilities; ESLint clean; TypeScript + Vite transformed 437 modules and emitted a
  distinct 6.65 kB `Session-*.js` chunk). `git diff --exit-code -- ui/src/bindings` and
  `git diff --check` were silent at exit 0; the scope guard printed
  `No Rust/core/API/MCP/schema/segmentation files changed.` Full verbatim output is preserved in
  `.superpowers/sdd/task-2-report.md`.

## Pass 8 — 2026-07-10 — 0.4.0 PR5 final documentation and integrated verification

- **Final review reconciliation:** The measured-width band packer uses the rendered `--hit-min` and
  `--space-1` tokens, clamps bands inside the measured ribbon, and packs collisions using rendered
  pixel bounds. The Scanline ribbon height is a minimum; dense token-height lanes remain in normal
  flow and expand it without hidden lanes or nested/horizontal scrolling. Band labels now carry
  absolute time and kind. Recap has an explicit Cancel action and cancels backend work on cancel,
  route change, or unmount; request-scoped view state prevents late callbacks from exposing a stale
  result. The drill-in surfaces lazy-summary errors separately from base detail.
- **Contract audit:** PR5 adds UI/IPC/report-source behavior only. It performs **no schema change or
  migration** (schema remains 11), opens **no new contradiction**, and requires **no new silent-spec
  gap**. It adds no API/MCP surface, taxonomy/segmenter change, audio, notification, score/streak, or
  NavRail route. Existing frame-level behavior remains additive per D10.
- **Manual acceptance:** `docs/TESTING.md` owns the exact native runbook for band → drill-in → Moment
  → back, exact-session Recap citations and cancellation, Moment/Deck framing, keyboard traversal,
  real open/low-confidence/no-exchange states, reduced motion, and the supported size/DPI layout
  matrix. The real Tauri/WebView2 execution is recorded in Pass 9; this Pass 8 transcript remains
  unchanged as the automated evidence.
- **Integrated verification:** full raw output follows below in the required UI-first order.


### Verbatim integrated verification output

#### `cd ui; npm ci` (exit 0)

```text

added 348 packages, and audited 349 packages in 3s

151 packages are looking for funding
  run `npm fund` for details

found 0 vulnerabilities
npm warn allow-scripts 1 package has install scripts not yet covered by allowScripts:
npm warn allow-scripts   esbuild@0.25.12 (postinstall: node install.js)
npm warn allow-scripts
npm warn allow-scripts Run `npm approve-scripts --allow-scripts-pending` to review, or `npm approve-scripts <pkg>` to allow.
```

#### `cd ui; npm run lint` (exit 0)

```text

> screensearch-ui@0.3.3 lint
> eslint .
```

#### `cd ui; npm run build` (exit 0)

```text

> screensearch-ui@0.3.3 build
> tsc --noEmit && vite build

vite v6.4.3 building for production...
transforming...
✓ 437 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                   0.80 kB │ gzip:  0.37 kB
dist/overlay.html                                 0.99 kB │ gzip:  0.42 kB
dist/assets/globals-BFNqRMpO.css                 32.09 kB │ gzip:  6.80 kB
dist/assets/timeRanges-BJgzkTNX.js                0.29 kB │ gzip:  0.19 kB
dist/assets/openExternal-D7S3WH13.js              0.31 kB │ gzip:  0.23 kB
dist/assets/useAdaptiveBucketCount-pnT9Dvsn.js    0.35 kB │ gzip:  0.26 kB
dist/assets/NotFound-D1MBPARR.js                  0.44 kB │ gzip:  0.32 kB
dist/assets/EmptyState-CIH9-Lyf.js                0.52 kB │ gzip:  0.31 kB
dist/assets/Panel-VqKlACsY.js                     0.68 kB │ gzip:  0.44 kB
dist/assets/timelineDraw-B37WQvuk.js              0.75 kB │ gzip:  0.44 kB
dist/assets/FrameTile-_FAny4N0.js                 0.97 kB │ gzip:  0.55 kB
dist/assets/time-cX9c3v95.js                      1.01 kB │ gzip:  0.46 kB
dist/assets/FrameImage-DlzbU9iL.js                1.62 kB │ gzip:  0.85 kB
dist/assets/HighlightedSnippet-DJxYx7AK.js        1.65 kB │ gzip:  0.83 kB
dist/assets/AnswerStream-D9JpilSJ.js              2.35 kB │ gzip:  1.21 kB
dist/assets/HotkeyField-5fLtLVb6.js               2.54 kB │ gzip:  1.32 kB
dist/assets/ReportView-DabARA68.js                2.92 kB │ gzip:  1.45 kB
dist/assets/path-Exkr9sp_.js                      3.83 kB │ gzip:  0.96 kB
dist/assets/Insights-Dfaw3iYX.js                  5.46 kB │ gzip:  2.11 kB
dist/assets/Session-Def2w79m.js                   7.26 kB │ gzip:  2.53 kB
dist/assets/Moment-CkOOOeEF.js                    8.01 kB │ gzip:  2.98 kB
dist/assets/Timeline-BJS5PDKF.js                 10.24 kB │ gzip:  4.28 kB
dist/assets/Deck-DVfjdvpa.js                     10.87 kB │ gzip:  3.67 kB
dist/assets/overlay-Nl_qXvU0.js                  10.95 kB │ gzip:  3.86 kB
dist/assets/globals-Ci0Ni9kZ.js                  16.66 kB │ gzip:  6.02 kB
dist/assets/main-DzQkOWxb.js                     22.86 kB │ gzip:  7.40 kB
dist/assets/query-u_0r_xiX.js                    35.77 kB │ gzip: 10.59 kB
dist/assets/Recall-D-rFxuFa.js                   37.86 kB │ gzip: 12.25 kB
dist/assets/Settings-CFmnmObf.js                 47.53 kB │ gzip: 13.45 kB
dist/assets/router-CL_afuJ-.js                   64.86 kB │ gzip: 22.21 kB
dist/assets/react-vendor-DTiTYlFD.js            143.42 kB │ gzip: 46.01 kB
dist/assets/CitationTile-CmOyNcPu.js            157.84 kB │ gzip: 47.99 kB
✓ built in 1.68s
```

#### `node scripts/stage-mcp.mjs` (exit 0)

```text
[stage-mcp] building screensearch-mcp (release)...
    Finished `release` profile [optimized] target(s) in 0.24s
[stage-mcp] up to date: C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\src-tauri\binaries\screensearch-mcp-x86_64-pc-windows-msvc.exe
```

#### `cargo fmt --all -- --check` (exit 0)

```text

```

#### `cargo clippy --workspace --all-targets -- -D warnings` (exit 0)

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s
```

#### `cargo build --workspace` (exit 0)

```text
   Compiling inference v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\inference)
   Compiling harness v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\harness)
   Compiling mcp v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\mcp)
   Compiling screensearch v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.63s
```

#### `cargo test --workspace` (exit 0)

```text
    Blocking waiting for file lock on build directory
   Compiling inference v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\inference)
   Compiling mcp v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\mcp)
   Compiling screensearch v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\src-tauri)
   Compiling api v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\api)
   Compiling harness v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\harness)
   Compiling sessions v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\sessions)
   Compiling ocr v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\ocr)
   Compiling capture v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\capture)
   Compiling uia v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\uia)
   Compiling sysmon v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\sysmon)
   Compiling traits v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\traits)
   Compiling textfilter v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\textfilter)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 27.32s
     Running unittests src\lib.rs (target\debug\deps\api-582ea0631a5e71d8.exe)

running 2 tests
test auth::tests::constant_time_eq_matches_only_identical_slices ... ok
test export::tests::utc_stamp_matches_known_instants ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\http_api.rs (target\debug\deps\http_api-c0e77c0557ba1ea0.exe)

running 15 tests
test live_server_for_curl ... ignored
test binds_loopback_only ... ok
test where_was_i_returns_null_when_nothing_qualifies ... ok
test ask_without_answer_model_is_503 ... ok
test unknown_route_is_json_404 ... ok
test export_window_excludes_out_of_range_frames ... ok
test ask_streams_sse_deltas ... ok
test inverted_time_range_is_400 ... ok
test export_over_http_is_valid_json ... ok
test search_returns_hits_from_fixture ... ok
test health_requires_token_and_reports_state ... ok
test token_swap_takes_effect_without_restart ... ok
test marks_crud_roundtrip ... ok
test frame_detail_image_and_not_found ... ok
test export_to_file_writes_valid_json_without_a_server ... ok

test result: ok. 14 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running unittests src\lib.rs (target\debug\deps\capture-9abedaa63a7f798a.exe)

running 26 tests
test events::tests::source_starts_and_stops_cleanly_repeatedly ... ignored, requires a real desktop (USER32 message pump); run locally
test diff::tests::content_hash_is_stable_and_distinct ... ok
test privacy::tests::own_window_pid_rejects_foreign_process ... ok
test privacy::tests::own_window_pid_matches_any_nonzero_own_process_window ... ok
test privacy::tests::own_window_pid_rejects_unknown_foreground_pid ... ok
test tests::degenerate_inputs_are_none ... ok
test tests::target_monitor_falls_back_to_primary_then_first ... ok
test tests::target_monitor_is_the_one_holding_the_foreground_window ... ok
test tests::window_offset_maps_relative_to_monitor_origin ... ok
test tests::window_on_another_monitor_is_none ... ok
test trigger::tests::disabled_foreground_never_emits ... ok
test trigger::tests::burst_of_events_collapses_to_one_capture ... ok
test tests::window_rect_normalizes_within_its_monitor ... ok
test trigger::tests::foreground_event_emits_after_debounce ... ok
test trigger::tests::idle_disabled_never_emits_from_polling ... ok
test trigger::tests::idle_fires_once_per_quiet_period ... ok
test trigger::tests::idle_poll_while_active_is_quiet ... ok
test diff::tests::gate_passes_bypass_forces_unchanged_frame_through ... ok
test trigger::tests::idle_retries_after_min_interval_block ... ok
test trigger::tests::min_interval_suppresses_a_second_capture ... ok
test trigger::tests::pending_event_retries_after_min_interval_block ... ok
test diff::tests::black_vs_white_is_near_full_difference ... ok
test diff::tests::identical_frames_have_zero_difference ... ok
test diff::tests::tiny_change_stays_below_default_threshold ... ok
test diff::tests::resolution_change_is_full_difference ... ok
test diff::tests::gate_passes_first_frame_and_real_change ... ok

test result: ok. 25 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\wgc_smoke.rs (target\debug\deps\wgc_smoke-930b91d667839d47.exe)

running 1 test
test wgc_captures_a_frame_from_the_primary_monitor ... ignored, requires a real desktop + GPU (WGC); run locally

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\doctor-51f5f66998236517.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\doctor-8746a4d1eb703f86.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\embeddings-2b03e9f3cde8be59.exe)

running 2 tests
test tests::loads_and_embeds_text ... ignored, downloads the EmbeddingGemma model; run locally with --ignored
test tests::embed_dim_is_768 ... ok

test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\harness-e42846f165808eea.exe)

running 119 tests
test digest::tests::collapses_consecutive_same_app_runs ... ok
test digest::tests::keyless_frames_are_absorbed_not_split ... ok
test digest::tests::labels_template_lists_marks_as_comments ... ok
test digest::tests::digest_renders_table_marks_and_top_titles ... ok
test digest::tests::leading_keyless_counted_separately ... ok
test export::tests::git_root_walks_above_the_crate_dir ... ok
test group::tests::anchorless_focus_below_floor_is_dropped ... ok
test group::tests::anchored_ai_is_exempt_from_the_density_gate ... ok
test group::tests::anchorless_focus_above_floor_is_kept ... ok
test group::tests::ai_track_spans_through_a_meeting_band ... ok
test group::tests::back_to_back_sustained_tools_split_at_the_handoff ... ok
test group::tests::enforce_non_overlap_then_resort_restores_global_order ... ok
test group::tests::band_interior_unrecognized_is_owned_by_the_meeting ... ok
test group::tests::empty_and_keyless_produce_no_sessions ... ok
test group::tests::density_gate_suppresses_sparse_focus_but_keeps_dense ... ok
test group::tests::focus_ramp_converts_to_ai_keeping_start ... ok
test group::tests::host_precedence_picks_terminal_over_desktop ... ok
test group::tests::gap_at_merge_gap_splits ... ok
test group::tests::leading_none_ramp_attaches_to_an_opening_track ... ok
test group::tests::intra_session_lull_below_merge_gap_holds ... ok
test group::tests::meeting_band_is_a_hard_session_at_presence_endpoints ... ok
test group::tests::low_density_background_ai_track_survives ... ok
test group::tests::mixed_day_output_is_sorted_and_non_overlapping ... ok
test group::tests::meeting_band_splits_the_surrounding_work ... ok
test group::tests::concurrent_walk_is_deterministic ... ok
test group::tests::none_run_over_budget_becomes_focus_overlapping_the_track ... ok
test group::tests::none_sandwich_within_budget_absorbs_into_the_track ... ok
test group::tests::output_globally_sorted_with_cross_track_overlap_present ... ok
test group::tests::per_track_gap_close_is_independent ... ok
test group::tests::overlapping_meetings_emit_overlapping_sessions ... ok
test group::tests::ramp_does_not_fire_when_a_track_is_open ... ok
test group::tests::same_tool_two_instances_fold_into_one_track ... ok
test group::tests::same_track_never_overlaps_itself ... ok
test group::tests::scattered_sub_qualify_presence_emits_no_ai_session ... ok
test group::tests::short_foreign_ai_run_is_absorbed ... ok
test group::tests::short_foreign_run_dropped_by_qualification ... ok
test group::tests::short_meeting_chain_is_demoted_not_a_band ... ok
test group::tests::single_ai_run_is_one_anchored_session ... ok
test group::tests::sustained_foreign_ai_runs_split ... ok
test group::tests::sub_qualify_ai_run_does_not_flip_a_focus_session ... ok
test group::tests::trailing_none_extends_the_last_touched_track ... ok
test group::tests::sustained_foreign_run_no_longer_splits_a_track ... ok
test group::tests::two_tools_interleaved_form_two_overlapping_sessions ... ok
test labels::tests::accepts_touching_sessions ... ok
test labels::tests::accepts_cross_identity_overlap ... ok
test labels::tests::end_at_2400_is_local_midnight_next_day ... ok
test group::tests::unrecognized_excursion_above_budget_splits_off_focus ... ok
test labels::tests::parses_and_resolves_template ... ok
test labels::tests::rejects_ai_without_tool ... ok
test labels::tests::rejects_bad_enum ... ok
test labels::tests::rejects_malformed_time ... ok
test labels::tests::rejects_end_at_or_before_start ... ok
test group::tests::unrecognized_excursion_below_budget_is_absorbed ... ok
test labels::tests::rejects_out_of_start_order ... ok
test labels::tests::rejects_start_at_2400 ... ok
test labels::tests::rejects_same_tool_and_focus_overlap ... ok
test labels::tests::rejects_true_overlap ... ok
test labels::tests::rejects_tool_when_not_ai ... ok
test score::tests::edge_boundaries_are_excluded_both_sides ... ok
test score::tests::missed_and_spurious_boundaries_lower_pr ... ok
test labels::tests::serial_label_files_still_validate ... ok
test score::tests::old_boundary_comparison_is_symmetric ... ok
test score::tests::optimal_match_beats_greedy ... ok
test score::tests::optimal_match_respects_tolerance_and_one_to_one ... ok
test score::tests::partitioned_match_never_exceeds_pooled ... ok
test score::tests::partitioned_perfect_concurrent_day_scores_one ... ok
test score::tests::perfect_day_scores_one ... ok
test score::tests::pooling_sums_then_recomputes ... ok
test score::tests::stability_counts_identity_swaps_as_drift ... ok
test score::tests::tool_accuracy_ignores_larger_overlapping_non_ai_span ... ok
test score::tests::tool_accuracy_max_overlap_and_no_overlap_is_wrong ... ok
test score::tests::typed_matching_does_not_cross_start_and_end ... ok
test score::tests::sweep_1d_varies_one_knob ... ok
test segmenter::tests::brief_excursion_is_absorbed_into_one_span ... ok
test score::tests::stability_small_lookback_unstable_large_lookback_stable ... ok
test score::tests::sweep_grid_scores_every_cell_through_the_grouped_pipeline ... ok
test segmenter::tests::browser_ai_vs_plain_browser_are_distinct ... ok
test segmenter::tests::empty_and_keyless_produce_no_spans ... ok
test segmenter::tests::gap_close_splits_same_context_after_idle ... ok
test segmenter::tests::keyless_stretch_over_gap_close_splits ... ok
test segmenter::tests::fragmented_interrupter_reaching_dwell_splits ... ok
test segmenter::tests::meeting_recognition_sets_kind_without_tool ... ok
test data::tests::rejects_labels_whose_date_mismatches_the_day ... ok
test segmenter::tests::multimonitor_equal_timestamps_are_deterministic ... ok
test segmenter::tests::segment_micro_keeps_sub_floor_runs_that_segment_drops ... ok
test segmenter::tests::same_context_within_gap_close_stays_one_span ... ok
test segmenter::tests::sub_min_len_run_is_dropped ... ok
test segmenter::tests::tool_identity_splits_same_app_into_adjacent_sessions ... ok
test segmenter::tests::sustained_interruption_splits_into_three_spans ... ok
test segmenter::tests::single_sustained_run_is_one_span ... ok
test taxonomy::tests::app_hint_matches_by_exact_stem_not_substring ... ok
test taxonomy::tests::braille_prefix_matches_claude_code ... ok
test taxonomy::tests::claude_desktop_vs_claude_code_precedence ... ok
test taxonomy::tests::case_insensitive_and_none_on_empty_context ... ok
test taxonomy::tests::claude_code_needs_terminal_app_and_claude_title ... ok
test taxonomy::tests::browser_ai_needs_browser_app_and_ai_title ... ok
test taxonomy::tests::codex_desktop_app ... ok
test taxonomy::tests::codex_stem_does_not_collide_with_a_code_ide_hint ... ok
test taxonomy::tests::invalid_prefix_range_rejected_at_parse ... ok
test data::tests::round_trips_an_exported_day ... ok
test taxonomy::tests::rejects_entry_with_empty_matcher ... ok
test taxonomy::tests::leading_whitespace_before_spinner_tolerated ... ok
test taxonomy::tests::meeting_recognition ... ok
test taxonomy::tests::plain_shell_title_does_not_match_prefix ... ok
test taxonomy::tests::renamed_chatgpt_stem_maps_to_codex_except_classic ... ok
test taxonomy::tests::plain_terminal_without_ai_title_is_unrecognized ... ok
test taxonomy::tests::spinner_prefix_confined_to_terminal_stems ... ok
test taxonomy::tests::seed_parses_and_has_the_d7_set ... ok
test taxonomy::tests::sparkle_prefix_matches_claude_code ... ok
test taxonomy::tests::substring_fallback_still_recognizes_claude_code ... ok
test export::tests::day_bounds_are_24h_and_offset_is_consistent ... ok
test export::tests::day_marks_read ... ok
test export::tests::day_frames_ordered_with_multimonitor_and_keyless ... ok
test export::tests::read_only_connection_rejects_writes ... ok
test export::tests::backup_refuses_destination_in_repo_tree ... ok
test export::tests::survey_counts_signals ... ok
test export::tests::backup_snapshots_and_verifies ... ok
test export::tests::write_day_emits_all_files ... ok
test export::tests::re_export_preserves_hand_edited_labels ... ok

test result: ok. 119 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s

     Running unittests src\main.rs (target\debug\deps\harness-52d5ebe66e6a73f0.exe)

running 1 test
test tests::overlaps_any_excludes_self_copy_by_value ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\shipped_parity.rs (target\debug\deps\shipped_parity-d4f55a6b5929ecca.exe)

running 3 tests
test shipped_open_projection_keeps_the_parity_span_fields ... ok
test shipped_matches_frozen_harness_concurrent_on_synthetic_day ... ok
test shipped_parity_covers_none_meetings_gap_edges_density_and_qualification ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\inference-6c26c062e8096d06.exe)

running 105 tests
test answer::tests::estimate_tokens_does_not_undercount_cjk ... ok
test answer::tests::reply_budget_leaves_room_for_grounding_in_a_small_context ... ok
test answer::tests::builds_grounded_prompt_with_frame_tags ... ok
test answer::tests::report_summary_drops_overflow_and_cites_only_what_fit ... ok
test answer::tests::handles_tag_split_across_chunks ... ok
test answer::tests::report_model_label_extracts_the_gguf_filename ... ok
test answer::tests::drops_chunks_that_exceed_the_context_budget ... ok
test answer::tests::report_summary_messages_use_the_given_system_prompt_and_tag_frames ... ok
test answer::tests::splits_inline_think_tags ... ok
test answer::tests::plain_content_is_all_tokens ... ok
test client::tests::http_error_includes_status_and_body ... ok
test answer::tests::pump_deltas_completes_when_stream_finishes ... ok
test client::tests::http_error_truncates_long_body_on_char_boundary ... ok
test answer::tests::truncate_to_tokens_never_splits_a_multibyte_char ... ok
test answer::tests::truncates_an_oversized_top_chunk_instead_of_dropping_everything ... ok
test client::tests::response_format_is_omitted_from_body_when_none ... ok
test client::tests::response_format_is_serialized_when_set ... ok
test client::tests::http_error_handles_empty_body ... ok
test download::tests::answer_repo_needs_no_mmproj ... ok
test download::tests::byte_counter_accumulates_streamed_bytes ... ok
test download::tests::content_range_total_parses_suffix ... ok
test download::tests::falls_back_to_first_gguf_when_no_q4_k_m ... ok
test download::tests::lfs_sha256_trusts_only_x_linked_etag_not_cdn_etag ... ok
test download::tests::no_vulkan_asset_returns_none ... ok
test download::tests::no_vulkan_in_any_release_returns_none ... ok
test download::tests::parse_sha256_normalizes_etag_forms ... ok
test download::tests::picks_q4_k_m_weights_and_mmproj_for_vision ... ok
test download::tests::picks_win_vulkan_x64_asset ... ok
test download::tests::place_if_cached_returns_false_when_nothing_cached ... ok
test download::tests::failed_binary_extraction_cleans_partial_install ... ok
test download::tests::prefers_the_newest_release_that_has_vulkan ... ok
test download::tests::chunked_download_errors_when_server_ignores_range ... ok
test download::tests::range_plan_requires_ranges_and_known_size ... ok
test download::tests::skips_release_with_incomplete_assets ... ok
test download::tests::stall_limit_is_timeout_over_poll_and_never_zero ... ok
test download::tests::stall_step_resets_on_progress_and_counts_otherwise ... ok
test download::tests::installed_binary_candidates_include_normal_and_overrides ... ok
test download::tests::place_if_cached_short_circuits_when_dest_already_present ... ok
test flags::tests::conservative_fallback_only_pins_context ... ok
test flags::tests::detects_missing_flash_attn ... ok
test flags::tests::parses_legacy_boolean_flash_attn ... ok
test flags::tests::parses_modern_value_taking_flash_attn ... ok
test flags::tests::parses_parenthesised_value_taking_flash_attn ... ok
test models::tests::answer_resolution_needs_no_mmproj ... ok
test answer::tests::pump_deltas_cancels_and_aborts_sidecar_on_consumer_drop ... ok
test models::tests::repo_mapping_matches_registry ... ok
test download::tests::manifest_load_or_init_distinguishes_missing_valid_and_mismatched ... ok
test download::tests::env_override_wins_over_existing_install ... ok
test process::tests::escapes_embedded_quotes_and_trailing_backslashes ... ok
test process::tests::query_private_bytes_is_none_for_pid_zero ... ok
test process::tests::query_private_bytes_reports_for_self ... ok
test process::tests::quotes_only_when_needed ... ok
test process::tests::quotes_paths_with_spaces ... ok
test models::tests::resolution_carries_device_selector ... ok
test process::tests::total_physical_ram_is_nonzero ... ok
test supervisor::tests::auto_ceiling_is_half_ram_within_band ... ok
test supervisor::tests::build_args_adds_device_when_configured ... ok
test supervisor::tests::build_args_adds_mmproj_only_for_vision ... ok
test supervisor::tests::build_args_distinguishes_auto_from_on_flash_attn ... ok
test supervisor::tests::build_args_drops_explicit_on_flash_when_binary_unsupported ... ok
test supervisor::tests::build_args_emits_full_tuning_when_supported ... ok
test supervisor::tests::build_args_leaves_f16_kv_implicit ... ok
test supervisor::tests::build_args_omits_ctx_when_unsupported ... ok
test supervisor::tests::build_args_omits_kv_quant_without_flash ... ok
test supervisor::tests::build_args_uses_bare_flash_attn_for_bool_flag ... ok
test supervisor::tests::evict_predicate_respects_inflight_backfill_and_ttl ... ok
test supervisor::tests::idle_predicate ... ok
test models::tests::prefers_q4_k_m_and_excludes_mmproj ... ok
test supervisor::tests::resolve_ceiling_branches ... ok
test supervisor::tests::restart_on_tuning_change ... ok
test supervisor::tests::restart_only_on_model_change ... ok
test supervisor::tests::running_sidecar_is_reused_only_when_process_and_health_are_alive ... ok
test supervisor::tests::should_recycle_disabled_when_ceiling_zero ... ok
test models::tests::auto_ctx_size_resolves_per_lane_and_override_passes_through ... ok
test models::tests::vision_resolution_requires_mmproj ... ok
test supervisor::tests::should_recycle_only_at_or_above_ceiling ... ok
test vision::tests::activity_type_is_normalised_case_and_space_insensitively ... ok
test vision::tests::app_hint_is_trimmed_and_kept_when_real ... ok
test vision::tests::explicit_null_activity_type_becomes_none ... ok
test vision::tests::falls_back_to_raw_text_on_non_json ... ok
test vision::tests::missing_confidence_becomes_unknown_sentinel ... ok
test download::tests::chunk_requests_follow_redirect_and_preserve_range ... ok
test vision::tests::off_enum_activity_type_becomes_none ... ok
test vision::tests::null_app_hint_string_is_dropped_case_insensitively ... ok
test vision::tests::out_of_range_confidence_becomes_unknown_sentinel ... ok
test vision::tests::parses_well_formed_json ... ok
test vision::tests::response_format_allows_null_activity_and_drops_it_from_required ... ok
test vision::tests::tolerates_code_fences_and_prose ... ok
test vision::tests::vlm_request_dims_match_encoded_output ... ok
test vision::tests::zero_confidence_becomes_unknown_sentinel ... ok
test download::tests::fresh_part_discards_stale_all_done_manifest ... ok
test download::tests::oversized_part_discards_stale_manifest ... ok
test download::tests::fresh_part_discards_stale_partial_manifest ... ok
test download::tests::truncated_part_discards_stale_partial_manifest ... ok
test download::tests::resume_skips_already_completed_chunks ... ok
test download::tests::chunked_download_assembles_byte_identical_file ... ok
test download::tests::integrity_accepts_matching_sha256_and_rejects_a_wrong_one ... ok
test process::tests::spawn_suspended_captures_child_stdout_to_log ... ok
test supervisor::tests::request_gate_allows_concurrent_regular_requests ... ok
test download::tests::chunked_download_fails_fast_on_stuck_chunk ... ok
test supervisor::tests::switch_gate_waits_for_active_request_to_drop ... ok
test vision::tests::small_frame_passes_through_at_native_size ... ok
test download::tests::chunk_retries_transient_403_then_succeeds ... ok
test vision::tests::downscales_oversized_frame_to_max_edge ... ok
test download::tests::exhausted_transient_is_not_reported_as_ignored_range ... ok

test result: ok. 105 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.04s

     Running unittests src\bin\jobhelper.rs (target\debug\deps\jobhelper-cd7052ccf94ce638.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\no_orphan.rs (target\debug\deps\no_orphan-b040a5695761adba.exe)

running 1 test
test killing_parent_terminates_job_bound_child ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests\reap.rs (target\debug\deps\reap-b98a2c932c290591.exe)

running 3 tests
test reaps_a_matching_stray_from_any_owned_install_path ... ok
test never_reaps_a_foreign_pid ... ok
test reaps_a_matching_stray ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests\sidecar_client.rs (target\debug\deps\sidecar_client-d68a2248fbcd4151.exe)

running 8 tests
test health_false_when_unavailable ... ok
test health_reports_success ... ok
test vision_completion_returns_message_content ... ok
test answer_stream_yields_ordered_pieces ... ok
test stream_connect_times_out_when_initial_post_hangs ... ok
test stream_times_out_when_no_sse_chunk_arrives ... ok
test completion_times_out_when_sidecar_hangs ... ok
test health_times_out_quickly_when_sidecar_hangs ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running tests\smoke.rs (target\debug\deps\smoke-c453c7d39f21397c.exe)

running 2 tests
test real_answer_streams_tokens ... ignored, downloads a multi-GB model and runs a real llama-server on a GPU
test real_vision_tags_an_image ... ignored, downloads a multi-GB model + projector and runs a real llama-server on a GPU

test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\kernel-10936c16b9f0a783.exe)

running 57 tests
test capture_loop::tests::image_paths_are_webp_and_day_sharded ... ok
test reports::tests::grid_size_is_one_per_day_capped_at_max ... ok
test reports::tests::plan_depth_floor_wins_over_global_cap_on_long_ranges ... ok
test reports::tests::plan_depth_gives_every_active_period_its_budget ... ok
test reports::tests::fits_single_pass_detects_overflow ... ok
test reports::tests::split_chunks_batches_every_chunk_without_dropping ... ok
test resume::tests::all_keyless_frames_is_none ... ok
test resume::tests::brief_excursion_is_absorbed_into_the_run ... ok
test resume::tests::browser_domain_parsing ... ok
test resume::tests::dwell_exactly_at_threshold_qualifies ... ok
test resume::tests::distinct_browser_domains_are_distinct_contexts ... ok
test capture_loop::tests::max_width_at_or_above_native_is_noop ... ok
test resume::tests::equal_timestamps_are_deterministic ... ok
test resume::tests::excluded_app_is_skipped_for_next_candidate ... ok
test resume::tests::no_frames_is_none ... ok
test resume::tests::fragmented_interrupter_reaching_dwell_splits ... ok
test resume::tests::returns_prior_sustained_run_with_span_and_last_frame ... ok
test resume::tests::screensearch_context_never_qualifies ... ok
test resume::tests::single_context_only_is_none ... ok
test resume::tests::single_frame_run_fails_dwell ... ok
test resume::tests::sustained_interruption_splits_the_run ... ok
test sessions_intel::tests::even_sampling_includes_both_session_endpoints ... ok
test sessions_scheduler_contract_tests::frozen_guard_treats_equal_last_frame_timestamp_as_overlap ... ok
test sessions_scheduler_contract_tests::frozen_guard_trims_only_the_same_identity_track ... ok
test sessions_scheduler_contract_tests::historical_cut_extends_to_the_next_global_merge_gap ... ok
test sessions_scheduler_contract_tests::historical_cut_never_skips_unscanned_future_frames ... ok
test sessions_scheduler_contract_tests::new_session_projection_keeps_open_rows_null_ended ... ok
test sessions_scheduler_contract_tests::overlap_matching_does_not_reuse_a_merely_touching_session ... ok
test sessions_scheduler_contract_tests::overlap_matching_reuses_one_unfrozen_id_per_draft ... ok
test settings::tests::marks_hotkey_empty_falls_back_to_default ... ok
test settings::tests::recycle_rss_mb_clamps_explicit_but_keeps_auto_zero ... ok
test settings::tests::resume_min_dwell_secs_clamps_to_band ... ok
test throttle::tests::enters_high_only_after_sustained_dwell ... ok
test throttle::tests::escalates_to_sustained_after_continued_pressure ... ok
test throttle::tests::exits_one_level_at_a_time_after_recovery_dwell ... ok
test throttle::tests::flapping_spike_does_not_trip ... ok
test throttle::tests::gpu_hot_blocks_exit_even_when_cpu_cool ... ok
test throttle::tests::gpu_pressure_alone_can_throttle ... ok
test throttle::tests::gpu_unmonitored_is_cpu_only ... ok
test throttle::tests::hysteresis_band_holds_level ... ok
test throttle::tests::normal_stays_normal_below_enter ... ok
test worker_pool::tests::active_job_guard_tracks_in_flight_job_until_drop ... ok
test worker_pool::tests::completion_info_only_for_changed_frame_jobs ... ok
test capture_loop::tests::native_max_width_zero_does_not_downscale ... ok
test reports::tests::empty_range_is_honest_with_no_sidecar_call ... ok
test reports::tests::session_recap_without_usable_content_makes_zero_model_calls ... ok
test reports::tests::daily_small_range_uses_single_pass ... ok
test reports::tests::cancellation_between_passes_returns_err ... ok
test sessions_scheduler_contract_tests::incremental_reconciliation_keeps_the_unfrozen_id_stable ... ok
test reports::tests::session_recap_with_overlapping_spans_cites_only_the_named_session ... ok
test sessions_scheduler_contract_tests::delayed_backfill_trims_before_frozen_incremental_overlap ... ok
test reports::tests::dense_single_period_splits_into_passes_without_truncating ... ok
test reports::tests::reduce_overflow_preserves_all_days_via_hierarchical_reduce ... ok
test reports::tests::weekly_covers_every_active_day_and_cites_first_and_last ... ok
test sessions_scheduler_contract_tests::historical_backfill_retries_past_a_fixed_target_cutting_a_live_track ... ok
test sessions_scheduler_contract_tests::historical_backfill_reuses_an_exact_unfrozen_partial_row ... ok
test capture_loop::tests::positive_max_width_downscales_keeping_aspect ... ok

test result: ok. 57 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.15s

     Running tests\enrichment.rs (target\debug\deps\enrichment-3fad8fbdc7840796.exe)

running 9 tests
test process_job_dead_letters_missing_frame_id ... ok
test process_job_vision_tag_retries_without_provider ... ok
test process_job_completes_on_empty_ocr_without_embedding ... ok
test process_job_retries_then_dead_letters_on_persistent_embed_failure ... ok
test process_job_embeds_text_and_completes ... ok
test process_job_vision_tag_writes_analysis ... ok
test vision_tag_failure_records_full_error_chain ... ok
test vision_jobs_drain_when_embeddings_disabled ... ok
test attach_embedder_drains_backlog_and_vector_arm_finds_frame ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests\pipeline.rs (target\debug\deps\pipeline-2bca85d4b7e2e281.exe)

running 12 tests
test kernel_refuses_to_start_capture_when_ocr_is_unavailable ... ok
test add_mark_capture_now_fails_when_capture_off ... ok
test add_mark_by_frame_id_marks_directly_and_validates_source ... ok
test add_mark_capture_now_propagates_denial_and_inserts_no_mark ... ok
test kernel_start_then_stop_flips_capture_readiness ... ok
test capture_loop_skips_embed_jobs_when_disabled ... ok
test stop_capture_notifies_ocr_provider ... ok
test reload_capture_restarts_loop_with_fresh_config ... ok
test add_mark_capture_now_inserts_manual_frame_and_mark ... ok
test capture_loop_stores_frames_ocr_jpegs_and_enqueues_embed_jobs ... ok
test kernel_clears_capture_and_marks_error_when_source_shuts_down ... ok
test source_shutdown_notifies_ocr_provider ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

     Running tests\sessions_intel.rs (target\debug\deps\sessions_intel-b4dd4ed302db897e.exe)

running 1 test
test lazy_intelligence_calls_summarize_once_then_serves_the_cache ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests\settings.rs (target\debug\deps\settings-aa36f96da5df33a9.exe)

running 16 tests
test unknown_tier_falls_back_to_default_without_rewrite ... ok
test session_settings_round_trip_and_clamp_to_the_final_contract ... ok
test save_settings_never_writes_retired_keys ... ok
test persisted_beta_tier_remaps_to_quality_and_persists ... ok
test save_settings_persists_sanitized_numeric_values ... ok
test overlay_hotkey_custom_value_survives ... ok
test overlay_hotkey_empty_string_resets_to_default ... ok
test round_trips_non_default_values ... ok
test round_trips_defaults ... ok
test sidecar_device_round_trips_empty_as_none ... ok
test load_settings_sanitizes_persisted_numeric_values ... ok
test load_drops_retired_event_keys_without_error ... ok
test overlay_hotkey_legacy_default_remaps_once ... ok
test sidecar_ctx_size_zero_is_preserved_as_auto_sentinel ... ok
test overlay_hotkey_failed_remap_is_retried_not_latched ... ok
test overlay_hotkey_deliberate_legacy_survives_after_migration ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

     Running tests\throttle.rs (target\debug\deps\throttle-a638624ad56f688d.exe)

running 2 tests
test throttle_disabled_drains_everything ... ok
test throttle_pauses_heavy_enrichment_then_resumes_on_recovery ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.66s

     Running unittests src\lib.rs (target\debug\deps\mcp-b92ba198223ba953.exe)

running 35 tests
test client::tests::api_error_is_code_colon_message ... ok
test client::tests::not_reachable_and_no_token_carry_the_contract_phrase ... ok
test config::tests::unknown_flag_and_missing_value_are_bad_usage ... ok
test config::tests::empty_token_is_none ... ok
test config::tests::flag_overrides_env_overrides_default ... ok
test config::tests::non_loopback_url_is_rejected ... ok
test config::tests::help_and_version_short_circuit ... ok
test rpc::tests::malformed_object_with_id_is_invalid_request ... ok
test config::tests::loopback_hosts_are_accepted ... ok
test config::tests::equals_and_space_flag_forms ... ok
test config::tests::trailing_slash_trimmed ... ok
test rpc::tests::batch_array_yields_32600 ... ok
test config::tests::defaults_when_nothing_set ... ok
test rpc::tests::malformed_object_without_id_is_dropped ... ok
test rpc::tests::notification_has_no_id ... ok
test rpc::tests::valid_request_is_parsed ... ok
test client::tests::unauthorized_mentions_401_and_regeneration ... ok
test rpc::tests::responses_are_single_line_even_with_newlines_in_strings ... ok
test server::tests::downgrades_unknown_or_absent_version ... ok
test server::tests::echoes_each_supported_version ... ok
test rpc::tests::parse_error_yields_32700_with_null_id ... ok
test server::tests::initialize_result_shape ... ok
test sse::tests::citations_deduped_in_arrival_order ... ok
test sse::tests::crlf_is_stripped ... ok
test sse::tests::done_and_error_terminate ... ok
test sse::tests::empty_data_object_ignored ... ok
test sse::tests::keepalives_and_blank_lines_ignored ... ok
test sse::tests::thinking_discarded_tokens_concatenated ... ok
test sse::tests::lines_reassemble_across_every_byte_boundary ... ok
test tools::tests::add_mark_body_captures_now_when_frame_id_absent_or_null ... ok
test tools::tests::add_mark_body_rejects_non_integer_frame_id ... ok
test tools::tests::add_mark_body_uses_integer_frame_id ... ok
test tools::tests::error_result_is_flagged ... ok
test tools::tests::exactly_six_tools_with_object_schemas ... ok
test tools::tests::required_fields_are_declared ... ok

test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\screensearch_mcp-f064a446419bae45.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\stdio_mcp.rs (target\debug\deps\stdio_mcp-ff6686a65554a830.exe)

running 19 tests
test unknown_method_is_method_not_found ... ok
test get_moment_unknown_frame_is_tool_error ... ok
test get_moment_text_only_and_include_image ... ok
test child_exits_zero_on_stdin_close ... ok
test malformed_line_is_parse_error ... ok
test ask_tool_aggregates_answer_and_citation ... ok
test ask_without_model_is_tool_error ... ok
test batch_line_is_invalid_request ... ok
test search_tool_roundtrips_fixture ... ok
test add_mark_frame_id_then_list_marks_roundtrip ... ok
test ping_returns_empty_result ... ok
test get_moment_purged_image_notes_purge_without_error ... ok
[screensearch-mcp] warning: no API token configured (SCREENSEARCH_API_TOKEN unset and --token not given); tool calls will return a guidance error until it is set.
test missing_token_still_serves_tools_list_but_calls_are_guided ... ok
test add_mark_now_surfaces_unavailable ... ok
test handshake_and_tools_list_over_stdio ... ok
test unknown_tool_is_protocol_error ... ok
test where_was_i_null_returns_human_message ... ok
test wrong_token_returns_guided_401_message ... ok
test api_off_tool_calls_return_guided_error ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.07s

     Running unittests src\lib.rs (target\debug\deps\ocr-87f88b55c9cff589.exe)

running 2 tests
test tests::winrt_ocr_recognizes_blank_image ... ignored, requires WinRT OCR language pack; run locally
test tests::normalize_rect_maps_and_clamps_to_unit_square ... ok

test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\screensearch_lib-d40f226e2fda898f.exe)

running 21 tests
test local_api::tests::port_clamps_to_floor ... ok
test tests::ipc_presentation_limits_are_clamped ... ok
test tests::hydrate_ask_context_returns_clear_error_when_ocr_texts_fails ... ok
test tests::safe_frame_path_accepts_only_relative_frames_children ... ok
test tests::parses_llama_cpp_device_ids ... ok
test tests::sanitize_report_stem_produces_a_safe_leaf_name ... ok
test tests::session_query_normalizes_time_kind_tool_and_limit ... ok
test tray::tests::capture_status_maps_to_visual ... ok
test tests::open_store_reports_error_when_db_cannot_open ... ok
test tray::tests::labels_track_state ... ok
test tray::tests::composed_icons_differ_per_state ... ok
test tests::unique_markdown_path_appends_2_3_on_collision ... ok
test tests::db_file_family_size_includes_wal_and_shm ... ok
test local_api::tests::fresh_profile_defaults_off ... ok
test local_api::tests::bind_failure_keeps_enabled_with_error ... ok
test tests::ui_resume_context_hydrates_session_without_changing_external_resume_shape ... ok
test tests::session_recap_evidence_probe_rejects_missing_text_before_provider_acquisition ... ok
test tests::merge_purged_spans_once_merges_backlog_then_watermarks_and_is_idempotent ... ok
test tests::session_detail_samples_24_frames_and_returns_only_exchanges_without_inference ... ok
test local_api::tests::enabling_generates_token_once_and_persists ... ok
test tests::open_store_creates_db_file_and_reports_ready ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running unittests src\main.rs (target\debug\deps\screensearch-e067c9d301e668e6.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\config_guard.rs (target\debug\deps\config_guard-24418d3f8fbadcd1.exe)

running 1 test
test overlay_window_is_precreated_hidden_and_capture_protected ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\e2e_capture.rs (target\debug\deps\e2e_capture-24b27b5439f666f3.exe)

running 1 test
test capture_pipeline_stores_frames_ocr_and_enqueues_embed_jobs ... ignored, real WGC + WinRT capture; requires a desktop session, run locally

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\sessions-693e335fd4aabc8e.exe)

running 4 tests
test contract_tests::confidence_tiers_keep_anchorless_below_anchored ... ok
test taxonomy::tests::parsing_normalizes_match_inputs_once ... ok
test taxonomy::tests::invalid_prefix_range_is_rejected ... ok
test taxonomy::tests::seed_has_nine_entries ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\engine.rs (target\debug\deps\engine-9241b455e1799f98.exe)

running 21 tests
test claude_code_markers_extract_only_explicit_roles ... ok
test codex_desktop_chrome_is_not_misclassified_as_an_agent_turn ... ok
test desktop_and_browser_markers_extract_heading_blocks ... ok
test no_marker_means_no_exchange_and_duplicates_collapse ... ok
test empty_claude_prompt_does_not_capture_the_terminal_status_bar ... ok
test browser_ai_requires_browser_stem_and_ai_title ... ok
test long_unrecognized_run_becomes_focus_material ... ok
test consolidated_short_excursion_frames_belong_to_the_surviving_micro ... ok
test bundled_taxonomy_v3_parses_at_startup ... ok
test exclusive_frame_ownership_survives_cross_track_overlap_and_none_absorption ... ok
test meetings_never_absorb_and_can_overlap_ai_and_each_other ... ok
test same_track_splits_only_at_its_own_merge_gap ... ok
test windows_breadcrumb_chevrons_are_not_claude_code_prompts ... ok
test spinner_prefix_recognizes_claude_code ... ok
test chatgpt_renamed_desktop_maps_to_codex_but_classic_does_not ... ok
test sub_qualification_ai_track_is_dropped ... ok
test open_flag_tracks_inactivity_at_now ... ok
test sparse_focus_is_density_gated_but_ai_is_exempt ... ok
test two_tools_interleaved_form_overlapping_tracks ... ok
test sustained_foreign_identity_does_not_close_incumbent ... ok
test confidence_penalizes_absorbed_time_and_keeps_ai_above_focus ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\store-3d8c8c6035071314.exe)

running 38 tests
test marks::tests::mark_survives_image_purge_with_text_kept ... ok
test frames::tests::image_older_than_excludes_purged_and_recent ... ok
test marks::tests::insert_rejects_unknown_frame_with_clear_error ... ok
test marks::tests::list_orders_unresolved_first_then_newest_first ... ok
test frames::tests::sample_returns_all_when_count_under_limit ... ok
test frames::tests::sample_degenerate_windows_are_empty ... ok
test marks::tests::set_note_round_trips_and_rejects_unknown ... ok
test marks::tests::resolve_is_idempotent_but_errors_on_unknown ... ok
test frames::tests::recent_frame_contexts_newest_first_capped_with_id_tiebreak ... ok
test frames::tests::sample_spreads_evenly_and_includes_the_earliest_frame ... ok
test records::tests::merge_spans_to_lines_collapses_words_and_unions_boxes ... ok
test records::tests::merge_spans_to_lines_empty_is_empty ... ok
test frames::tests::purge_frame_image_drops_image_but_keeps_text_proof ... ok
test records::tests::merge_spans_to_lines_is_idempotent ... ok
test records::tests::merge_spans_to_lines_prefers_content_role ... ok
test records::tests::primary_source_for_maps_engine_to_db_token ... ok
test search::tests::escalating_knn_caps_at_the_k_ceiling ... ok
test search::tests::escalating_knn_stops_at_window_count_not_ceiling ... ok
test search::tests::escalating_knn_stops_when_table_exhausted ... ok
test migration_tests::migration_v10_adds_marks_with_cascade ... ok
test search::tests::escalating_knn_truncates_to_pool ... ok
test search::tests::escalating_knn_widens_until_target_reached ... ok
test search::tests::normalized_limit_clamps_to_the_backend_ceiling ... ok
test migration_tests::migration_v11_adds_sessions_structure_only ... ok
test frames::tests::sample_returns_full_quota_when_just_over_limit ... ok
test frames::tests::sample_caps_at_limit_within_window ... ok
test migration_tests::fresh_and_migrated_schemas_agree_at_latest ... ok
test migration_tests::migration_v11_artifact_role_kind_coupling ... ok
test search::tests::count_embedded_frames_dedups_chunks_caps_and_bounds_the_scan ... ok
test migration_tests::migration_v8_indexes_image_retention_sweep ... ok
test migration_tests::migration_v11_sessions_check_constraints ... ok
test records::tests::filtered_insert_records_text_source_from_engine ... ok
test migration_tests::migration_v7_adds_image_purged_present_by_default ... ok
test migration_tests::migration_v6_widens_capture_trigger_check_without_dropping_children ... ok
test migration_tests::migration_v11_fk_set_null_and_cascade ... ok
test migration_tests::migration_v9_drops_image_lane_and_embed_image_jobs ... ok
test search::tests::include_chrome_searches_raw_text_independently_of_content ... ok
test migration_tests::migration_v11_preserves_frame_surfaces ... ok

test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s

     Running tests\perf.rs (target\debug\deps\perf-1cadcb60d0bbcbc5.exe)

running 1 test
test hybrid_search_under_200ms_on_realistic_db ... ignored, seeds 10k frames + 768-dim vectors; run locally: cargo test -p store --test perf -- --ignored --nocapture

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\sessions.rs (target\debug\deps\sessions-275f51a9a0bc8140.exe)

running 9 tests
test title_summary_cache_updates_the_row_without_touching_boundaries ... ok
test session_crud_is_unfrozen_only_and_ids_stay_stable ... ok
test session_reference_for_frame_omits_deleted_session ... ok
test frozen_session_frames_cannot_be_cleared_or_reassigned ... ok
test session_queries_use_half_open_overlap_and_request_time_for_open_rows ... ok
test deleting_unfrozen_sessions_preserves_frames_text_and_marks ... ok
test artifact_checks_and_delete_by_kind_are_enforced ... ok
test frame_metadata_and_content_reads_are_chronological ... ok
test session_frame_sample_reports_total_and_even_chronological_endpoints_without_leakage ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s

     Running tests\store.rs (target\debug\deps\store-7274e91ea5422cb5.exe)

running 64 tests
test completing_or_failing_an_unknown_job_is_an_error ... ok
test degrade_frame_to_text_purges_even_without_spans ... ok
test complete_job_requires_running_state ... ok
test complete_job_moves_to_done ... ok
test claim_filters_by_kind ... ok
test claim_returns_highest_priority_first_and_marks_running ... ok
test cancel_pending_vision_jobs_removes_only_pending_vision ... ok
test claim_honors_not_before_schedule ... ok
test degrade_frame_to_text_merges_spans_and_purges_atomically ... ok
test empty_time_window_returns_nothing_via_vector_arm ... ok
test delete_frame_cascades_and_purges_vectors ... ok
test export_frames_page_honors_half_open_time_window ... ok
test backfill_filter_version_recleans_old_frames_against_warm_catalog ... ok
test backfill_filter_version_invalidates_stale_text_embedding ... ok
test concurrent_claims_never_double_claim ... ok
test dense_time_window_returns_the_pool_nearest_in_window_matches ... ok
test export_frames_page_left_join_yields_none_content_for_textless_frames ... ok
test fail_without_retry_at_dead_letters_immediately ... ok
test export_frames_page_zero_limit_is_empty ... ok
test fail_job_requires_running_state ... ok
test frame_enrichment_input_reads_path_and_optional_text ... ok
test fail_retries_with_backoff_then_dead_letters_at_max_attempts ... ok
test frames_in_range_lists_window_recent_first ... ok
test frames_older_than_lists_bounded_retention_candidates ... ok
test frames_with_app_hint_matches_case_insensitively ... ok
test live_db_copy_migrates_to_v11_fast_and_clean ... ignored, manual Gate 0: set SCREENSEARCH_MIGRATION_CHECK_DB to a THROWAWAY copy of the live DB
test hybrid_search_empty_query_returns_nothing ... ok
test hybrid_search_fts_only_without_embedder ... ok
test hybrid_search_fuses_fts_and_vector_arms_via_rrf ... ok
test hybrid_search_honors_time_range ... ok
test insert_frame_then_get_frame_returns_context ... ok
test insert_ocr_persists_spans_with_pr2_defaults ... ok
test hybrid_search_respects_limit ... ok
test insert_ocr_then_get_frame_has_text ... ok
test insert_vision_then_get_frame_has_analysis ... ok
test insights_summary_aggregates_truthfully ... ok
test merge_frame_spans_to_lines_is_noop_without_spans ... ok
test insights_summary_uses_requested_bucket_count ... ok
test job_stats_splits_out_vision_pending_and_running ... ok
test open_path_rejects_future_schema_version ... ok
test nearest_frame_in_range_ignores_frames_outside_window ... ok
test insert_ocr_filtered_suppresses_repeated_chrome_after_threshold ... ok
test merge_frame_spans_to_lines_shrinks_rows_but_keeps_search_and_reconstruction ... ok
test nearest_frame_picks_closest_with_after_winning_ties ... ok
test neighbour_frames_brackets_anchor_with_closest_each_side ... ok
test hybrid_search_clamps_excessive_limit ... ok
test open_in_memory_migrates_to_latest_schema_version ... ok
test purged_frame_ids_lists_only_purged_after_cursor ... ok
test reset_stale_running_jobs_spares_fresh_running ... ok
test ocr_texts_bulk_fetches_nonempty_only ... ok
test settings_round_trip_and_overwrite ... ok
test set_settings_batch_writes_all_and_overwrites ... ok
test reset_stale_running_jobs_requeues_running ... ok
test timeline_buckets_survives_extreme_ranges ... ok
test timeline_buckets_are_sparse_and_half_open ... ok
test text_embedding_knn_orders_by_cosine_distance ... ok
test untagged_frame_ids_excludes_tagged_and_honors_range ... ok
test untagged_frame_ids_excludes_in_flight_vision_jobs ... ok
test upsert_text_embedding_replaces_vector_in_place ... ok
test sparse_time_window_returns_every_in_window_match ... ok
test wrong_dimension_embedding_is_rejected ... ok
test works_through_the_store_trait_object ... ok
test export_frames_page_pages_through_all_frames_in_id_order ... ok
test vector_arm_finds_in_range_match_buried_beyond_pool ... ok

test result: ok. 63 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.58s

     Running unittests src\lib.rs (target\debug\deps\sysmon-af668d7fbcee06d0.exe)

running 11 tests
test cpu::tests::clamps_when_idle_exceeds_total ... ok
test cpu::tests::half_idle_is_fifty_pct ... ok
test cpu::tests::fully_idle_is_zero_pct ... ok
test cpu::tests::fully_busy_is_hundred_pct ... ok
test cpu::tests::user_time_counts_as_busy ... ok
test cpu::tests::zero_total_delta_returns_none ... ok
test gpu::tests::clamps_to_hundred ... ok
test gpu::tests::empty_is_zero ... ok
test gpu::tests::ignores_non_finite ... ok
test gpu::tests::sums_engines ... ok
test tests::sample_is_well_formed ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s

     Running unittests src\lib.rs (target\debug\deps\textfilter-f294f0d80943d220.exe)

running 12 tests
test tests::empty_spans_produce_empty_output ... ok
test tests::default_frame_drops_system_and_background_keeps_content ... ok
test tests::no_target_rect_never_classifies_background_or_system ... ok
test tests::window_title_echoed_as_body_is_excluded ... ok
test tests::reconcile_is_idempotent ... ok
test tests::reconcile_demotes_warm_catalog_chrome_preserving_content ... ok
test tests::reconcile_cleans_catalogued_chrome_even_without_target_rect ... ok
test tests::no_target_rect_never_suppresses_even_a_saturated_signature ... ok
test tests::reconcile_with_cold_catalog_changes_nothing ... ok
test tests::short_interior_body_is_never_catalogued ... ok
test tests::reconcile_demotes_only_the_catalogued_region ... ok
test tests::toolbar_becomes_chrome_at_the_seen_threshold ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\traits-cce057699f881ff6.exe)

running 78 tests
test domain::tests::capture_trigger_from_unknown_db_str_is_none ... ok
test domain::tests::capture_trigger_db_str_round_trips ... ok
test domain::export_bindings_visionanalysis ... ok
test domain::export_bindings_textsource ... ok
test domain::export_bindings_textrole ... ok
test ipc::export_bindings_activitycount ... ok
test domain::export_bindings_suppressreason ... ok
test domain::export_bindings_capturetrigger ... ok
test domain::export_bindings_monitorinfo ... ok
test ipc::export_bindings_answerdelta ... ok
test ipc::export_bindings_apistatus ... ok
test ipc::export_bindings_appcount ... ok
test ipc::export_bindings_appsuppression ... ok
test ipc::export_bindings_askrequest ... ok
test ipc::export_bindings_capturetick ... ok
test ipc::export_bindings_capturecontrol ... ok
test ipc::export_bindings_componentstatus ... ok
test ipc::export_bindings_exportrequest ... ok
test ipc::export_bindings_exportresult ... ok
test ipc::export_bindings_flashattnsetting ... ok
test ipc::export_bindings_framemeta ... ok
test ipc::export_bindings_hotkeystatus ... ok
test ipc::export_bindings_answerevent ... ok
test ipc::export_bindings_kvcachetype ... ok
test ipc::export_bindings_mark ... ok
test ipc::export_bindings_componentreadiness ... ok
test ipc::export_bindings_modeldownloadphase ... ok
test ipc::export_bindings_modellane ... ok
test ipc::export_bindings_modeltier ... ok
test ipc::export_bindings_openmoment ... ok
test ipc::export_bindings_pressuresample ... ok
test ipc::export_bindings_jobprogress ... ok
test ipc::export_bindings_reportkind ... ok
test ipc::export_bindings_reportprogress ... ok
test ipc::export_bindings_marktoast ... ok
test ipc::export_bindings_reportresponse ... ok
test ipc::export_bindings_resumecontext ... ok
test ipc::export_bindings_searchhit ... ok
test ipc::export_bindings_jobcompleted ... ok
test domain::export_bindings_textspan ... ok
test ipc::export_bindings_sessionrecaprequest ... ok
test ipc::export_bindings_modeldownloadstatus ... ok
test ipc::export_bindings_framedetail ... ok
test ipc::export_bindings_sidecarstate ... ok
test ipc::export_bindings_searchquery ... ok
test ipc::export_bindings_insightssummary ... ok
test ipc::export_bindings_readiness ... ok
test ipc::export_bindings_storagestats ... ok
test ipc::export_bindings_timelinebucket ... ok
test ipc::ts_number_guard::no_bigint_in_ipc_types ... ok
test ipc::export_bindings_reportrequest ... ok
test ipc::export_bindings_timerange ... ok
test ipc::export_bindings_toastlevel ... ok
test privacy::tests::allows_unrelated_apps ... ok
test privacy::tests::empty_excluded_entry_never_matches ... ok
test privacy::tests::matches_process_name_case_insensitively ... ok
test privacy::tests::matches_window_title ... ok
test ipc::export_bindings_updatestatus ... ok
test ipc::export_bindings_visiontarget ... ok
test ipc::export_bindings_throttlestatus ... ok
test ipc::export_bindings_sessionquery ... ok
test jobs::export_bindings_jobkind ... ok
test ipc::export_bindings_sessionreference ... ok
test ipc::export_bindings_setmodeltier ... ok
test jobs::export_bindings_jobstate ... ok
test jobs::export_bindings_jobstats ... ok
test ipc::export_bindings_toast ... ok
test ipc::export_bindings_sidecarstatus ... ok
test sessions::export_bindings_sessionartifactkind ... ok
test sessions::export_bindings_sessionartifactrole ... ok
test ipc::export_bindings_settings ... ok
test sessions::export_bindings_sessionhost ... ok
test sessions::export_bindings_sessionkind ... ok
test sessions::export_bindings_session ... ok
test ipc::export_bindings_uiresumecontext ... ok
test sessions::export_bindings_sessionartifact ... ok
test ipc::export_bindings_sessiondetail ... ok
test ipc::export_bindings_uiframedetail ... ok

test result: ok. 78 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s

     Running tests\sessions_contract.rs (target\debug\deps\sessions_contract-7afba26c650911a0.exe)

running 4 tests
test shipped_segmentation_params_pin_the_pr2_gate_values ... ok
test external_frame_and_resume_contracts_remain_session_free ... ok
test session_database_tokens_match_schema_eleven ... ok
test session_ui_contract_exports_without_bigint ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\uia-2c1113c675b32bee.exe)

running 30 tests
test breaker::tests::signal_bad_on_hard_timeout_and_over_budget ... ok
test classify::tests::high_frequency_interactive_triggers_never_run_uia ... ok
test classify::tests::chromium_window_classes_are_detected_others_left_alone ... ok
test breaker::tests::signal_neutral_on_busy_and_within_budget_err ... ok
test breaker::tests::breaker_cooldown_expiry_closes_and_resets ... ok
test breaker::tests::breaker_reports_transitions_once ... ok
test breaker::tests::breaker_isolates_apps ... ok
test breaker::tests::breaker_good_resets_the_streak ... ok
test breaker::tests::breaker_neutral_neither_counts_nor_resets ... ok
test breaker::tests::threshold_is_clamped_to_at_least_one ... ok
test classify::tests::containers_are_skipped_but_content_controls_emit ... ok
test classify::tests::input_gate_is_disabled_by_zero_window ... ok
test classify::tests::input_gate_only_touches_timer_triggers ... ok
test classify::tests::input_gate_skips_timer_walks_during_active_input ... ok
test breaker::tests::breaker_opens_after_three_consecutive_bad ... ok
test breaker::tests::signal_good_within_budget_ok ... ok
test classify::tests::low_frequency_triggers_run_uia ... ok
test classify::tests::never_emits_password_or_offscreen_or_container ... ok
test classify::tests::only_document_and_text_controls_want_textpattern ... ok
test geometry::tests::degenerate_inputs_are_zero ... ok
test classify::tests::split_words_groups_lines_and_skips_blanks ... ok
test geometry::tests::left_top_straddling_box_reports_only_on_frame_extent ... ok
test geometry::tests::overrunning_box_is_clamped_to_unit_square ... ok
test geometry::tests::primary_monitor_maps_proportionally ... ok
test input::tests::reports_an_idle_time ... ignored, requires a real desktop session
test tests::uia_provider_spawns_and_recognizes_foreground ... ignored, requires a real desktop (UI Automation); run locally
test geometry::tests::secondary_monitor_subtracts_its_origin ... ok
test tests::uia_worker_exits_on_shutdown ... ignored, requires a real desktop (UI Automation); run locally
test window::tests::live_hwnd_classification ... ignored, requires a real desktop; pass UIA_PROBE_HWND=<i64>
test worker::tests::within_target_filters_by_center ... ok

test result: ok. 26 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests api

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests capture

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests doctor

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests embeddings

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests harness

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests inference

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kernel

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests mcp

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests ocr

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests screensearch_lib

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests sessions

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests store

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests sysmon

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests textfilter

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests traits

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests uia

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

#### `git diff --exit-code -- ui/src/bindings` (exit 0)

```text

```

## Pass 9 — 2026-07-10 — 0.4.0 PR5 native Tauri/WebView2 acceptance

- **Runtime provenance:** Launched only through `npm run dev` against the live schema-11 database.
  The actual process was this worktree's `target/debug/screensearch.exe`; the inspected WebView had
  `window.__TAURI_INTERNALS__ === true`, proving native Tauri rather than a browser mock. Live status:
  515 captures / 514 tagged. Screenshot evidence remains outside the repo at
  `%TEMP%\screensearch-pr5-timeline.png`.
- **Additive surfaces and navigation:** Deck showed `JUMP BACK ... until 17:08 session 16:44–17:14`.
  Timeline rendered eight real overlapping sessions in two lanes. CDP native Enter
  (`rawKeyDown`/`char`/`keyUp`) on the first focused band opened `/timeline/session/3`. The exact
  round trip passed: AI band → session 3 → representative `/timeline/2651`; Moment showed `SESSION`
  and `PART OF SESSION ScreenSearch Workflow`; SESSION returned to `/timeline/session/3`.
- **Session truth:** Session 3 showed AI, codex/desktop, Confidence 78.5%, Jul 10 01:21–02:50, 41
  frames / 24 representatives, and honest `No exchanges captured for this session.` Session 21
  supplied the neutral low-confidence case at 47.2% with 50 frames and no invented exchanges.
- **Recap:** Cancel was observed at `Summarizing 1 of 4 · 1/4`; CANCEL returned to GENERATE RECAP and
  no stale result appeared. A clean run completed in 15 seconds: 5 passes, 1/1 periods, 39/39 frames
  summarized, truthful trimmed footer. All 39 cited frame ids were individually resolved with the
  live `get_frame` command; every returned frame had `session.id === 3`.
- **Layout/accessibility:** At 1280×720 DPR 1, 1920×1080 DPR 1, and emulated 3440×1440 DPR 1.5,
  document width equalled viewport width; every band was at least 32×32; band overlap, nested vertical
  scroll contexts, and horizontal overflow were all zero. Session 3 and session 21 each had one
  vertical scroller and no horizontal overflow. Band labels exposed kind, full date/time, tool/host.
  Under CDP `prefers-reduced-motion: reduce`, the query matched and animated elements were `[]`.
- **Settings/states:** Sessions settings were 120 (min 30 / max 3600) and 300 (min 60 / max 3600),
  with copy explaining the next session pass. Live `list_sessions` returned 21 sessions and open ids
  `[]`, so the open-session variant was **unavailable in this dataset**—neither pass nor failure, and
  no live observation is claimed. Low-confidence and no-exchange variants were observed as above.
- **Noise/contract audit:** Only the existing `favicon.ico` 404 and informational WebView lazy-image
  intervention appeared; no app runtime errors. PR5 native acceptance **passes except for the
  transparently unavailable open-session variant**. No new product ambiguity, contradiction, known
  gap, schema/migration change, code change, or generated-binding change was introduced.

## Pass 10 — 2026-07-11 — 0.4.0 PR5 final-review stability fixes (`02e5cad`)

- **Finding — dense-band CLS ambiguity:** The reviewed normal-flow lane expansion was correct for
  visibility but not for D9: five simultaneous sessions could change Timeline route height after
  data arrived. The new focused layout test went RED because zero fixed rows were reserved where the
  contract needed four:

  ```text
  npm test
  0 !== 4
  0 pass / 1 fail
  ```

- **User decision (option 1):** Keep D9 strict. Timeline reserves **exactly four session lanes** in
  initial route skeleton/loading/error/empty/populated. Sessions whose measured collision packing
  needs lane 5+ are aggregated into a neutral keyboard-operable overflow control; activating it
  focuses the existing range presets. There is no fifth lane, modal, new range control, or CLS.
- **Layout GREEN/controller evidence:** `npm run test` reported 1 pass / 0 fail; the controller rerun
  duration was 65.3531 ms. `npm run typecheck` and `npm run lint` were clean. Production build:

  ```text
  ✓ 438 modules transformed.
  ✓ built in 1.59s
  ```

- **Finding — mounted-query freshness:** A scheduler pass could commit new session rows/ownership
  while mounted Timeline/detail queries remained cached. The focused Rust test first went RED with
  E0425 (missing `run_scheduler_pass`) and E0599 (missing `KernelEvent::SessionsChanged`). The final
  protocol emits typed-null `sessions_changed` only after successful awaited scheduler/store work;
  mounted session queries refetch. It is pull-based invalidation, not a notification/toast.
- **Refresh GREEN/controller evidence:** focused controller output:

  ```text
  running 1 test
  successful_scheduler_pass_emits_sessions_changed_after_rows_commit ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 57 filtered out
  ```

  Focused `cargo clippy -p kernel -p screensearch --all-targets -- -D warnings` finished clean.
- **Final scope audit:** Code commit `02e5cad` is `fix(sessions): stabilize live session bands`. No
  schema migration/version change, API/MCP, audio, notification, nudge, score, new NavRail route, or
  frame-level behavior change. Pass 8's raw integrated suite and Pass 9's native evidence above are
  unchanged; this pass records the final focused RED/GREEN review and explicit user resolution.

## Pass 11 — 2026-07-11 — 0.4.0 PR5 final review-fix native acceptance

- **Runtime provenance:** Actual `npm run dev` Tauri process was the worktree's
  `target/debug/screensearch.exe`; `window.__TAURI_INTERNALS__` was `true`. Code under test was
  `02e5cad`; contract docs were `67b76ce`.
- **Live refresh:** Registered the real typed `sessions_changed` listener through
  `/src/lib/ipc/events.ts`. The startup scheduler pass produced the observed probe `{count:1}`. No
  toast or notification appeared; the signal remained pull-based query invalidation.
- **Initial loading:** Forced the real Timeline initial-loading state through the existing dev-state
  seam. Computed lane grid was `32px 32px 32px 32px`, grid height 140, session outer height 192, with
  five skeleton elements.
- **Empty/populated geometry:** Live empty Today retained grid/outer 140/192 with no horizontal
  overflow. Live populated 7-day retained 140/192 with 21 visible bands, zero overlaps, and document
  width 1280 equal to viewport width 1280.
- **Dense overflow:** Live populated 30-day retained grid/outer 140/192 with 12 visible bands,
  `9 more sessions — narrow the range`, zero overlaps, and document width 1280 equal to viewport
  width 1280. The neutral overflow button was keyboard-focusable.
- **Keyboard:** CDP native `rawKeyDown` / `char` / `keyUp` Enter on the overflow button moved focus to
  `TODAY`; its parent carried `aria-label="Time range"`. The control therefore routes keyboard users
  to the existing range presets without adding a fifth lane or new range surface.
- **Acceptance/scope:** Fixed four-lane geometry is identical across actual loading, empty, populated,
  and dense-overflow states; live scheduler refresh and neutral overflow focus both pass. No schema,
  API/MCP, or frame behavior changed. Passes 8–10 above remain unchanged.


## Pass 12 — 2026-07-11 — 0.4.0 PR5 final clean integrated verification

- **Scope:** Final color-disabled UI-first suite at tip `8629e0c`, including the focused session-band layout regression gate. All commands exited 0. The npm allow-scripts warning is non-failing. Raw output follows verbatim; empty commands have empty fenced blocks.

### `cd ui && npm ci` (exit 0)

```text

added 348 packages, and audited 349 packages in 4s

151 packages are looking for funding
  run `npm fund` for details

found 0 vulnerabilities
npm warn allow-scripts 1 package has install scripts not yet covered by allowScripts:
npm warn allow-scripts   esbuild@0.25.12 (postinstall: node install.js)
npm warn allow-scripts
npm warn allow-scripts Run `npm approve-scripts --allow-scripts-pending` to review, or `npm approve-scripts <pkg>` to allow.
```

### `npm run test` (exit 0)

```text

> screensearch-ui@0.3.3 test
> node --experimental-strip-types --test test/sessionBandLayout.test.mjs

✔ five simultaneous sessions use four fixed rows and aggregate the fifth (0.8881ms)
ℹ tests 1
ℹ suites 0
ℹ pass 1
ℹ fail 0
ℹ cancelled 0
ℹ skipped 0
ℹ todo 0
ℹ duration_ms 67.4623
```

### `npm run lint` (exit 0)

```text

> screensearch-ui@0.3.3 lint
> eslint .
```

### `npm run build` (exit 0)

```text

> screensearch-ui@0.3.3 build
> tsc --noEmit && vite build

vite v6.4.3 building for production...
transforming...
✓ 438 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                   0.80 kB │ gzip:  0.36 kB
dist/overlay.html                                 0.99 kB │ gzip:  0.42 kB
dist/assets/globals-80FYB378.css                 32.04 kB │ gzip:  6.80 kB
dist/assets/timeRanges-BJgzkTNX.js                0.29 kB │ gzip:  0.19 kB
dist/assets/openExternal-CrF7EMqa.js              0.31 kB │ gzip:  0.23 kB
dist/assets/useAdaptiveBucketCount-pnT9Dvsn.js    0.35 kB │ gzip:  0.26 kB
dist/assets/NotFound-Dj8WojQC.js                  0.44 kB │ gzip:  0.32 kB
dist/assets/EmptyState-CIH9-Lyf.js                0.52 kB │ gzip:  0.31 kB
dist/assets/Panel-BACJHu4M.js                     0.68 kB │ gzip:  0.44 kB
dist/assets/timelineDraw-B37WQvuk.js              0.75 kB │ gzip:  0.44 kB
dist/assets/FrameTile-DDAv1yCm.js                 0.97 kB │ gzip:  0.55 kB
dist/assets/time-cX9c3v95.js                      1.01 kB │ gzip:  0.46 kB
dist/assets/FrameImage-Ch_VvMBg.js                1.62 kB │ gzip:  0.85 kB
dist/assets/HighlightedSnippet-CPtxR-fM.js        1.65 kB │ gzip:  0.83 kB
dist/assets/AnswerStream-CdQ9QPy3.js              2.35 kB │ gzip:  1.20 kB
dist/assets/HotkeyField-tDLhRbul.js               2.54 kB │ gzip:  1.32 kB
dist/assets/ReportView-Ivlez-m4.js                2.92 kB │ gzip:  1.45 kB
dist/assets/path-r4_HdewI.js                      3.83 kB │ gzip:  0.96 kB
dist/assets/Insights-Cu1sIsAi.js                  5.46 kB │ gzip:  2.11 kB
dist/assets/Session-CYblZx9Y.js                   7.26 kB │ gzip:  2.53 kB
dist/assets/Moment-B0-Xf5Ro.js                    8.01 kB │ gzip:  2.98 kB
dist/assets/Deck-DUS0BbEM.js                     10.87 kB │ gzip:  3.67 kB
dist/assets/overlay-CSt7Tmyw.js                  10.95 kB │ gzip:  3.87 kB
dist/assets/Timeline-CC6gdvGO.js                 11.04 kB │ gzip:  4.51 kB
dist/assets/globals-CWyGAXYX.js                  16.74 kB │ gzip:  6.03 kB
dist/assets/main-DpLuKAIj.js                     22.86 kB │ gzip:  7.40 kB
dist/assets/query-u_0r_xiX.js                    35.77 kB │ gzip: 10.59 kB
dist/assets/Recall-D8TsYrRq.js                   37.86 kB │ gzip: 12.25 kB
dist/assets/Settings-BCjKt1B7.js                 47.53 kB │ gzip: 13.45 kB
dist/assets/router-CL_afuJ-.js                   64.86 kB │ gzip: 22.21 kB
dist/assets/react-vendor-DTiTYlFD.js            143.42 kB │ gzip: 46.01 kB
dist/assets/CitationTile-CarmCXg3.js            157.84 kB │ gzip: 47.99 kB
✓ built in 1.69s
```

### `node scripts/stage-mcp.mjs` (exit 0)

```text
[stage-mcp] building screensearch-mcp (release)...
    Finished `release` profile [optimized] target(s) in 0.21s
[stage-mcp] up to date: C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\src-tauri\binaries\screensearch-mcp-x86_64-pc-windows-msvc.exe
```

### `cargo fmt --all -- --check` (exit 0)

```text

```

### `cargo clippy --workspace --all-targets -- -D warnings` (exit 0)

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.57s
```

### `cargo build --workspace` (exit 0)

```text
   Compiling inference v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\inference)
   Compiling mcp v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\mcp)
   Compiling screensearch v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.06s
```

### `cargo test --workspace` (exit 0)

```text
   Compiling mcp v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\mcp)
   Compiling inference v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\crates\inference)
   Compiling screensearch v0.3.3 (C:\Users\nicol\Documents\GitHub\screensearch-v2c\.worktrees\feat-0.4.0-pr5-sessions-ui\src-tauri)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.61s
     Running unittests src\lib.rs (target\debug\deps\api-582ea0631a5e71d8.exe)

running 2 tests
test auth::tests::constant_time_eq_matches_only_identical_slices ... ok
test export::tests::utc_stamp_matches_known_instants ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\http_api.rs (target\debug\deps\http_api-c0e77c0557ba1ea0.exe)

running 15 tests
test live_server_for_curl ... ignored
test binds_loopback_only ... ok
test unknown_route_is_json_404 ... ok
test ask_without_answer_model_is_503 ... ok
test where_was_i_returns_null_when_nothing_qualifies ... ok
test export_window_excludes_out_of_range_frames ... ok
test ask_streams_sse_deltas ... ok
test inverted_time_range_is_400 ... ok
test export_over_http_is_valid_json ... ok
test search_returns_hits_from_fixture ... ok
test token_swap_takes_effect_without_restart ... ok
test health_requires_token_and_reports_state ... ok
test marks_crud_roundtrip ... ok
test frame_detail_image_and_not_found ... ok
test export_to_file_writes_valid_json_without_a_server ... ok

test result: ok. 14 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running unittests src\lib.rs (target\debug\deps\capture-9abedaa63a7f798a.exe)

running 26 tests
test events::tests::source_starts_and_stops_cleanly_repeatedly ... ignored, requires a real desktop (USER32 message pump); run locally
test diff::tests::content_hash_is_stable_and_distinct ... ok
test privacy::tests::own_window_pid_matches_any_nonzero_own_process_window ... ok
test tests::degenerate_inputs_are_none ... ok
test tests::target_monitor_is_the_one_holding_the_foreground_window ... ok
test tests::window_rect_normalizes_within_its_monitor ... ok
test privacy::tests::own_window_pid_rejects_foreign_process ... ok
test privacy::tests::own_window_pid_rejects_unknown_foreground_pid ... ok
test tests::target_monitor_falls_back_to_primary_then_first ... ok
test trigger::tests::burst_of_events_collapses_to_one_capture ... ok
test tests::window_on_another_monitor_is_none ... ok
test trigger::tests::disabled_foreground_never_emits ... ok
test tests::window_offset_maps_relative_to_monitor_origin ... ok
test diff::tests::gate_passes_bypass_forces_unchanged_frame_through ... ok
test trigger::tests::idle_disabled_never_emits_from_polling ... ok
test trigger::tests::foreground_event_emits_after_debounce ... ok
test trigger::tests::idle_fires_once_per_quiet_period ... ok
test trigger::tests::idle_poll_while_active_is_quiet ... ok
test trigger::tests::idle_retries_after_min_interval_block ... ok
test trigger::tests::min_interval_suppresses_a_second_capture ... ok
test trigger::tests::pending_event_retries_after_min_interval_block ... ok
test diff::tests::black_vs_white_is_near_full_difference ... ok
test diff::tests::tiny_change_stays_below_default_threshold ... ok
test diff::tests::identical_frames_have_zero_difference ... ok
test diff::tests::resolution_change_is_full_difference ... ok
test diff::tests::gate_passes_first_frame_and_real_change ... ok

test result: ok. 25 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\wgc_smoke.rs (target\debug\deps\wgc_smoke-930b91d667839d47.exe)

running 1 test
test wgc_captures_a_frame_from_the_primary_monitor ... ignored, requires a real desktop + GPU (WGC); run locally

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\doctor-51f5f66998236517.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\doctor-8746a4d1eb703f86.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\embeddings-2b03e9f3cde8be59.exe)

running 2 tests
test tests::loads_and_embeds_text ... ignored, downloads the EmbeddingGemma model; run locally with --ignored
test tests::embed_dim_is_768 ... ok

test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\harness-e42846f165808eea.exe)

running 119 tests
test digest::tests::collapses_consecutive_same_app_runs ... ok
test digest::tests::leading_keyless_counted_separately ... ok
test digest::tests::labels_template_lists_marks_as_comments ... ok
test digest::tests::keyless_frames_are_absorbed_not_split ... ok
test digest::tests::digest_renders_table_marks_and_top_titles ... ok
test export::tests::git_root_walks_above_the_crate_dir ... ok
test group::tests::anchorless_focus_below_floor_is_dropped ... ok
test group::tests::anchored_ai_is_exempt_from_the_density_gate ... ok
test group::tests::back_to_back_sustained_tools_split_at_the_handoff ... ok
test group::tests::anchorless_focus_above_floor_is_kept ... ok
test group::tests::ai_track_spans_through_a_meeting_band ... ok
test group::tests::enforce_non_overlap_then_resort_restores_global_order ... ok
test group::tests::band_interior_unrecognized_is_owned_by_the_meeting ... ok
test group::tests::empty_and_keyless_produce_no_sessions ... ok
test group::tests::focus_ramp_converts_to_ai_keeping_start ... ok
test group::tests::gap_at_merge_gap_splits ... ok
test group::tests::density_gate_suppresses_sparse_focus_but_keeps_dense ... ok
test group::tests::concurrent_walk_is_deterministic ... ok
test group::tests::host_precedence_picks_terminal_over_desktop ... ok
test group::tests::intra_session_lull_below_merge_gap_holds ... ok
test group::tests::low_density_background_ai_track_survives ... ok
test group::tests::leading_none_ramp_attaches_to_an_opening_track ... ok
test group::tests::meeting_band_is_a_hard_session_at_presence_endpoints ... ok
test group::tests::meeting_band_splits_the_surrounding_work ... ok
test group::tests::mixed_day_output_is_sorted_and_non_overlapping ... ok
test group::tests::none_run_over_budget_becomes_focus_overlapping_the_track ... ok
test group::tests::none_sandwich_within_budget_absorbs_into_the_track ... ok
test group::tests::output_globally_sorted_with_cross_track_overlap_present ... ok
test group::tests::overlapping_meetings_emit_overlapping_sessions ... ok
test group::tests::ramp_does_not_fire_when_a_track_is_open ... ok
test group::tests::same_tool_two_instances_fold_into_one_track ... ok
test group::tests::per_track_gap_close_is_independent ... ok
test group::tests::same_track_never_overlaps_itself ... ok
test group::tests::short_foreign_ai_run_is_absorbed ... ok
test group::tests::short_meeting_chain_is_demoted_not_a_band ... ok
test group::tests::short_foreign_run_dropped_by_qualification ... ok
test group::tests::scattered_sub_qualify_presence_emits_no_ai_session ... ok
test group::tests::single_ai_run_is_one_anchored_session ... ok
test data::tests::rejects_labels_whose_date_mismatches_the_day ... ok
test group::tests::sub_qualify_ai_run_does_not_flip_a_focus_session ... ok
test group::tests::sustained_foreign_ai_runs_split ... ok
test group::tests::sustained_foreign_run_no_longer_splits_a_track ... ok
test group::tests::trailing_none_extends_the_last_touched_track ... ok
test group::tests::two_tools_interleaved_form_two_overlapping_sessions ... ok
test labels::tests::end_at_2400_is_local_midnight_next_day ... ok
test labels::tests::accepts_cross_identity_overlap ... ok
test labels::tests::accepts_touching_sessions ... ok
test group::tests::unrecognized_excursion_above_budget_splits_off_focus ... ok
test group::tests::unrecognized_excursion_below_budget_is_absorbed ... ok
test labels::tests::parses_and_resolves_template ... ok
test labels::tests::rejects_ai_without_tool ... ok
test labels::tests::rejects_bad_enum ... ok
test labels::tests::rejects_end_at_or_before_start ... ok
test labels::tests::rejects_malformed_time ... ok
test labels::tests::rejects_out_of_start_order ... ok
test labels::tests::rejects_start_at_2400 ... ok
test labels::tests::rejects_same_tool_and_focus_overlap ... ok
test labels::tests::rejects_tool_when_not_ai ... ok
test labels::tests::rejects_true_overlap ... ok
test score::tests::edge_boundaries_are_excluded_both_sides ... ok
test labels::tests::serial_label_files_still_validate ... ok
test score::tests::missed_and_spurious_boundaries_lower_pr ... ok
test score::tests::old_boundary_comparison_is_symmetric ... ok
test score::tests::optimal_match_beats_greedy ... ok
test score::tests::optimal_match_respects_tolerance_and_one_to_one ... ok
test score::tests::partitioned_match_never_exceeds_pooled ... ok
test data::tests::round_trips_an_exported_day ... ok
test score::tests::partitioned_perfect_concurrent_day_scores_one ... ok
test score::tests::perfect_day_scores_one ... ok
test score::tests::pooling_sums_then_recomputes ... ok
test score::tests::stability_counts_identity_swaps_as_drift ... ok
test score::tests::tool_accuracy_ignores_larger_overlapping_non_ai_span ... ok
test score::tests::tool_accuracy_max_overlap_and_no_overlap_is_wrong ... ok
test score::tests::typed_matching_does_not_cross_start_and_end ... ok
test score::tests::sweep_1d_varies_one_knob ... ok
test segmenter::tests::brief_excursion_is_absorbed_into_one_span ... ok
test segmenter::tests::fragmented_interrupter_reaching_dwell_splits ... ok
test segmenter::tests::browser_ai_vs_plain_browser_are_distinct ... ok
test score::tests::sweep_grid_scores_every_cell_through_the_grouped_pipeline ... ok
test score::tests::stability_small_lookback_unstable_large_lookback_stable ... ok
test segmenter::tests::keyless_stretch_over_gap_close_splits ... ok
test segmenter::tests::empty_and_keyless_produce_no_spans ... ok
test segmenter::tests::multimonitor_equal_timestamps_are_deterministic ... ok
test segmenter::tests::same_context_within_gap_close_stays_one_span ... ok
test segmenter::tests::gap_close_splits_same_context_after_idle ... ok
test segmenter::tests::meeting_recognition_sets_kind_without_tool ... ok
test segmenter::tests::single_sustained_run_is_one_span ... ok
test segmenter::tests::sub_min_len_run_is_dropped ... ok
test segmenter::tests::tool_identity_splits_same_app_into_adjacent_sessions ... ok
test segmenter::tests::segment_micro_keeps_sub_floor_runs_that_segment_drops ... ok
test taxonomy::tests::app_hint_matches_by_exact_stem_not_substring ... ok
test taxonomy::tests::braille_prefix_matches_claude_code ... ok
test segmenter::tests::sustained_interruption_splits_into_three_spans ... ok
test taxonomy::tests::case_insensitive_and_none_on_empty_context ... ok
test taxonomy::tests::browser_ai_needs_browser_app_and_ai_title ... ok
test taxonomy::tests::claude_code_needs_terminal_app_and_claude_title ... ok
test taxonomy::tests::invalid_prefix_range_rejected_at_parse ... ok
test taxonomy::tests::claude_desktop_vs_claude_code_precedence ... ok
test taxonomy::tests::codex_stem_does_not_collide_with_a_code_ide_hint ... ok
test taxonomy::tests::codex_desktop_app ... ok
test taxonomy::tests::leading_whitespace_before_spinner_tolerated ... ok
test taxonomy::tests::rejects_entry_with_empty_matcher ... ok
test taxonomy::tests::meeting_recognition ... ok
test taxonomy::tests::plain_shell_title_does_not_match_prefix ... ok
test taxonomy::tests::plain_terminal_without_ai_title_is_unrecognized ... ok
test taxonomy::tests::seed_parses_and_has_the_d7_set ... ok
test taxonomy::tests::renamed_chatgpt_stem_maps_to_codex_except_classic ... ok
test taxonomy::tests::sparkle_prefix_matches_claude_code ... ok
test taxonomy::tests::substring_fallback_still_recognizes_claude_code ... ok
test taxonomy::tests::spinner_prefix_confined_to_terminal_stems ... ok
test export::tests::read_only_connection_rejects_writes ... ok
test export::tests::survey_counts_signals ... ok
test export::tests::backup_refuses_destination_in_repo_tree ... ok
test export::tests::day_marks_read ... ok
test export::tests::day_bounds_are_24h_and_offset_is_consistent ... ok
test export::tests::day_frames_ordered_with_multimonitor_and_keyless ... ok
test export::tests::re_export_preserves_hand_edited_labels ... ok
test export::tests::write_day_emits_all_files ... ok
test export::tests::backup_snapshots_and_verifies ... ok

test result: ok. 119 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running unittests src\main.rs (target\debug\deps\harness-52d5ebe66e6a73f0.exe)

running 1 test
test tests::overlaps_any_excludes_self_copy_by_value ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\shipped_parity.rs (target\debug\deps\shipped_parity-d4f55a6b5929ecca.exe)

running 3 tests
test shipped_open_projection_keeps_the_parity_span_fields ... ok
test shipped_matches_frozen_harness_concurrent_on_synthetic_day ... ok
test shipped_parity_covers_none_meetings_gap_edges_density_and_qualification ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\inference-6c26c062e8096d06.exe)

running 105 tests
test answer::tests::estimate_tokens_does_not_undercount_cjk ... ok
test answer::tests::plain_content_is_all_tokens ... ok
test answer::tests::handles_tag_split_across_chunks ... ok
test answer::tests::builds_grounded_prompt_with_frame_tags ... ok
test answer::tests::drops_chunks_that_exceed_the_context_budget ... ok
test answer::tests::reply_budget_leaves_room_for_grounding_in_a_small_context ... ok
test answer::tests::report_model_label_extracts_the_gguf_filename ... ok
test answer::tests::report_summary_drops_overflow_and_cites_only_what_fit ... ok
test answer::tests::splits_inline_think_tags ... ok
test answer::tests::report_summary_messages_use_the_given_system_prompt_and_tag_frames ... ok
test client::tests::http_error_handles_empty_body ... ok
test client::tests::http_error_includes_status_and_body ... ok
test answer::tests::truncate_to_tokens_never_splits_a_multibyte_char ... ok
test answer::tests::truncates_an_oversized_top_chunk_instead_of_dropping_everything ... ok
test answer::tests::pump_deltas_completes_when_stream_finishes ... ok
test client::tests::response_format_is_omitted_from_body_when_none ... ok
test client::tests::http_error_truncates_long_body_on_char_boundary ... ok
test client::tests::response_format_is_serialized_when_set ... ok
test download::tests::byte_counter_accumulates_streamed_bytes ... ok
test download::tests::answer_repo_needs_no_mmproj ... ok
test download::tests::content_range_total_parses_suffix ... ok
test download::tests::falls_back_to_first_gguf_when_no_q4_k_m ... ok
test download::tests::lfs_sha256_trusts_only_x_linked_etag_not_cdn_etag ... ok
test download::tests::no_vulkan_asset_returns_none ... ok
test download::tests::no_vulkan_in_any_release_returns_none ... ok
test download::tests::parse_sha256_normalizes_etag_forms ... ok
test download::tests::picks_q4_k_m_weights_and_mmproj_for_vision ... ok
test download::tests::picks_win_vulkan_x64_asset ... ok
test download::tests::place_if_cached_returns_false_when_nothing_cached ... ok
test download::tests::failed_binary_extraction_cleans_partial_install ... ok
test download::tests::prefers_the_newest_release_that_has_vulkan ... ok
test download::tests::range_plan_requires_ranges_and_known_size ... ok
test download::tests::manifest_load_or_init_distinguishes_missing_valid_and_mismatched ... ok
test download::tests::skips_release_with_incomplete_assets ... ok
test download::tests::place_if_cached_short_circuits_when_dest_already_present ... ok
test download::tests::stall_limit_is_timeout_over_poll_and_never_zero ... ok
test download::tests::stall_step_resets_on_progress_and_counts_otherwise ... ok
test flags::tests::conservative_fallback_only_pins_context ... ok
test flags::tests::detects_missing_flash_attn ... ok
test flags::tests::parses_legacy_boolean_flash_attn ... ok
test flags::tests::parses_modern_value_taking_flash_attn ... ok
test flags::tests::parses_parenthesised_value_taking_flash_attn ... ok
test download::tests::chunked_download_errors_when_server_ignores_range ... ok
test download::tests::installed_binary_candidates_include_normal_and_overrides ... ok
test models::tests::answer_resolution_needs_no_mmproj ... ok
test models::tests::repo_mapping_matches_registry ... ok
test download::tests::env_override_wins_over_existing_install ... ok
test models::tests::auto_ctx_size_resolves_per_lane_and_override_passes_through ... ok
test models::tests::prefers_q4_k_m_and_excludes_mmproj ... ok
test process::tests::escapes_embedded_quotes_and_trailing_backslashes ... ok
test process::tests::query_private_bytes_is_none_for_pid_zero ... ok
test process::tests::query_private_bytes_reports_for_self ... ok
test process::tests::quotes_only_when_needed ... ok
test process::tests::quotes_paths_with_spaces ... ok
test process::tests::total_physical_ram_is_nonzero ... ok
test supervisor::tests::auto_ceiling_is_half_ram_within_band ... ok
test supervisor::tests::build_args_adds_device_when_configured ... ok
test supervisor::tests::build_args_adds_mmproj_only_for_vision ... ok
test supervisor::tests::build_args_distinguishes_auto_from_on_flash_attn ... ok
test supervisor::tests::build_args_drops_explicit_on_flash_when_binary_unsupported ... ok
test supervisor::tests::build_args_emits_full_tuning_when_supported ... ok
test supervisor::tests::build_args_leaves_f16_kv_implicit ... ok
test answer::tests::pump_deltas_cancels_and_aborts_sidecar_on_consumer_drop ... ok
test supervisor::tests::build_args_omits_ctx_when_unsupported ... ok
test supervisor::tests::build_args_omits_kv_quant_without_flash ... ok
test supervisor::tests::build_args_uses_bare_flash_attn_for_bool_flag ... ok
test supervisor::tests::evict_predicate_respects_inflight_backfill_and_ttl ... ok
test supervisor::tests::idle_predicate ... ok
test supervisor::tests::resolve_ceiling_branches ... ok
test supervisor::tests::restart_on_tuning_change ... ok
test supervisor::tests::restart_only_on_model_change ... ok
test supervisor::tests::running_sidecar_is_reused_only_when_process_and_health_are_alive ... ok
test supervisor::tests::should_recycle_disabled_when_ceiling_zero ... ok
test supervisor::tests::should_recycle_only_at_or_above_ceiling ... ok
test models::tests::resolution_carries_device_selector ... ok
test vision::tests::activity_type_is_normalised_case_and_space_insensitively ... ok
test models::tests::vision_resolution_requires_mmproj ... ok
test vision::tests::app_hint_is_trimmed_and_kept_when_real ... ok
test vision::tests::explicit_null_activity_type_becomes_none ... ok
test vision::tests::falls_back_to_raw_text_on_non_json ... ok
test vision::tests::missing_confidence_becomes_unknown_sentinel ... ok
test vision::tests::null_app_hint_string_is_dropped_case_insensitively ... ok
test vision::tests::off_enum_activity_type_becomes_none ... ok
test vision::tests::out_of_range_confidence_becomes_unknown_sentinel ... ok
test vision::tests::parses_well_formed_json ... ok
test vision::tests::response_format_allows_null_activity_and_drops_it_from_required ... ok
test download::tests::fresh_part_discards_stale_all_done_manifest ... ok
test vision::tests::tolerates_code_fences_and_prose ... ok
test vision::tests::vlm_request_dims_match_encoded_output ... ok
test vision::tests::zero_confidence_becomes_unknown_sentinel ... ok
test download::tests::fresh_part_discards_stale_partial_manifest ... ok
test download::tests::oversized_part_discards_stale_manifest ... ok
test download::tests::chunk_requests_follow_redirect_and_preserve_range ... ok
test download::tests::truncated_part_discards_stale_partial_manifest ... ok
test download::tests::resume_skips_already_completed_chunks ... ok
test download::tests::chunked_download_assembles_byte_identical_file ... ok
test download::tests::integrity_accepts_matching_sha256_and_rejects_a_wrong_one ... ok
test supervisor::tests::request_gate_allows_concurrent_regular_requests ... ok
test process::tests::spawn_suspended_captures_child_stdout_to_log ... ok
test supervisor::tests::switch_gate_waits_for_active_request_to_drop ... ok
test download::tests::chunked_download_fails_fast_on_stuck_chunk ... ok
test vision::tests::small_frame_passes_through_at_native_size ... ok
test download::tests::chunk_retries_transient_403_then_succeeds ... ok
test vision::tests::downscales_oversized_frame_to_max_edge ... ok
test download::tests::exhausted_transient_is_not_reported_as_ignored_range ... ok

test result: ok. 105 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.04s

     Running unittests src\bin\jobhelper.rs (target\debug\deps\jobhelper-cd7052ccf94ce638.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\no_orphan.rs (target\debug\deps\no_orphan-b040a5695761adba.exe)

running 1 test
test killing_parent_terminates_job_bound_child ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests\reap.rs (target\debug\deps\reap-b98a2c932c290591.exe)

running 3 tests
test reaps_a_matching_stray ... ok
test never_reaps_a_foreign_pid ... ok
test reaps_a_matching_stray_from_any_owned_install_path ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests\sidecar_client.rs (target\debug\deps\sidecar_client-d68a2248fbcd4151.exe)

running 8 tests
test health_reports_success ... ok
test vision_completion_returns_message_content ... ok
test health_false_when_unavailable ... ok
test answer_stream_yields_ordered_pieces ... ok
test stream_connect_times_out_when_initial_post_hangs ... ok
test health_times_out_quickly_when_sidecar_hangs ... ok
test completion_times_out_when_sidecar_hangs ... ok
test stream_times_out_when_no_sse_chunk_arrives ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running tests\smoke.rs (target\debug\deps\smoke-c453c7d39f21397c.exe)

running 2 tests
test real_answer_streams_tokens ... ignored, downloads a multi-GB model and runs a real llama-server on a GPU
test real_vision_tags_an_image ... ignored, downloads a multi-GB model + projector and runs a real llama-server on a GPU

test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\kernel-10936c16b9f0a783.exe)

running 58 tests
test capture_loop::tests::image_paths_are_webp_and_day_sharded ... ok
test reports::tests::grid_size_is_one_per_day_capped_at_max ... ok
test reports::tests::plan_depth_floor_wins_over_global_cap_on_long_ranges ... ok
test reports::tests::plan_depth_gives_every_active_period_its_budget ... ok
test reports::tests::fits_single_pass_detects_overflow ... ok
test reports::tests::split_chunks_batches_every_chunk_without_dropping ... ok
test resume::tests::all_keyless_frames_is_none ... ok
test resume::tests::browser_domain_parsing ... ok
test resume::tests::distinct_browser_domains_are_distinct_contexts ... ok
test resume::tests::brief_excursion_is_absorbed_into_the_run ... ok
test resume::tests::dwell_exactly_at_threshold_qualifies ... ok
test resume::tests::equal_timestamps_are_deterministic ... ok
test capture_loop::tests::max_width_at_or_above_native_is_noop ... ok
test resume::tests::excluded_app_is_skipped_for_next_candidate ... ok
test resume::tests::fragmented_interrupter_reaching_dwell_splits ... ok
test resume::tests::no_frames_is_none ... ok
test resume::tests::returns_prior_sustained_run_with_span_and_last_frame ... ok
test resume::tests::single_context_only_is_none ... ok
test resume::tests::screensearch_context_never_qualifies ... ok
test resume::tests::single_frame_run_fails_dwell ... ok
test resume::tests::sustained_interruption_splits_the_run ... ok
test sessions_intel::tests::even_sampling_includes_both_session_endpoints ... ok
test sessions_scheduler_contract_tests::frozen_guard_treats_equal_last_frame_timestamp_as_overlap ... ok
test sessions_scheduler_contract_tests::frozen_guard_trims_only_the_same_identity_track ... ok
test sessions_scheduler_contract_tests::historical_cut_extends_to_the_next_global_merge_gap ... ok
test sessions_scheduler_contract_tests::historical_cut_never_skips_unscanned_future_frames ... ok
test sessions_scheduler_contract_tests::new_session_projection_keeps_open_rows_null_ended ... ok
test sessions_scheduler_contract_tests::overlap_matching_does_not_reuse_a_merely_touching_session ... ok
test sessions_scheduler_contract_tests::overlap_matching_reuses_one_unfrozen_id_per_draft ... ok
test settings::tests::marks_hotkey_empty_falls_back_to_default ... ok
test settings::tests::recycle_rss_mb_clamps_explicit_but_keeps_auto_zero ... ok
test settings::tests::resume_min_dwell_secs_clamps_to_band ... ok
test throttle::tests::enters_high_only_after_sustained_dwell ... ok
test throttle::tests::escalates_to_sustained_after_continued_pressure ... ok
test throttle::tests::exits_one_level_at_a_time_after_recovery_dwell ... ok
test throttle::tests::flapping_spike_does_not_trip ... ok
test throttle::tests::gpu_hot_blocks_exit_even_when_cpu_cool ... ok
test throttle::tests::gpu_pressure_alone_can_throttle ... ok
test throttle::tests::gpu_unmonitored_is_cpu_only ... ok
test throttle::tests::hysteresis_band_holds_level ... ok
test throttle::tests::normal_stays_normal_below_enter ... ok
test worker_pool::tests::active_job_guard_tracks_in_flight_job_until_drop ... ok
test capture_loop::tests::native_max_width_zero_does_not_downscale ... ok
test worker_pool::tests::completion_info_only_for_changed_frame_jobs ... ok
test reports::tests::session_recap_without_usable_content_makes_zero_model_calls ... ok
test reports::tests::empty_range_is_honest_with_no_sidecar_call ... ok
test reports::tests::cancellation_between_passes_returns_err ... ok
test reports::tests::daily_small_range_uses_single_pass ... ok
test sessions_scheduler_contract_tests::successful_scheduler_pass_emits_sessions_changed_after_rows_commit ... ok
test sessions_scheduler_contract_tests::incremental_reconciliation_keeps_the_unfrozen_id_stable ... ok
test reports::tests::session_recap_with_overlapping_spans_cites_only_the_named_session ... ok
test sessions_scheduler_contract_tests::delayed_backfill_trims_before_frozen_incremental_overlap ... ok
test reports::tests::dense_single_period_splits_into_passes_without_truncating ... ok
test reports::tests::reduce_overflow_preserves_all_days_via_hierarchical_reduce ... ok
test reports::tests::weekly_covers_every_active_day_and_cites_first_and_last ... ok
test sessions_scheduler_contract_tests::historical_backfill_retries_past_a_fixed_target_cutting_a_live_track ... ok
test sessions_scheduler_contract_tests::historical_backfill_reuses_an_exact_unfrozen_partial_row ... ok
test capture_loop::tests::positive_max_width_downscales_keeping_aspect ... ok

test result: ok. 58 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.16s

     Running tests\enrichment.rs (target\debug\deps\enrichment-3fad8fbdc7840796.exe)

running 9 tests
test process_job_vision_tag_retries_without_provider ... ok
test process_job_dead_letters_missing_frame_id ... ok
test process_job_completes_on_empty_ocr_without_embedding ... ok
test process_job_retries_then_dead_letters_on_persistent_embed_failure ... ok
test process_job_embeds_text_and_completes ... ok
test process_job_vision_tag_writes_analysis ... ok
test vision_tag_failure_records_full_error_chain ... ok
test vision_jobs_drain_when_embeddings_disabled ... ok
test attach_embedder_drains_backlog_and_vector_arm_finds_frame ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests\pipeline.rs (target\debug\deps\pipeline-2bca85d4b7e2e281.exe)

running 12 tests
test kernel_refuses_to_start_capture_when_ocr_is_unavailable ... ok
test add_mark_capture_now_fails_when_capture_off ... ok
test add_mark_by_frame_id_marks_directly_and_validates_source ... ok
test kernel_start_then_stop_flips_capture_readiness ... ok
test add_mark_capture_now_propagates_denial_and_inserts_no_mark ... ok
test capture_loop_skips_embed_jobs_when_disabled ... ok
test stop_capture_notifies_ocr_provider ... ok
test reload_capture_restarts_loop_with_fresh_config ... ok
test capture_loop_stores_frames_ocr_jpegs_and_enqueues_embed_jobs ... ok
test add_mark_capture_now_inserts_manual_frame_and_mark ... ok
test kernel_clears_capture_and_marks_error_when_source_shuts_down ... ok
test source_shutdown_notifies_ocr_provider ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

     Running tests\sessions_intel.rs (target\debug\deps\sessions_intel-b4dd4ed302db897e.exe)

running 1 test
test lazy_intelligence_calls_summarize_once_then_serves_the_cache ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests\settings.rs (target\debug\deps\settings-aa36f96da5df33a9.exe)

running 16 tests
test round_trips_non_default_values ... ok
test save_settings_never_writes_retired_keys ... ok
test overlay_hotkey_empty_string_resets_to_default ... ok
test unknown_tier_falls_back_to_default_without_rewrite ... ok
test overlay_hotkey_custom_value_survives ... ok
test load_settings_sanitizes_persisted_numeric_values ... ok
test save_settings_persists_sanitized_numeric_values ... ok
test round_trips_defaults ... ok
test session_settings_round_trip_and_clamp_to_the_final_contract ... ok
test sidecar_device_round_trips_empty_as_none ... ok
test overlay_hotkey_legacy_default_remaps_once ... ok
test load_drops_retired_event_keys_without_error ... ok
test overlay_hotkey_failed_remap_is_retried_not_latched ... ok
test persisted_beta_tier_remaps_to_quality_and_persists ... ok
test sidecar_ctx_size_zero_is_preserved_as_auto_sentinel ... ok
test overlay_hotkey_deliberate_legacy_survives_after_migration ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

     Running tests\throttle.rs (target\debug\deps\throttle-a638624ad56f688d.exe)

running 2 tests
test throttle_disabled_drains_everything ... ok
test throttle_pauses_heavy_enrichment_then_resumes_on_recovery ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.67s

     Running unittests src\lib.rs (target\debug\deps\mcp-b92ba198223ba953.exe)

running 35 tests
test client::tests::api_error_is_code_colon_message ... ok
test client::tests::not_reachable_and_no_token_carry_the_contract_phrase ... ok
test config::tests::unknown_flag_and_missing_value_are_bad_usage ... ok
test rpc::tests::malformed_object_without_id_is_dropped ... ok
test config::tests::equals_and_space_flag_forms ... ok
test config::tests::loopback_hosts_are_accepted ... ok
test config::tests::help_and_version_short_circuit ... ok
test config::tests::flag_overrides_env_overrides_default ... ok
test config::tests::non_loopback_url_is_rejected ... ok
test rpc::tests::batch_array_yields_32600 ... ok
test rpc::tests::malformed_object_with_id_is_invalid_request ... ok
test config::tests::empty_token_is_none ... ok
test rpc::tests::notification_has_no_id ... ok
test rpc::tests::parse_error_yields_32700_with_null_id ... ok
test config::tests::defaults_when_nothing_set ... ok
test config::tests::trailing_slash_trimmed ... ok
test client::tests::unauthorized_mentions_401_and_regeneration ... ok
test rpc::tests::responses_are_single_line_even_with_newlines_in_strings ... ok
test rpc::tests::valid_request_is_parsed ... ok
test server::tests::downgrades_unknown_or_absent_version ... ok
test server::tests::echoes_each_supported_version ... ok
test server::tests::initialize_result_shape ... ok
test sse::tests::citations_deduped_in_arrival_order ... ok
test sse::tests::crlf_is_stripped ... ok
test sse::tests::done_and_error_terminate ... ok
test sse::tests::empty_data_object_ignored ... ok
test sse::tests::keepalives_and_blank_lines_ignored ... ok
test sse::tests::thinking_discarded_tokens_concatenated ... ok
test sse::tests::lines_reassemble_across_every_byte_boundary ... ok
test tools::tests::add_mark_body_captures_now_when_frame_id_absent_or_null ... ok
test tools::tests::add_mark_body_rejects_non_integer_frame_id ... ok
test tools::tests::add_mark_body_uses_integer_frame_id ... ok
test tools::tests::error_result_is_flagged ... ok
test tools::tests::exactly_six_tools_with_object_schemas ... ok
test tools::tests::required_fields_are_declared ... ok

test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\screensearch_mcp-f064a446419bae45.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\stdio_mcp.rs (target\debug\deps\stdio_mcp-ff6686a65554a830.exe)

running 19 tests
test ping_returns_empty_result ... ok
test malformed_line_is_parse_error ... ok
test batch_line_is_invalid_request ... ok
test child_exits_zero_on_stdin_close ... ok
test get_moment_text_only_and_include_image ... ok
test add_mark_now_surfaces_unavailable ... ok
test ask_without_model_is_tool_error ... ok
[screensearch-mcp] warning: no API token configured (SCREENSEARCH_API_TOKEN unset and --token not given); tool calls will return a guidance error until it is set.
test get_moment_unknown_frame_is_tool_error ... ok
test missing_token_still_serves_tools_list_but_calls_are_guided ... ok
test get_moment_purged_image_notes_purge_without_error ... ok
test ask_tool_aggregates_answer_and_citation ... ok
test handshake_and_tools_list_over_stdio ... ok
test unknown_method_is_method_not_found ... ok
test add_mark_frame_id_then_list_marks_roundtrip ... ok
test search_tool_roundtrips_fixture ... ok
test unknown_tool_is_protocol_error ... ok
test where_was_i_null_returns_human_message ... ok
test wrong_token_returns_guided_401_message ... ok
test api_off_tool_calls_return_guided_error ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.06s

     Running unittests src\lib.rs (target\debug\deps\ocr-87f88b55c9cff589.exe)

running 2 tests
test tests::winrt_ocr_recognizes_blank_image ... ignored, requires WinRT OCR language pack; run locally
test tests::normalize_rect_maps_and_clamps_to_unit_square ... ok

test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\screensearch_lib-d40f226e2fda898f.exe)

running 21 tests
test local_api::tests::port_clamps_to_floor ... ok
test tests::ipc_presentation_limits_are_clamped ... ok
test tests::parses_llama_cpp_device_ids ... ok
test tests::hydrate_ask_context_returns_clear_error_when_ocr_texts_fails ... ok
test tests::safe_frame_path_accepts_only_relative_frames_children ... ok
test tests::session_query_normalizes_time_kind_tool_and_limit ... ok
test tests::sanitize_report_stem_produces_a_safe_leaf_name ... ok
test tray::tests::capture_status_maps_to_visual ... ok
test tray::tests::composed_icons_differ_per_state ... ok
test tests::open_store_reports_error_when_db_cannot_open ... ok
test tray::tests::labels_track_state ... ok
test tests::db_file_family_size_includes_wal_and_shm ... ok
test tests::unique_markdown_path_appends_2_3_on_collision ... ok
test local_api::tests::fresh_profile_defaults_off ... ok
test tests::ui_resume_context_hydrates_session_without_changing_external_resume_shape ... ok
test local_api::tests::bind_failure_keeps_enabled_with_error ... ok
test tests::session_recap_evidence_probe_rejects_missing_text_before_provider_acquisition ... ok
test tests::merge_purged_spans_once_merges_backlog_then_watermarks_and_is_idempotent ... ok
test local_api::tests::enabling_generates_token_once_and_persists ... ok
test tests::session_detail_samples_24_frames_and_returns_only_exchanges_without_inference ... ok
test tests::open_store_creates_db_file_and_reports_ready ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

     Running unittests src\main.rs (target\debug\deps\screensearch-e067c9d301e668e6.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\config_guard.rs (target\debug\deps\config_guard-24418d3f8fbadcd1.exe)

running 1 test
test overlay_window_is_precreated_hidden_and_capture_protected ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\e2e_capture.rs (target\debug\deps\e2e_capture-24b27b5439f666f3.exe)

running 1 test
test capture_pipeline_stores_frames_ocr_and_enqueues_embed_jobs ... ignored, real WGC + WinRT capture; requires a desktop session, run locally

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\sessions-693e335fd4aabc8e.exe)

running 4 tests
test contract_tests::confidence_tiers_keep_anchorless_below_anchored ... ok
test taxonomy::tests::parsing_normalizes_match_inputs_once ... ok
test taxonomy::tests::invalid_prefix_range_is_rejected ... ok
test taxonomy::tests::seed_has_nine_entries ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\engine.rs (target\debug\deps\engine-9241b455e1799f98.exe)

running 21 tests
test codex_desktop_chrome_is_not_misclassified_as_an_agent_turn ... ok
test desktop_and_browser_markers_extract_heading_blocks ... ok
test empty_claude_prompt_does_not_capture_the_terminal_status_bar ... ok
test claude_code_markers_extract_only_explicit_roles ... ok
test no_marker_means_no_exchange_and_duplicates_collapse ... ok
test bundled_taxonomy_v3_parses_at_startup ... ok
test browser_ai_requires_browser_stem_and_ai_title ... ok
test consolidated_short_excursion_frames_belong_to_the_surviving_micro ... ok
test exclusive_frame_ownership_survives_cross_track_overlap_and_none_absorption ... ok
test windows_breadcrumb_chevrons_are_not_claude_code_prompts ... ok
test meetings_never_absorb_and_can_overlap_ai_and_each_other ... ok
test long_unrecognized_run_becomes_focus_material ... ok
test same_track_splits_only_at_its_own_merge_gap ... ok
test chatgpt_renamed_desktop_maps_to_codex_but_classic_does_not ... ok
test open_flag_tracks_inactivity_at_now ... ok
test sub_qualification_ai_track_is_dropped ... ok
test two_tools_interleaved_form_overlapping_tracks ... ok
test sparse_focus_is_density_gated_but_ai_is_exempt ... ok
test sustained_foreign_identity_does_not_close_incumbent ... ok
test spinner_prefix_recognizes_claude_code ... ok
test confidence_penalizes_absorbed_time_and_keeps_ai_above_focus ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\store-3d8c8c6035071314.exe)

running 38 tests
test marks::tests::insert_rejects_unknown_frame_with_clear_error ... ok
test marks::tests::mark_survives_image_purge_with_text_kept ... ok
test marks::tests::list_orders_unresolved_first_then_newest_first ... ok
test frames::tests::sample_degenerate_windows_are_empty ... ok
test marks::tests::set_note_round_trips_and_rejects_unknown ... ok
test marks::tests::resolve_is_idempotent_but_errors_on_unknown ... ok
test frames::tests::recent_frame_contexts_newest_first_capped_with_id_tiebreak ... ok
test migration_tests::migration_v10_adds_marks_with_cascade ... ok
test frames::tests::image_older_than_excludes_purged_and_recent ... ok
test frames::tests::sample_returns_all_when_count_under_limit ... ok
test records::tests::merge_spans_to_lines_collapses_words_and_unions_boxes ... ok
test records::tests::merge_spans_to_lines_empty_is_empty ... ok
test records::tests::merge_spans_to_lines_is_idempotent ... ok
test records::tests::merge_spans_to_lines_prefers_content_role ... ok
test records::tests::primary_source_for_maps_engine_to_db_token ... ok
test frames::tests::sample_spreads_evenly_and_includes_the_earliest_frame ... ok
test search::tests::escalating_knn_caps_at_the_k_ceiling ... ok
test search::tests::escalating_knn_stops_at_window_count_not_ceiling ... ok
test search::tests::escalating_knn_stops_when_table_exhausted ... ok
test frames::tests::purge_frame_image_drops_image_but_keeps_text_proof ... ok
test search::tests::escalating_knn_truncates_to_pool ... ok
test search::tests::escalating_knn_widens_until_target_reached ... ok
test search::tests::normalized_limit_clamps_to_the_backend_ceiling ... ok
test migration_tests::migration_v11_adds_sessions_structure_only ... ok
test frames::tests::sample_returns_full_quota_when_just_over_limit ... ok
test frames::tests::sample_caps_at_limit_within_window ... ok
test migration_tests::migration_v11_sessions_check_constraints ... ok
test migration_tests::migration_v11_artifact_role_kind_coupling ... ok
test migration_tests::fresh_and_migrated_schemas_agree_at_latest ... ok
test migration_tests::migration_v7_adds_image_purged_present_by_default ... ok
test migration_tests::migration_v8_indexes_image_retention_sweep ... ok
test migration_tests::migration_v6_widens_capture_trigger_check_without_dropping_children ... ok
test migration_tests::migration_v9_drops_image_lane_and_embed_image_jobs ... ok
test migration_tests::migration_v11_fk_set_null_and_cascade ... ok
test records::tests::filtered_insert_records_text_source_from_engine ... ok
test search::tests::count_embedded_frames_dedups_chunks_caps_and_bounds_the_scan ... ok
test search::tests::include_chrome_searches_raw_text_independently_of_content ... ok
test migration_tests::migration_v11_preserves_frame_surfaces ... ok

test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s

     Running tests\perf.rs (target\debug\deps\perf-1cadcb60d0bbcbc5.exe)

running 1 test
test hybrid_search_under_200ms_on_realistic_db ... ignored, seeds 10k frames + 768-dim vectors; run locally: cargo test -p store --test perf -- --ignored --nocapture

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\sessions.rs (target\debug\deps\sessions-275f51a9a0bc8140.exe)

running 9 tests
test session_crud_is_unfrozen_only_and_ids_stay_stable ... ok
test session_queries_use_half_open_overlap_and_request_time_for_open_rows ... ok
test title_summary_cache_updates_the_row_without_touching_boundaries ... ok
test session_reference_for_frame_omits_deleted_session ... ok
test artifact_checks_and_delete_by_kind_are_enforced ... ok
test frame_metadata_and_content_reads_are_chronological ... ok
test frozen_session_frames_cannot_be_cleared_or_reassigned ... ok
test deleting_unfrozen_sessions_preserves_frames_text_and_marks ... ok
test session_frame_sample_reports_total_and_even_chronological_endpoints_without_leakage ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s

     Running tests\store.rs (target\debug\deps\store-7274e91ea5422cb5.exe)

running 64 tests
test complete_job_moves_to_done ... ok
test completing_or_failing_an_unknown_job_is_an_error ... ok
test claim_filters_by_kind ... ok
test claim_honors_not_before_schedule ... ok
test complete_job_requires_running_state ... ok
test degrade_frame_to_text_purges_even_without_spans ... ok
test claim_returns_highest_priority_first_and_marks_running ... ok
test cancel_pending_vision_jobs_removes_only_pending_vision ... ok
test degrade_frame_to_text_merges_spans_and_purges_atomically ... ok
test empty_time_window_returns_nothing_via_vector_arm ... ok
test export_frames_page_honors_half_open_time_window ... ok
test delete_frame_cascades_and_purges_vectors ... ok
test backfill_filter_version_invalidates_stale_text_embedding ... ok
test backfill_filter_version_recleans_old_frames_against_warm_catalog ... ok
test concurrent_claims_never_double_claim ... ok
test dense_time_window_returns_the_pool_nearest_in_window_matches ... ok
test export_frames_page_zero_limit_is_empty ... ok
test frame_enrichment_input_reads_path_and_optional_text ... ok
test export_frames_page_left_join_yields_none_content_for_textless_frames ... ok
test fail_without_retry_at_dead_letters_immediately ... ok
test frames_in_range_lists_window_recent_first ... ok
test fail_job_requires_running_state ... ok
test frames_older_than_lists_bounded_retention_candidates ... ok
test frames_with_app_hint_matches_case_insensitively ... ok
test fail_retries_with_backoff_then_dead_letters_at_max_attempts ... ok
test live_db_copy_migrates_to_v11_fast_and_clean ... ignored, manual Gate 0: set SCREENSEARCH_MIGRATION_CHECK_DB to a THROWAWAY copy of the live DB
test hybrid_search_empty_query_returns_nothing ... ok
test hybrid_search_fts_only_without_embedder ... ok
test hybrid_search_fuses_fts_and_vector_arms_via_rrf ... ok
test hybrid_search_honors_time_range ... ok
test insert_frame_then_get_frame_returns_context ... ok
test hybrid_search_respects_limit ... ok
test insert_ocr_persists_spans_with_pr2_defaults ... ok
test job_stats_splits_out_vision_pending_and_running ... ok
test insights_summary_uses_requested_bucket_count ... ok
test insights_summary_aggregates_truthfully ... ok
test insert_vision_then_get_frame_has_analysis ... ok
test insert_ocr_then_get_frame_has_text ... ok
test merge_frame_spans_to_lines_is_noop_without_spans ... ok
test insert_ocr_filtered_suppresses_repeated_chrome_after_threshold ... ok
test open_path_rejects_future_schema_version ... ok
test merge_frame_spans_to_lines_shrinks_rows_but_keeps_search_and_reconstruction ... ok
test nearest_frame_in_range_ignores_frames_outside_window ... ok
test nearest_frame_picks_closest_with_after_winning_ties ... ok
test neighbour_frames_brackets_anchor_with_closest_each_side ... ok
test hybrid_search_clamps_excessive_limit ... ok
test ocr_texts_bulk_fetches_nonempty_only ... ok
test open_in_memory_migrates_to_latest_schema_version ... ok
test set_settings_batch_writes_all_and_overwrites ... ok
test reset_stale_running_jobs_spares_fresh_running ... ok
test purged_frame_ids_lists_only_purged_after_cursor ... ok
test settings_round_trip_and_overwrite ... ok
test reset_stale_running_jobs_requeues_running ... ok
test timeline_buckets_survives_extreme_ranges ... ok
test timeline_buckets_are_sparse_and_half_open ... ok
test text_embedding_knn_orders_by_cosine_distance ... ok
test untagged_frame_ids_excludes_tagged_and_honors_range ... ok
test untagged_frame_ids_excludes_in_flight_vision_jobs ... ok
test upsert_text_embedding_replaces_vector_in_place ... ok
test sparse_time_window_returns_every_in_window_match ... ok
test wrong_dimension_embedding_is_rejected ... ok
test works_through_the_store_trait_object ... ok
test export_frames_page_pages_through_all_frames_in_id_order ... ok
test vector_arm_finds_in_range_match_buried_beyond_pool ... ok

test result: ok. 63 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.57s

     Running unittests src\lib.rs (target\debug\deps\sysmon-af668d7fbcee06d0.exe)

running 11 tests
test cpu::tests::clamps_when_idle_exceeds_total ... ok
test cpu::tests::fully_busy_is_hundred_pct ... ok
test gpu::tests::empty_is_zero ... ok
test cpu::tests::half_idle_is_fifty_pct ... ok
test cpu::tests::user_time_counts_as_busy ... ok
test cpu::tests::zero_total_delta_returns_none ... ok
test gpu::tests::clamps_to_hundred ... ok
test cpu::tests::fully_idle_is_zero_pct ... ok
test gpu::tests::sums_engines ... ok
test gpu::tests::ignores_non_finite ... ok
test tests::sample_is_well_formed ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s

     Running unittests src\lib.rs (target\debug\deps\textfilter-f294f0d80943d220.exe)

running 12 tests
test tests::empty_spans_produce_empty_output ... ok
test tests::default_frame_drops_system_and_background_keeps_content ... ok
test tests::short_interior_body_is_never_catalogued ... ok
test tests::no_target_rect_never_classifies_background_or_system ... ok
test tests::reconcile_demotes_only_the_catalogued_region ... ok
test tests::reconcile_cleans_catalogued_chrome_even_without_target_rect ... ok
test tests::reconcile_is_idempotent ... ok
test tests::reconcile_with_cold_catalog_changes_nothing ... ok
test tests::no_target_rect_never_suppresses_even_a_saturated_signature ... ok
test tests::window_title_echoed_as_body_is_excluded ... ok
test tests::toolbar_becomes_chrome_at_the_seen_threshold ... ok
test tests::reconcile_demotes_warm_catalog_chrome_preserving_content ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\traits-cce057699f881ff6.exe)

running 78 tests
test domain::tests::capture_trigger_db_str_round_trips ... ok
test domain::tests::capture_trigger_from_unknown_db_str_is_none ... ok
test domain::export_bindings_suppressreason ... ok
test ipc::export_bindings_activitycount ... ok
test domain::export_bindings_monitorinfo ... ok
test domain::export_bindings_capturetrigger ... ok
test domain::export_bindings_textrole ... ok
test domain::export_bindings_visionanalysis ... ok
test domain::export_bindings_textsource ... ok
test ipc::export_bindings_answerdelta ... ok
test ipc::export_bindings_appcount ... ok
test ipc::export_bindings_appsuppression ... ok
test ipc::export_bindings_askrequest ... ok
test ipc::export_bindings_apistatus ... ok
test ipc::export_bindings_capturecontrol ... ok
test ipc::export_bindings_capturetick ... ok
test ipc::export_bindings_componentstatus ... ok
test ipc::export_bindings_exportrequest ... ok
test ipc::export_bindings_exportresult ... ok
test ipc::export_bindings_flashattnsetting ... ok
test ipc::export_bindings_framemeta ... ok
test ipc::export_bindings_hotkeystatus ... ok
test ipc::export_bindings_answerevent ... ok
test ipc::export_bindings_kvcachetype ... ok
test ipc::export_bindings_mark ... ok
test ipc::export_bindings_componentreadiness ... ok
test ipc::export_bindings_modeldownloadphase ... ok
test ipc::export_bindings_modellane ... ok
test ipc::export_bindings_modeltier ... ok
test ipc::export_bindings_openmoment ... ok
test ipc::export_bindings_pressuresample ... ok
test ipc::export_bindings_jobprogress ... ok
test ipc::export_bindings_reportkind ... ok
test ipc::export_bindings_reportprogress ... ok
test ipc::export_bindings_marktoast ... ok
test ipc::export_bindings_reportresponse ... ok
test ipc::export_bindings_resumecontext ... ok
test domain::export_bindings_textspan ... ok
test ipc::export_bindings_searchhit ... ok
test ipc::export_bindings_jobcompleted ... ok
test ipc::export_bindings_sessionrecaprequest ... ok
test ipc::export_bindings_modeldownloadstatus ... ok
test ipc::export_bindings_framedetail ... ok
test ipc::export_bindings_sidecarstate ... ok
test ipc::export_bindings_storagestats ... ok
test ipc::export_bindings_searchquery ... ok
test ipc::export_bindings_readiness ... ok
test ipc::export_bindings_insightssummary ... ok
test ipc::export_bindings_timelinebucket ... ok
test ipc::ts_number_guard::no_bigint_in_ipc_types ... ok
test ipc::export_bindings_reportrequest ... ok
test ipc::export_bindings_timerange ... ok
test ipc::export_bindings_toastlevel ... ok
test privacy::tests::allows_unrelated_apps ... ok
test privacy::tests::empty_excluded_entry_never_matches ... ok
test privacy::tests::matches_process_name_case_insensitively ... ok
test privacy::tests::matches_window_title ... ok
test ipc::export_bindings_updatestatus ... ok
test ipc::export_bindings_visiontarget ... ok
test ipc::export_bindings_sessionquery ... ok
test ipc::export_bindings_throttlestatus ... ok
test jobs::export_bindings_jobkind ... ok
test ipc::export_bindings_sessionreference ... ok
test ipc::export_bindings_setmodeltier ... ok
test jobs::export_bindings_jobstats ... ok
test jobs::export_bindings_jobstate ... ok
test ipc::export_bindings_toast ... ok
test ipc::export_bindings_sidecarstatus ... ok
test sessions::export_bindings_sessionartifactkind ... ok
test ipc::export_bindings_settings ... ok
test sessions::export_bindings_sessionartifactrole ... ok
test sessions::export_bindings_sessionhost ... ok
test sessions::export_bindings_sessionkind ... ok
test sessions::export_bindings_session ... ok
test ipc::export_bindings_uiresumecontext ... ok
test sessions::export_bindings_sessionartifact ... ok
test ipc::export_bindings_sessiondetail ... ok
test ipc::export_bindings_uiframedetail ... ok

test result: ok. 78 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running tests\sessions_contract.rs (target\debug\deps\sessions_contract-7afba26c650911a0.exe)

running 4 tests
test shipped_segmentation_params_pin_the_pr2_gate_values ... ok
test external_frame_and_resume_contracts_remain_session_free ... ok
test session_database_tokens_match_schema_eleven ... ok
test session_ui_contract_exports_without_bigint ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\lib.rs (target\debug\deps\uia-2c1113c675b32bee.exe)

running 30 tests
test breaker::tests::breaker_cooldown_expiry_closes_and_resets ... ok
test breaker::tests::breaker_good_resets_the_streak ... ok
test breaker::tests::breaker_isolates_apps ... ok
test classify::tests::chromium_window_classes_are_detected_others_left_alone ... ok
test breaker::tests::breaker_opens_after_three_consecutive_bad ... ok
test breaker::tests::breaker_reports_transitions_once ... ok
test breaker::tests::signal_good_within_budget_ok ... ok
test breaker::tests::signal_neutral_on_busy_and_within_budget_err ... ok
test breaker::tests::breaker_neutral_neither_counts_nor_resets ... ok
test breaker::tests::signal_bad_on_hard_timeout_and_over_budget ... ok
test classify::tests::high_frequency_interactive_triggers_never_run_uia ... ok
test classify::tests::input_gate_only_touches_timer_triggers ... ok
test classify::tests::input_gate_skips_timer_walks_during_active_input ... ok
test classify::tests::only_document_and_text_controls_want_textpattern ... ok
test classify::tests::containers_are_skipped_but_content_controls_emit ... ok
test classify::tests::input_gate_is_disabled_by_zero_window ... ok
test classify::tests::low_frequency_triggers_run_uia ... ok
test geometry::tests::degenerate_inputs_are_zero ... ok
test geometry::tests::left_top_straddling_box_reports_only_on_frame_extent ... ok
test breaker::tests::threshold_is_clamped_to_at_least_one ... ok
test input::tests::reports_an_idle_time ... ignored, requires a real desktop session
test classify::tests::split_words_groups_lines_and_skips_blanks ... ok
test classify::tests::never_emits_password_or_offscreen_or_container ... ok
test tests::uia_provider_spawns_and_recognizes_foreground ... ignored, requires a real desktop (UI Automation); run locally
test tests::uia_worker_exits_on_shutdown ... ignored, requires a real desktop (UI Automation); run locally
test geometry::tests::overrunning_box_is_clamped_to_unit_square ... ok
test window::tests::live_hwnd_classification ... ignored, requires a real desktop; pass UIA_PROBE_HWND=<i64>
test geometry::tests::primary_monitor_maps_proportionally ... ok
test geometry::tests::secondary_monitor_subtracts_its_origin ... ok
test worker::tests::within_target_filters_by_center ... ok

test result: ok. 26 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests api

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests capture

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests doctor

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests embeddings

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests harness

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests inference

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests kernel

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests mcp

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests ocr

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests screensearch_lib

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests sessions

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests store

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests sysmon

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests textfilter

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests traits

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests uia

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### `git diff --exit-code -- ui/src/bindings` (exit 0)

```text

```

## Pass 13 — 2026-07-11 — 0.4.0 PR5 PR #104 review follow-up (`c360d7e`)

- **Review disposition:** All four unresolved inline threads were relevant and addressed. Two
  threads duplicated the same store concern, so the implementation clusters into three fixes. Per
  maintainer instruction, no bot replies were posted and no review-thread state was mutated.
- **Store (duplicate threads):** `session_has_usable_content` no longer materializes every matching
  content row in Rust. Its indexed SQLite `EXISTS` query stops after the first usable row. Passing an
  explicit character set to SQLite `trim` preserves the pre-existing Rust `str::trim` contract
  exactly across every Unicode scalar for which `char::is_whitespace()` is true; U+200B remains
  usable content because Rust does not classify it as whitespace.
- **Timeline:** Error and empty top-level branches now render the same fixed session layer already
  used by loading and populated states, preserving the settled four-lane D9 geometry.
- **Scheduler:** Reconciliation now returns a semantic-change bit and performs delta ownership and
  exchange-artifact writes. An identical pass neither rewrites correct state nor emits
  `sessions_changed`; failures after a possible earlier write conservatively invalidate mounted
  readers.

### Store RED

```text
test sessions::tests::usable_content_query_bounds_results_inside_sqlite ... FAILED
the VM must stop after SQLite finds one usable row; opcodes: ["Init", "OpenRead", "OpenRead", "Variable", "IsNull", "Affinity", "SeekGE", "IdxGT", "IdxRowid", "SeekRowid", "Column", "ResultRow", "Next", "Halt", "Transaction", "Goto"]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 38 filtered out
```

### Store GREEN and focused gates

```text
test sessions::tests::usable_content_query_bounds_results_inside_sqlite ... ok
test session_usable_content_matches_rust_trim_unicode_whitespace ... ok
test result: ok. 40 passed; 0 failed
test result: ok. 10 passed; 0 failed
test result: ok. 63 passed; 0 failed; 1 ignored
```

Focused store clippy and `cargo fmt --all -- --check` exited 0.

### Timeline RED

```text
✖ Timeline keeps fixed session lanes in every content state
AssertionError [ERR_ASSERTION]: the loading skeleton must reserve the fixed session lanes
0 !== 1
ℹ pass 1
ℹ fail 1
```

### Timeline GREEN and focused gates

```text
✔ five simultaneous sessions use four fixed rows and aggregate the fifth (0.9343ms)
✔ Timeline keeps fixed session lanes in every content state (0.3541ms)
ℹ tests 2
ℹ pass 2
ℹ fail 0
```

UI typecheck and lint exited 0. Production build:

```text
✓ 438 modules transformed.
✓ built in 1.64s
```

The rendered forced error/loading check measured four `31.9965px` rows, a `139.9653`-pixel group,
`documentWidth === viewportWidth === 1704`, and nested scrollers `[]`.

### Scheduler RED

```text
running 1 test
test sessions_scheduler_contract_tests::no_op_scheduler_pass_does_not_emit_sessions_changed ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 58 filtered out; finished in 0.01s
```

The assertion at `crates/kernel/src/lib.rs:602` observed that the receiver was not `Empty` after the
second, unchanged pass.

### Scheduler GREEN and focused gates

```text
running 2 tests
test sessions_scheduler_contract_tests::successful_scheduler_pass_emits_sessions_changed_after_rows_commit ... ok
test sessions_scheduler_contract_tests::no_op_scheduler_pass_does_not_emit_sessions_changed ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 57 filtered out; finished in 0.01s
test result: ok. 59 passed; 0 failed
```

### Controller targeted recheck

The controller independently reran the touched gates at code `c360d7e`; all exited 0. Store
summaries remained 40/0, 10/0, and 63/0/1 ignored; kernel remained 59/0; fmt and focused clippy were
clean. UI test output was 2 passed / 0 failed in 66.6182 ms; typecheck and lint were clean; build:

```text
✓ 438 modules transformed.
✓ built in 1.62s
```

### Independent post-fix review

The independent review found **Critical: none; Important: none; Minor: none; Ready: yes**. Its own
focused rerun passed the scheduler contract 13/13, store sessions 10/10, and UI tests 2/2. No bot
reply or review-thread mutation was made.

- **Scope/status:** No schema/migration, API/MCP, audio, notification, nudge, score, NavRail,
  frame-level behavior, or generated binding changed. This pass records focused review evidence;
  the post-review full UI-first suite has not yet run and will be recorded separately.

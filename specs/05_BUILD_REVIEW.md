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

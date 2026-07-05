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

> Pre-0.2.x (v0.1.0) history → `specs/archive/05_BUILD_REVIEW.v0.1.0.md`.
> Shipped 0.2.x history (0.2.0–0.2.2) → `specs/archive/05_BUILD_REVIEW.v0.2.x.md`.
> Shipped 0.3.0 history (the whole arc: PR1–PR9 + post-0.2.2 bridge fixes) →
> `specs/archive/05_BUILD_REVIEW.v0.3.0.md`.
> Shipped 0.3.1 history (the P7.1 triage patch: post-0.3.0 bridge fixes PR #79/#80 + PR1–PR4) →
> `specs/archive/05_BUILD_REVIEW.v0.3.1.md`.
> Live file holds only the current arc — empty until the next arc begins.

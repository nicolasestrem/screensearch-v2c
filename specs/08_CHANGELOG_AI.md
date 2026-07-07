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

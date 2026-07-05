# 08 — AI Changelog

> Append-only record of what the agent changed during the build, **with reasons**. One entry per
> meaningful change set. Empty until P0 begins. (This tracks build work; the design-phase history
> lives in git.)

## <date> — <short title>
- **Change:** what was added/modified.
- **Why:** the reason, tied to a spec section.
- **Verification:** the command run + verbatim result.

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

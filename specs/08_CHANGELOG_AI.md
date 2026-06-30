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
> Live file holds only the current (post-0.2.2) arc.

---

## 2026-06-30 — Spec archival sweep + close gaps #43/#44 (`chore/archive-known-gaps`)
- **Change:** (1) Restored the archive-on-release convention: moved the shipped 0.2.0/0.2.1/0.2.2
  entries out of the live build-loop logs (`05`/`06`/`07`/`08`) and `CHANGELOG.md` into per-arc
  `specs/archive/*.v0.2.x.md` files and `CHANGELOG-ARCHIVE.md`, verbatim with original `#N` ids. `07`
  also moved its resolved-engineering-decisions list and retired five accepted-as-is rows (#40, #45,
  #59, #60, #61); live `07` went 35 → 12 rows. (2) `#44`: added a privacy-safe `info` log of
  `frame_id` + relative capture path before each VLM request in `crates/kernel/src/worker_pool.rs`.
  (3) `#43`: added a dev-only `?__devState` route-state override at the `ui/src/lib/ipc/queries.ts`
  `useQuery` seam (+ `ui/src/lib/dev/*`, `DevStateBadge.tsx`, `docs/DEV_STATE_OVERRIDE.md`).
- **Why:** CLAUDE.md "Archive on release" had been applied to v0.1.0 only, so the live logs had
  re-bloated across the whole 0.2.x arc. `#44`/`#43` were the two reachable open gaps (`07`): the VLM
  log was missing because the only prior candidate was below the default `info` filter, and audit
  loading/error states couldn't be forced without mocking production data — the dev-gated flag
  override solves it without shipping any override or fake payload to production.
- **Verification:** `cargo fmt`/`clippy -D warnings`/`build`/`test --workspace` all `EXIT 0` (kernel
  enrichment 10 passed incl. `process_job_vision_tag_writes_analysis`); `npm run lint`+`build` clean
  (`✓ built in 1.81s`); `grep -rl __devState ui/dist/assets/` **absent** (prod tree-shake);
  `git diff --exit-code -- ui/src/bindings` clean; archived blocks diff **byte-identical** against
  `git HEAD`; all `#N` cross-references resolve live-or-archived.

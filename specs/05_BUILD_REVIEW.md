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
> Live file holds only the current (post-0.2.2) arc.

---

## Pass — 2026-06-30 — Spec archival sweep + close reachable gaps #43/#44 (`chore/archive-known-gaps`)

From a `/superpowers:brainstorming` design (plan approved): shrink `07_KNOWN_GAPS.md` by restoring
the archive-on-release convention, and close the two reachable open gaps.

### Implemented
- **Archive-on-release brought current.** Only v0.1.0 had been archived; the shipped 0.2.0/0.2.1/0.2.2
  history still sat in the live logs. Moved it out **verbatim** (original `#N` ids preserved) into
  per-log `*.v0.2.x.md` archives: `07` (18 resolved + 5 accepted-as-is rows + the
  resolved-engineering-decisions list; 35 rows → 12), `05`, `08`, `06` (keeps its one open
  upstream-leak row #15), and `CHANGELOG.md` → `CHANGELOG-ARCHIVE.md` (the 0.2.x sections that had
  piled under `[Unreleased]`). Verified byte-identical to `git HEAD` (the project's own archival
  check) and that every `#N` cross-reference still resolves live-or-archived.
- **#44 — privacy-safe VLM image-path log.** One `tracing::info!(frame_id, image_path=…)` in
  `crates/kernel/src/worker_pool.rs::vision_tag_outcome`, immediately before `vision.analyze`. At
  `info` (the default `EnvFilter`), so it reaches `screensearch.log` — a `debug!` was why the audit
  scan found nothing. Logs only the frame id + relative path; no screen content (the inference client
  never sees the path, so the kernel is the only correct layer).
- **#43 — dev-only deterministic route-state triggers.** A `?__devState=loading|error` URL param
  forces any P5 route into that state, applied centrally at the `ui/src/lib/ipc/queries.ts` `useQuery`
  seam (all 17 read hooks; 0 mutations), dev-gated by `import.meta.env.DEV` and tree-shaken from prod.
  New `ui/src/lib/dev/{devState,useDevStateOverride}.ts` + `DevStateBadge.tsx`; documented in
  `docs/DEV_STATE_OVERRIDE.md`. Forces result *flags* only — empty/partial/populated stay real
  (no mocks in the production path).

### Verification (Windows, full CI sequence) — verbatim
- `cargo fmt --all -- --check` → `EXIT 0`
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile … in 14.12s` / `EXIT 0`
- `cargo build --workspace` → `Finished dev profile … in 24.95s` / `EXIT 0`
- `cargo test --workspace` → all suites green (kernel 27; `kernel --test enrichment` **10 passed**
  incl. `process_job_vision_tag_writes_analysis`; store 14+49; inference 95; traits 53; uia 16; sysmon
  11; textfilter 12; capture 27; e2e/perf/smoke ignored on this host) / `EXIT 0`
- `cd ui && npm run lint` → eslint clean (Rules-of-Hooks gate) / `EXIT 0`
- `npm run build` → `✓ built in 1.81s`
- prod-strip: `grep -rl __devState ui/dist/assets/` → **ABSENT** (dev override tree-shaken out)
- `git diff --exit-code -- ui/src/bindings` → clean (`EXIT 0`)

### Skipped / deferred
- The 10 still-open `07` rows (hardware-only checks, upstream fixes, future features, accepted
  trade-offs) are unchanged. The other reachable-but-heavier idea (`07` #43's seeded-fixture harness)
  was deliberately not built — the dev-only flag override is the lower-risk close the gap asked for.

### Still risky
- `#43` is dev-only by construction (stripped from prod, proven by the `dist` grep); the documented
  per-route manual screenshot pass (`docs/DEV_STATE_OVERRIDE.md`) is the live acceptance and was not
  run headless here.

### Follow-up — PR #60 review fix (`#43` prod-path correctness)
Three reviewers (Codex P3, Gemini high, Claude bot) independently flagged the same real defect:
`useMaybeOverride` called `useSearchParams()` **before** the `import.meta.env.DEV` guard. Because the
helper is invoked from the production `queries.ts` call-site, that hook call is *not* tree-shaken — so
release builds subscribed all 17 query consumers to router-history changes (extra re-renders on every
client-side navigation), and coupled every global read query to a `<Router>` context (crash risk in
hook usage outside a Router). The `dist` grep had only proven the `__devState` *string* was stripped,
not the hook call.
- **Fix:** drop `useSearchParams`; read `window.location.search` directly *inside* the DEV guard
  (`ui/src/lib/dev/useDevStateOverride.ts`). The helper now calls **no** React hook, so the early
  return makes the production path a plain `return result` identity — no subscription, no Router
  coupling, and no Rules-of-Hooks concern (nothing conditional is a hook). `readDevState` already
  accepted a raw `location.search` string (leading `?` handled by `URLSearchParams`). Doc + CHANGELOG
  updated to state the stronger guarantee.
- **Verification (verbatim):** `npm run lint` → `LINT_EXIT=0`; `npm run build` → `✓ built in 1.55s` /
  `BUILD_EXIT=0`; `grep -rl __devState dist/assets/` → **absent** (`GREP_EXIT=1`);
  `grep -rl "dev: forced route error" dist/assets/` → **absent** (the whole override module, not just
  the param string, is gone); `git diff --stat -- ui/src/bindings` → clean.

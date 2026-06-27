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

## Pass — 2026-06-27 — PR7 audit follow-ups

**Branch:** `codex/pr7-audit-followups`.

### Implemented
- Relabeled Recall Ask source-frame tiles from `Cited frames` to `Frames checked`, matching the
  existing backend semantics: those frame ids are context/provenance supplied to the answer model,
  not model-authored evidence for a positive claim.
- Updated nearby Ask comments in the UI, Tauri command, and inference provider so future readers do
  not reintroduce the PR7 confusion.
- Reconciled PR7 audit docs: the static-chrome search finding is now recorded as resolved by the
  later PR3 self-exclude/backfill fix (`07` #66) with residual rect-None / secondary-monitor risk
  left in `07` #58; the no-evidence Ask finding (`07` #41/#63) is resolved by the relabel approach;
  the PR8 stale-bitmap follow-up is renumbered to `07` #69 to remove the duplicate #66.
- Updated `docs/ARCHITECTURE.md` for the current backend search cap (`1..=2,000`, candidate pool
  capped at 2,000) and updated `docs/TESTING.md` to make PR7 audit artifacts local-only ignored
  evidence.

### Skipped / deferred
- No schema, migration, typed IPC, binding, or prompt/protocol change. True model-authored
  claim-level citations remain deferred until the app has a structured citation protocol; this pass
  only makes the current reviewed-context UI truthful.

### Verification
Automated gates passed on 2026-06-27; raw command output is pasted in the final session response:
- `cd ui && npm ci && npm run lint && npm run build`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --workspace`
- `cargo test --workspace`
- `git diff --exit-code -- ui/src/bindings`

Manual dev-exe verification passed with `npm run tauri dev`, launching
`target/debug/screensearch.exe`; logs are stored under
`.playwright-mcp/pr7-followups-2026-06-27/` and remain ignored local evidence.
- Recall Ask no-evidence query `PR7_NO_EVIDENCE_UNIQUE_TOKEN_20260627_X9Q` rendered an honest
  refusal and displayed retrieved tiles under `FRAMES CHECKED`; the old `Cited frames` label was not
  present.
- Daily report generation progress displayed the existing range-neutral copy:
  `Reports summarize active periods in bounded passes, so larger ranges can take a little longer.`
- Default Recall search for `chrome` returned result rows with the `CONTENT TEXT ONLY` control and
  static-toolbar filter copy visible, including Chrome hits plus backfilled non-Chrome rows, with no
  self-capture/static-chrome regression observed in the sampled dev app state.

---

## Audit — 2026-06-26 — 0.2.0 PR3 attention-first filtering

**Branch:** `codex/0.2.0-pr3-audit`. Runtime: `npm run tauri dev` launching
`target/debug/screensearch.exe`. DB policy: existing
`%APPDATA%\app.screensearchv2c.desktop\screensearch.db`, online backup to
`.playwright-mcp/pr3-2026-06-26/screensearch-pr3-before.sqlite`, no reset/backfill/destructive SQL.

### Implemented / audited
- Added the audit artifact `docs/AUDIT_0.2.0_PR3_2026-06-26.md`.
- Verified PR3's storage/retrieval plumbing: raw text is preserved, filtered content/spans/filter
  version are written, embeddings read `content_text`, default search uses content FTS, and
  `include_chrome=true` keeps raw/static recovery available.
- Verified Settings text-filter thresholds and per-app suppression readout load and match grouped
  SQL for the audited corpus.

### Broke / regressed / release blocker
- **Release blocker:** strict PR3 acceptance is not met. Default content search still has content FTS
  hits for static/app chrome terms (`Firefox` 24, `Steam` 24, `Deck` 68, `Recall` 42,
  `GPU Memory` 15) on the baseline DB. A fresh Notepad capture preserved the deliberate foreground
  content, but also indexed `Firefox`, `Deck`, `Recall`, and `COMMAND` in default `content_text`.
  See `docs/AUDIT_0.2.0_PR3_2026-06-26.md`, `06` patch #8, and `07` gap #64.

### Verbatim verification
Raw logs are preserved under `.playwright-mcp/pr3-2026-06-26/29-verify-ui-npm-ci-lint-build.txt`
through `34-verify-bindings-diff.txt`; the audit report includes the command output summary and
the exact evidence paths. All required commands exited 0:
`cd ui && npm ci && npm run lint && npm run build`, `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace`,
`cargo test --workspace`, and `git diff --exit-code -- ui/src/bindings`.

---


> Pre-0.2.x (v0.1.0) history archived in specs/archive/05_BUILD_REVIEW.v0.1.0.md.

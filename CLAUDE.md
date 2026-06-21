# CLAUDE.md — ScreenSearch V2c

Guidance for any AI agent (Claude Code) working in this repository.

## What this is
A standalone, **Windows-only**, local-first desktop app (Rust + Tauri 2) that captures the screen,
makes it searchable by text and meaning, and answers questions about it — fully on-device. This is
a **clean-slate** project; it shares no code or data with any prior version.

**Current state: specification complete, no application code yet.** The build starts at P0.

## ⛔ Read the spec before doing anything (mandatory order)
1. `specs/01_PROJECT_CONTEXT.md` — what is true today (env, constraints, non-goals)
2. `specs/02_STRATEGIC_PLAN.md` — what to build, in what phase order (P0→P5)
3. `specs/03_MASTER_PRODUCTION_SPEC.md` — exactly how (schema, traits, protocols, DoD)
4. `specs/UI_REFERENCE.md` — the frontend contract (identity, tokens, screens, states) — for P5
5. `specs/04_CLAUDE_CODE_BUILD_PROMPT.md` — how to operate (this is your operating manual)
Consult `specs/00_PROJECT_INTAKE.md` and `specs/MODEL_REGISTRY.md` for facts (models, license).

Re-read each session — the files are the source of truth, not your memory.

## Source of truth
- *Why / scope / phases* → `02` · *Constraints / non-goals* → `01` · *How (schema, traits, job
  queue, sidecar, settings, DoD)* → `03` · *UI* → `UI_REFERENCE.md` · *Exact model repos/quants*
  → `MODEL_REGISTRY.md` · *How to operate* → `04`.

## Hard rules (non-negotiable)
- **Stop at ambiguity.** Spec explicit → implement exactly. Spec silent → STOP, ask, append to
  `specs/07_KNOWN_GAPS.md`. Spec contradictory → STOP, ask, append to `specs/06_PATCH_PLAN.md`.
  Never guess a product decision to keep momentum.
- **Verbatim verification.** Never claim something works without pasting the raw output of the
  command (build / test / clippy / run). No paraphrase. "Done" = observed running, not "compiles."
- **No stubs / placeholders / hardcoded expected values** to make something look like it works.
  If blocked, stop and ask.
- **Windows-only by design** — use Windows-native APIs (WGC, WinRT OCR, WebView2); do not add
  cross-platform abstractions or stub them away.
- **Rust-only ML runtime.** The shipped app's ML is Rust-only — embeddings via fastembed,
  inference via the local llama.cpp sidecar; no Python *ML sidecar* in the runtime (the V1 approach
  that failed). Python is fine for build/dev tooling (model prep, the `hf` CLI, CI scripts).
  No cloud calls (localhost + model downloads only).
- **No real-time vision** — vision runs on-demand / timer / idle only (`03 §5`).
- **Sidecar must never orphan** — implement the Job-Object lifecycle exactly (`03 §6`); do not ship
  P4 until the no-orphan test passes.
- **Schema changes = forward-only migration** with a `schema_version` bump. No schema drift.
- **Branches, not main.** New work on a feature branch; no force-push; no commit to `main` without
  review. Never commit models, secrets, or DB files (see `.gitignore`).
- **UI:** typed IPC via `ts-rs` only; every view defines all states (loading/empty/error/partial/
  populated); Rules-of-Hooks is an error-level gate; tokens only (no hardcoded hex/font/spacing).

## Build/verify (once P0 has scaffolded the workspace)
- Rust: `cargo fmt --all -- --check` · `cargo clippy --workspace -- -D warnings` ·
  `cargo build` · `cargo test`
- UI: `cd ui && npm ci && npm run build`
- App: `cargo tauri dev` / `cargo tauri build`
Paste the verbatim output when reporting status.

## Build-loop notes (keep current)
Append your work record to `specs/05_BUILD_REVIEW.md`, `06_PATCH_PLAN.md`, `07_KNOWN_GAPS.md`,
`08_CHANGELOG_AI.md` as you go (`04 §7`).

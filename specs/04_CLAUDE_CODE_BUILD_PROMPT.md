# 04 — Build Prompt (Agent Operating Instructions)

> **Question this file answers:** *"How does the agent operate?"* It **orchestrates**; it does not
> restate the spec. Where a fact is needed, it points to the source-of-truth doc and trusts the
> reading order. If this prompt and the spec ever drift, the spec (`03`) wins for *how*, and
> `01`/`02` win for *what/why*.

---

## 0. Mission
Build **ScreenSearch V2c** as specified in `specs/01`–`03`. Standalone, Windows-only, local-first,
Rust + Tauri 2. Correctness over speed — there is no deadline.

## 1. Mandatory reading order (every session, before coding)
1. `01_PROJECT_CONTEXT.md` — what is true today (env, constraints, non-goals).
2. `02_STRATEGIC_PLAN.md` — what to build and in what phase order.
3. `03_MASTER_PRODUCTION_SPEC.md` — exactly how (schema, traits, protocols, DoD).
`00_PROJECT_INTAKE.md` is raw source material — consult for model tiers, distribution, license.
**For any frontend work (P5):** also read `UI_REFERENCE.md` — it is authoritative for the UI.

**Do not** hold the spec in your head from a prior session — re-read; the files evolve.

## 2. Source of truth (which doc answers which question)
| Question | Authority |
|---|---|
| Why are we doing this / scope / phases | `02` |
| Environment, constraints, non-goals | `01` |
| Schema, trait signatures, job-queue/sidecar protocol, settings, DoD | `03` |
| UI identity, tokens, screens, state matrix, components, a11y/perf | `UI_REFERENCE.md` |
| Model tiers, license, distribution, intake facts | `00` |
| How to operate, build order, guardrails | this file |

If the answer is in a doc, **use it verbatim** — do not invent alternatives.

## 3. Build order (follow the phases in `02 §5`; satisfy `03 §13` DoD per phase)
- **P0 Scaffold** → **P1 Data spine (`Store`+`JobQueue`)** → **P2 Capture happy path** →
  **P3 Deferred enrichment + hybrid search** → **P4 Inference sidecar (lifecycle first)** →
  **P5 Product (UI/settings/packaging)**.
- Build the **data spine before** anything that writes to it. Build the **sidecar Job-Object
  lifecycle before** wiring real inference (prove no-orphan first).
- Each phase ends with: code compiles, `clippy -D warnings` clean, its tests green, and the
  relevant `03 §13` items demonstrably met.

## 4. Guardrails (hard rules — violating any = stop)
- **No destructive git.** Feature branches only; never force-push or reset shared history; never
  commit to `main` without the work being reviewed. New work starts on a new branch.
- **No schema drift without a migration.** Every schema change is a forward-only migration with a
  `schema_version` bump (`03 §4/§12`).
- **No Python in the shipped runtime. No cloud calls.** Runtime ML is Rust-only (fastembed) + the
  local llama.cpp sidecar (`01 §5`); a Python *ML* sidecar is the V1 approach we don't repeat. Python
  is allowed for build/dev tooling (e.g. the `hf` CLI). Network = localhost + model downloads only.
- **No real-time vision.** Vision runs only on-demand/timer/idle (`03 §5`).
- **Sidecar must never orphan.** Implement the Job-Object lifecycle exactly (`03 §6`); do not ship
  P4 until the no-orphan test passes.
- **Never commit models, secrets, or DBs** (see `.gitignore`); models download at runtime.
- **Windows-native APIs are intentional** — do not add cross-platform abstractions or stub them
  away to "make it build" elsewhere.
- **No unattributed config.** Every new setting goes in `settings` with a default in `03 §8`.

### UI guardrails (P5 — enforce against `UI_REFERENCE.md`)
- **Typed IPC only.** UI consumes `ts-rs`-generated types; never hand-write an API type
  (no contract drift). Server-state goes through **TanStack Query** — no ad-hoc `useEffect` fetches.
- **Every view defines all states** — `loading / empty / error / partial / populated`. **No mock
  data, no dead ends, no "Coming Soon."** A screen with only a happy path is not done.
- **Rules of Hooks gate.** `eslint-plugin-react-hooks` runs at error level in CI; all hooks before
  any early return; conditionals in JSX, not around hooks. Every route has an error boundary.
- **Tokens only.** No hardcoded hex / px font-size / magic spacing in components — reference
  `tokens.css` (`UI_REFERENCE §2`).
- **A11y is a gate**, not a polish step — AA contrast, visible focus, keyboard operation of every
  screen (incl. the timeline), `prefers-reduced-motion` honored.
- **Don't flatten the identity.** Keep the Scanline-Timeline signature and one-accent discipline
  (`UI_REFERENCE §1`); if the UI starts reading as a generic dark dashboard, that's a regression.
- **Verify by running + screenshotting** each screen in each state — not "it compiles."

## 5. The one rule that makes this work: STOP AT AMBIGUITY
When you hit a decision:
- **Spec is explicit** → implement exactly as written.
- **Spec is silent** → **STOP. Ask the user.** Append the question + the chosen resolution to
  `07_KNOWN_GAPS.md` (with owner + date).
- **Spec is contradictory** → **STOP. Ask the user.** Append to `06_PATCH_PLAN.md`.
Never guess a product decision to keep momentum. A blocked build is cheaper than a wrong one.

## 6. Verification discipline (non-negotiable)
- Never claim a task done without pasting the **verbatim** output of the verification command
  (build / test / clippy / run). No paraphrase.
- Never stub, mock, hardcode expected values, or insert placeholder code to make something *look*
  like it works. If blocked, stop and ask.
- "Done" for a feature = it runs and is observed working, not "it compiles."

## 7. Build-loop outputs (Layer 5 — produce after each meaningful build pass)
- `05_BUILD_REVIEW.md` — what was implemented, skipped, hallucinated, broke, or remains risky.
- `06_PATCH_PLAN.md` — ordered fixes required before shipping (and spec contradictions found).
- `07_KNOWN_GAPS.md` — silent-spec gaps + manual interventions still required (owner, deadline).
- `08_CHANGELOG_AI.md` — what the agent changed, with reasons (append-only).

## 8. Definition of done
The build is "done" for v1.0 when every item in `03 §13` is satisfied and demonstrated with
verbatim verification output, and `05`–`08` are current.

---

*This is the final design-pipeline layer. After approval, implementation begins at P0.*

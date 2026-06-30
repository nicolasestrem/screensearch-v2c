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

## 2026-06-30 — PR #60 review fix: dev override truly hook-free in prod (`#43`)
- **Change:** `ui/src/lib/dev/useDevStateOverride.ts` now reads `window.location.search` inside the
  `import.meta.env.DEV` guard instead of calling `useSearchParams()` above it. Doc + CHANGELOG updated.
- **Why:** Codex/Gemini/Claude reviewers all caught that the pre-guard hook call survives tree-shaking
  (the helper is on the production `queries.ts` path), so release builds subscribed all 17 query
  consumers to router history and required a `<Router>` context. With no hook call, the production
  helper folds to `return result`. No Rules-of-Hooks concern (nothing conditional is a hook).
- **Verification:** `npm run lint` `EXIT 0`; `npm run build` `✓ built in 1.55s`;
  `grep -rl __devState dist/assets/` **absent**; `grep -rl "dev: forced route error" dist/assets/`
  **absent**; `git diff --stat -- ui/src/bindings` clean.

## 2026-06-30 — Fix vision context overflow on full-res frames (`fix/vision-fullres-ctx-overflow`)
- **Change:** (1) `crates/inference/src/vision.rs` — `encode_data_url` downscales the VLM request
  image to a 1568 px longest edge (`VISION_MAX_EDGE`) before JPEG-encoding; captures/timeline keep
  full resolution. `downscale_for_vlm` resizes the borrowed frame directly (no full-res clone — PR
  #61 Gemini review). (2) `crates/inference/src/models.rs` — vision auto-ctx **left at the spec
  default 4096**; an interim 4096 → 8192 bump was reverted after the PR #61 Codex P2 (it contradicted
  `03 §8`'s "not bumped by default" and added KV-cache VRAM on weak GPUs — the downscale already
  bounds the worst case to ~2.5 K < 4096 tokens). (3)
  `crates/kernel/src/worker_pool.rs` — `vision_tag` failure formats the error with `{e:#}` (full
  anyhow chain) into `jobs.last_error`. (4) `crates/inference/src/process.rs` (+ `supervisor.rs`
  `SupervisorConfig.sidecar_log`, `src-tauri/src/lib.rs`) — capture the sidecar's stdout/stderr to
  `<sidecar dir>/llama-server.log` via an inheritable log handle (only that handle is inheritable).
- **Why:** native full-res captures (`07` #73) made a 3440×1440 frame ~4148 vision tokens > the
  4096 ctx, so `llama-server` returned HTTP 400 `exceed_context_size_error` for every tag (DB: 72
  dead / 0 done). The low RAM/VRAM that looked like a "miracle" was the model rejecting requests in
  ~0.1 s without inference. The cause was invisible because the worker logged only the top context
  and the sidecar's stderr was discarded — both fixed here (`07` #74).
- **Verification:** TDD red→green per change (models ctx; vision downscale large/small; worker
  `{e:#}` end-to-end via a failing provider → `jobs.last_error` contains `exceed_context_size_error`;
  process.rs real-child stdout capture). Full gate all `EXIT 0`: `cargo fmt --check`,
  `clippy --workspace --all-targets -D warnings`, `build --workspace`, `test --workspace`
  (inference **98 passed**; kernel enrichment incl. the new chain test), `npm run lint` + `build`
  (`✓ built in 1.64s`), `git diff --exit-code -- ui/src/bindings` clean. **Live E2E:** rebuilt dev
  app → sidecar launches with `--ctx-size 8192`, `llama-server.log` written (`n_ctx_slot = 8192`),
  `vision_tag done` 0 → 8, and a faithful downscaled request returned **HTTP 200** in 2.8 s
  (`prompt_tokens 1159`) with `{"description":"…Visual Studio Code…","activity_type":"coding",
  "confidence":0.95}` — was HTTP 400 `4148 > 4096` before the fix.

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

## Pass — 2026-06-30 — Model-downloader resume hardening (`fix/download-resume-hardening`)

Closed two open durability gaps in `crates/inference/src/download.rs` (download-hardening scope, per
user), both via TDD. The user also asked to drop the stale `#74` row (its only residual was re-tagging
dead `vision_tag` jobs in a throwaway dev DB — a don't-care; resolution history stays in the PR #61
records).

### Implemented
- **#69 — wrong-sized `.part` no longer publishes garbage.** `open_preallocated` now reports a part as
  `unbacked` when its pre-existing on-disk length is `!= total` (brand-new, externally truncated, or
  corruption-grown larger), not just when it created the file; the chunked-download caller discards the
  stale `.parts` bitmap in that case and refetches every chunk. No false positives on a legitimate
  resume — a real interrupted part is always preallocated to exactly `total`. Wrote
  `truncated_part_discards_stale_partial_manifest` first (observed red: published file all-zeros), then
  the fix (green); `oversized_part_discards_stale_manifest` covers the `> total` case (the `< total`→
  `!= total` broadening came from a PR #62 review note).
- **Cache re-check on lock retry (PR #27 Codex-P2).** Extracted the clean-layout + HF-cache fast paths
  into `place_if_cached`; folded the single-stream lock-retry into `fetch_one` so each `LockAcquisition`
  backoff re-checks the cache and short-circuits if the holder finished mid-sleep. Extended the backoff
  (`LOCK_RETRY_BACKOFF_CAP` 15 s; `LOCK_RETRY_MAX_ATTEMPTS` 5→24 ≈ 5 min) so a real multi-GB download is
  outlasted, not abandoned at ~20 s. Added `place_if_cached_*` unit tests. The doc-hidden
  `download_file_with_lock_retry_for_diagnostics` (the `examples/repro_8b.rs` entry point) keeps a
  minimal inline backoff loop.

### Verification (Windows, after the PR #62 review fix) — verbatim
- `cargo test -p inference --lib` → `test result: ok. 102 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.05s`
- `cargo fmt --all -- --check` → `EXIT 0`
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile … in 2.43s` / `EXIT 0`
- `cargo build --workspace` → `Finished dev profile … in 8.90s` / `EXIT 0`
- `cargo test --workspace` → every suite `ok` (store 49+14; inference 102 lib + integration; traits 53;
  uia 16/2-ignored; sysmon 11; textfilter 12; kernel; screensearch 7; e2e/perf/smoke ignored) / `EXIT 0`
- `git diff --exit-code -- ui/src/bindings` → clean (`EXIT 0`)

### Skipped / deferred
- The `#46` row proper (orphaned **detached** chunk writers in the single-stream hf-hub fallback) — left
  open; the real fix replaces hf-hub's high-level `download_with_progress`, out of this scope.
- "Gemini — single-instance focus" PR #27 follow-up — out of the download-hardening scope; still open.
- **Declined a PR #62 review note** (Gemini, medium): wrap `place_if_cached` in `tokio::task::spawn_blocking`.
  `place_if_cached` is a verbatim extraction of code that shipped throughout 0.2.x; its only heavy op (the
  multi-GB copy) is already offloaded via `place_in_clean_layout_async` (which exists precisely for that),
  and the rest are stat-level (`exists`/`metadata`/`Cache::get`) — the same inline-stat pattern used across
  `fetch_one`/`chunked_download`. Gemini's rewrite would re-implement that offload inline and diverge from
  the established pattern for no measurable gain. The sibling note (broaden the size check to `!= total`) was
  **applied**.

### Still risky
- The lock-retry / `place_if_cached` HF-cache branch can't be unit-tested portably (a valid hf-hub cache
  layout needs symlinked snapshots/blobs; Windows-restricted). It's an unchanged extraction of
  already-shipped working code; the dest-present and miss branches *are* unit-tested. The
  `LockAcquisition` path itself is network/contention-dependent and exercised by `examples/repro_8b.rs`,
  not CI.

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

## Pass — 2026-06-30 — Fix vision context overflow on full-res frames (`fix/vision-fullres-ctx-overflow`)

User-reported symptom: vision tagging "failing for the first time" and "significantly slower," while
the model appeared to use "merely 3 GB RAM" (a "miracle?"). A live read of the running dev session
proved it was **not** a memory win — it was a regression.

### Diagnosis (evidence)
- The sidecar was fully GPU-resident (6.4 GB dedicated VRAM on the RTX 5060 Ti; `-ngl 99`,
  `--ctx-size 4096`), so the low footprint was the existing memory-tuning, not magic.
- A faithful reproduction (real captured frame → JPEG q80 → exact request body) returned
  `HTTP 400 — {"error":{"message":"request (4148 tokens) exceeds the available context size (4096
  tokens)…","type":"exceed_context_size_error","n_prompt_tokens":4148,"n_ctx":4096}}`. Frame
  3440×1440 (native, from #73). The DB had `vision_tag` **72 dead / 0 done**.
- The error was hidden: the worker recorded only the collapsed top context (`"vision completion"`),
  and the sidecar's stderr was discarded entirely (`process.rs` spawned with `CREATE_NO_WINDOW`,
  no redirect).

### Implemented (`07` #74; TDD red→green per change)
- **Downscale the VLM image** to a 1568 px longest edge in `vision::encode_data_url` (captures keep
  full resolution). Tests: oversized 3440×1440 → 1568×656; small frame passes through unscaled.
- **Vision auto-ctx left at the spec default 4096** (`models.rs`). _(An initial 4096 → 8192 "safety
  net" bump was reverted after the PR #61 Codex review — see the follow-up below.)_
- **Surface the real cause:** `vision_tag_outcome` formats with `{e:#}` (`worker_pool.rs`). Test:
  a failing provider's chained error now lands `exceed_context_size_error` in `jobs.last_error`
  (`kernel --test enrichment`).
- **Capture sidecar stdout/stderr** to `<sidecar dir>/llama-server.log` (`process.rs` inheritable
  log handle + `SupervisorConfig.sidecar_log`, wired in `src-tauri/src/lib.rs`). Test: a real child
  (`cmd /c echo …`) writes to the log and is read back.

### Verification (Windows, full CI sequence) — verbatim
- `cargo fmt --all -- --check` → `FMT_EXIT=0`
- `cargo clippy --workspace --all-targets -- -D warnings` → `CLIPPY_EXIT=0`
- `cargo build --workspace` → `Finished … in 27.08s` / `BUILD_EXIT=0`
- `cargo test --workspace` → `TEST_EXIT=0` (inference **98 passed** incl. new vision/process tests;
  `kernel --test enrichment` incl. `vision_tag_failure_records_full_error_chain`)
- `cd ui && npm run lint` → `EXIT 0`; `npm run build` → `✓ built in 1.64s`
- `git diff --exit-code -- ui/src/bindings` → `BINDINGS_DIFF_EXIT=0`
- **Live E2E** (`npm run tauri dev`, new binary): this run was on the interim 8192 build, so the
  sidecar launched `--ctx-size 8192` (`n_ctx_slot = 8192` in `llama-server.log`); `vision_tag done`
  0 → 8; faithful downscaled request → **HTTP 200** in 2.8 s, `prompt_tokens 1159`, content
  `{"description":"…Visual Studio Code…","activity_type":"coding","confidence":0.95}`. The measured
  **1159 prompt tokens** is what matters here: it sits far under the reverted **4096** default, so the
  downscale alone clears the overflow without the ctx bump.

### Still risky / follow-up
- The ~115 `vision_tag` rows that dead-lettered **before** the fix stay dead (terminal state) — a
  manual requeue is needed to re-tag those frames (`07` #74 residual).
- 1568 px lands ~1009 image tokens — right at the model's logged ≥1024 grounding recommendation;
  fine for holistic tagging. The worst case (a square frame) is ≤ 1568×1568 ≈ 2.46 MP → ~2.5 K
  prompt tokens, still under 4096; `sidecar.ctx_size` remains the power-user knob for more headroom.

### Follow-up — PR #61 review fix (drop the per-frame full-res clone)
Gemini flagged `downscale_for_vlm` (`vision.rs`) cloning the whole `RgbaImage` (~20 MB for a
3440×1440 capture) on every call, then discarding the clone during resize. Fixed by checking the
dimensions on the borrowed frame and, when it overflows, calling `image::imageops::resize(img, …)`
directly on the reference — no clone on the (common, ultra-wide) resize path; the pass-through
branch still clones the already-small frame. The fitted dimensions reuse the same round-to-nearest
scale `DynamicImage::resize` applied (`ratio = VISION_MAX_EDGE / longest edge`), so the cap math is
byte-for-byte identical and the two existing tests stay green unchanged.
- **Verification (verbatim):** `cargo fmt --all -- --check` → `FMT_EXIT=0`;
  `cargo clippy -p inference --all-targets -- -D warnings` → `CLIPPY_EXIT=0`; `cargo test -p
  inference --lib` → `98 passed; 0 failed` (incl. `downscales_oversized_frame_to_max_edge`
  3440×1440 → 1568×656 and `small_frame_passes_through_at_native_size`).

### Follow-up — PR #61 review fix (revert the vision auto-ctx bump; Codex P2)
Codex flagged that bumping the vision auto-context 4096 → 8192 contradicts the spec contract
(`03 §8:438` vision auto = 4096; `§:522` "`sidecar.ctx_size` … **not** bumped by default") and
raises KV-cache VRAM on weak GPUs for *every* tag — and since this PR already downscales the request
image, the bump is unnecessary. Reverted `default_ctx_for(ModelLane::Vision)` back to **4096**
(`models.rs`) and restored the per-lane test assertion (`vision auto ctx == 4096`; answer stays
8192). The downscale alone fixes the overflow: the 1568 px cap bounds the worst case (a square
frame) to ~2.5 K prompt tokens < 4096, and the live run measured only 1159. `sidecar.ctx_size`
remains the documented power-user VRAM knob. Doc/CHANGELOG updated to drop the bump.
- **Verification (verbatim):** `cargo fmt --all -- --check` → `FMT_EXIT=0`;
  `cargo clippy -p inference --all-targets -- -D warnings` → `CLIPPY_EXIT=0`; `cargo test -p
  inference --lib` → `98 passed; 0 failed` (incl. `auto_ctx_size_resolves_per_lane_and_override_passes_through`).

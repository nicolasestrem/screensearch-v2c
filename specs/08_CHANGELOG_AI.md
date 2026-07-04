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

## 2026-07-04 — 0.3.0 PR9: integration audit + release (`feat/pr9-audit-release`)
- **Change:** The arc-closing audit + release PR — no feature code; docs, audit evidence, version,
  and release mechanics only.
  - **Audit:** every `03 §13b.1–.7` acceptance line verified end-to-end on a real Windows desktop
    (fresh profile — none existed): grep gates; event-mode live-fire (foreground/idle/timer
    fallback); all four (lane, tier) model pairs downloaded + loaded + used (real vision tags at
    0.95 confidence, real SSE ask with citations); overlay latency 6–9 ms visible / 17–23 ms
    input-ready vs the 150 ms bar; D7 self-exclusion proven at OS level (WDA — the overlay is
    invisible even to external capture) + 0 hits for overlay-only text across stored frame text;
    the D6 conflict path demonstrated against a *real* pre-existing `Ctrl+Alt+M` holder;
    where-was-i correct-run + honest-empty; D8 static-screen mark at +135 ms; capture-off /
    excluded-app / own-window honest refusals; D10 "Text kept" after retention purge; the full
    API curl matrix (401s, SSE + disconnect-cancel, loud port conflict + guided retry, live port
    change, regenerate, export-with-API-off, exit-frees-port, no orphaned sidecar); MCP stdio
    session + API-off/wrong-token guided errors + a real `claude mcp add` client round-trip;
    warn-once settings migrations re-proven on seeded boots. Tracked record: `05` PR9 pass; raw
    transcript: local-only `docs/audits/AUDIT_0.3.0_PR9_2026-07-04.md`.
  - **Docs:** `03 §2` crate tree → 13 crates (`06` #20 ✅); `docs/ARCHITECTURE.md` current to PR8
    (13-crate map, v1→v10, new §9b, command list, 0.3.0 settings keys) + `README.md` +
    `CLAUDE.md`/`AGENTS.md` swept (`07` #81 ✅); `docs/TESTING.md` PR4 section annotated with the
    0.2.x-waiver; `05` PR5 pass backfilled (labeled).
  - **Gaps:** new `07` #91 (monitor hot-unplug stalls capture silently; bounded, post-0.3.0) and
    #92 (populated-0.2.x live-check waiver, user decision).
  - **Release:** CHANGELOG cut to `[0.3.0] — 2026-07-04` with **removals first** + an Audited
    section; version bump 0.2.2 → 0.3.0 across the four manifests + lockfiles + `docs/API.md`
    example; 0.3.0 NSIS build with `screensearch-mcp.exe` inclusion verified; `04 §7` archival
    sweep folded into this PR (user decision): live `05`–`08` + CHANGELOG history →
    `specs/archive/*.v0.3.0.md` + `CHANGELOG-ARCHIVE.md`.
- **Why:** `03 §13b.8` + `docs/0.3.0.md` PR9 — the audit is the arc's definition of done; the
  release mechanics follow `04 §7`.
- **Verification:** baseline suite ALL GREEN on the untouched tip `6a46dc0` and re-run green on the
  final branch tip (fmt --check / clippy -D warnings / build / test 0 failed / ui lint+build /
  bindings diff clean — verbatim in `05` and `docs/audits/pr9-baseline-suite.log`); perf fixture
  `10000 frames: median 27.26 ms, p95 67.51 ms — 1 passed, 0 failed`.

## 2026-07-04 — 0.3.0 PR8: MCP server (`feat/pr8-mcp-server`)
- **Change:** Added `crates/mcp` → `screensearch-mcp.exe`, a stdio MCP server wrapping the PR7 local API.
  - `crates/mcp` (lib + bin): hand-rolled newline-delimited JSON-RPC 2.0 (`rpc`), config resolution
    (`config`, flag>env>default), an `ApiClient` over `127.0.0.1:<port>` with guided `ApiFailure` mapping
    (`client`), SSE line-buffer + `AnswerDelta` aggregator (`sse`), method dispatch + version negotiation
    (`server`), and the six tools (`tools`): `search_screen_history`, `ask_screen_history`, `get_moment`
    (+`include_image`), `where_was_i`, `list_marks`, `add_mark`. Deps: reqwest/tokio/serde_json/
    futures-util/base64 only — **no axum, no store, no app crate** (D13); `api`/`store`/`traits` are
    test-only dev-deps.
  - Packaging: `bundle.externalBin: ["binaries/screensearch-mcp"]` + `scripts/stage-mcp.mjs` (builds
    `-p mcp --release`, stages `screensearch-mcp-<host-triple>.exe`), wired into `beforeDevCommand`/
    `beforeBuildCommand` + a CI step; `src-tauri/binaries/` git-ignored; `npm run stage:mcp`.
  - Docs: `docs/MCP.md` (client config + threat model + tool table + troubleshooting); `docs/TESTING.md`
    §PR8 manual acceptance; `CHANGELOG.md`; fresh-clone staging note in `CLAUDE.md`/`AGENTS.md`.
- **Why:** `03 §7c` (MCP server, D13) + `§13b.7` DoD; `docs/0.3.0.md` PR8. Meets the agent ecosystem where
  it is without moving a byte off-device — the open-source audience's "no API, no automations" ask.
- **Verification:** full suite green — `cargo fmt --all -- --check` (exit 0), `cargo clippy --workspace
  --all-targets -- -D warnings` (clean), `cargo build --workspace` (Finished), `cargo test --workspace`
  (**494 passed, 0 failed**; 30 mcp unit + 19 spawned-exe stdio integration tests), `ui` lint+build,
  `git diff --exit-code -- ui/src/bindings` (exit 0, no bindings touched). Live cross-process: the exe
  driven over scripted stdio against the real API on `127.0.0.1:43210` round-tripped every tool. externalBin
  failure mode confirmed (`cargo check -p screensearch` exit 101 with the sidecar removed, 0 restored).
  `npm run tauri build` produced `ScreenSearch_0.2.2_x64-setup.exe`; `7z l` shows both `screensearch.exe`
  and `screensearch-mcp.exe`. No schema change (v10); protocol-underspecification resolution recorded
  (`07` #90), stale `03 §2` crate tree flagged for PR9 (`06` #20).

## 2026-07-04 — 0.3.0 PR7: local HTTP API + export (`feat/pr7-local-api`)
- **Change:** Added the opt-in local HTTP API and streaming JSON export, end to end.
  - `crates/traits`: `ApiStatus`/`ExportRequest`/`ExportResult` ts-rs types (+ `no_bigint` guards,
    committed bindings) and a Serialize-only `ExportFrameRow` in `domain`. Token lives on `ApiStatus`,
    never the `Settings` struct.
  - `crates/store`: inherent `export_frames_page(after_id, from, to, limit)` — keyset cursor,
    `frames LEFT JOIN frame_text`, half-open window, connection released per page. 4 tests.
  - `crates/inference`: fixed the SSE cancellation leak (`§7c`) — `AnswerSidecar::run`'s consume loop
    extracted to `pump_deltas` with a `tokio::select!` on `tx.closed()` + an `AbortOnDrop` guard on the
    detached stream task; a dropped receiver (SSE disconnect / `cancel_ask`) now aborts the sidecar so
    generation actually stops. 2 tests.
  - `crates/api` (new crate, depends on `traits` only): `ApiHost` trait seam; `ApiServer`
    (`127.0.0.1`-only by construction — `BIND_IP` const, no address param; bind-before-spawn;
    graceful 3s-then-abort stop); bearer-auth middleware on every route (constant-time compare, live
    `RwLock` token); `ApiError`/`ErrorBody`; routes health/search/frames(+image)/where-was-i/marks/
    ask(SSE)/export(streamed); one export code path shared by `GET /v1/export` and `export_to_file`
    (`.partial`+rename to Downloads). axum 0.8 the sole new dep. 14 tests + an `#[ignore]` curl harness.
  - `src-tauri`: `local_api.rs` — `TauriApiHost` over the concrete store/kernel + existing helpers;
    `ApiRuntime` slot + live token in `AppState`; `apply_api_config` (persist `api.*` KV, token-on-
    first-enable, restart, loud bind-failure state); 4 commands; autostart on boot; server stopped
    first on exit. 4 tests. Added `anyhow` + `api` deps.
  - `ui`: `ApiPanel` (five states, threat-model copy, token reveal/copy/regenerate, port-in-use
    retry) + a Data export button (Downloads, works with the API off); `apiStatus` query/key + 3
    mutations + 4 command wrappers; fixed the stale "backend never emits toast" comments.
  - Docs: `docs/API.md` (new); `docs/TESTING.md` PR7 manual-acceptance; `CHANGELOG.md`; `05`/`06`/`07`
    (#88 export-destination decision, #89 v1 residuals, #80 shipped); `UI_REFERENCE §5` `ExportPanel`.
- **Why:** 0.3.0 PR7 (`docs/0.3.0.md` Part III; `03 §7c`/`§7`/`§8`/`§13b.6`; D11/D12). Turns the app
  into a local platform (the open-source ask) without moving a byte off the machine — reusing hybrid
  search, the ask pipeline, where-was-i, and marks; no new retrieval code.
- **Verification:** `cargo fmt --check` · `cargo clippy --workspace --all-targets -D warnings` ·
  `cargo build --workspace` · `cargo test --workspace` (all green; new: 4 store + 2 inference + 14 api
  + 4 local_api + 3 ts-rs guards) · `ui` `npm run lint && npm run build` · bindings diff clean. **Live:**
  a real `ApiServer` over the fixture store on `127.0.0.1:43210`, exercised by external `curl` —
  401/401/200 auth, every endpoint round-tripped, export valid JSON (frames + content text + marks, no
  images), `format=csv`→400, unknown frame→404, and a second bind on 43210 → `AddrInUse` (the loud
  port-conflict path). Verbatim in `05`.

## 2026-07-04 — 0.3.0 PR7 review fixes (PR #76; Gemini + Codex)
- **Change:** Addressed the five applicable inline bot suggestions on the open PR (bots not replied to).
  - `crates/api/src/extract.rs` (new): `ApiQuery`/`ApiPath`/`ApiJson` wrappers that map axum's
    stock extractor rejections to `ApiError::BadRequest`, so malformed query/body/path values stay on
    the `{error,message}` JSON contract (Codex P2). Wired into every route (`routes.rs`, `export.rs`);
    2 integration assertions strengthened (missing `q`, non-integer frame id → `bad_request` JSON).
  - `ui/.../ApiPanel.tsx`: a "Restart on {port}" affordance when the drafted port differs from the
    live one, so `api.port` is configurable while running (Codex P2); `parsedPort` now clamps to
    `1024..=65535` (Gemini) so a stray value can't fail `u16` IPC deserialization.
  - `crates/api/src/export.rs`: `export_to_file` removes the `.partial` file if `flush`/`rename`
    fails (Gemini) — a failed export never leaves a plausible file.
  - `src-tauri/src/local_api.rs`: `frame_image` maps a `NotFound` file read to `Ok(None)` → clean
    404 instead of 500 (Gemini).
- **Why:** review hardening on the open PR #76; no behavior change for well-formed requests, no schema
  or ts-rs type change (bindings untouched).
- **Verification:** `cargo fmt --check` · `cargo clippy --workspace --all-targets -D warnings` (exit 0)
  · `cargo test --workspace` (all green; `api` http_api 12 passed/1 ignored, `inference` 104, `store`
  61) · `ui` `npm run lint && npm run build` · `git diff --exit-code -- ui/src/bindings` clean. Verbatim
  in `05`.

## 2026-07-04 — 0.3.0 PR7 review fixes, round 2 (PR #76; Gemini + Codex)
- **Change:** Seven more inline suggestions from a later bot pass (bots not replied to).
  - `crates/api/src/export.rs`: extracted `drain_to_file` (owns + drops the file handle on return) so
    `export_to_file` can delete the `.partial` after the handle is closed — the round-1 cleanup failed
    on **Windows** (sharing violation) with the write handle still open (Gemini HIGH). Added
    `validate_window` (`from ≤ to` → 400) on the `/v1/export` route.
  - `src-tauri/src/local_api.rs`: `ApiRuntime.config_lock: tokio::Mutex<()>` held across the whole
    `apply_api_config` transition, so overlapping enable/disable can't leave the API running against a
    disabled intent (Codex P2). `export_data` command rejects `from > to`.
  - `crates/api/src/lib.rs`: `build_router` gains a `.fallback(routes::not_found)` (before the auth
    layer) so unknown paths return `{error:"not_found"}` not axum's plaintext 404 (Codex P2);
    `build_search_query` rejects `from > to` (Gemini).
  - `crates/inference/src/answer.rs`: `emit_segment` returns whether the channel is still open;
    `pump_deltas` returns `Cancelled` on the first failed send, so a disconnected consumer stops the
    pump (and aborts the sidecar) immediately instead of draining the backlog (Gemini). Completed path
    unchanged.
  - Tests: `unknown_route_is_json_404`, `inverted_time_range_is_400`; docs API.md/CHANGELOG/05.
- **Why:** review hardening on the open PR; the Windows partial-cleanup bug is the notable one (this is
  a Windows-only app). No schema or ts-rs type change (bindings untouched).
- **Verification:** `cargo fmt --check` · `cargo clippy --workspace --all-targets -D warnings` (exit 0)
  · `cargo test --workspace` (all green; `api` http_api **14 passed**/1 ignored, `inference` 104,
  `store` 61) · `git diff --exit-code -- ui/src/bindings` clean. Verbatim in `05`.

## 2026-07-04 — 0.3.0 PR7 review fixes, round 3 (PR #76; claude + Codex)
- **Change:** Three more inline suggestions (bots not replied to).
  - `src-tauri/src/local_api.rs`: `apply_api_config` now writes `runtime.token` **only when empty**
    (first enable / autostart) so a concurrent `regenerate_api_token` can't be clobbered by a stale
    DB read (claude bot token race). It also returns `Result<ApiStatus, String>` — persistence
    (`api.port`/`api.enabled`/`api.token`) happens before the server is touched and a failed write is
    an `Err`, so a disable can't stop the server while leaving `enabled=true` on disk (Codex P2);
    `set_api_config` propagates the `Err`, `autostart` logs it, the 4 unit tests unwrap the `Result`.
  - `crates/api/src/export.rs`: `export_to_file` filename gains a 6-hex CSPRNG suffix
    (`rand_suffix`) so two same-second exports don't collide on the final/`.partial` path — a
    Windows `rename`-over-existing failure (Codex P2).
- **Why:** review hardening on the open PR; concurrency + Windows-filesystem correctness. No schema or
  ts-rs type change (bindings untouched).
- **Verification:** `cargo fmt --check` · `cargo clippy --workspace --all-targets -D warnings` (exit 0)
  · `cargo test --workspace` (all green; `api` http_api 14/1 ignored, `inference` 104, `store` 61,
  `screensearch_lib` 12) · `git diff --exit-code -- ui/src/bindings` clean. Verbatim in `05`.

## 2026-07-04 — 0.3.0 PR6: where-was-i + mark-this-moment (`pr6-where-was-i-and-marks`)
- **Change:** Added the flow-recall core — a where-was-i heuristic and mark-this-moment — end to end.
  - `crates/traits`: `FrameContextRow`, `CaptureNowRequest`/`CaptureNowReceiver`, `CapturedFrame.demanded`;
    `ResumeContext`/`Mark`/`MarkToast` ts-rs types; `Settings.resume_min_dwell_secs` (120) +
    `marks_hotkey` ("Ctrl+Alt+M"); `Store` methods `insert_mark`/`list_marks`/`resolve_mark`/
    `set_mark_note`/`recent_frame_contexts` (default bodies); moved the pure `is_excluded` matcher to
    `traits::privacy` so the kernel reuses it without depending on `capture`.
  - `crates/store`: migration **v10** (`marks` table + `idx_marks_open`, `LATEST_SCHEMA_VERSION 9→10`);
    `marks.rs` (CRUD with a clear FK-miss error, idempotent resolve, canonical list order);
    `recent_frame_contexts` query. Populated-DB migration test (mirrors the v9 pattern) + marks CRUD/
    ordering + purge-survival tests + a `recent_frame_contexts` ordering test.
  - `crates/kernel`: `resume.rs` — a pure `last_sustained_context` heuristic (context key = app_hint +
    browser domain; transient-excursion absorption using per-key presence-span; anchor = last non-self
    context; excludes anchor/ScreenSearch/excluded-apps) with a 15-case fixture suite, plus the
    `where_was_i(store, settings)` convenience. `capture_now` (serialized by a gate, two-timeout ack →
    frame-id, honest failures) + `add_mark`; `CaptureFactory` gains the demand receiver; the capture
    loop returns the demanded frame's id via a `pending_demand` slot. Settings load/save/sanitize for
    the two new keys + tests.
  - `crates/capture`: per-monitor diff-gate bypass (`CaptureRequest.bypass_for`) with a static-screen
    frame-pool recreate so a demanded frame is never dropped; the `capture_now` demand seam in
    `next_frame` (a `Wake` enum racing the timer/event wait against the demand channel; privacy gates
    still apply, denials acked honestly); `select_target_monitor` (foreground monitor → primary → first);
    a pure `diff::gate_passes` helper. Unit tests for the gate, target-monitor selection.
  - `src-tauri`: `overlay.rs` gains the `marks.hotkey` registration (mirrors the overlay hotkey, loud D6
    failure), a non-focus-stealing `show_mark_toast` (`set_focusable(false)` → show without focus), and
    `focus_overlay_for_note`/`dismiss_mark_toast`. New commands `where_was_i`/`add_mark`/`list_marks`/
    `resolve_mark`/`set_mark_note` (mutations emit `marks_changed`); setup registers the marks hotkey,
    `set_settings` reregisters it. Kernel-level mark tests (demanded frame + mark; capture-off honest
    failure; denial propagates; mark-by-frame-id).
  - `ui`: IPC wrappers/queries/mutations/events for all of the above (`marks_changed` invalidates the
    strip cross-window); `OverlayRuntime` flow|mark view + `MarkToast` (optional note, ~6s auto-dismiss
    paused while typing); overlay empty state → `WhereWasIStrip` (+ Enter jumps to the resume frame);
    Deck `WhereWasICard` + `IntentionsStrip` (open/done/dismiss, no badge counts); Settings mark hotkey
    + dwell fields; `IconMark`.
  - Docs: `03 §7` gains the `set_mark_note` row + `§7b` note; `05`/`06`/`07`; `docs/TESTING.md` PR6
    manual-acceptance section; `CHANGELOG.md`.
- **Why:** 0.3.0 PR6 (`docs/0.3.0.md` Part II; `03 §7b`/`§4`/`§7`/`§13b.5`; D8/D9/D10/D14/D15). The
  ADHD core of the arc — pull-based recall that reuses the store, capture pipeline, and overlay with no
  new subsystem.
- **Verification:** `cargo fmt --check` · `cargo clippy --workspace --all-targets -D warnings` ·
  `cargo build --workspace` · `cargo test --workspace` (all green; new: 15 resume + v10 migration +
  marks CRUD + diff/target-monitor + 4 kernel mark tests) · `ui` `npm run lint && npm run build` ·
  bindings diff clean. Live desktop checks per `docs/TESTING.md` PR6.

## 2026-07-03 — 0.3.0 PR5: Flow overlay (`feat/pr5-flow-overlay`)
- **Change:** Added the PR5 Flow overlay shell, IPC, UI, settings surface, and capture-self-exclusion
  tests.
  - `src-tauri`: added a hidden pre-created `overlay` window (`overlay.html`) with `contentProtected`,
    `alwaysOnTop`, no decorations, and no taskbar entry; added `tauri-plugin-global-shortcut`; added
    `overlay.rs` for hotkey registration/status, conflict toasts, show/hide/toggle, foreground-monitor
    placement, `overlay_shown`/`overlay_hidden`, and `open_moment` routing to the main window.
  - `crates/capture`: factored the own-process foreground predicate so the existing self-exclude gate
    explicitly covers the overlay window as well as the main app window.
  - `crates/traits` / `kernel`: added `overlay.hotkey`, `overlay.max_results`, `HotkeyStatus`, and
    `OpenMoment`; settings load/save sanitizes the hotkey and clamps top-N results to `1..=50`.
  - `ui`: added the separate Vite overlay entry, router-free provider shell, Search/Ask overlay UI,
    keyboard navigation (`Esc`, `Tab`, arrows, `Enter`, `?` Ask prefix), lazy Ask streaming, reusable
    highlighted snippets/citation open callbacks, Settings hotkey recorder, conflict warning, and result
    count control.
  - Docs: updated README, `docs/ARCHITECTURE.md`, `docs/TESTING.md`, `CHANGELOG.md`, `03`, `05`, `07`,
    and this log.
- **Why:** `docs/0.3.0.md` PR5 + `03 §7b`/`§8`/`§13b.4`, D6/D7 — instant recall needs a summonable
  overlay that does not become part of the user's captured history; hotkey conflicts must be visible,
  not silent.
- **Verification:** targeted checkpoint verification passed before the implementation commit:

```text
> screensearch-ui@0.2.2 typecheck
> tsc --noEmit
```

```text
> screensearch-ui@0.2.2 lint
> eslint .
```

```text
> screensearch-ui@0.2.2 build
> tsc --noEmit && vite build

vite v6.4.3 building for production...
transforming...
✓ 418 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html ... 0.71 kB gzip 0.34
dist/overlay.html ... 0.99 kB gzip 0.42
...
✓ built in 1.55s
```

```text
running 1 test
test overlay_window_is_precreated_hidden_and_capture_protected ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

```text
running 7 tests
test privacy::tests::empty_excluded_entry_never_matches ... ok
test privacy::tests::own_window_pid_matches_any_nonzero_own_process_window ... ok
test privacy::tests::own_window_pid_rejects_unknown_foreground_pid ... ok
test privacy::tests::matches_process_name_case_insensitively ... ok
test privacy::tests::allows_unrelated_apps ... ok
test privacy::tests::own_window_pid_rejects_foreign_process ... ok
test privacy::tests::matches_window_title ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 19 filtered out; finished in 0.00s
```

```text
running 55 tests
...
test result: ok. 55 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full CI-order verification is run after the docs pass and recorded in the final delivery notes.

## 2026-07-03 — 0.3.0 PR4: image-embedding lane removal (`feat/pr4-image-lane-removal`)
- **Change:** Removed the dark-launched, flag-off **nomic-embed-vision image-embedding lane**, with a
  forward-only **schema v8 → v9** migration (D5/D15).
  - `crates/store/src/schema.rs`: `LATEST_SCHEMA_VERSION` 8 → 9; new `MIGRATION_V9`
    (`DROP TABLE image_embedding_vectors; DROP TABLE image_embeddings; DELETE FROM jobs WHERE
    kind = 'embed_image';`). `MIGRATION_V1` left frozen (append-only doctrine) — a fresh DB creates the
    image tables in V1 and drops them in V9, ending at the same schema as an upgraded DB. **No** `jobs.kind`
    CHECK, **no** jobs rebuild (`07` #82).
  - `crates/store/src/lib.rs`: two populated-DB migration tests patterned on the v6 frames-rebuild test —
    `migration_v9_drops_image_lane_and_embed_image_jobs` (seed a frame with **both** embedding lanes + a
    mixed jobs queue; assert the image tables/trigger/vec0-shadows are gone, `embed_image` jobs deleted in
    every state, the text lane + other job kinds + FK integrity survive) and
    `fresh_and_migrated_schemas_agree_at_latest` (compare `sqlite_master` object+DDL sets between a fresh
    bootstrap and a v8→v9 upgrade — `03 §13b.3`). Updated the v6 test's `version == 8` literal to `9`.
  - `crates/traits`: dropped `EmbeddingProvider::embed_image` + `image_model_name`,
    `Store::upsert_image_embedding`, `JobKind::EmbedImage`, and `Settings.enrich_image_embeddings`.
  - `crates/embeddings`: `FastEmbedProvider` is text-only (`new(cache_dir)`, no `with_image`/image lane/
    `IMAGE_MODEL_NAME`). `Cargo.toml` drops the `image` dep; root `Cargo.toml` sets fastembed
    `default-features = false` (drops the `image-models` default feature — verified against fastembed
    5.17.2's manifest; keeps only the ORT + hf-hub download stack).
  - `crates/store`: removed `upsert_image_embedding` / `nearest_image_frames` / `image_embedding_count`
    and the `EmbedImage` token arms; `knn_frames` (shared with text) stays. Hybrid search already had no
    image arm — untouched.
  - `crates/kernel`: removed the `EmbedImage` capture-enqueue arm, the worker-pool claim/dispatch arm +
    `embed_image_outcome`, `Shared.enable_embed_image`; the throttle now pauses `vision_tag` only (the
    level-2 `embed_text` floor gate is unchanged). Added `enrich.image_embeddings` to
    `RETIRED_SETTINGS_KEYS` (reuses PR2's `drop_retired_settings` drop-once mechanism). Redesigned the
    throttle integration test around `vision_tag`, preserving the L1-pause / recovery / disabled-inert
    proof shape; pruned the two `embed_image` enrichment tests.
  - `src-tauri`: removed the `embedder_with_image` flag + the `needs_image_embedder` reload branch;
    `init_embeddings` gates on `enrich_embed_text` only and calls `FastEmbedProvider::new(models_dir)`.
  - UI: deleted the "Embed images" Settings toggle + the dead `embed_image` job-completion arm; ts-rs
    regenerated `ui/src/bindings/Settings.ts` + `JobKind.ts`.
  - Docs: `README.md`, `docs/ARCHITECTURE.md` (§5.2/§5.3/data-flow/settings), `docs/TESTING.md` (new PR4
    manual-acceptance section), `CHANGELOG.md`; `specs/02 §4` diagram caption fixed
    (`text+image vectors` → `text vectors`, recorded in `06`); `07` #81 image-lane doc sweep marked done.
- **Why:** `docs/0.3.0.md` PR4 + `02 §5c` / `03 §4`/`§13b.3` — a flag-off lane is pure carrying cost
  (a second vec0 table, a second model download, code that rots untested); text embeddings + vision tags
  cover semantic reach. D5 (drop derived vectors + dead jobs), D15 (+1 schema bump with a populated-DB
  test) settled in the roadmap.
- **Verification:** `cargo fmt --check` / `clippy --workspace --all-targets -D warnings` /
  `build --workspace` / `test --workspace` all exit 0 (store migration tests 5/5 incl. the 2 new; full
  workspace 0 failed); `npm run lint` (clean) / `npm run build` (`✓ built`); `git diff -- ui/src/bindings`
  = only `Settings.ts` + `JobKind.ts` (regenerated, committed). **Grep gate:** `nomic` / `EmbedImage` /
  `image_embedding` / `embed_image` appear only in history/rationale (CHANGELOG, archives, `docs/0.3.0.md`,
  specs) and the enumerated live-code exceptions — the frozen `MIGRATION_V1` DDL, the new `MIGRATION_V9`,
  the migration-test seed, and the `enrich.image_embeddings` retired-key literal; zero elsewhere in
  `crates/`, `src-tauri/`, `ui/src/`, README, ARCHITECTURE, TESTING. **Parity:** `parity-digest` lines
  identical on the 10k-frame fixture before (commit 1) and after (p95 < 200 ms both runs) — the image
  arm was never in hybrid search, so parity is structural and shown. Full verbatim in `05`
  (Pass 2026-07-03 PR4).

## 2026-07-03 — 0.3.0 PR3: Beta model tier removal (`feat/pr3-beta-tier-removal`)
- **Change:** Retired the **Beta** tier from both inference lanes — **Default / Quality only** (D3/D4).
  Deleted the two Beta models (vision `jc-builds/Qwen3.5-9B-VLM-Q4_K_M-GGUF`, answer
  `nvidia/NVIDIA-Nemotron-3-Nano-4B-GGUF`). **No schema change** (tiers live in the `settings` table,
  not the schema).
  - `crates/traits/src/ipc.rs`: `ModelTier` → `{ Default, Quality }` (dropped `Beta`); doc comment
    records the retirement + load-remap. ts-rs regenerated `ui/src/bindings/ModelTier.ts` to
    `"default" | "quality"`.
  - `crates/inference/src/models.rs`: deleted the two `repo_for` Beta arms + the `tier_slug` `Beta`
    arm (the only two exhaustive matches on the enum in the workspace). Extended
    `repo_mapping_matches_registry` to assert all **four** surviving `(lane, tier)` → repo + mmproj
    pairs per `MODEL_REGISTRY §1/§2` (supports acceptance line 3).
  - `crates/kernel/src/settings.rs`: new `load_tier(store, key, default)` helper replaces the generic
    `json()` read for the two `models.*_tier` keys. A persisted `"beta"` is mapped to `Quality`,
    **persisted** via `set_setting` (best-effort, warn-and-swallow on error), and returned — so it
    logs **once** (the retired token leaves the DB; next load reads `"quality"`), the same mechanism
    as `drop_retired_settings`. The remap lives in the **load path**, not the startup-maintenance
    sweep, because the composition root builds the sidecars straight from `load_settings`' output; a
    sweep-side remap would race it and the first post-upgrade session would run `Default`. Any *other*
    unparsable tier value keeps the old behavior (fall back to default, no rewrite).
  - Tests: `crates/kernel/tests/settings.rs` — new `persisted_beta_tier_remaps_to_quality_and_persists`
    (seed `"beta"` both lanes → load = Quality, DB rewritten to `"quality"`, second load idempotent)
    and `unknown_tier_falls_back_to_default_without_rewrite` (a non-Beta bad value is not migrated);
    fixed `round_trips_non_default_values` (`Beta` → `Quality`).
  - UI: `ModelTierPicker.tsx` (TIERS + MODEL_NAMES beta rows + header comments) and `Settings.tsx`
    (`TIER_LABEL`) lose Beta — TypeScript's `Record<ModelTier, …>` is the tripwire that forced them.
  - Docs: `README.md` (2-tier table), `docs/ARCHITECTURE.md` §7.3, `docs/TESTING.md` (new model-tier
    manual-acceptance section), `CHANGELOG.md`.
- **Why:** `docs/0.3.0.md` PR3 + `02 §5c`/`03 §8`/`§13b.2` — cut the model-testing matrix by a third
  and make licensing uniformly Apache-2.0; Nemotron (OML license, hybrid arch) was the single riskiest
  registry row. D3 (beta→quality on load), D4 (leave on-disk GGUFs) settled in the roadmap.
- **Verification:** `cargo fmt --check` / `clippy --workspace --all-targets -D warnings` /
  `build --workspace` / `test --workspace` all exit 0 (kernel settings **10** incl. the 2 new; inference
  **102** incl. the extended registry test; traits 53; store 24+58; full workspace 0 failed);
  `npm run lint` (clean) / `npm run build` (`✓ built`); `git diff -- ui/src/bindings` = only
  `ModelTier.ts` (regenerated, committed). **Grep gate:** `Nemotron` / `Qwen3.5-9B` appear only in
  history/rationale docs (CHANGELOG entries, archives, `docs/0.3.0.md`, specs retirement language) —
  zero in `crates/`, `src-tauri/`, `ui/src/`, README, ARCHITECTURE; `beta` survives in source only as
  the `load_tier` migration literal + incidental test fixtures. **Live (real desktop, `npm run tauri
  dev`):** seeded the fresh dev DB with `models.vision_tier=models.answer_tier='"beta"'`, relaunched →
  two `WARN kernel::settings: settings: retired \`beta\` tier mapped to \`quality\`` lines (one per
  lane), the DB rows persisted to `'"quality"'`, and a second in-session load emits no further warn
  ("logged once"); the app ran on the remapped Quality tiers (`inference providers attached; sidecar
  ready`). Full verbatim in `05` (Pass 2026-07-03 PR3).

## 2026-07-03 — 0.3.0 PR2: event-trigger trim (`feat/pr2-trigger-trim`)
- **Change:** Cut the six opt-in event-capture triggers to **foreground + idle** (D1), deleting the
  `WH_MOUSE_LL` global mouse hook (click/scroll-stop), the `AddClipboardFormatListener` clipboard
  listener, and the typing-pause edge — plus their five `capture.event_*` settings fields. **No schema
  change** (D2); the `CaptureTrigger` enum, its DB-token maps, the `frames.capture_trigger` CHECK, and
  the Moment `TRIGGER_LABEL` all stay so legacy frames still render their trigger.
  - `crates/capture/src/trigger.rs`: `InputEventKind`→`{Foreground}`; `TriggerConfig`→5 surviving
    fields; `poll()` idle-only; 14 tests → 9 (retired-only deleted; two surviving-logic tests rewritten
    off the retired `Clipboard` kind; typing-pause test → idle-edge).
  - `crates/capture/src/events.rs`: **deleted the message-only window + the whole mouse-hook `unsafe`
    path + the clipboard listener**; `start()` is now param-less; the hook thread forces its message
    queue with `PeekMessageW(PM_NOREMOVE)` before signaling ready (the window used to guarantee it,
    which `Drop`'s `WM_QUIT` post depends on), installs one out-of-context foreground WinEvent hook.
  - `crates/traits`: 5 fields removed from `Settings` + `CaptureConfig`; new required Store method
    `delete_settings`; `CaptureTrigger` retired variants reworded **legacy — no longer emitted**.
  - `crates/store`: `delete_settings` impl + delegation (no schema change).
  - `crates/kernel/src/settings.rs`: retired reads/writes/clamp/maps removed; new
    `RETIRED_SETTINGS_KEYS` + `drop_retired_settings` (one `warn!`, error-swallowing).
    `src-tauri/src/lib.rs`: call it once at startup (before the maintenance sweep).
  - UI: `Settings.tsx` event panel → master + app-switch + idle + 3 thresholds; `Settings.ts` binding
    regenerated (5 fields gone); `CaptureTrigger.ts` unchanged.
  - Docs: `docs/ARCHITECTURE.md`, `docs/TESTING.md`, `README.md`, `CHANGELOG.md`.
- **Why:** `docs/0.3.0.md` PR2 + `02 §5c` — remove the invasive global mouse hook the 0.2.0 design
  avoided, the clipboard privacy-optics liability, and the idle-redundant typing-pause; every removal
  deletes user config surface, maintainer decision surface, and audit surface (`03 §8` L616–631,
  `§13b.1`; settings-load-tolerance = D1's "drop + log once, never crash").
- **Verification:** `npm run lint`/`build` (exit 0 / `✓ built`); `cargo fmt --check`/`clippy -D
  warnings`/`build`/`test --workspace` all exit 0 (capture 22+1ign, kernel settings 8, store 24+58);
  `git diff -- ui/src/bindings` = only `Settings.ts`. Grep gate clean (retired symbols only in
  history notes + the read-path exemptions). **Live (real desktop):** window-less foreground hook 50×
  start/drop `ok`; seeded dev DB with the 5 retired keys → dropped on load with one `warn` line, none
  on relaunch, boots clean; live DB `schema_version=8` (unchanged) accepts + reads back a
  `capture_trigger='click'` frame. Full verbatim in `05` (Pass 2026-07-03).

## 2026-07-03 — 0.3.0 arc specs contract (PR1, specs-only) (`feat/0.3.0-pr1-specs-contract`)
- **Change:** Normalized the 0.3.0 roadmap (`docs/0.3.0.md`, decisions D1–D15) into the spec contract
  so PR2–PR9 are implementable from the specs alone. **No code / no schema code / no UI** — only
  `specs/`, `docs/`, `CLAUDE.md`, `AGENTS.md`, `CHANGELOG.md`.
  - `02`: new **§5c** (0.3.0 arc — problem/thesis/additions/ships-in/deferred); two-tier §2/§3; §6 risk
    rows (drop the Nemotron row; add hotkey-conflict + API-token-leak rows); §7 non-goals (+ proactive
    nudges, audio *for now*); §8 Status → 0.3.0 active.
  - `03`: §8 removed the 5 retired event keys + `enrich.image_embeddings`; added `overlay.*`/`resume.*`/
    `marks.*`/`api.*` groups + the `beta`→`quality` load mapping (D3/D4) + the settings-load-tolerance /
    no-schema-change contract (D1/D2). §4 added the `marks` table + documented **both** forward-only
    migrations (PR4 image-lane drop, PR6 marks; D5/D10/D15) with the relative-version rule, and removed
    the image-embedding DDL + `embed_image` refs across §3/§4/§5. New **§7b** (where-was-i + marks +
    `capture_now` — D7/D8/D9) and **§7c** (localhost HTTP API + export + SSE + MCP — D11/D12/D13).
    §12/§13 reconciled to two tiers; new **§13b** DoD (PR2–PR9 acceptance).
  - `UI_REFERENCE`: Overlay screen (identity / five states / keyboard / <150 ms perf / reduced-motion /
    self-exclude), Deck where-was-i card + Intentions strip, Settings hotkeys + Local API row (threat
    model + loud port-in-use), `ModelTierPicker` → Default/Quality, `Domain (0.3.0)` components.
  - `MODEL_REGISTRY`: deleted both Beta rows + the image-embedding row + the Nemotron invariant.
  - `00`/`01`: two-tier consistency (**required** — `04 §2` routes model-tiers to `00`) + image-model
    strike; `00 §D` flags (image embeddings removed / reranker never implemented).
  - `04`: 0.3.0 reading-order line, source-of-truth row, PR1→PR9 build-order sequence.
  - `07`: five deferrals (#75–#79), the resolved API port-bind UX (#80 — "loud + guided change"), and a
    doc-sweep tracking row (#81 — `docs/ARCHITECTURE.md`/`TESTING.md` assigned to PR2/PR4/PR9).
  - `CLAUDE.md`/`AGENTS.md`: current-state paragraph → 0.3.0.
- **Why:** the arc ships specs-first (same method as the 0.2.x PR1); `docs/0.3.0.md` "What PR1 must
  change, file by file" + its acceptance ("a fresh agent can implement PR2 from the specs alone").
- **New ambiguity (not in D1–D15):** API port-bind failure UX — surfaced to the user, resolved to
  **"loud + guided port change"**, contract written into `03 §7c` + `UI_REFERENCE`, recorded in `07` #80.
- **PR #70 review round (bot comments; not replied to per user instruction; each verified vs. real
  code first):** `03 §2` moved the `capture_now` note outside the `CaptureSource` trait ("not a trait
  method"); `03 §7`/`§7c`/`§4` settled one canonical `list_marks` order (all marks, unresolved first
  then newest-first) + fixed `idx_marks_open` to `created_at DESC`; `03 §7b` anchors where-was-i on the
  **last non-ScreenSearch foreground** (overlay-focus bug) and absorbs transient excursions via the D9
  dwell threshold; `03 §7b` pins `capture_now` to the **foreground-window monitor** (multi-monitor
  determinism); `03 §7c` cancels `/v1/ask` inference on client disconnect and streams `/v1/export`
  (flat memory, bounded window). A `CHECK` on `jobs.kind` was **declined** (live `schema.rs` has none;
  would force an unplanned PR4 rebuild) and recorded as opt-in hardening in `07` #82. Full mapping: `05`.
- **Verification — verbatim** (specs-only PR — the untouched tree must still build):
  - `cargo fmt --all -- --check` → `FMT_EXIT=0`
  - `cargo build --workspace` → `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 19.79s` / `BUILD_EXIT=0`
  - `git status --short` diff limited to `specs/*`, `docs/0.3.0.md`, `CLAUDE.md`, `AGENTS.md`,
    `CHANGELOG.md`, `.gitignore` — **no `.rs`/`.ts`/`.tsx`/`.toml`/`ui/` files touched** (bindings
    untouched by construction).

## 2026-07-01 — UIA cache-batched walk: efficiency lever (#71) (`fix/uia-findall-buildcache`)
- **Change:** `crates/uia/src/worker.rs` — the foreground-window UIA walk now batches each node's ~5
  separate `Current*` property reads into **one `BuildUpdatedCache`** call + cached getters
  (`build_cache_request`: ControlType/Name/IsPassword/IsOffscreen/BoundingRectangle/ValueValue +
  ValuePattern, `_Element` scope, `_Full` mode). Same walker DFS structure/bounds as the shipped code;
  live `TextPattern` stays gated/capped; `Value`/`Name` read from the cache. ~2.5× fewer cross-process
  COM calls per walk.
- **Why:** `07` #71 efficiency lever (deferred from the 0.2.1 hang mitigation). The gap required live
  verification. Two bulk-fetch designs were live-tested and **rejected** as unbounded: a single
  `FindAllBuildCache(Subtree)` (~1.4 s on a large window) and a `FindAllBuildCache(Children)` BFS (a
  single wide-node fetch overran the budget on VS Code-scale trees). The granular per-node
  `BuildUpdatedCache` keeps small, deadline-interruptible calls — no wide-node cliff.
- **Review fixes (3, adversarial):** cache the `ValueValue` property (else `CachedValue()` fails and
  edit-field/omnibox text is silently dropped); descend past a `BuildUpdatedCache` failure (a transient
  timeout must not prune a subtree); full coverage parity (descend into everything, like the old DFS).
- **Verification — verbatim:** `cargo fmt --all -- --check` EXIT 0; `cargo clippy --workspace
  --all-targets -- -D warnings` EXIT 0; `cargo test --workspace` 0 failed (uia 16 + 2-ignored). Live:
  `cargo test -p uia -- --ignored` passes **bounded** on a heavy window that timed out the bulk-fetch
  variants; `npm run tauri dev` captured `primary_source='uia'` Chrome frames (1186–1748 chars, omnibox
  URL present, no over-budget warnings).
- **PR #68 review fixes (2026-07-01):** (1) reworded 7 stale `FindAllBuildCache` doc-comments to the
  shipped `BuildUpdatedCache` design (comment-only). (2) **Raw-view cache filter** — a cache request's
  `TreeFilter` defaults to control-view, so with `capture.uia_view_control_only` off the `RawViewWalker`
  navigated to raw-only nodes whose properties the filter skipped (`Cached*` empty → text lost to OCR).
  `build_cache_request` now takes the view flag and `SetTreeFilter(Control|Raw ViewCondition)` in
  lock-step with the walker; control-view default unchanged. Verify: `fmt`/`clippy`/`cargo test -p uia`
  EXIT 0; live `--ignored` control-view path non-regressed (3×: 282 spans / 6316 chars / ~90 ms).
  (3) **Don't cache field values before the privacy guard (Codex P2).** Caching `ValueValue` meant
  `BuildUpdatedCache` prefetched every node's field value — including password/offscreen fields —
  *before* `should_emit` runs, a visible-only/"password fields are never read" regression vs. the
  pre-#71 live walk (which read `Value` only after the guard). Removed `ValueValue`/`ValuePattern`
  from the batched cache; `extract_text` now reads `Value` **live** via `GetCurrentPattern`, and it is
  only called after the guard passes — so a masked/hidden value is never fetched. `Name`/metadata stay
  batched (the bulk of nodes are static text), and value-bearing inputs are a small live-read fraction.
  Verify: `cargo fmt -p uia -- --check` EXIT 0; `cargo clippy -p uia --all-targets -- -D warnings`
  EXIT 0; `cargo test -p uia` 16 passed/2 ignored; live `--ignored` walk yields text (4 spans / 30 chars).

## 2026-07-01 — Degrade-to-text DB shrink: merge purged spans to lines (#73a) (`fix/degrade-to-text-db-growth`)
- **Change:** Degrade-to-text retention now shrinks the DB too. For a purged frame, the per-word
  `text_spans` are merged into per-line spans: new pure `merge_spans_to_lines` (group by `line_index`,
  union bbox, join text, content-wins role/searchable) + store `merge_frame_spans_to_lines` (one
  transaction). Wired into `run_retention_once` (via the atomic `degrade_frame_to_text`, see the
  PR #67 review fix below) and a one-time watermark-gated backfill `merge_purged_spans_once`
  (`maintenance.purged_spans_merged`) over the pre-existing purged backlog, backed by new
  cursor-batched `store::purged_frame_ids`.
- **Why:** `07` #73 (a). The DB (~40% of growth) didn't shrink on retention. `text_spans` are the
  largest prunable artifact but power `FrameReconstruction` for purged frames (`MomentDetail.tsx`
  renders it in place of the purged image), so they're **merged** (keeps a line-level reconstruction),
  not pruned. Search is unaffected (FTS reads `content_text`; the vector arm reads `embeddings`).
- **Review fix (CONFIRMED low):** `merge_purged_spans_once` set the completion watermark even when
  individual frames failed to merge, diverging from the `purge_self_captures` retry pattern. Now a
  `clean_drain` flag withholds the watermark on any list- or per-frame failure, so the idempotent
  backfill retries next launch. Covered by a new `screensearch_lib` test.
- **PR #67 review fixes (2026-07-01):**
  - **Codex P2 — stranded per-word rows after a mid-sweep merge failure (fixed).** The sweep degraded
    a frame in two writes: `purge_frame_image` (sets `image_purged = 1`) then a non-fatal
    `merge_frame_spans_to_lines`. If the merge failed *after* the flag was set, the frame — now
    excluded from `frames_with_image_older_than` (`WHERE image_purged = 0`) and, once the backfill
    watermark was set, from the backfill too — kept its per-word rows forever. Replaced with the
    **atomic** `store::degrade_frame_to_text` (merge **and** flag in one transaction); on failure
    nothing commits, `image_purged` stays `0`, the whole frame retries next sweep. New store tests
    `degrade_frame_to_text_merges_spans_and_purges_atomically` / `_purges_even_without_spans`.
  - **Gemini "N+1 / bulk `IN`" ×2 — declined, recorded (`TODO.md` TODO-2).** Embedded SQLite has no
    network round-trip; neither the one-time backfill nor the hourly sweep is hot; and a single
    `IN`-clause transaction would forfeit the per-frame failure isolation the `clean_drain` backfill
    relies on to converge (one busy frame rolls back a whole 256-batch). Kept per-frame transactions
    with a documented deferral + how to batch safely if it ever matters.
- **Verification — verbatim:** RED then GREEN across `merge_spans_to_lines` (4 unit),
  `merge_frame_spans_to_lines_*` / `degrade_frame_to_text_*` / `purged_frame_ids_*` (store
  integration), and `merge_purged_spans_once_*` (`screensearch_lib`). Full CI (re-run after the
  PR #67 review fix, 2026-07-01): `npm run lint` EXIT 0 / `npm run build` `✓ built in 1.70s`;
  `cargo fmt --all -- --check` EXIT 0; `cargo clippy --workspace --all-targets -- -D warnings` EXIT 0
  (3.58s); `cargo build --workspace` EXIT 0; `cargo test --workspace` all green, 0 failed (store 18
  lib + **54** integration incl. 2 new `degrade_frame_to_text_*`; `screensearch_lib` 16/18, 2 ignored);
  `git diff --exit-code -- ui/src/bindings` clean. Adversarial 3-lens review: 1 low finding, fixed;
  PR #67 external review: 1 P2 fixed (atomic degrade), 2 N+1 declined + recorded.

## 2026-07-01 — Vector-arm time-range recall: adaptive KNN escalation (#8) (`fix/vector-arm-time-range-recall`)
- **Change:** `crates/store/src/search.rs::text_knn_in_range` now escalates the KNN `k` for time-windowed
  search instead of running a single `k = pool` pass. A bounded `time_range` re-runs the cosine KNN with a
  geometrically larger `k` (factor 8, ceiling 20 000) until the pool fills with in-range frames, the vector
  table is exhausted (KNN returned `< k` rows), or the ceiling is hit; an unbounded range is unchanged
  (one pass, the time filter a no-op). New constants `KNN_ESCALATION_FACTOR` / `MAX_TIME_RANGE_KNN`; the
  time filter + frame de-dup moved from SQL into Rust so the loop can see the raw KNN row count (its
  exhaustion signal). New test `vector_arm_finds_in_range_match_buried_beyond_pool` + `vec_at_angle` helper.
- **Why:** `07` #8 — sqlite-vec 0.1.9 can't filter inside a KNN `MATCH` (0.1.10-alpha is broken), so the
  old post-KNN time filter silently dropped in-range matches ranked beyond the top-`pool` nearest vectors
  (recall under-count on tight windows). `03 §4/§13`.
- **Verification — verbatim:** RED `vector_arm_finds_in_range_match_buried_beyond_pool` → `left: [] right: [56]`;
  after fix `cargo test -p store` → `50 passed; 0 failed`. Full CI: `npm run lint` EXIT 0 / `npm run build`
  `✓ built in 2.11s`; `cargo fmt --all -- --check` EXIT 0; `cargo clippy --workspace --all-targets -- -D
  warnings` EXIT 0; `cargo build --workspace` EXIT 0; `cargo test --workspace` all green 0 failed; perf
  `p95 = 80.3555ms` < 200 ms; `git diff --exit-code -- ui/src/bindings` clean. Adversarial 3-lens review
  workflow: **no findings**.
- **Review response (PR #66, Codex P2 — count-capped escalation target):** a *sparse* bounded window
  (fewer distinct embedded frames than `pool`) on a DB with > `MAX_TIME_RANGE_KNN` vectors trips neither
  the pool-fill nor the exhaustion gate, so it climbed to the 20 000 `k` ceiling on **every** query even
  after finding all in-window matches. Now the escalation `target` is capped at
  `count_embedded_frames_in_range(start, end, cap=pool)` — an index-served `EXISTS` semi-join
  (`idx_frames_captured_at` range + `idx_embeddings_frame`), `LIMIT`-bounded so it stays O(pool) not
  O(window) (resolves the reviewer's residual-cost concern). `target = min(pool, count)`; `count == 0`
  skips the KNN. Loop extracted into pure `escalate_in_range_knn(pool, target, fetch)`. New tests:
  5 `escalating_knn_*` unit tests (**3 observed red** on a naive single-pass first),
  `count_embedded_frames_dedups_chunks_and_honors_cap`, and integration `sparse_/dense_/empty_time_window_*`.
  Verbatim: `cargo test --workspace` all green **0 failed** (store 53 integration + 20 lib); `cargo fmt
  --all -- --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo build --workspace`
  all EXIT 0; perf `median = 27.3359ms, p95 = 65.8744ms` < 200 ms; bindings clean;
  `EXPLAIN QUERY PLAN` → `COVERING INDEX idx_frames_captured_at` + `EXISTS … idx_embeddings_frame`.
  Adversarial re-review (3-lens, refute-by-default verify): 1 **LOW** finding (uncapped-count cost)
  already resolved by the `LIMIT`; no correctness findings.
- **Follow-up review response (PR #66, Codex P2 — bound the pre-count scan):** Codex refuted the
  "O(pool) via `LIMIT`" claim above — the `LIMIT` caps *matches*, so a window with many captured frames
  but few embedded ones (embed backlog / wide range) never fills it and the count walked the whole frame
  range (O(frames-in-window)). Fixed by bounding frames *examined*: `count_embedded_frames_in_range` now
  takes `(pool, scan_cap)` and returns `Option<usize>`; the inner select is `LIMIT scan_cap`, the outer
  returns `(scanned, embedded)`. `scanned == scan_cap` → too large to prove sparse → `Some(pool)` (dense
  assumption; only *raises* the target, never drops a match); else exact → `None` if zero (skip KNN) or
  `Some(min(pool, embedded))`. `COUNT_SCAN_CAP = MAX_TIME_RANGE_KNN` (20 000) — the count never examines
  more frames than a ceiling KNN examines vectors, so it is now genuinely O(pool) even on a sparse wide
  window. `escalate_in_range_knn` + its unit tests unchanged. TDD: rewrote the count test with a
  scan-budget case (**observed red** `Some(2)` → green `Some(pool)`). Verbatim: `cargo test -p store`
  lib **20**/integration **53** all `ok`; fmt/clippy/build EXIT 0; perf `median = 26.9464ms, p95 =
  68.57ms` < 200 ms; bindings clean.

## 2026-06-30 — PR #63 review fixes: NavRail tab-stop sync + palette focus-on-navigate (#42)
- **Change:** Two real bugs in the #42 a11y work, flagged by reviewers (Gemini/Claude/Codex all caught
  the first):
  1. **NavRail roving tab-stop didn't follow external navigation.** `focusIndex` was seeded once at
     mount, so navigating via the Command Palette / an in-app link / browser back left `tabIndex=0` on a
     stale link. Added `useEffect(() => setFocusIndex(activeIndexFor(pathname)), [pathname])` — re-derives
     the tab stop only (never calls `.focus()`); arrow moves don't change the path so they're untouched.
  2. **Command Palette focus-restore stole focus on navigation.** Restoring focus to the opener on every
     close yanked it back to the ⌘K trigger when a command navigated to a route that autofocuses (Recall's
     search input). Fix: restore **only on dismiss** — `run()` sets `restoreFocusRef.current = false` so a
     command's destination/action owns focus; the cleanup also now guards `openerRef.current?.isConnected`.
     (Gemini's literal `document.body.contains` suggestion alone wouldn't fix it — the trigger lives in the
     always-mounted NavRail, so it's always connected; the dismiss-vs-run distinction is the real fix.)
- **Why:** Both regress keyboard/SR navigation introduced by the #42 changes; `UI_REFERENCE` §7.
- **Verification — verbatim:** `npm run lint` EXIT 0; `npm run build` `✓ built in 1.45s`. **Live Playwright
  probe:** (1) palette-navigate `/`→`/timeline` ⇒ NavRail tab stop = **Timeline** (was stale Deck before);
  (2) palette-navigate →`/recall` ⇒ `document.activeElement` = the **"Search query" `INPUT`** (not the ⌘K
  button); (3) focus ⌘K → `Ctrl+K` → `Esc` ⇒ focus **restored to the ⌘K button** (dismiss path intact).

## 2026-06-30 — Cancel Inno installer (#26) + single-instance focus + a11y matrix (#42) (`chore/cancel-inno-and-a11y-matrix`)
- **Change:** Three known-gap closures.
  1. **#26 packaging — Inno/portable-ZIP/MSI formally dropped, gap closed.** Tauri 2 shipped an
     unsigned NSIS installer in v0.1.0 (`bundle.targets=["nsis"]`); the specs still demanded an "Inno
     Setup installer + portable ZIP" in 9 live places. Rewrote every one to NSIS — `00` §A/§G, `01`,
     `02` P5, `03` §11 + DoD §13.9, `docs/ARCHITECTURE.md` §12, `.github/workflows/ci.yml`, `README.md`
     — re-scoped DoD §13.9 to "NSIS builds successfully" (met), and flipped `07` #26 to ✅ with
     **code-signing as the lone open packaging item** (already tracked under `07` "Manual steps").
  2. **Single-instance focus (Gemini PR #27 follow-up).** The `src-tauri/src/lib.rs` single-instance
     callback now calls `window.show()` before `unminimize()`/`set_focus()`, so a hidden/tray-minimized
     window is restored (not just unminimized) on a second launch.
  3. **#42 keyboard/focus matrix — five UI fixes.** NavRail roving-tabindex (Arrow/Home/End, wrap) +
     `aria-current="page"`; Command Palette focus restoration on close; Recall Ask focus-to-answer on
     stream completion; Settings `<Panel group>` (`role="group"` + `aria-labelledby`, the ARIA
     fieldset/legend equivalent, card layout untouched).
- **Why:** `07` #26/#42 + the `07` single-instance TODO. #26 was a standing spec-vs-reality
  contradiction (logged in `06` #16); #42 was an open P5 a11y audit follow-up; the single-instance
  bullet was a deferred PR #27 review note.
- **Verification (Windows) — verbatim:**
  - `npm run lint` → `EXIT 0`; `npm run build` → `✓ built in 1.96s`
  - `cargo fmt --all -- --check` → `EXIT 0`
  - `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile … in 53.41s` / `EXIT 0`
  - `cargo build --workspace` → `Finished … in 22.11s`; `cargo test --workspace` → every suite `ok`,
    **0 failed** (inference 102, traits 53, store 49+14, kernel 27, capture 27, uia 16/2-ignored,
    sysmon 11, textfilter 12, screensearch_lib 7, embeddings 1, ocr 1, doctor 0)
  - `git diff --exit-code -- ui/src/bindings` → clean (`EXIT 0`)
  - **Live focus probe (Playwright/Chromium vs the Vite dev server):** NavRail `Deck {tabIndex 0,
    aria-current page}`, ArrowDown→Recall (tabIndex follows), End→Settings, ArrowDown wraps→Deck,
    ArrowUp wraps→Settings, re-seeds to active route on navigation; Command Palette `Ctrl+K`→
    `role=combobox` input, `Esc`→focus restored to the ⌘K `BUTTON`. (Settings-group + Ask-focus need
    live backend data the IPC-less probe can't supply → build + code-verified.)

## 2026-06-30 — Model-downloader resume hardening (`fix/download-resume-hardening`)
- **Change:** Two localized fixes in `crates/inference/src/download.rs`, both TDD'd.
  1. **Gap #69 — wrong-sized `.part` no longer publishes garbage.** `open_preallocated` now returns
     `unbacked = (pre_existing_len != total)` instead of "created"; the chunked-download caller
     discards a header-matching `.parts` bitmap whenever the part is `unbacked`. This covers the
     external-cleanup case (a tool truncates an existing `.part`; `set_len` re-grows it with zeros)
     that the old "created"-only check missed, plus the corruption-grown (`> total`) case (broadened
     from `< total` to `!= total` per a PR #62 review note), while never flagging a legitimate resume
     (always preallocated to exactly `total`). New red→green tests
     `truncated_part_discards_stale_partial_manifest` + `oversized_part_discards_stale_manifest`.
  2. **PR #27 Codex-P2 — re-check cache before retrying a locked download.** Extracted the
     clean-layout + HF-cache fast paths into `place_if_cached`; folded the single-stream
     lock-retry into `fetch_one` so that after each `LockAcquisition` backoff it re-checks
     `place_if_cached` and short-circuits if the lock holder finished during the sleep (no
     re-download / publish collision). Extended the backoff (added `LOCK_RETRY_BACKOFF_CAP` 15 s,
     `LOCK_RETRY_MAX_ATTEMPTS` 5→24 ≈ 5 min total) so a real multi-GB download by the holder is
     outlasted rather than abandoned at ~20 s. New `place_if_cached_*` unit tests. The doc-hidden
     `download_file_with_lock_retry_for_diagnostics` (used by `examples/repro_8b.rs`) keeps its own
     minimal backoff loop.
- **Why:** Both are open durability gaps in `07_KNOWN_GAPS.md` — silent corruption (zeros published,
  length check passes, sha256 skipped when the CDN advertises no `X-Linked-ETag`) and wasted
  re-download/collision on lock contention. `03 §13` wants the downloader robust and resumable. The
  separate `#46` row (orphaned **detached** writers in the same fallback) stays open — it needs
  replacing hf-hub's high-level downloader, out of this scope.
- **Verification (Windows, after the PR #62 review fix) — verbatim:**
  - `cargo test -p inference --lib` → `test result: ok. 102 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.05s` (incl. `truncated_part_discards_stale_partial_manifest`, `oversized_part_discards_stale_manifest`, `place_if_cached_short_circuits_when_dest_already_present`, `place_if_cached_returns_false_when_nothing_cached`; existing `resume_*`/`fresh_part_*`/`integrity_*` unchanged)
  - `cargo fmt --all -- --check` → `EXIT 0`
  - `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile … in 2.43s` / `EXIT 0`
  - `cargo build --workspace` → `Finished dev profile … in 8.90s` / `EXIT 0`
  - `cargo test --workspace` → all suites green / `EXIT 0`
  - `git diff --exit-code -- ui/src/bindings` → clean (`EXIT 0`)

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

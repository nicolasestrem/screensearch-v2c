# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

### 0.2.0 PR8 — Parallel model download
Replaces the single-stream first-run model fetch with a **multi-connection chunked downloader** so
the GGUF/mmproj download saturates the user's bandwidth instead of being capped at hf-hub's one
HuggingFace-CDN connection (~20 MB/s). Confined to `crates/inference/src/download.rs` (+ its tests),
independent of the retrieval chain (`docs/0.2.0.md` PR8).

- **Chunked, resumable, parallel.** A `Range` probe learns the total size, range support, and the
  LFS sha256 (when advertised); if the server honours ranges, the file is fetched with N parallel
  `Range` requests (`DOWNLOAD_CONNECTIONS`, default 8 — `SSV2C_DOWNLOAD_CONNECTIONS` to override,
  clamped 1–16) writing into one pre-allocated `.part` via positioned writes. A per-chunk completion
  **bitmap** (`<base>.parts`) is fsync'd *after* its chunk's data, so an interrupted download resumes
  exactly where it stopped with no torn-write window.
- **Per-request resolve (HF Xet bridge).** HuggingFace's signed CDN URLs are cryptographically bound
  to the exact `Range` they were minted for (the CloudFront policy pins
  `ByteRange.ExpectedHeader`), so a single resolved URL **cannot** be reused across chunks — reuse
  returns `403`. Each chunk therefore re-requests the stable `…/resolve/main/…` URL with its own
  `Range` and follows the redirect (reqwest preserves `Range` across the hop), minting a fresh
  range-matched signed URL per chunk — the same approach `hf_transfer` uses. A transient
  `403`/`401`/`429`/`5xx` is retried with backoff.
- **Correct integrity target (HF Xet bridge).** A file's true content sha256 is exposed **only** in
  the `X-Linked-ETag` header on the `resolve` *redirect* (alongside `X-Linked-Size` and
  `Accept-Ranges`). The CDN blob *behind* the redirect carries a bare `ETag` that is the **Xet
  content hash** (a different 64-hex digest of the same bytes, also surfaced as `X-Xet-Hash`), not
  the file sha256 — and on classic LFS it is an S3 multipart validator (`md5-partcount`). The probe
  therefore reads the redirect's **own** headers *without following it*, and verifies only against
  `X-Linked-ETag`; the bare `ETag` is never trusted. Previously the probe followed the redirect and
  read the CDN `ETag`, so a correctly-downloaded Xet model (e.g. Qwen3-VL-4B `Q4_K_M`) failed sha256
  every time and re-downloaded in a loop — visible as a stall at ~75% (the GGUF is 75% of the
  GGUF+mmproj total, so it finished, failed verification, and restarted before the mmproj began).
- **Reuses the existing machinery, doesn't bypass it.** All chunks feed the same downloaded-byte
  counter, so the UI percentage and the aggregate-progress stall watchdog stay truthful with
  concurrent writers ("no aggregate progress across all chunks" is the stall condition, unchanged).
  The finished blob still lands in the clean layout via an atomic rename, and `fetch_one`'s
  already-in-layout / already-a-cache-blob fast paths are still checked first.
- **Range-less fallback + integrity.** A server that doesn't advertise ranges (no `Accept-Ranges` /
  no length) falls back to the single-stream hf-hub path, so a non-resumable mirror still downloads.
  The assembled file is verified against the LFS sha256 when present (else byte length); a mismatch
  discards the partial for a clean retry. A per-chunk `206` assertion refuses a server that silently
  ignores `Range` rather than corrupting the file; an expired CDN signed URL (`401`/`403`) triggers
  one re-resolve.
- **Cancellable (gap #46 for this path).** The chunk futures are awaited inside the fetch task with
  `buffer_unordered`, so the stall watchdog's `task.abort()` drops every in-flight chunk — no orphaned
  writers, unlike hf-hub's internally-detached chunk tasks on the fallback path.
- **Tests (this PR owns them):** parallel byte-identity, resume-skips-completed-chunks,
  fail-fast-on-stuck-chunk, range-ignored rejection, sha256 accept/reject, `X-Linked-ETag`-only
  integrity (rejects the bare CDN/Xet `ETag`), and the `range_plan` decision — all fully mocked
  (wiremock + a local hang-after-206 server), no network.

### 0.2.0 PR7 — Integration audit
Ran the PR7 end-to-end audit against the existing populated app-data DB using `npm run tauri dev`
and the real debug executable (`target/debug/screensearch.exe`). Evidence screenshots were captured
locally under ignored `.playwright-mcp/pr7-2026-06-25/` storage and are not tracked.

- **Search audit:** content searches for real work terms (`Calendar-Grid`, `cargo test`, etc.) worked
  and the `include app chrome + raw text` toggle recovered raw/static labels. Default content search
  still surfaced static/app-chrome-heavy results for terms like `Firefox`, `Steam`, and especially
  `Deck` in the populated corpus, so PR7's strict static-chrome acceptance is documented as not fully
  closed rather than hidden by a DB rewrite.
- **Ask audit:** a Calendar-Grid Coverage Map-Reduce question grounded on content text and showed
  cited frames. A unique-token no-evidence question refused honestly, but still displayed retrieved
  context frames under `CITED FRAMES`, leaving citation semantics for refusals as an open audit
  finding.
- **Reports audit:** Daily and Weekly reports generated through the UI, cited source frames, and
  reported honest pass/frame footer metadata. Fixed one small UI copy bug where the progress helper
  said "Weekly reports..." even while generating Daily; it now uses range-neutral bounded-pass copy.
- **Capture audit:** started capture briefly, observed a fresh `20:26` frame/tick and queue advance,
  then stopped capture. No destructive DB operation was performed.

### 0.2.0 PR6 — Recall reports + Ask shortcuts
Adds a third Recall capability on top of the attention-first `content_text`: **Reports** —
on-device Daily / Weekly / Custom summaries that **cite the frames they used** — plus per-request Ask
depth and premade Ask prompt cards (`03 §7`/`§8`/`§8b`, `docs/0.2.0.md` PR6). The defining
requirement (user directive): **report context scales with the time range and guarantees temporal
coverage, not just relevance** — a weekly report genuinely covers the whole week (every active day),
never a flat window biased to the most-recent or most-relevant frames.

- **Coverage-first report engine (Calendar-Grid Coverage Map-Reduce).** Instead of growing the model
  window for longer ranges (which scales KV-cache VRAM and forces a sidecar relaunch), the model
  context stays **pinned flat at 8192** and the *number* of bounded passes grows with the range. A
  range is split into a per-calendar-day grid; a single density probe finds the active days; each
  active day gets its own frame budget (floored so a quiet Saturday is never starved by a busy
  Monday, capped so a dense day can't dominate) sampled **evenly across the day** in chronological
  order; one summary pass runs per active day, then a bounded, time-ordered hierarchical reduce folds
  them into the final report. This makes per-day coverage a structural guarantee at the same VRAM as
  a single Ask. A **Custom report with a prompt** instead drives semantic retrieval
  (`hybrid_search`) for relevance.
- **Honest by construction.** An empty range returns a no-evidence report with **zero** model calls.
  Every report footer shows what actually happened — model, passes, covered/total active periods,
  summarized/sampled frame counts, an estimated token figure, and a truncation notice if a
  pathological range hit the safety ceiling — and the body **cites the exact source frames** (chips
  that open the Moment).
- **Reports UI mode.** Recall gains a third mode beside Search and Ask: pick Daily / Weekly / Custom
  (Custom = date range + optional prompt), Generate with live progress ("summarizing day k of N…")
  and Cancel, then read the rendered markdown with clickable source-frame chips, **Copy**, and **.md
  download**. All view states (empty / generating / error / populated / honest no-evidence) are
  defined.
- **Premade Ask cards.** The Ask idle state now offers five one-tap prompts — Day Recap, Standup
  Update, Time Breakdown, Top of Mind, AI Habits — that fill and submit a grounded, cited answer.
- **Ask depth is now a setting, not a constant.** Removed the hardcoded `ASK_TOP_K`; Ask reads
  `retrieval.default_top_k` (with an optional per-request override). A new **"Reports & retrieval"**
  Settings panel exposes the four tuning keys (`retrieval.default_top_k` 8, `reports.daily_top_k` 40 =
  per-active-day budget, `reports.weekly_top_k` 200 = global frame cap, `reports.map_reduce_min_frames`
  20 = single-pass threshold), all clamped.
- **No schema change.** The temporal sampler is a query; the report types are additive IPC
  (`ts-rs` bindings regenerated). Ask output is unchanged (the shared context-packer is byte-identical
  to before). Deviations from the literal spec and three accepted limitations (DST grid skew,
  structural bounds as constants, estimated-token footer) are recorded in `specs/06`/`07`.
- **Review fixes (PR #33).** (1) Removed an N+1 over the report's day grid — the coverage path now
  bulk-fetches every active day's text in one query (Gemini). (2) A trailing single summary in the
  hierarchical reduce passes through instead of spending a model call to "combine" one node (Gemini).
  (3) Prompted (Custom-with-prompt) reports no longer cap relevance retrieval at the search-UI's 100
  frames — the backend ceiling was raised so `reports.weekly_top_k` (up to 2000) is honored (Codex).
  (4) The report planner budgets passes against the provider's **actual** answer-lane context window
  (via the new `AnswerProvider::answer_context_budget`) so a user-lowered `sidecar.ctx_size` can't make
  it pack summaries the sidecar then truncates (Codex). (5) Doc-comment clarified that the default
  `summarize` does not forward the system prompt (Claude).
- **Review fixes (PR #33, round 2).** (1) The temporal sampler returned ~half the requested frames
  whenever a period had just over the limit (a `ceil(total/limit)` stride doubling at `total > limit`,
  e.g. 41 frames → 21); replaced with even-rank bucketing that returns the full `min(total, limit)`
  quota (Codex). (2) Prompted reports no longer show the "range trimmed to fit" notice spuriously —
  the relevance path now flags `truncated` only when the search cap is actually hit, not when
  empty-text hits are filtered (Claude). (3) Empty/no-evidence reports no longer require the answer
  sidecar, so they work on first launch while the model is still downloading (Codex). (4) Daily/Weekly/
  Custom ranges are computed from local calendar days instead of fixed 86.4M-ms spans, so DST
  transition days no longer include/drop an hour (Codex). (5) The `frames_summarized` doc-comment no
  longer claims equality with `cited_frame_ids.len()` (it can exceed it when the citation cap fires)
  (Claude).
- **Coverage fix — dense single periods no longer truncate (PR #33).** A single calendar period
  (always the case for a Daily report, and for short Custom ranges) was one map group, and map-reduce
  only fanned out *across* groups — so a dense day's frames were crammed into one 8192-token pass and
  silently trimmed ("more was captured than summarized"). Each over-large period is now split into
  pass-sized sub-batches before the map step, so a dense day fans out into several passes and the
  reduce folds them back: within-period coverage is complete, not just per-period. The collapse-to-one-
  pass fast path now fires only when the whole report genuinely fits one window. New kernel regression
  test.

### 0.2.0 PR3 — Attention-first text filtering
Replaces PR2's `content_text` passthrough with a real span-aware filter so search, Ask, and
embeddings stop ranking on chrome (toolbars, taskbar, desktop icons, background windows). Searching
"Firefox" / "Steam" no longer surfaces frames purely because those static labels were on screen
(`03 §3b/§4/§8`, `docs/0.2.0.md` PR3). Top risk by design is **false suppression (silent data
loss)**, so the filter is conservative and fully recoverable.

- **New `textfilter` crate — a pure, deterministic classifier.** Groups OCR words by `line_index`
  and assigns each line one of five roles (`system` / `background` / `chrome` / `content` /
  `unknown`) by **geometry before repetition**: a line is `background`/`system` only with a
  confident foreground-window rect; the window title is treated as metadata (never repeated body
  text); long, information-rich lines are **never** catalogued or suppressed for merely repeating.
  Default `content_text` = `content` + `unknown` inside the target window, built from classified
  spans (not string-subtraction). No I/O, no Windows APIs — unit-tested with an anonymized synthetic
  OCR fixture.
- **Static-chrome suppression with a guardrail.** A line's signature
  (`app_hint ⏐ normalized_text ⏐ region_bucket`) is marked chrome only after it has been seen
  `text.chrome_suppress_min_seen` times **and** is shorter than `text.chrome_protect_min_chars`
  **and** the foreground-window rect is known (an unknown rect can't tell a short toolbar label
  from short body text, so it never catalogs or suppresses — keeping the invariant that a missing/
  wrong rect can only *under*-suppress, never silently lose content). All thresholds are
  **settings, never hardcoded** (`chrome_suppress_min_seen` 12, `chrome_protect_min_chars` 48,
  `chrome_region_buckets` 8).
- **Filtered write in one transaction.** New `insert_ocr_filtered` classifies, writes the filtered
  `content_text` directly (so the content FTS index is written **once** — no transient unfiltered
  window a concurrent search could match), stores the classified spans, and bumps the chrome
  catalog. Embeddings run over the filtered text because the embed job is enqueued only after this
  commits. `raw_text` is always preserved; `include_chrome` recovers suppressed terms.
- **Capture learns the foreground window.** Each frame carries a normalized `target_rect` mapped
  from the foreground window's visual bounds (`DwmGetWindowAttribute` extended-frame bounds) against
  the captured monitor's `rcMonitor`; `None` (other monitor / minimized / unresolved) is the safe
  fallback that disables positional suppression.
- **Per-app suppression-rate readout.** New `get_text_filter_stats` command + a read-only per-app
  list in Settings (all view states) surface the silent-data-loss alarm; the Recall search gains an
  `include_chrome` toggle.
- **Schema `schema_version` 3 → 4 (forward-only).** Adds `text_spans.line_index` so the classifier
  groups lines exactly (robust on multi-column layouts). `filter_version` is now `1`; bumping it
  wipes the chrome catalog so suppression re-learns. No backfill of old frames (clean-DB
  assumption). ts-rs bindings regenerated.
- **Review fixes (PR #32).** (1) Removed the N+1 catalog lookup on the OCR hot path — `seen_count`
  was one `SELECT` per candidate line; `insert_ocr_filtered` now pre-fetches the foreground app's
  catalog rows in a single bulk query into a `HashMap` (Claude/Gemini). (2) An unknown `target_rect`
  no longer lets repetition suppress short body text — static-chrome cataloguing/suppression now
  requires a known rect, with a regression test asserting a catalog-saturated signature survives a
  rect-less frame (Codex P2).

### 0.2.0 PR2 — Text-signal data model + OCR spans
The schema, types, and OCR geometry the attention-first retrieval pipeline needs (`03 §3/§3b/§4`,
`docs/0.2.0.md` PR2). No behaviour change for users yet — `content_text` is a passthrough copy of
`raw_text` until PR3's classifier lands (clean-DB assumption, `07` #51).

- **OCR now emits per-word spans.** `crates/ocr` walks WinRT `Lines().Words()` and emits one
  `TextSpan` per word with its `BoundingRect` normalized to `[0,1]` (origin top-left). WinRT exposes
  no per-word confidence, so the `CONFIDENCE_UNKNOWN` sentinel stays; PR2 spans are role `unknown`
  (PR3 classifies). Verified live on real WinRT OCR (gated test asserts every bbox is in `[0,1]`).
- **Schema `schema_version` 2 → 3 (forward-only).** New `frame_text` (preserved `raw_text` +
  filtered `content_text` + `primary_source`/`filter_version`/`suppressed_count` + foreground
  metadata), `frame_text_fts` (content-text FTS) and `frame_text_raw_fts` (raw FTS), per-word
  `text_spans` (normalized geometry + role, CHECK-constrained), and `chrome_text_catalog` (PR3's
  suppression counter). Every per-frame table cascades on frame delete. On a clean DB the legacy
  `ocr_text` table is dropped — `frame_text` is the single text store (`03 §4`).
- **New types.** `TextSource` / `TextRole` / `SuppressReason` enums + `TextSpan`; `OcrResult` gains
  `spans`; `FrameDetail` replaces `text` with `raw_text` + `content_text` + `text_source` +
  `suppressed_text_count`; `SearchQuery` gains `include_chrome` (default `false`). ts-rs bindings
  regenerated.
- **Default retrieval is now content text.** Hybrid search's FTS arm runs over `content_text`;
  `include_chrome=true` adds a raw-text FTS arm (chosen over a role-filtered spans FTS — roles
  aren't populated until PR3). Embeddings and Ask grounding read `content_text`. The FTS fallback is
  never removed.
- **Moment view** surfaces `content_text` with `raw_text` always viewable via a disclosure — raw is
  preserved and viewable even though search defaults to content.

### Docs — Planned 0.2.0 roadmap: attention-first text retrieval + recall reports
- **Added `docs/0.2.0.md` (now tracked).** Plans the 0.2.0 line. Core fix: today the app indexes
  raw full-screen OCR with no filtering, so search / Ask / embeddings get dominated by static
  chrome (toolbars, taskbar, desktop icons, the app's own sidebar labels). 0.2.0 derives a filtered
  **content-text** layer used by default; raw full-screen text is preserved and stays searchable
  opt-in via `include_chrome`.
- **Scope:** PR1 specs → PR2 data model + OCR bounding-box spans → PR3 attention-first filtering
  (clean DB, no backfill) → PR6 Recall reports + premade prompt cards → PR7 audit. Event-driven
  capture, UIA text, and smart enrichment throttling are deferred to **0.2.1**. Each PR recycles
  `specs/04_CLAUDE_CODE_BUILD_PROMPT.md` as its operating prompt.
- **Planning doc only — no runtime or code changes.** Also un-ignored `docs/0.2.0.md` in
  `.gitignore` so the roadmap is version-controlled.
- **Added PR8 (parallel model download) to the 0.2.0 line.** Promotes the previously-deferred
  single-stream download-speed limitation to a scheduled, individually-gated PR: a multi-connection
  chunked downloader (N parallel HTTP `Range` requests → one pre-allocated file) that reuses the
  existing progress / resume / stall / clean-layout machinery in `crates/inference/src/download.rs`.
  Independent of the retrieval chain, sequenced last. Planning only — no runtime or code changes
  (`docs/0.2.0.md` PR8; the `specs/07_KNOWN_GAPS.md` item is relabeled from a deferred 0.1.1 TODO to
  scheduled 0.2.0 PR8).

### Docs — 0.2.0 PR1: specs contract for attention-first content text + Recall reports
- **Wrote the 0.2.0 contract into `/specs/` (specs-only; no runtime or code changes).** The roadmap
  in `docs/0.2.0.md` is now the authoritative spec contract that PR2–PR7 implement:
  - `specs/02 §5b` — the **0.2.x arc** (P6: attention-first text signal + recall workflows), framed
    as a post-1.0 arc, **not** retrofitted into the P0–P5 v1.0 framing.
  - `specs/03` — raw vs **content text** (filtered OCR/UIA, *not* vision descriptions; the default
    retrieval input), active/target-window semantics, text spans + roles, static chrome suppression;
    the `frame_text` / `text_spans` / `chrome_text_catalog` schema (`schema_version` 2→3);
    `TextSource` / `TextRole` / `SuppressReason` plus `OcrResult.spans`,
    `FrameDetail`, and `SearchQuery.include_chrome`; and `generate_report` reports (`§8b`). Default
    search stays **hybrid (FTS + vector) over content text**; raw text is preserved but opt-in via
    `include_chrome`.
  - `specs/UI_REFERENCE.md` — Recall = **Search / Ask / Reports**, content-text default + raw/chrome
    toggle, premade Ask cards, all five view states for the new/changed screens.
  - `specs/07` — deferrals (event-driven capture, UIA text, smart enrichment throttle → 0.2.1;
    scheduled reports) and the **PR2→PR3 interim**: `content_text` is a passthrough copy of
    `raw_text` until PR3's filter lands, with no backfill (clean-DB assumption).
  - `specs/04` — `docs/0.2.0.md` added to the reading order; the PR1→PR2→PR3→PR6→PR7 build order
    appended alongside P0–P5.
  - Review-round hardening (PR #30): `suppress_reason` is now `Option<SuppressReason>` (no redundant
    in-enum `None`, mapping the nullable column); `frame_text` / `text_spans` DDL gained `CHECK`
    constraints on the enum columns; `reports.map_reduce_min_frames` default lowered 40→20 (worst-case
    single-pass fit, so frames batch rather than drop before the 8192 answer context overflows); and
    `docs/0.2.0.md`'s status now records PR1 as complete (PR2 next), resolving the roadmap/contract
    contradiction now that `04` makes the roadmap mandatory reading.

## [0.1.0] — 2026-06-24

### Packaging — First public release: standalone unsigned Windows installer
- **First tagged release (`v0.1.0`).** Ships a standalone **NSIS installer** (`.exe`) produced by
  `tauri build`. `bundle.targets` is pinned to `["nsis"]` (was `"all"`) so the build emits a single
  installer and does not require the MSI/WiX toolchain.
- **Version bumped `0.0.0` → `0.1.0`** across the workspace `Cargo.toml`, `src-tauri/tauri.conf.json`,
  and both `package.json` files (root + `ui/`).
- **The installer carries only the ~11 MB app binary.** The llama.cpp sidecar and all GGUF / embedding
  models download into the per-user app-data folder (`%APPDATA%\app.screensearchv2c.desktop\`) on first
  run — nothing heavy is bundled. ONNX
  Runtime is **statically linked** into the binary via `ort`'s `download-binaries` (not `load-dynamic`),
  so no separate `onnxruntime.dll` is shipped.
- **Unsigned by design.** Windows SmartScreen warns on first launch (*More info → Run anyway*); a
  code-signing certificate remains a follow-up (see `specs/07_KNOWN_GAPS.md` #26). First run requires
  internet and downloads ~8–10 GB of models; a Vulkan-capable GPU is recommended (CPU fallback exists).
- Everything below (P0–P5: capture, data spine, OCR/embeddings, vision + sidecar inference, the
  Command-Deck UI) is part of this first release.

### Fixed — Quality (8B) vision download stuck in a retry storm (2026-06-24)
- **Selecting the Quality vision tier could leave the model perpetually "downloading" / failing.**
  Root cause (reproduced deterministically): nothing prevented a *second* app instance from running
  against the **same** app-data dir — and users relaunch/spam when nothing seems to be happening — so
  two processes raced hf-hub's per-model cache lock. The loser failed with `LockAcquisition` after
  ~5 s and the vision scheduler retried instantly (a download "storm"); the largest model (8B) was the
  most exposed because it holds the lock longest. Two complementary fixes:
  - **Single-instance enforcement** (`tauri-plugin-single-instance`, registered first): a second
    launch focuses the running window and exits, so only one ScreenSearch ever touches the SQLite DB
    and the model cache (also prevents DB corruption / double-capture). (`src-tauri/src/lib.rs`.)
  - **Resilient downloader**: a `LockAcquisition` during any residual contention now **backs off and
    retries** (linear, bounded) instead of hard-failing into a storm. A lock left by a force-killed
    process is released by the OS, so this only ever waits on a genuinely live holder.
    (`crates/inference/src/download.rs`.)

### Fixed — Settings opened a flashing console window (2026-06-24)
- **Opening Settings briefly flashed a terminal window.** Enumerating GPU devices for the inference
  panel ran `llama-server --list-devices` as a console process without Windows' `CREATE_NO_WINDOW`
  flag, so a console flashed each time Settings opened (alarming, looks malware-ish). Now suppressed,
  matching the sidecar/`probe_caps` spawns. (`src-tauri/src/lib.rs::list_devices_from_binary`.)

### Known limitation — first-run model download speed (RESOLVED in 0.2.0 PR8)
- Models used to download **single-stream** via hf-hub (~20 MB/s; HuggingFace's CDN throttles single
  connections), so the ~5 GB Quality vision model took a few minutes. **Resolved by the 0.2.0 PR8
  multi-connection chunked downloader above** (single-stream remains the fallback for a range-less
  server).

### Added — Model-tier tooltips (2026-06-24)
- **The Default / Quality / Beta tier buttons now show which model they map to** on hover and
  keyboard focus (e.g. Answer → Quality → "Qwen3-4B-Thinking"), so the tier names aren't opaque.
  The names mirror `crates/inference/src/models.rs::repo_for`. (`ui/src/components/domain/ModelTierPicker.tsx`.)

### Fixed — Review hardening (PR #26): size-probe timeout, broadcast lag, orphan pidfile (2026-06-24)
- **The HF tree-API size probe can no longer hang the download.** `total_download_bytes`
  built a `reqwest::Client` with no timeout, and it runs *before* the stall watchdog is
  spawned — so a hung API would block the download from ever starting. It now uses a 15 s
  timeout (`HTTP_API_TIMEOUT`); the size is best-effort anyway and the download proceeds
  regardless. (`crates/inference/src/download.rs`.)
- **The status/download event bridges survive a lagging receiver.** Both
  `while let Ok(_) = rx.recv().await` broadcast loops terminated on *any* error, including
  `Lagged` — a burst of events would silently kill the bridge and freeze the StatusRail /
  download chip for the rest of the session. They now skip on `Lagged` and stop only on
  `Closed`. (`src-tauri/src/lib.rs`.)
- **A surviving sidecar is no longer stranded as an orphan.** `kill_and_confirm` removed the
  pidfile unconditionally; if the process outlived the kill, the next launch's
  `reap_stray_any` then had no record to clean it up. The pidfile is now kept when the
  process is still alive (and only removed once it has exited). (`crates/inference/src/supervisor.rs`.)
- **The Ask context budget no longer undercounts non-Latin text.** The token estimate used a
  `chars/3` ratio, which *under*-counts dense scripts (a CJK character is ~3 bytes yet ≈1–1.5
  tokens), so CJK OCR snippets could admit several× too much context and re-trigger the very
  `exceed_context_size_error` the budgeting prevents. It now estimates from UTF-8 *bytes* at a
  conservative **2 bytes/token** upper bound (`BYTES_PER_TOKEN`) — safe for both Latin and the
  worst-case Mistral-family CJK tokenizer of the default answer model — and truncates on char
  boundaries. (`crates/inference/src/answer.rs`.)
- **Idle keep-warm can no longer lose a race with the evictor.** `maybe_evict` sampled the
  `backfill_active` / `pinned` keep-warm flags *before* taking the state lock but only re-checked
  `in_flight` *after* — so a keep-warm set in that window still evicted the model, reintroducing
  cold-start churn mid-drain. All three keep-warm signals are now re-checked under the lock.
  (`crates/inference/src/supervisor.rs`.)
- **The binary-download network calls can't hang either.** `resolve_binary_url` (GitHub releases
  JSON) gained the same 15 s request timeout, and `http_get_bytes` (the multi-MB llama.cpp zip)
  gained a 15 s **connect** timeout **plus a 30 s per-read-chunk `read_timeout`** — so a CDN that
  accepts the socket then goes silent mid-transfer fails fast (the binary path has no separate
  stall watchdog) without aborting a slow-but-progressing download. (`crates/inference/src/download.rs`.)
- **A small `ctx_size` no longer breaks grounded Ask.** With a low `sidecar.ctx_size` (Settings
  allows down to 512) the fixed 2048-token reply budget could reserve the *entire* window, so every
  Ask dropped all retrieved snippets and answered "(no relevant snippets found)". The reply budget
  is now capped to half the context window, leaving room for grounding; normal 4K/8K windows are
  unaffected. (`crates/inference/src/answer.rs`.)
- **A slow disk no longer makes a download falsely "stall".** Publishing a cached or
  just-downloaded multi-GB blob into the clean layout is local disk I/O, but it ran under the 180 s
  *network* stall watchdog — on a slow disk the copy could exceed it and report a spurious failure.
  The watchdog now pauses its stall counter during the clean-layout copy phase. (`crates/inference/src/download.rs`.)
- **Documented (not yet fixed):** a stall-abort can't cancel hf-hub 0.4.3's *detached* per-chunk
  tasks; the blast radius is bounded (committed-counter resume + per-lane serialization) and a real
  fix needs a cancellable downloader we own — tracked as known gap #46 (`specs/07_KNOWN_GAPS.md`).

### Fixed — "Ask" answer truncated to nothing; download now visible from anywhere (2026-06-24)
- **The Ask answer is no longer cut off / "always thinking."** The default answer models are
  *reasoning* models that stream a `<think>…</think>` trace before the answer; the reply budget was
  `ASK_MAX_TOKENS = 512`, which the reasoning alone exhausted — so generation was truncated
  mid-thought and the answer area stayed empty (only the Thinking box rendered). Raised to **2048**,
  enough to finish reasoning *and* produce the answer while still leaving most of the 8K context
  window for retrieved snippets. (`ui/src/routes/Recall.tsx`.)
- **Model downloads now show in the top status rail**, not just on the Settings page — a "↓ 42%"
  chip (with a model/size tooltip) appears whenever a fetch is in progress, so the multi-GB download
  is visible from any screen. (`ui/src/components/shell/StatusRail.tsx`, new `IconDownload`.)

### Fixed — Model download could hang on a resumed/incomplete fetch; progress bar was untruthful (2026-06-24)
- **The Inference-engine download no longer hangs on an interrupted prior attempt, and the
  progress bar now shows *real* progress.** Root cause (proven against the live partial 8B fetch):
  the previous bar polled the on-disk file size, but hf-hub `set_len`s the `.sync.part` to the file's
  full length *up front* — so the bar jumped to ~81% (≈5 GB allocated of 6.2 GB) while only **33%**
  had actually downloaded, then froze. The fetch itself stalled because hf-hub 0.4.3 builds a
  `reqwest` client with **no socket timeout** and we ran it with no retries, so a dead CDN connection
  blocks forever. Fixes:
  - Progress is now driven by hf-hub's `Progress` callbacks (true streamed bytes, **resume-aware** —
    a restart picks up where it left off), not the pre-allocated file size.
  - A **stall watchdog** aborts a download that receives no new bytes for 180 s and surfaces a clear,
    retryable error (`"download stalled — no data received; click Load to resume"`) instead of hanging
    indefinitely; the partial is kept on disk so the next Load/tag **resumes** from there.
  - A finalized-but-not-copied cached blob is now reused instead of re-downloaded (cache pre-check).
  - The **Inference engine** panel shows a failed-download banner with the reason and resume hint.
  (`crates/inference/src/download.rs`, `ui/src/components/domain/ModelPanel.tsx`.)

### Added/Fixed — Model-download progress, download reliability, load-pinning, thinking auto-scroll (2026-06-24)
- **Model downloads now show progress and surface errors.** A new coordinated
  `ModelDownloader` broadcasts `model_download` events (`downloaded_bytes` / `total_bytes` from the
  HF tree API); the **Settings → Inference engine** panel renders a live progress bar (e.g.
  "Downloading vision model… 42% · 2.6 / 6.2 GB") and toasts on completion/failure. Previously a
  multi-GB fetch was opaque network activity with no feedback. (`crates/inference/src/download.rs`,
  `ui/src/components/domain/ModelPanel.tsx`.)
- **Quality vision tagging no longer dead-letters from concurrent download races.** Downloads are
  now serialized per lane, so multiple enrichment workers can't fetch the same multi-GB model at
  once (the race that left `vision_tag` jobs `dead` with "download vision model"). The model
  downloads once (with progress); jobs then tag normally.
- **A manually loaded model stays loaded.** "Load model" now *pins* the model so the idle-TTL won't
  evict it (the "evicted right after it downloaded" surprise); "Unload" (or switching models) clears
  the pin. (`crates/inference/src/supervisor.rs`.)
- **The answer's "Thinking" trace auto-scrolls** and is height-bounded, so a long reasoning stream
  follows the latest line instead of overflowing the view (it stays put if you scroll up to re-read).
  (`ui/src/components/domain/AnswerStream.tsx`.)

### Fixed — Idle backfill, sidecar reload/status, and the "Ask" context-overflow failure (2026-06-24)
- **Idle vision tagging now drains the whole backlog while the PC is idle, instead of one batch
  then dormant.** The idle scheduler used to enqueue a single batch *only* on the transition into
  idle, so a long idle period processed one batch and stopped. It now keeps topping the queue up to
  a batch whenever in-flight vision work falls below a low watermark, and tells the sidecar to stay
  loaded (keep-warm) for the duration of the drain. When the backlog is empty or the user returns,
  keep-warm is released so the normal idle-TTL eviction frees the VRAM. The timer trigger is
  unchanged. (`crates/kernel/src/vision_scheduler.rs`, new `Store::pending_vision_job_count`.)
- **The sidecar idle-TTL no longer fights idle backfill.** A new keep-warm flag
  (`BackfillControl`, implemented by the supervisor) suppresses idle eviction *only* while the idle
  backfill is actively draining, so the model isn't evicted between batches. Extracted a pure
  `should_evict` predicate (unit-tested). (`crates/inference/src/supervisor.rs`, `crates/traits`.)
- **Model status no longer lies about "Ready".** An idle-evicted sidecar used to map to
  `ComponentStatus::Ready`; it now maps to a neutral `Disabled`, and the UI labels the engine from
  the *raw* `SidecarState` — "Loaded" / "Loading…" / "Idle — unloaded" / "Error" — in both the
  StatusRail chip and a new **Settings → Inference engine** panel. `SidecarStatus` gained a `lane`
  field so the panel shows vision vs. answer truthfully. (`crates/kernel/src/lib.rs`,
  `crates/traits/src/ipc.rs`, `ui/src/components/domain/ModelPanel.tsx`, `ui/src/lib/status.ts`.)
- **Manual Load / Unload controls.** The Inference engine panel can pre-load the answer model (so
  the next Ask is instant) or unload the resident model now to free VRAM, via new `load_model` /
  `unload_model` commands backed by `ModelSupervisor::{preload, unload}`.
- **"Ask" no longer fails with the opaque "sidecar returned an error status."** Root cause (verified
  by reproducing it directly against `llama-server`): the grounded prompt concatenated *all*
  retrieved chunks with no token budget, so a large context exceeded the model's window and
  llama-server returned `HTTP 400 exceed_context_size_error` — which the client swallowed. Fixes:
  (1) the client now surfaces the real HTTP status + response body; (2) `build_messages` budgets the
  included context to the model's `ctx_size` (reserving the reply + prompt overhead), dropping or
  truncating chunks that don't fit and citing only the frames actually included.
  (`crates/inference/src/client.rs`, `crates/inference/src/answer.rs`.)
- **Eviction/unload now verify the kill.** Every sidecar teardown (idle eviction, manual unload,
  model switch, shutdown) routes through a shared `kill_and_confirm` that waits for the process to
  exit and logs `killed` / `still_alive`, so a kill that fails to free VRAM is visible rather than
  silent. (`crates/inference/src/supervisor.rs`.)

### Added — Sidecar memory tuning: pinned context + KV quantization + flash attention (2026-06-24)
- **The llama.cpp sidecar now pins a small, workload-appropriate context window and quantizes its
  KV cache**, instead of inheriting the model's full trained context. `build_args` previously passed
  no `--ctx-size`, so llama.cpp defaulted to `0` ("loaded from model") — for the default models that
  is a **262 144-token** trained context, and the server auto-sized a ~118 k-token KV cache that
  alone consumed most of a 16 GB GPU. New balanced defaults: context **auto** (vision 4096 / answer
  8192), KV cache **q8_0**, flash attention **auto**.
  - **Measured (RTX 5060 Ti, bundled Vulkan `llama-server`, default answer model):** peak VRAM for
    the answer model dropped from **14082 MiB** (untuned, `n_ctx` 118272) to **4253 MiB** (tuned,
    `n_ctx` 8192) — a **~9.8 GB / ~70%** reduction, with no expected quality loss for this workload.
- **Three new user-adjustable settings** (Settings → Sidecar (advanced)), all applied on the next
  sidecar launch via the existing relaunch-on-next-request path:
  - `sidecar.ctx_size` (`Settings.sidecar_ctx_size`, default **0 = auto**, else clamped
    **512–32768**) — `0` substitutes a per-lane default (vision 4096 / answer 8192); any other value
    overrides both lanes.
  - `sidecar.kv_cache_type` (`Settings.sidecar_kv_cache_type`, new `KvCacheType` enum: `f16` /
    `q8_0` / `q4_0`, default **q8_0**) — quantized KV is emitted only when flash attention is active.
  - `sidecar.flash_attn` (`Settings.sidecar_flash_attn`, new `FlashAttnSetting` enum: `auto` / `on` /
    `off`, default **auto**).
- **Version-safe flag emission.** The bundled `llama-server` is the *latest* llama.cpp Vulkan release
  fetched at runtime, so its flag spelling varies (`--flash-attn` bare boolean vs. `on|off|auto`;
  quantized `--cache-type-v` needs flash attention). A new `inference::flags::probe_caps` reads the
  binary's `--help` once at init; `build_args` emits only verified flags and silently degrades (e.g.
  drops KV quantization when flash attention is unavailable) rather than producing an arg list the
  binary would reject.
- **Review-round hardening (PR #25):**
  - `FlashAttnSetting::Auto` now emits `--flash-attn auto` (defer to llama.cpp) on value-taking
    binaries instead of forcing `on`, so `Auto` and `On` are observably distinct; `On` still forces
    `on`. Both keep flash active for the purpose of unlocking quantized KV.
  - The `probe_caps` `--help` call (a blocking syscall) now runs on `spawn_blocking` so it never
    parks the async executor, falling back to the conservative flag set if the task fails.
  - `push_kv_cache` warns when quantization is configured but the binary advertises neither
    `--cache-type-k` nor `--cache-type-v` (previously a silent f16 fallback).
  - The `--flash-attn` capability probe also recognizes a parenthesised value set (`(on/off/auto)`).
  - **Round 2 (diagnostics + robustness):** `push_flash_attn` now returns a `FlashState`
    (`Active` / `BinaryUnsupported` / `UserDisabled`) so the quantized-KV downgrade warning names the
    real cause — a binary limitation vs. the user turning flash attention off — instead of always
    blaming the build. An explicit `flash_attn = on` on a binary that lacks the flag now logs a warn
    rather than being dropped silently. And `set_settings` sanitizes the incoming `Settings` *before*
    building `SidecarParams`, so a direct (non-UI) IPC call with an out-of-range `sidecar_ctx_size`
    can no longer run the raw value in the next sidecar spawn while the DB stores the clamped one.

### Fixed — Vision scheduler: configurable batch size + pending-job dedup (2026-06-24)
- **The timer/idle vision batch size is now a setting.** It was a hardcoded `const BATCH = 20`, so
  the scheduler logs showed a fixed `count=20` every tick with no way to drain a backlog faster.
  Added `enrich.vision_batch_size` (`Settings.enrich_vision_batch_size`, default **20**, clamped
  **1–500**) wired through `load`/`save`/`sanitize`, surfaced as a "Frames per run" field in the
  Settings → Enrichment schedule control, and read fresh each run so it applies on the next
  scheduled tick.
- **The scheduler no longer re-enqueues frames that already have an in-flight `vision_tag` job.**
  `untagged_frame_ids` only filtered on "no `vision_analysis` row", so every tick re-queued the same
  oldest-untagged frames while their jobs were still `pending`/`running`, and the timer + idle lanes
  double-queued the same frames when they fired together. Added a `NOT EXISTS` guard on
  `jobs(kind='vision_tag', state IN ('pending','running'))` so a trigger enqueues only genuinely
  eligible frames; a finished (`done`) job or a job of another kind still leaves an untagged frame
  eligible. Resolves the "batch size N" and "no pending-job dedup" threads of gap #19.
- **Review hardening (PR #23):**
  - **No infinite retry of poisoned frames.** The dedup now also excludes frames whose `vision_tag`
    job `dead`-lettered (exhausted retries without writing a `vision_analysis` row); previously such a
    frame was re-enqueued every tick forever. On-demand single-frame re-tagging still bypasses the
    query, so a dead frame can be force-retagged.
  - **Indexed the dedup lookup.** Forward migration `schema_version` → **2** adds
    `idx_jobs_frame_kind_state` on `jobs(frame_id, kind, state)` so the correlated `NOT EXISTS` doesn't
    scan the whole `jobs` table per candidate frame.
  - **Atomic scheduled enqueue.** The timer and idle producers now share an async mutex across their
    read-then-enqueue, so a simultaneous wake can't both read the same frames before either inserts —
    closing the residual timer/idle double-queue race.

### Docs — README refresh + visual showcase (2026-06-24)
- **Added a Screenshots section** to `README.md` showing five live Command-Deck screens (Deck,
  Recall/Ask, Insights, Moment, Timeline). Renamed the committed `screenshots/screensearch_*.png`
  captures to descriptive names (`deck.png`, `recall-ask.png`, `insights.png`, `moment.png`,
  `timeline.png`).
- **Fixed the Build & run order.** The Rust build was listed before the UI build, but
  `src-tauri`'s `generate_context!` embeds the git-ignored `ui/dist`, so cargo fails unless the UI
  is built first. Reordered to UI → Rust, switched `cargo build` to `cargo build --workspace`, added
  the `npm run lint` Rules-of-Hooks gate and the `git diff --exit-code -- ui/src/bindings` binding
  guard — matching CI.
- **Linked the live-verification audit** (`docs/AUDIT_V2C_2026-06-23.md`) from the status blockquote
  and added `AGENTS.md`, `screenshots/`, and the audit doc to the repository-layout map.
- **Aligned the status with the audit.** Reworded "P0–P5 complete" to "P0–P4 complete and verified;
  P5 UI feature-complete with keyboard/state/a11y verification in progress," marked the P5 row
  `🚧 Feature-complete; UI verification in progress`, and named the open UI gaps (the keyboard/state/
  a11y matrix and a no-evidence answer still rendering cited-frame tiles).

### Docs — Added V2c audit artifact (2026-06-23)
- Added `docs/AUDIT_V2C_2026-06-23.md`, recording the non-packaging audit pass, including CI
  gates, hardware/model smokes, GPU runtime evidence, and the later live GUI follow-up for
  synthetic capture/search/Ask, no-evidence refusal, persisted Moment VLM analysis, and P5
  route/state/a11y observations.
- PR #21 review follow-up mirrors the audit follow-up into `specs/05_BUILD_REVIEW.md`,
  `specs/06_PATCH_PLAN.md`, and `specs/07_KNOWN_GAPS.md`, and clarifies the local Node 26
  verification caveat against CI's Node 22 runtime.

### Fixed — P5 comprehensive review hardening (2026-06-23)
- **Date ranges now use local calendar midnights.** Deck, Timeline, Insights, and Recall no longer
  derive "today" / trailing windows from fixed 24-hour offsets, avoiding DST drift.
- **Navigation and IPC are bounded.** Timeline nearest-frame opens are constrained to the selected
  range, while frame-list, frame-context, Timeline, and Insights command sizes are clamped for direct
  IPC callers.
- **Operational state is explicit.** Deck panels now show retryable error states, the Command
  Palette exposes combobox/listbox ARIA semantics, Timeline canvas colors come from CSS tokens, and
  backend `toast` / richer `job_completed` events drive more precise UI refreshes.
- **Storage and retention are real.** `storage.retention_days` is enforced at startup and hourly in
  safe bounded batches, and StatusRail displays DB, frame, and total storage size via
  `get_storage_stats`.
- **Streaming answers are request-scoped and cancellable.** `answer_delta` carries a request id,
  superseded asks are cancelled, and stale deltas are ignored by the UI.
- **Settings are more live and discoverable.** Embedding workers reconfigure when enrichment lanes
  change, monitor selection uses real `get_monitors` data with a manual fallback, Timeline/Insights
  bucket counts adapt to chart width, and advanced llama.cpp device selection uses
  `--list-devices` / `--device` through the optional `sidecar.device` setting.
- **PR #19 review follow-up:** fixed ask-task cleanup race, made retention continue past a
  per-frame DB delete failure, corrected monitor toggling from the default "all monitors" state,
  refreshed sidecar devices when readiness becomes ready, removed redundant device controls, and
  clarified that capture enqueue settings apply on the next capture start while worker claiming
  updates after save.
- **PR #19 Codex follow-up:** enabling image embeddings after startup now reloads the FastEmbed
  provider with the image lane instead of letting `embed_image` jobs fail until restart, and
  retention removes frame files before DB rows so transient Windows file locks do not create
  permanent orphan JPEGs.
- Packaging remains deferred as the separate DoD §13.9 follow-up.

### Docs — Refreshed root `CLAUDE.md` to current state (2026-06-23)
- **Corrected the stale "current state" headline.** It claimed *"specification complete, no
  application code yet — the build starts at P0"*; it now reflects reality: **P0–P5 complete and
  merged to `main`, in post-merge hardening**, with a 9-crate Rust workspace + React/TS UI.
- **Fixed the app run/build command.** Replaced `cargo tauri dev` / `cargo tauri build` (no
  `cargo-tauri` installed) with `npm run tauri dev` / `npm run build`.
- **Made Build/verify match CI.** Documented the UI-before-cargo order (`generate_context!` embeds
  the git-ignored `ui/dist`), the `npm run lint` gate, `--all-targets` on clippy, `--workspace` on
  build/test, the `git diff --exit-code -- ui/src/bindings` binding guard, and the Rust 1.82 / Node
  22 toolchain.
- **Added a compact "Where the code lives" crate map** so an agent can orient without reading the
  full spec. No code or behavior changes.

### Fixed — P4 sidecar hardening (2026-06-23)
- **Model switches no longer cut off active sidecar requests.** `ModelSupervisor` now serializes
  sidecar leases, so a lane/tier switch waits for the current answer stream or vision tag request to
  finish before killing and replacing the running `llama-server`.
- **Same-model reuse validates the running sidecar.** Before handing out an existing process, the
  supervisor checks both OS liveness and bounded `/health`; a dead or unhealthy sidecar is marked
  crashed, stopped, and respawned for the current request.
- **Sidecar HTTP calls now have deadlines.** Health checks, non-streaming vision completions, and
  streaming answer waits are bounded so a hung localhost sidecar returns an error instead of pinning
  a worker or answer stream forever. Vision jobs retry through the durable queue; answers surface a
  terminal error delta.
- **`SSV2C_LLAMA_RELEASE_URL` really overrides existing installs.** The override now resolves before
  normal install reuse and extracts into a URL-specific override directory, preserving the app-managed
  Vulkan release.
- **PR #18 review follow-up:** sidecar leases now use shared/exclusive gate semantics, so same-model
  requests can run concurrently while model switches and crash recovery still drain all request slots
  before stopping `llama-server`. Stream connection and SSE-idle deadlines are split, override zip
  extraction is staged atomically in a `.partial` directory on a blocking thread, and startup reap now
  recognizes exact binaries from both normal and URL-specific override installs. A second review
  follow-up emits the observed crashed model in a crash-recovery/model-switch race and runs installed
  binary reaping before override download, so a bad override URL cannot strand an old sidecar.

### Fixed — P3 deferred enrichment hardening (2026-06-23)
- **Vision-only enrichment no longer waits on embeddings.** The worker pool now starts from either
  provider slot: embedding jobs are claimed only when an embedder is attached and enabled, while
  `vision_tag` jobs drain as soon as inference attaches. This fixes stalled vision backlogs when text
  and image embeddings are disabled or unavailable.
- **Backend search limits are clamped.** `SearchQuery.limit` is normalized to `1..=100` and the
  hybrid-search candidate pool is capped at 500, matching the Recall UI and protecting direct IPC
  callers from oversized hydration work.
- Added regression coverage for vision jobs draining without an embedder, excessive search limits,
  and zero-limit normalization. Updated P3 architecture/spec notes and the live-event comment to
  match the current worker/event behavior. No schema, IPC, `ts-rs`, or trait signature changes.
- **PR #17 review follow-up:** the worker loop now builds claim kinds in a fixed stack array instead
  of allocating a `Vec` on every idle poll, the `job_progress` comment now says it fires after each
  job attempt completes, and the startup-scoped embedding-lane flags are tracked as a live
  reconfiguration follow-up.

### Fixed — P2 capture hardening (2026-06-23)
- **Capture readiness now clears after unexpected source shutdown.** If the WGC capture source exits
  without the user pressing Stop, the kernel removes the live capture handle and reports
  `capture = Error` with a detail instead of leaving the UI stuck on `Ready`.
- **OCR-unavailable machines no longer create empty OCR rows.** If WinRT OCR cannot be created (for
  example, no recognizer language is installed), the app still boots but capture start fails with
  `capture = Unavailable`; the defensive fallback provider now errors instead of silently returning
  empty text.
- **Backend settings are sanitized on load and save.** Direct IPC or hand-edited DB values are clamped
  to the same numeric bounds as the Settings UI, including a finite `[0,1]` capture diff threshold.
  Added regression tests for source shutdown, OCR-unavailable start refusal, and malformed persisted
  settings. No schema, IPC, or `ts-rs` binding changes.
- **PR #16 review follow-up:** the unexpected-source-shutdown supervisor now keeps the capture mutex
  held while publishing `capture = Error`, closing a restart race where a new capture session could
  report `Ready` and then be overwritten by the old loop's shutdown error.

### Fixed — P0/P1 store hardening (2026-06-23)
- **Job finalization now requires a claimed running job.** `complete_job` and `fail_job` no longer
  rewrite pending, done, or dead jobs by id alone. This protects the durable queue state machine from
  stale-worker finalization after a retry, dead-letter, or stale-running recovery.
- **Older builds reject newer database schemas.** Opening a SQLite store with a `schema_version`
  greater than the compiled migration set now fails clearly instead of reporting the DB as ready.
- **PR #15 review follow-up:** the future-schema guard now derives the supported version from the
  compiled `MIGRATIONS` set and debug-asserts that `LATEST_SCHEMA_VERSION` stays in sync. The
  future-schema regression test now uses `tempfile::tempdir`, with `tempfile` centralized in workspace
  dependency versions for Rust tests.
- **PR #15 Codex review follow-up:** the periodic enrichment stale-job sweep now skips while the
  current worker pool has in-flight jobs. A long but live provider call stays `running`, so its later
  retry/dead-letter failure is accounted by `fail_job` instead of being requeued out from under the
  worker. Startup recovery still requeues leftover `running` jobs before workers start.
- Added regression tests for pending/done/dead job finalization and future-schema rejection. No IPC,
  `ts-rs`, schema, or trait signatures changed.

### Fixed — Vision-tagging quality (2026-06-22)
- **Vision tags no longer record a fabricated confidence.** The vision prompt previously showed a
  literal `"confidence": 0.0`, which the model echoed — so every tag was stored with a `0.0`
  certainty that *looked* real. The prompt now describes the confidence field instead of demonstrating
  a value, and parsing treats `0.0` (and any out-of-range value) as the same `-1.0` "unknown" sentinel
  the OCR path uses — we never invent a score.
- **`activity_type` is now a closed set, not free-form.** Vision tagging is constrained with a
  `response_format` JSON schema (the sidecar turns it into a sampling grammar), and any label outside
  the known set — coding, browsing, email, reading, chat, terminal, design, video — including the
  model's own "unknown", is dropped to none rather than stored. Keeps the Insights activity breakdown
  meaningful. Resolves `07_KNOWN_GAPS.md` #19/#20.
- **Low-signal frames stay untagged instead of being mislabelled (PR #14 review).** The activity
  grammar now also permits `null`, and the field is no longer required — a blank desktop, a lock
  screen, or the synthetic two-tone smoke image no longer forces the model to pick one of the eight
  labels just to satisfy the schema (which had been skewing the Insights breakdown). The prompt tells
  the model to answer `null` rather than guess. Verified on a real GPU: the gated smoke's two-tone
  test frame now comes back `activity=none, confidence=unknown` — the model honestly declines —
  whereas the forced-enum schema had it confidently (and wrongly) report `browsing` at `0.95`.
- **`app_hint` "null" is dropped case-insensitively (PR #14 review).** A model that emits the text
  `"null"`/`"NULL"`/`"Null"` (instead of a JSON null) no longer leaks it as an application name; the
  surviving hint is also trimmed.
- **Unknown confidence renders as `n/a`, not `-100%` (PR #14 review).** The Moment detail view now
  shows a neutral `n/a` chip when the stored confidence is the `-1.0` sentinel, instead of multiplying
  it into a misleading `-100%`.

### Added — P5 (M5) Settings & Insights (2026-06-22)
- **Settings**: a single editable form over every persisted setting — capture (interval, change
  threshold, monitors), storage (JPEG quality, max width) and retention, model tiers, the answer
  "thinking" toggle, enrichment (text/image embeddings, worker concurrency) and the deferred-vision
  schedule, privacy (excluded apps, pause-on-lock), and advanced sidecar knobs. Saves are optimistic
  and reconcile against the backend; model tiers additionally **hot-apply the instant you pick them**
  (the running provider switches without waiting for Save). Every field states *when* it takes effect
  — tiers now, the thinking flag on your next question, capture/storage/privacy on the next capture
  start, enrichment/sidecar on restart — and retention is labelled honestly as recorded-but-not-yet-
  enforced. A failed save keeps your edits and explains.
- **Insights**: truthful activity analytics computed entirely from your own history — total and
  vision-tagged capture counts, a captures-over-time density chart, your top foreground apps, and the
  vision activity-type breakdown. No fabricated numbers: when there's nothing yet it says "not enough
  history yet", and while vision tagging is still catching up the activity breakdown is labelled
  "tagged only" with the count it's based on. Lightweight inline charts (no chart library) keep the
  bundle small — Settings and Insights are each their own lazily-loaded route chunk.
- New reusable controls: `ModelTierPicker`, `ScheduleControl`, `RetentionControl`, and the Insights
  `CapturesTrend` / `InsightsBars` charts.

### Added — P5 (M3+M4) Recall, Deck, Timeline & Moment (2026-06-22)
- **Deck (home)**: capture status with one-click start/stop, today's activity (capture count,
  density minimap, top apps), the enrichment-queue meter, and a "jump back in" grid of your most
  recent captures. Onboards from empty ("start capture"), and flags when captures are recorded but
  not yet enriched.
- **Recall (search + ask)**: hybrid search over your screen text/meaning, shown as a virtualized
  result list that links straight to the frame; and a grounded **Ask** that streams an answer with a
  collapsible "thinking" trace and clickable citation thumbnails to the source moments. Tells you
  honestly when it's "searching text only" (embeddings still indexing) or the answer model isn't
  loaded — never a dead end.
- **Timeline (the Scanline)**: the signature instrument — a canvas density ribbon over a chosen
  window (Today / 7 days / 30 days) with a sweeping signal-orange scan-head, hover thumbnails, and a
  faint scanline texture. Fully keyboard-operable (arrows scrub, Shift for big steps, Home/End jump,
  Enter opens the moment under the head); the open lands on the real nearest frame. Ambient motion
  honors "reduce motion".
- **Moment (one frame)**: the capture image, its recognized text, the vision description/tags and
  context, prev/next and a strip of nearby frames, plus an on-demand **"Tag with vision"** action
  when a frame hasn't been analyzed yet. Deep-linkable and resilient to a missing/deleted frame.
- **Frame browsing data** (`get_frames`, `get_nearest_frame`): the backend can now list frames in a
  window (newest-first) and resolve a point in time to the nearest captured frame — what the Timeline
  thumbnails, the Deck recents, and "Enter opens the moment" need. New typed `FrameMeta`.
- Every screen renders all of loading / empty / error / partial / populated — no mock data, no dead
  ends. Initial JS stays ~87 KB gzipped (the markdown renderer ships only with Recall).

### Fixed — P5 (M3+M4) review follow-ups (2026-06-22)
- **Moment Prev/Next & "around this moment" now work in real sessions.** They were sourced from a
  newest-first window that, in a busy capture session, returned only frames from the far edge of the
  window — so the buttons silently went dead and the strip showed unrelated frames. The backend now
  serves the captures immediately *bracketing* a moment (`get_frame_context`), so navigation always
  lands on the true neighbours.
- **Answer links open safely.** Links inside a streamed answer now open in your browser instead of
  navigating away inside the app window.
- **The answer's "Thinking" trace no longer snaps shut** the instant the answer finishes — it stays
  as you left it so you can read it.

### Added — P5 (M1+M2) UI foundation, shell & primitives (2026-06-22)
- **The "Command Deck" shell**: the app now opens to a real frame — a top StatusRail (live
  capture / data store / enrichment queue / inference status), a left NavRail (Deck · Recall ·
  Timeline · Insights · Settings), a ⌘K command palette (jump-to + start/stop capture), and a
  readiness banner that appears only while something is starting or needs attention.
- **Design system**: warm-graphite dark theme with a single signal-orange accent and Windows-native
  fonts, defined once as design tokens — every color, font, radius, and spacing in the UI references
  a token (no hardcoded styling). Ambient motion (the scanline texture) honors "reduce motion".
- **Typed data layer end-to-end**: every screen reads the backend through generated types only —
  typed command wrappers, a live-event subscription that keeps the status readouts fresh, and a
  streaming reducer for grounded answers. No hand-written API types.
- **Reusable building blocks**: Panel, Button, IconButton, Field, Select, Toggle, Chip, Toast,
  EmptyState, ErrorState, Skeleton, and Tooltip — accessible (visible focus, ≥32px hit targets,
  AA contrast) and built once for the screens that follow.
- **Routing & resilience**: real, deep-linkable routes for every screen (including a per-frame
  `/timeline/:id` and a friendly not-found), each wrapped in its own error boundary so a broken
  view never blanks the whole app. Routes are code-split to keep startup small (~86 KB gzipped).
- The data-bearing screen bodies (search, the Scanline Timeline, settings, insights) build on this
  foundation in the next milestones.

### Fixed — P5 (M1+M2) PR #11 review (2026-06-22)
- **Command palette recovers from a no-match search**: after typing a query that matches nothing
  and pressing the down arrow, the highlight no longer gets stuck — clearing back to a matching
  query re-highlights a command and Enter runs it again.
- **Correct keyboard hint**: the palette shortcut now reads **Ctrl+K** (this is a Windows-only app),
  not the Mac ⌘K.
- **Clearer error screens**: a route error now shows the actual status/detail of a failed navigation
  (not just a generic message), and the command-palette input no longer triggers browser
  autocomplete/spellcheck overlays.
- **Safer answers (forward-looking)**: starting a new question while one is still streaming is now
  blocked, so a previous answer's late text can't bleed into the next one.

### Added — P5 (M0) backend completion (2026-06-22)
- **Timeline data** (`get_timeline`): frame-density buckets over a time range — the data behind
  the Scanline Timeline. Sparse and half-open, with a presentation-driven bucket count.
- **Insights data** (`get_insights`): real activity aggregates — total and vision-tagged frame
  counts, capture density over time, top apps, and an activity-type breakdown. Honest-empty when
  there isn't enough history; never fabricated.
- **Settings read/write** (`get_settings` / `set_settings`): the full settings object round-trips
  through the key/value store. Model-tier changes apply live; other settings persist and take
  effect on the next capture start or app restart (the Settings screen will label which is which).
- **Frame images in the UI**: enabled Tauri's asset protocol (scoped to the capture-frames folder)
  so the interface can show stored screenshots, and replaced the permissive dev CSP with a tight,
  local-only content-security policy.
- New typed IPC: `InsightsSummary` / `AppCount` / `ActivityCount`.
- Packaging (installer + portable ZIP) is intentionally deferred to a later pass.

### Fixed — P5 (M0) PR #10 review (2026-06-22)
- **Timeline math is overflow-safe**: extreme or malformed time ranges from the UI can no longer
  panic the timeline query — an unrepresentable range simply returns no buckets.
- **Settings save is now atomic**: writing settings commits all keys in a single transaction, so a
  crash or error part-way through can never leave a half-updated settings store.
- **Insights skips needless work** on an empty or invalid time range, returning the honest-empty
  summary immediately.

### Added — P4 Inference sidecar (2026-06-21)
- **No-orphan guarantee, proven first** (`02 §2`, `03 §6`, DoD #7): the `llama-server`
  sidecar is bound to a Windows **Job Object** with `KILL_ON_JOB_CLOSE`, and the child
  is assigned to it **before** its main thread resumes — so if the app dies for any
  reason, the OS takes the sidecar down with it. A cross-process test (kill the parent,
  assert the child dies) is the gate; it passes.
- **Model supervisor** (`03 §6`): lazy spawn on first use, `/health`-gated startup,
  idle eviction after `sidecar.idle_ttl_secs`, crash restart, a startup **reap** of a
  stray sidecar from a prior run (pidfile + image-path sentinel — never an unrelated
  process), and model switching (stop + restart with a new GGUF; vision adds `--mmproj`).
- **Vision tagging** (`03 §3/§5`): on-demand (the `enqueue_vision` command), plus
  opt-in **timer** and **idle** producers (`GetLastInputInfo`) that batch still-untagged
  frames — never real-time. The worker runs the frame's JPEG through the sidecar and
  stores the analysis.
- **Grounded, streaming answers** (`03 §13.5`): the `ask` command retrieves top-K
  hybrid hits, grounds the answer in their OCR text, and streams typed `answer_delta`
  events — `thinking` trace, answer tokens, and one citation per source frame. Reasoning
  is handled both as a `reasoning_content` field and as inline `<think>` tags.
- **Tiered models, runtime-downloaded, Rust-only** (`MODEL_REGISTRY`, `01 §5`): vision +
  answer each offer Default/Quality/Beta via `set_model_tier`. The `llama-server` binary
  (prebuilt Vulkan release) and the GGUF weights (+ same-repo mmproj) download on first
  use — the binary from GitHub, the models via `hf-hub` (no Python in the runtime).
- New IPC: `ask`, `enqueue_vision`, `set_model_tier` commands; `answer_delta` +
  `sidecar_status` events; `sidecar` readiness now tracks the supervisor's lifecycle.
- Real end-to-end inference (download + GPU) is covered by `#[ignore]`d smoke tests
  (`cargo test -p inference --test smoke -- --ignored`); the always-green suite proves
  the lifecycle, the HTTP client (against a mock), and the providers deterministically.

### Fixed — P4 sidecar binary resolution (2026-06-22)
- **Resilient llama.cpp release selection** — `ensure_binary` previously read GitHub's
  single `/releases/latest`, which fails with `no win-vulkan-x64 asset in the latest
  llama.cpp release` whenever that release ships an **incomplete asset set** (llama.cpp's
  CI occasionally publishes a build with only a partial set of platform zips — observed:
  `b9753` had 1 asset, no Vulkan zip). Resolution now scans the recent-releases list and
  takes the newest release that actually carries the `*-win-vulkan-x64.zip` asset. Pure
  selector `pick_vulkan_from_releases` is unit-tested (incomplete-newest-then-complete,
  newest-wins, none-found). Vulkan stays the shipped lane — it is vendor-neutral and runs
  on Blackwell GPUs (RTX 50-series) where the prebuilt `cuda-12.4` asset would not.

### Fixed — P3 review (PR #7, 2026-06-21)
- **Stale-job recovery** never misses a job now: the startup sweep requeues every
  `running` job unconditionally, and `claim` + the periodic sweep share one DB clock
  (no Rust-millisecond vs SQLite-second mismatch).
- **Image embedding** retries on a transient file lock (AV / indexer / backup) and
  only dead-letters a genuinely missing JPEG — robuster on Windows.
- The worker pool now stops its background tasks even if its handle is dropped
  without a graceful shutdown (`Drop` signals stop), so it can't keep draining the
  queue after an unexpected teardown.

### Added — P3 Deferred enrichment (2026-06-21)
- **Embedding worker, end-to-end** (`02 §5`, `03 §5/§13.2`): a bounded worker pool
  drains the `embed_text` jobs capture enqueues into vectors via **fastembed**
  (in-process ONNX, no Python) — `EmbeddingGemma-300M` text embeddings (768-dim),
  with optional `nomic-embed-vision-v1.5` image embeddings behind
  `enrich.image_embeddings`. Workers run in the background, draining the backlog
  independent of capture; concurrency is `enrich.worker_concurrency`.
- **Hybrid search is live** (`03 §13.4`): once the model loads, the vector arm of
  `hybrid_search` (FTS5 + sqlite-vec KNN → RRF) lights up. Measured **p95 ≈ 33 ms
  over 10 000 frames** — well under the ~200 ms bar. A typed `search` command
  (`SearchQuery → SearchHit[]`) makes it reachable (UI lands in P5).
- **Durable, self-healing queue**: jobs retry with exponential backoff and
  dead-letter at `max_attempts`; a job a crashed worker left `running` is requeued
  by a startup + periodic sweep (`03 §6`). A `job_progress` event surfaces queue
  depth; `embed_model` readiness reports loading → ready/unavailable.
- The embedding model loads off the launch thread, so app start never blocks on the
  first-run model download.
- Vision tagging stays fully deferred to P4 (no `vision_tag` is ever auto-enqueued).

### Added — P2 Capture happy path (2026-06-21)
- Always-on **capture → OCR → store** pipeline (`02 §5`, `03 §3/§5`): screen capture
  via Windows.Graphics.Capture with a diff gate that stores only changed frames,
  text recognition via WinRT `Media.Ocr` on a COM STA worker, JPEG storage, and a
  row written to `frames` + `ocr_text` — each stored frame enqueueing an
  `embed_text` job (consumed in P3). Vision is never auto-run (`13.3`).
- `kernel` orchestrator: the capture loop, a typed event bus (`capture_tick` /
  `readiness_changed`), a typed-`Settings` loader, and idempotent
  `start_capture`/`stop_capture`. Capture is **off until you start it**
  (privacy-first); `privacy.excluded_apps` and `privacy.pause_on_lock` are honored.
- App: `capture_control` (start/stop) and `get_frame` commands, live `db` +
  `capture` readiness, and a **minimal live timeline** UI (Start/Stop, readiness
  strip, a row per captured frame). The full Command-Deck UI lands in P5.
- Captured frames carry the foreground app/window as `app_hint` / `window_title`.
- OCR `mean_confidence` is recorded as `-1.0` ("unknown") — WinRT OCR exposes no
  confidence; no value is fabricated (see `specs/06_PATCH_PLAN.md` #2).

### Documentation (P0/P1 review pass)
- Reviewed merged P0 + P1 against the spec — both complete and compliant, no
  correctness bugs (record in `specs/05_BUILD_REVIEW.md` Pass 3). Doc/clarity
  touch-ups only: `TimeRange` now states its half-open `[start, end)` semantics
  explicitly; the concurrent-claim test documents what it actually proves; and the
  hybrid-search latency (`03 §13`) and vector-arm time-range approximations are now
  tracked gaps for P3 (`specs/07_KNOWN_GAPS.md` #7/#8). No behavior change.

### Fixed (post-P1 review, PR #4)
- `open_state` reports `db = Error` (not `Ready`) if the post-open schema-version
  probe fails — no "ready but unqueryable" store reaches the UI.
- `complete_job` / `fail_job` now error on an unknown job id instead of silently
  doing nothing (consistent with the queue's no-silent-loss contract).
- `insert_vision` also fills `frames.activity_type` (`03 §4`: "filled by vision")
  so the timeline can filter by activity without a join.
- Hybrid-search hydration uses two bulk `IN` queries instead of per-hit queries
  (removes an N+1 pattern); shared `f32_blob` / `dedup_keep_order` helpers.

### Added — P1 Data Spine (2026-06-21)
- `store` crate — the durable data spine on SQLite (WAL) + sqlite-vec (`vec0`) + FTS5
  (`03 §4/§5`): forward-only schema migrations tracked in `schema_version`, and the
  full `Store` contract — frame / OCR / vision inserts, key-value settings, text &
  image embedding upserts with synchronized vector shadows, and `get_frame` /
  `delete_frame`.
- Durable **job queue** — atomic claim (`UPDATE … RETURNING`), priority + scheduled
  (`not_before`) dispatch, retry-with-backoff, and dead-lettering at `max_attempts`
  (no silent loss).
- **Hybrid search** — FTS5 (BM25) fused with sqlite-vec cosine-KNN via Reciprocal
  Rank Fusion, with time-range filtering and highlighted snippets. The vector arm is
  driven by an injected embedding provider (FTS-only until P3 wires fastembed).
- App wiring — the desktop shell opens `screensearch.db` at the app-data dir on
  launch, reports real `db` readiness, exposes a `get_job_stats` command, and writes
  a daily-rotating file log under `<app-data>/logs/` (`03 §9`).

### Added — P0 Scaffold (2026-06-21)
- Cargo workspace with a modular kernel layout (`03 §2`): `traits` (contracts +
  domain/jobs/IPC types) and skeleton crates `kernel`, `store`, `capture`, `ocr`,
  `embeddings`, `inference`.
- `src-tauri` — Tauri 2 desktop shell (composition root) with typed `ping` and
  `get_readiness` commands.
- React 18 + TypeScript + Vite UI skeleton (`ui/`) with a P0 typed-IPC smoke screen
  and an ESLint flat config enforcing the Rules-of-Hooks gate at error level.
- Typed UI↔core contract generated with `ts-rs` into `ui/src/bindings/`, with a
  regression guard keeping 64-bit ids as TS `number` (Tauri JSON wire).
- `doctor` environment smoke-check (`cargo run -p doctor`) for WebView2 / Vulkan /
  llama-server.
- GitHub Actions CI (`.github/workflows/ci.yml`, windows-latest): UI lint/build,
  `cargo fmt`/`clippy -D warnings`/`build`/`test`, and a ts-rs binding-drift guard.
- Generated application icons from `assets/icon-source.png`.

### Fixed (post-P0 review)
- `doctor`: load `vulkan-1.dll` with `LOAD_LIBRARY_SEARCH_SYSTEM32` so the Windows
  loader resolves it only from System32 — prevents DLL search-order hijacking
  without trusting the (manipulable) `SystemRoot` env var or a hardcoded path.
- UI: issue the independent `ping` / `get_readiness` IPC calls in parallel.
- CI: Claude review now actually posts — added the `--comment` flag (the skill
  produces a review but only posts with it) and granted the workflows write
  permissions (posts were silently denied before); `concurrency` added; actions
  bumped to Node-24 majors.

_Data spine landed in P1; capture, embeddings, and inference still to come (P2–P4)._

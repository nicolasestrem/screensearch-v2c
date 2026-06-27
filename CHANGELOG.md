# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

### Docs — Token-bloat optimization (archive v0.1.0 history, de-duplicate, fix drift)
Reorganized the documentation so an LLM reads only the current 0.2.x arc instead of wading through
shipped v0.1.0 history. No content was lost — pre-0.2.x entries were moved **verbatim** (byte-identical
to `git HEAD`) into sibling archives:
- Build-loop logs split to `specs/archive/`: `05_BUILD_REVIEW` (167 KB → 5 KB), `08_CHANGELOG_AI`
  (146 KB → 36 KB), `07_KNOWN_GAPS` (38 of 69 resolved v0.1.0 rows archived, ids preserved),
  `06_PATCH_PLAN` (4 of 11 rows archived).
- Released history split out of this file into `CHANGELOG-ARCHIVE.md` (the `[0.1.0]` section).
- De-duplicated `docs/ARCHITECTURE.md` (crate map / schema now point to `03 §2`/`§4`) and
  `docs/0.2.0.md` (rationale → `02 §5b`; audit summaries → `docs/audits/`).
- Relocated point-in-time `AUDIT_0.2.0_PR*.md` evidence into `docs/audits/` (kept git-ignored).
- Fixed README's broken phantom `AUDIT_V2C_2026-06-23.md` link (×3) and refreshed the stale
  "current state" lines in `CLAUDE.md`/`AGENTS.md`.
- Added an **archive-on-release** convention to `CLAUDE.md`/`AGENTS.md`/`04` so logs stay lean.

Total: ~355 KB trimmed from the live docs. Verified: every archive split diffs empty against `git HEAD`;
row counts reconcile (06: 7+4=11, 07: 31+38=69); `cargo check --workspace` green.

### Fixed — PR7 Ask source-frame labels
Followed up on the PR7 no-evidence Ask audit finding: source-frame tiles that represent retrieved
context are now labeled **Frames checked** instead of **Cited frames**. This keeps the existing typed
IPC stream and local-model prompt unchanged while avoiding the misleading implication that unrelated
context frames support an honest no-evidence refusal.
Verified in the dev executable launched by `npm run tauri dev`: a no-evidence token refusal rendered
retrieved tiles under **Frames checked** and did not render the old **Cited frames** label.

### Docs — PR7 audit status reconciliation
Reconciled tracked docs after the later PR3 audit fix: the old PR7 static-chrome finding is now
recorded as resolved by the self-exclude/backfill work, with the real residual kept in the
multi-monitor / rect-None gap. Also updated the Architecture search-limit wording to the current
`1..=2,000` backend cap and clarified that detailed `docs/AUDIT*.md` / `.playwright-mcp/` artifacts
are local-only ignored evidence.
The same dev-exe pass also spot-checked the already-fixed range-neutral report progress copy and the
default Chrome search path.

### Docs — 0.2.0 PR8 audit checkpoint
Recorded the PR8 parallel model download audit on `codex/0.2.0-pr8-audit`. The audit used the real
dev executable launched by `npm run tauri dev` from a reset app-data state, downloaded the default
answer model, exercised the Vision Quality 8B GGUF + mmproj path through Moment -> Tag with vision,
and verified an interrupted Vision Beta download resumed from its `.parts` bitmap instead of
restarting from zero. Static review found no PR8 schema/IPC/binding drift beyond the documented
`sha2` dependency and downloader env overrides. The existing PR3 static-chrome release blocker
remains separate; one narrow accepted PR8 follow-up around a pre-existing truncated `.part` plus
stale bitmap is tracked in `specs/07_KNOWN_GAPS.md`.

### Fixed — 0.2.0 PR3 attention-filter release blocker + Privacy/Excluded-Apps hot-apply
Addressed the 2026-06-26 PR3 audit (`docs/AUDIT_0.2.0_PR3_2026-06-26.md`): default content search
no longer ranks frames because of static/app chrome.

- **Self-exclude the app's own window.** The capture privacy gate now skips frames whose foreground
  window belongs to ScreenSearch's own process (PID-based, so it never mismatches a third-party
  window merely titled "screensearch"). Indexing our own UI only stored sidebar nav / command
  palette / a results pane echoing other captures — the dominant `Deck`/`Recall` self-capture leak.
  A gated, one-time startup purge removes any pre-existing own-window captures (a clean install finds
  none and no-ops).
- **Attention-filter backfill.** `FILTER_VERSION` 1→2: on startup the store re-cleans every frame
  below the active version against the now-warm chrome catalog (`textfilter::reconcile`, pure +
  monotonic — it only ever suppresses more, never resurrects content, and needs no `target_rect`).
  Changed frames re-enqueue an `embed_text` job so the vector arm re-embeds from clean text. This
  retroactively fixes the classifier's cold-start window (chrome kept until its signature crossed the
  threshold) — previously frozen with no backfill.
- **Faster cold-start.** `text.chrome_suppress_min_seen` default 12→4, so a repeated short edge label
  is caught within a few captures; long lines, interior body, and rect-less frames stay protected,
  and `include_chrome` + raw text recover any false positive.
- **Privacy / Excluded Apps now apply immediately.** `set_settings` compares the derived
  `CaptureConfig` and reloads a running capture loop on change, so adding an excluded app (or
  changing monitors / interval / pause-on-lock) takes effect at once instead of silently waiting for
  the next manual capture start (a user-reported bug).

Evidence (shipped backfill + purge replayed over the real 313-frame audit corpus copy, default
content FTS hits before→after): `Deck` 68→26, `Recall` 42→15, `Firefox` 24→15, `Steam` 24→15,
`GPU Memory` 19→15 — the remainder is legitimate content (frames literally discussing those terms;
`GPU Memory` in Task Manager). `include_chrome`/raw still recovers everything and real work terms are
intact (`cargo test`=41, `embeddings`=13). Honest residual: rect-None / multi-monitor secondary
captures of *other* apps' desktop chrome remain (gap #58 — needs `target_rect` for those frames).

Review hardening (PR #41, Codex + Gemini):
- **Stale embeddings invalidated during backfill (Codex P1).** When the backfill rewrites a frame's
  `content_text`, it now deletes that frame's stale text embedding (`source = 'ocr'`) in the same
  transaction (the `embeddings_ad` trigger cascades to the vec0 shadow). Otherwise the dropped chrome
  terms could keep surfacing the frame through `hybrid_search`'s vector arm — which fuses even with
  `include_chrome=false` — until the async re-embed ran, or *indefinitely* if text embedding is off.
- **Backfill no longer re-queries the catalog per frame (Gemini).** The read-only chrome catalog is
  cached by `app_hint` across the whole backfill instead of one `chrome_text_catalog` query per frame.
- **Self-capture purge no longer orphans files (Gemini).** A transient JPEG-delete failure now skips
  the row delete (matching the retention sweeper) so the file isn't orphaned; the no-progress guard
  still stops the loop if a batch makes no headway.
- **Backfill releases the store connection between batches (Codex P2, 2nd pass).** The whole backfill
  no longer runs under a single `with_conn`, which would hold the store's one DB connection for the
  entire pass and block search / settings / capture inserts on a large upgraded DB. It now snapshots
  the catalog once up front and processes each batch in its own short-lived `with_conn`, so other DB
  work interleaves during the background startup backfill.
- **Self-purge watermark only on full drain (Codex P2, 2nd pass).** The one-time purge records its
  `maintenance.self_capture_purged` watermark only after every own-window frame is actually gone — so
  a transient failure that leaves frames behind retries on the next launch instead of permanently
  skipping the purge (this matters more now that the orphan fix above can leave a locked frame behind).
- **Accepted, documented as-is:** the lower `chrome_suppress_min_seen` default (12→4) reaches new
  installs only — existing users keep their persisted value (no settings migration, and the
  backfill-clamp alternative was also declined, by choice); and the `reload_capture` stop→start window
  is left non-atomic — its doc comment was corrected (Tauri 2 async commands are *not* serialized, so
  the race is real but accepted: sub-millisecond window, no UI affordance to fire both paths at once).

### Docs — 0.2.0 PR6 audit checkpoint
Recorded a scoped PR6 audit checkpoint for Recall reports and Ask shortcuts. The audit created a
local-only ignored artifact at `docs/AUDIT_0.2.0_PR6_2026-06-26.md` plus evidence under
`.playwright-mcp/pr6-2026-06-26/`. Static wiring and targeted regression tests found no PR6
implementation blocker: reports and Ask shortcuts are wired over `frame_text.content_text`, Ask
defaults to `include_chrome=false`, and the report hardening tests for coverage splitting,
empty/no-sidecar reports, settings clamps, cancellation, and report summary parsing pass. A second
Computer Use run against the real dev executable completed the live UI audit: Search/Ask/Reports
rendered, all five Ask cards were visible, Day Recap submitted with cited frames, Daily/Weekly/
prompted-Custom/no-evidence-Custom reports generated, Settings showed `8/40/200/20`, and a
controlled Windows Notepad capture stored the PR6 probe token in `content_text`. The upstream PR3
static-chrome release blocker remains separate. Full verification passed after clearing a stale
repo-local Vite/esbuild process left over from the dev run.

### Docs — 0.2.0 PR3 audit
Added `docs/AUDIT_0.2.0_PR3_2026-06-26.md` and release-tracking notes for the PR3
attention-first filtering audit. The audit used the real dev app and existing app DB, confirmed the
storage/retrieval plumbing is mostly wired correctly, and records a **release blocker**: default
content search still surfaces static/app chrome for `Firefox`, `Steam`, `Deck`, `Recall`, and
`GPU Memory`, including a fresh Notepad capture that also indexed desktop/app navigation terms in
`content_text`. No code fixes were made in this audit pass.

### Docs — 0.2.0 PR1/PR2 audit follow-ups
Recorded a short `docs/0.2.0.md` handoff list for a later LLM pass before tagging 0.2.0. The note
captures the open PR7 static-chrome retrieval acceptance issue, roadmap/status drift around PR7 and
PR8, missing PR8 sequencing in `02`/`04`, `03` schema/type drift versus schema v4
`text_spans.line_index`, stale mandatory-reading docs, and the PR2 audit verdict. Follow-up review
clarified that the PR7 static-chrome failure remains a release blocker unless the acceptance
contract changes.

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
- **PR8 review hardening (PR #35 bot review).** Five robustness fixes in `download.rs`, all with
  tests: (1) a brand-new (zero-filled) `.part` now **discards a stale all-done `.parts` manifest**
  rather than trusting it — `open_preallocated` reports whether it created the file (atomic via
  `create_new`), and an all-complete bitmap over a fresh part is re-initialised, closing a path that
  could publish a zero-filled model when no `X-Linked-ETag` was advertised; (2) a transient
  **network error** on a chunk request (dropped connection / timeout / DNS blip) is now retried with
  backoff instead of failing the whole download on the first hiccup; (3) a **present-but-unreadable**
  `.parts` manifest (a Windows sharing violation from AV / the indexer) now surfaces an error so the
  job-queue retries, instead of silently truncating the bitmap and discarding real progress; (4)
  streamed body frames are **coalesced to ~256 KiB** before each positioned write, cutting
  `spawn_blocking` churn; (5) the terminal chunk error now distinguishes "server ignored Range"
  (a `200`) from "failed after N retries" (an exhausted `403`/`429`), so logs don't send an operator
  chasing a non-existent range-support bug. Two reviewer suggestions to make `download.rs` compile on
  macOS/Linux (`#[cfg(unix)]` `write_at`) were intentionally **declined** — the project is
  Windows-only by design (CLAUDE.md hard rule; CI is `windows-latest`), matching the
  Windows-native convention already in `flags.rs`/`lib.rs`.
- **PR8 follow-up (PR #35 bot review).** Generalised the fresh-part stale-manifest guard: a
  brand-new (zero-filled) `.part` now discards a header-matching `.parts` bitmap that marks **any**
  chunk complete, not only an all-complete one. The earlier guard caught the all-done case (failed
  post-publish cleanup) but missed a **partly-done** bitmap surviving over a fresh part — e.g. an
  interrupted download whose multi-GB `.part` a user/cleanup tool later reclaims. Trusting it would
  skip the done-marked ranges, leaving zeros in the assembled GGUF (length passes; sha256 is skipped
  with no advertised `X-Linked-ETag`). New `Manifest::any_complete()` drives the check; covered by
  `fresh_part_discards_stale_partial_manifest`, which fails on the prior condition.

### 0.2.0 PR7 — Integration audit
Ran the PR7 end-to-end audit against the existing populated app-data DB using `npm run tauri dev`
and the real debug executable (`target/debug/screensearch.exe`). Evidence screenshots were captured
locally under ignored `.playwright-mcp/pr7-2026-06-25/` storage and are not tracked.

- **Search audit:** content searches for real work terms (`Calendar-Grid`, `cargo test`, etc.) worked
  and the `include app chrome + raw text` toggle recovered raw/static labels. Default content search
  still surfaced static/app-chrome-heavy results for terms like `Firefox`, `Steam`, and especially
  `Deck` in the populated corpus; that finding was later addressed by the PR3 self-capture/backfill
  fix, with residual rect-None / secondary-monitor chrome tracked separately.
- **Ask audit:** a Calendar-Grid Coverage Map-Reduce question grounded on content text and showed
  source frames. A unique-token no-evidence question refused honestly, but still displayed retrieved
  context frames under `CITED FRAMES`; the PR7 follow-up relabels those tiles as `Frames checked`.
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


## Older versions

Releases 0.1.0 and earlier are archived in [CHANGELOG-ARCHIVE.md](./CHANGELOG-ARCHIVE.md).

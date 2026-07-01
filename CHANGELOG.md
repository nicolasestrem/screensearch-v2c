# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

### Changed — Expired screenshots now shrink the database too, not just disk (gap #73a)
When a screenshot expires under retention, the app keeps a text + layout reconstruction of it. That
reconstruction is drawn from per-word text positions, which are the biggest remaining database cost
for old frames. Rather than delete them (which would blank out the reconstruction), the app now
**merges each expired frame's words into per-line entries** — reclaiming roughly 80% of those rows
while the reconstruction still reads correctly at the line level. Merging happens as frames expire,
and a one-time pass on first launch cleans up frames that expired before this shipped. Search is
unaffected. (Full-text and semantic search read separate stored text/vectors, not these positions.)
Following PR review, an expired frame is now retired in a single atomic step, so a momentary
database hiccup can never leave a frame half-degraded (marked expired but still carrying its
per-word rows); if it fails, the frame is simply retried on the next sweep.

### Fixed — Semantic search could miss in-range results on tight time windows (gap #8)
When a search combined a query with a narrow time filter, the vector (meaning-based) arm fetched a
fixed pool of nearest matches and *then* dropped those outside the window — so an in-window match that
happened to rank just past the pool was silently missed. The vector arm now **widens its search
adaptively** when a time range is set: it re-runs with a larger nearest-neighbour count until the pool
fills with in-window matches or the index is exhausted. Unfiltered searches are unchanged. (sqlite-vec
can't filter inside its nearest-neighbour query, so widening the pool is the fix.) Search latency stays
well within budget (10k-frame fixture p95 ~66 ms, bar is 200 ms).
Following review (PR #66, P2): the widening now stops as soon as it has gathered every embedded frame
the window actually **holds**, bounded by a cheap, index-served count of in-window embedded frames.
Without that cap, a *sparse* window — one with fewer captures than the pool — on a database larger than
the widening ceiling would have run the maximum 20 000-neighbour pass on every query even after already
finding all its matches. An empty window now skips the vector query entirely.
Following the follow-up review (PR #66, P2): that count itself is now hard-bounded. A `LIMIT` on
*matches* can't stop early when a window has many captured frames but few embedded ones (an embedding
backlog, or a wide multi-day range), so the count would have walked the whole frame range — O(window),
not O(pool). The count now examines at most a fixed budget of frames; a window too large to prove
sparse within that budget falls back to assuming it is dense (widen up to the pool), which can only
*raise* the target and never drops an in-window match. The count is now O(pool) even on a wide,
sparsely-embedded window.

### Changed — Packaging spec re-scoped to NSIS (Inno / portable ZIP / MSI dropped)
ScreenSearch ships an unsigned **NSIS** installer (Tauri 2 native, since v0.1.0). The specs had still
called for an "Inno Setup installer + portable ZIP"; all nine live references (project intake,
context, plan, master-spec DoD §13.9, architecture doc, CI note, README) are rewritten to NSIS, and
DoD §13.9 is re-scoped to "NSIS installer builds successfully" and met. **Code-signing** is now the
lone remaining packaging follow-up (known-gap #26 closed).

### Fixed — Second launch restores a hidden window
The single-instance handler now shows the existing window before unminimizing and focusing it, so a
hidden / tray-minimized window is properly restored (not just unminimized) on a second launch.

### Accessibility — Keyboard & focus pass (gap #42)
- **Navigation rail** is a single Tab stop with a roving tabindex: Arrow Up/Down (wrapping) and
  Home/End move focus between the five destinations, and the active route carries `aria-current="page"`.
- **Command palette** returns focus to the control that opened it when it closes.
- **Recall → Ask** moves focus to the answer once streaming finishes, so keyboard and screen-reader
  users land on the result.
- **Settings** exposes each section's controls as a labelled group for assistive technology.

### Fixed — Model-downloader resume hardening (two corruption/waste edge cases)
Two narrow but real durability gaps in the parallel chunked model downloader
(`crates/inference/src/download.rs`), neither previously test-covered:
- **A truncated `.part` no longer publishes zeros (gap #69).** The resume bitmap was only discarded
  when the downloader *created* a brand-new `.part`. If an external cleanup tool truncated an
  existing `.part` without deleting it, `set_len` re-grew the file with zeros while a header-matching
  bitmap still marked those (now-zero) chunks "done" — so the zero-filled ranges were published (the
  length check passes; sha256 is skipped when the CDN advertises no `X-Linked-ETag`).
  `open_preallocated` now reports a part as untrustworthy whenever its on-disk length is **not
  exactly** `total` (brand-new, truncated, *or* corruption-grown larger), forcing a full refetch.
  Safe for normal resumes: a legitimate interrupted `.part` is always preallocated to exactly
  `total`, so it is never falsely discarded.
- **A locked download re-checks the cache before re-downloading (PR #27 Codex-P2).** When another
  live downloader holds hf-hub's per-blob advisory lock, the single-stream fallback backs off and
  retries; it now re-checks the clean-layout and HF-cache fast paths **after each backoff**, so if
  the holder finishes during the sleep the loser copies the finished blob into place instead of
  re-downloading it or colliding on publish. The bounded backoff was extended (per-attempt cap + more
  attempts → ~5 min total) so a real multi-GB download by the holder is outlasted rather than
  abandoned after ~20 s, while the cache re-check exits the instant the holder is done.

### Fixed — Full-resolution frames overflowed the vision context (tagging failed + slow)
Native full-resolution captures (e.g. a 3440×1440 ultra-wide frame → ~4148 vision tokens) exceeded
the vision lane's pinned **4096**-token context, so `llama-server` rejected every tag with HTTP 400
`exceed_context_size_error`; jobs retried 3× and dead-lettered (DB showed `vision_tag` 72 dead / 0
done). The deceptively low RAM/VRAM that looked like a "memory miracle" was the model **rejecting
requests in ~0.1 s without running inference** — the GPU sat near-idle while the weights stayed
resident (~6.4 GB VRAM). Fix is two-part, plus the diagnosability gaps that hid it:
- **Downscale the VLM request image** to a 1568 px longest edge before JPEG-encoding
  (`crates/inference/src/vision.rs`). Captures/timeline keep full resolution — only the tag request
  shrinks — so it fixes the overflow *and* cuts prefill time (addresses the slowdown). The 1568 px
  cap bounds even a worst-case square frame to ~2.5 K prompt tokens, comfortably under the
  spec-contracted **4096**-token vision default — so the auto-context is left unchanged (per PR #61
  review: bumping it would have contradicted `03 §8` "not bumped by default" and raised KV-cache
  VRAM on weak GPUs for headroom the downscale makes unnecessary).
- **Record the real cause**: `vision_tag` failures now log the full anyhow chain (`{e:#}`) into
  `jobs.last_error` (`crates/kernel/src/worker_pool.rs`) instead of the collapsed `"vision
  completion"`, and the sidecar's stdout/stderr are captured to `<sidecar dir>/llama-server.log`
  (`crates/inference/src/process.rs`) — previously the sidecar's own error was discarded.

### Docs — Restore the archive-on-release convention across the build-loop logs
Brought the lean-logs convention current. v0.1.0 history was archived on 2026-06-27, but the shipped
**0.2.0, 0.2.1, and 0.2.2** history had piled back up in the live logs. Moved it out **verbatim**
(byte-identical to `git HEAD`; original `#N` ids preserved so cross-references stay valid):
- `specs/07_KNOWN_GAPS.md` — 18 resolved 0.2.x rows + 5 accepted-as-is rows + the ~85-line
  resolved-engineering-decisions list → `specs/archive/07_KNOWN_GAPS.v0.2.x.md`. The live table now
  holds only still-open + current-arc rows (35 → 12).
- `specs/05_BUILD_REVIEW.md`, `specs/08_CHANGELOG_AI.md`, `specs/06_PATCH_PLAN.md` — shipped 0.2.x
  entries → matching `*.v0.2.x.md` archives (06 keeps its one open upstream-leak row #15).
- This file — the 0.2.0/0.2.1/0.2.2 sections (which had accumulated under `[Unreleased]`) →
  `CHANGELOG-ARCHIVE.md` as proper versioned sections.

### Added — Deterministic dev-only route-state triggers (closes `07` #43)
A dev-only `?__devState=loading|error` URL param forces any P5 route into its loading or error state
for audit verification, applied centrally at the TanStack Query seam and **stripped from production
builds** (`import.meta.env.DEV`). Empty/partial/populated stay driven by real data — no mocks in the
production path. Documented in `docs/DEV_STATE_OVERRIDE.md`. Per PR #60 review, the helper reads
`window.location.search` directly instead of `useSearchParams()`, so it calls no React hook: the
production path truly folds to `return result` (no router-history subscription on the 17 query
consumers, no `<Router>`-context coupling) rather than merely stripping the `__devState` string.

### Added — Privacy-safe VLM request logging (closes `07` #44)
The kernel now logs `frame_id` + the relative capture path at `info` immediately before each vision
request, so the VLM image input is visible in `screensearch.log` (previously a log scan found nothing).
No screen content, OCR text, or image bytes are logged.

## Older versions

Releases 0.2.2 and earlier are archived in [CHANGELOG-ARCHIVE.md](./CHANGELOG-ARCHIVE.md).

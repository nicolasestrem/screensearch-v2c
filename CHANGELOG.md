# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

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

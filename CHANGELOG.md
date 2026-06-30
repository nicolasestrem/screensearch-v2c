# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

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

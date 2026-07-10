# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

### Added
- Groundwork for sessions (no visible changes yet). The database gains the tables that will let
  ScreenSearch group your frames into sessions: a meeting, a Claude Code run, a stretch of focused
  work. The upgrade runs on first launch, adds structure only, and creates no sessions on its own;
  the actual grouping arrives in a later stage. It changes nothing about today's features. Search,
  Ask, reports, marks, the overlay, and where-was-i are proven byte-for-byte identical before and
  after the upgrade on a real, populated database, and the upgrade completed in about 150 ms over a
  3,000-frame database in testing.

### Changed
- Specs only (no app changes): the 0.4.0 "sessions" arc contract is now written into the project
  specs, so the next stages of work can be built straight from them. Sessions group your captured
  frames into meaningful stretches — a meeting, a Claude Code run, an afternoon in one repo — and
  are additive: nothing about today's capture, search, Ask, reports, marks, or the overlay changes.
- Dev-only work (no app changes): the sessions design now handles **running several AI tools at
  once** — for example Claude Code and Codex working in parallel — as **overlapping** sessions
  rather than collapsing them into one. This was validated against real captured days in the
  dev-only harness; the app's data model is unaffected. A known limit is recorded honestly: the
  screen capture only sees the foreground window, so a tool running in the background is under-
  recorded, which caps how completely concurrent work can be reconstructed.

## Older versions

Releases 0.3.3 and earlier are archived in [CHANGELOG-ARCHIVE.md](./CHANGELOG-ARCHIVE.md).

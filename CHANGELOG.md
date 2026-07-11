# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

### Added
- Sessions are now reachable from the local API and its MCP wrapper, read-only. Scripts and AI
  clients can list the sessions overlapping a time range (filtered by kind or tool), open one session
  to see its representative exchanges and its cached summary if one exists, and ask a grounded
  question scoped to a single session so the answer only cites that session's own frames. Three
  matching MCP tools (list sessions, get session, ask session) join the existing set. The API never
  generates a summary on read and never writes, and questions without a session behave exactly as
  before.
- Sessions are now visible throughout the existing recall flow: Timeline shows accessible,
  lane-packed session bands in exactly four stable rows; sessions needing lane five or later are
  summarized by a neutral keyboard control that focuses the existing range presets. A code-split
  drill-in shows lazy summaries, representative frames, exchanges, and cited Recaps; Moment and Deck
  link the same context back to its session; and Advanced Settings exposes the two existing
  session-pass thresholds. No new navigation item or frame-level behavior was added.
- The in-app sessions surface now has typed core commands to list overlapping sessions, load a
  bounded drill-in with representative frames and extracted exchanges, connect Moment and
  where-was-i frames back to their session, lazily cache session title/summary intelligence, and
  generate a cancellable Recap through the existing coverage-first report engine. Session Recaps
  are scoped by exact frame ownership, so overlapping tracks cannot leak citations.
- The sessions engine now groups captured frames in the background into stable focus, meeting, and
  recognized AI-tool sessions. It supports overlapping tools while keeping each frame owned by one
  session, incrementally updates the recent tail, and resumably backfills existing history without
  blocking capture. Recognition ships from the real-data-tuned v3 taxonomy (Claude Code, Codex,
  Claude desktop, browser AI, and five meeting identities); explicit chat exchanges are extracted
  best-effort from filtered content text, and session title/summary generation is lazy and cached for
  the forthcoming in-app surface. No new UI/API commands land in this stage.
- Groundwork for sessions (no visible changes yet). The database gains the tables that will let
  ScreenSearch group your frames into sessions: a meeting, a Claude Code run, a stretch of focused
  work. The upgrade runs on first launch, adds structure only, and creates no sessions on its own;
  the actual grouping arrives in a later stage. It changes nothing about today's features. Search,
  Ask, reports, marks, the overlay, and where-was-i are proven byte-for-byte identical before and
  after the upgrade on a real, populated database, and the upgrade completed in about 150 ms over a
  3,000-frame database in testing.

### Changed
- PR #104 review follow-up resolves all four unresolved inline findings (two were the same store
  issue): session-content preflight now stops inside SQLite with an indexed `EXISTS` query while
  preserving Rust `str::trim`'s exact Unicode-whitespace semantics; Timeline error and empty states
  reserve the same fixed four session lanes as loading and populated states; and the session
  scheduler now emits quiet `sessions_changed` invalidation only for semantic changes, using delta
  ownership/artifact writes so an unchanged pass is also a physical no-op.
- PR5 review hardening makes Timeline session-band packing use measured token-sized hit targets,
  holds loading/error/empty/populated at exactly four lanes to prevent layout shift, and summarizes
  dense overflow without nested/horizontal scrolling. Mounted session queries now refetch after a
  successful scheduler pass through a quiet typed event. Recap cancellation stops backend work on
  cancel, route change, or unmount. Native WebView2 acceptance passed against the live schema-11
  database; the dataset had no open session, so that visual variant remains transparently unobserved
  rather than claimed.
- Review hardening now includes the final frame when sampling session intelligence, parses taxonomy
  match strings once instead of allocating per frame, treats frozen boundary timestamps as
  inclusive, refuses to match merely touching mutable sessions, and resumes a crash-interrupted
  historical row without creating a duplicate session.
- Session backfill now safely resumes when its original historical cutoff lands inside a continuous
  activity track, reconciles delayed work before immutable frozen tails, and preserves exact frame
  ownership across absorbed short excursions and frozen boundaries.
- Specs only (no app changes): the 0.4.0 "sessions" arc contract is now written into the project
  specs, so the next stages of work can be built straight from them. Sessions group your captured
  frames into meaningful stretches (a meeting, a Claude Code run, an afternoon in one repo), and
  are additive: nothing about today's capture, search, Ask, reports, marks, or the overlay changes.
- Dev-only work (no app changes): the sessions design now handles **running several AI tools at
  once** (for example Claude Code and Codex working in parallel) as **overlapping** sessions
  rather than collapsing them into one. This was validated against real captured days in the
  dev-only harness; the app's data model is unaffected. A known limit is recorded honestly: the
  screen capture only sees the foreground window, so a tool running in the background is under-
  recorded, which caps how completely concurrent work can be reconstructed.

### Documentation
- Documentation refresh and repository-exposure scrub. Demo screenshots that had been captured from
  a live install (and therefore showed real personal screen content) were removed pending re-shoot
  against synthetic data. Absolute user paths and personal-activity notes were replaced with
  placeholders in the build-loop logs, test fixtures were changed to synthetic user/host values, and
  the crate author email now uses a noreply address. `docs/ARCHITECTURE.md`, `README.md`,
  `CLAUDE.md`, `AGENTS.md`, and `docs/API.md` were updated for v0.3.3 and the 0.4.0 sessions arc (15
  crates, schema v11). The pre-0.4.0 manual acceptance checklists moved to
  `specs/archive/TESTING.pre-0.4.0.md`, and three obsolete design notes under `docs/superpowers/`
  were removed.
- Restored the README screenshots, now rendered against **synthetic seed data** (invented frames,
  sessions, and text — no personal content): Deck, Timeline (with session bands), Recall, Insights, and
  Moment. A dev-only, CI-ignored seeder (`crates/store/tests/seed_demo.rs`) populates an isolated
  schema-11 database through the public store API, and `docs/SCREENSHOTS.md` documents the process.

## Older versions

Releases 0.3.3 and earlier are archived in [CHANGELOG-ARCHIVE.md](./CHANGELOG-ARCHIVE.md).

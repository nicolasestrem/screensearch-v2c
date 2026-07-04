# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

### Changed
- **Flow overlay default hotkey is now `Ctrl+Alt+Z`** (was `Ctrl+Alt+Space`, which collided
  with Claude Desktop's global quick-entry shortcut). Existing installs still on the old
  default are migrated once on load; a chord you deliberately chose is left untouched. The
  migration is a genuine one-shot (latched by a stored marker), so if you *want*
  `Ctrl+Alt+Space` you can set it back in Settings and it now sticks across restarts instead of
  being re-migrated. A hotkey that fails to register (e.g. another app already owns it) is
  surfaced in Settings, not swallowed.

## Older versions

Releases 0.3.0 and earlier are archived in [CHANGELOG-ARCHIVE.md](./CHANGELOG-ARCHIVE.md).

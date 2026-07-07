# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

### Fixed
- Browsers no longer freeze during capture. The text reader (UI Automation) could hang Chromium
  and Electron windows (Chrome, Edge, Slack, Discord, VS Code, Claude Desktop, and similar) by
  walking their accessibility tree, occasionally leaving a window "Not Responding". These windows
  are now read via OCR instead, which never freezes them; their on-screen content stays fully
  captured and searchable. Native apps are unaffected and keep the higher-fidelity reader.

## Older versions

Releases 0.3.2 and earlier are archived in [CHANGELOG-ARCHIVE.md](./CHANGELOG-ARCHIVE.md).

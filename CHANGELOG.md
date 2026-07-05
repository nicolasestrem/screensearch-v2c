# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

### Added — 0.3.2 PR2: auto-update (#69)
ScreenSearch can now update itself. It checks for a new version on launch (and on demand from a
quiet **Check for updates** control in the left rail and the **Settings → App** section), downloads
a signed update in the background, and installs it **only when you choose to restart** — no modal,
no nag, no auto-restart. When an update is waiting, a quiet dot appears in the left rail; when there
is none, there is nothing to see. Built on `tauri-plugin-updater` with a **minisign-signed**
`latest.json` manifest published on GitHub Releases: a tampered, unsigned, or wrong-key update is
rejected before it can install (`03 §11b`). A new tag-triggered release workflow
(`.github/workflows/release.yml`) builds, signs, and drafts each release with the installer, its
signature, and the manifest; `scripts/make-latest-json.mjs` generates the manifest and refuses to
emit one for an unsigned build. **This release itself is still a manual download** — auto-update
delivers every release *after* it (0.4.0 will be the first to arrive automatically). Note: the
minisign updater signature is **not** Windows code signing (Authenticode) — that remains a separate
open item, so SmartScreen still warns on the manual installer.

### Docs — 0.3.2 PR1: specs contract (specs-only; no code / schema / UI)
The 0.3.2 roadmap (`docs/0.3.2.md` — "P7.2: product shell mini-arc", lifecycle + interface) is
normalized into the specs so PR2 through PR5 are implementable from the specs alone. The contract
locks in: the PR order (PR1 specs, then Rust lane PR2 auto-update #69 and PR3 systray #56 + quick
actions #57, parallel to UI lane PR4 shell-layout hardening (reproduce-first), then PR5 Settings
two-tier IA, then PR6 audit + tag `v0.3.2`); auto-update mechanics (`tauri-plugin-updater`,
minisign-signed GitHub-Releases manifest, passive pull-based UX, `03 §11b`); tray + close-to-tray +
single-instance lifecycle (`03 §7d`); two new settings keys (`app.close_to_tray` on,
`app.run_at_startup` off) and two dead-setting retirements (JPEG quality, `uia_run_on_interactive`)
with config-load tolerance (`03 §8`, D8); the shell layout contract (one scroll context per route,
no nested or horizontal scrollbars, no layout shift on load, `UI_REFERENCE §8`, D9); and the
Settings, App, tray, and updater surfaces (`UI_REFERENCE §3`/`§4`). Hard mini-arc constraints
recorded: zero DB schema migrations (D10), presentation-first restructure (D7), no visual redesign
(D12). Deferrals recorded in known-gaps: #88 folded into the 0.4.0 sessions arc (#102), settings
search (#103), visual-refresh possibility (#104), plus the updater key-custody manual step (release
blocker). This change touches only specs and docs.

## Older versions

Releases 0.3.1 and earlier are archived in [CHANGELOG-ARCHIVE.md](./CHANGELOG-ARCHIVE.md).

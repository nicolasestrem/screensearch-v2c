# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

### Fixed - 0.3.2 PR4: shell layout hardening
The app shell now holds one scroll context per route. Recall's search results, the Ask answer's
"Frames checked" strip, the report source frames, and a moment's "Around this moment" filmstrip no
longer have their own inner scrollbars: each grows or wraps inline and the page scrolls as a single
column, with the scrollbar at the content-pane edge. No screen shows a horizontal scrollbar at any
supported size (1280x720 through ultrawide, 100 to 150 percent scaling). Loading no longer jumps the
layout: the status-rail chips have stable widths, panel headers reserve their height, and the Deck,
Insights, and Settings loading skeletons match the shape of the real content (Insights even shows its
live header while data loads). The intermittent left-rail "ghost" glitch (nav items faintly
duplicated at the wrong height) is a known display-driver compositing quirk in the WebView2 runtime,
not an app bug; the rail is now painted on its own isolated layer to avoid it, and the issue is
tracked for a runtime fix. Structural only, no visual redesign; no settings or database changes.

### Added — 0.3.2 PR3: system tray + quick actions (#56/#57)
ScreenSearch now lives in the system tray. A tray icon shows live capture state at a glance
(capturing / paused / capture error, as a colored status dot + tooltip) — the passive "app running
reminder" of #56, with **no notifications, nudges, or counting badges** of any kind. Its menu is
Open ScreenSearch · Pause/Resume capture · Load/Unload answer model · Start/Stop vision tagging ·
Check for updates · Quit, each acting through the same commands the app already uses. **Closing the
main window now keeps ScreenSearch running in the tray** (capture continues); a one-time,
non-shaming toast explains this the first time you reopen the window, and a new **Settings → App**
toggle turns it off (window close then quits cleanly). A second **Run at startup** toggle (default
off) registers launch-at-login. The same **Load/Unload answer model** and **Start/Stop vision
tagging** quick actions also join the left-rail quick menu and the command palette, completing #57 —
"Start vision tagging" tags the untagged backlog, "Stop" cancels the pending vision jobs (a running
one finishes). Left-click on the tray icon, a second launch, or the tray's **Open** all restore a
tray-hidden window (the existing single-instance behavior). Quit from the tray shuts down cleanly
via the existing Job-Object lifecycle (`03 §7d`). No DB schema change. The tray reuses the sister
app's product-shell *patterns* (atomic pause toggle with rollback, register-before-persist for
startup, teardown-safe state access) — patterns, not code; its pause/resume notifications are
deliberately excluded (D4).

### Fixed — 0.3.2 PR3 review follow-up
Four correctness fixes from the PR #92 automated review: (1) the command palette now offers only
the contextually valid half of each lifecycle toggle (never "Load answer model" while it is already
loaded, nor "Stop vision tagging" with nothing running), mirroring the left-rail quick menu;
(2) the **Settings → App** toggles (Run at startup, Keep running in the tray) no longer silently
revert when you later Save an unrelated Settings field — their live values are mirrored into the
form draft; (3) the tray now seeds its **Start/Stop vision tagging** label from the durable job
queue at startup, so a restart with a leftover backlog opens on "Stop" (and can cancel it) instead
of "Start"; (4) if persisting settings fails after the launch-at-login registration was changed,
that OS registration is now rolled back, so run-at-startup can never drift from the stored setting.

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

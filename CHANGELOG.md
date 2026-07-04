# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

### Fixed
- **UI Automation no longer leaves Chromium/Electron apps hung.** The UIA text source keeps
  an accessibility client connected, which flips apps like Chrome, Edge, Codex, and Claude
  Desktop into accessibility mode; previously that client was never released, so those apps
  could stay slow or unresponsive **even after you disabled capture**, and no restart of
  ScreenSearch's capture cleared it. Now:
  - Disabling **Use UI Automation text**, changing any UIA setting, or **stopping capture**
    actually disconnects the client, so the affected apps leave accessibility mode.
  - A **per-app circuit breaker** backs UIA off to OCR for 30 minutes after an app's tree
    walk repeatedly runs over budget or times out, so a heavy app isn't re-walked every frame.
  - A walk that blows its hard timeout is now **cancelled** instead of running to completion
    against the struggling app.
  - **Every UIA setting now takes effect immediately** on save (budget, node caps, control
    view, input-suppression) — no app restart, matching the "Applies now" hints. (A settings
    save that raced the very first client spawn could previously bake the pre-save budget into
    the new client; the client now reads the live config at spawn time, closing that window.)
  - **Recovery for an already-hung app:** disable UI Automation text (or stop capture) — which
    now truly disconnects — then restart the affected browser/Electron app to clear its sticky
    accessibility mode.

## Older versions

Releases 0.3.0 and earlier are archived in [CHANGELOG-ARCHIVE.md](./CHANGELOG-ARCHIVE.md).

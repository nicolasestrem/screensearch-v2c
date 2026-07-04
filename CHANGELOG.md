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

### Fixed
- **Vision throughput is back to the pre-WebP baseline.** The 0.3.0 switch to lossless WebP
  storage made every vision job synchronously decode a native-resolution WebP before dispatching
  to the local vision model, dropping the measured repeated-frame workload from 61.68 frames/min
  (`v0.2.1` JPEG baseline) to 26.95 frames/min. ScreenSearch now keeps WebP as the stored image
  format but prepares an internal 1280 px JPEG vision proxy beside each WebP and uses that for
  vision dispatch. The fixed build measured 61.69 frames/min on the same workload/model, with GPU
  utilization returning to the steady baseline shape. No schema, settings, or UI changes.

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

### Docs — 0.3.1 patch specs contract (PR1, specs-only; no code / schema / UI)
The 0.3.1 roadmap (`docs/0.3.1.md` — "P7.1: post-0.3.0 triage", a regression-fix + polish patch)
is normalized into the specs so the later PRs are implementable from the specs alone. **This
change touches only specs and docs.** The contract locks in: the PR order (PR1 specs → PR2 the
#64 vision-throughput regression, profile-first with a stop condition and a fixed fix-preference
order → PR3 polish: #59 Moment text grows inline with no nested scrollbar, #65 dated report
filenames (`screensearch-report-YYYY-MM-DD-HHmm.md`, local time) + a report footer stating app
version/model/time span/filters, and the #57-partial version link — which lands in the **NavRail
footer** and opens the GitHub repo → PR4 audit + tag `v0.3.1`), and the hard patch constraint (no
new subsystems, no schema migrations, no new settings surface). Deferrals are recorded in
known-gaps: **#69 auto-update → 0.3.2, hard-sequenced before 0.4.0 ships**; #56 systray + the
#57 quick actions → the 0.3.2 lifecycle mini-arc, bound by the pull-based / non-shaming reminders
principle; #54 closed and folded into the 0.4.0 sessions arc.

## Older versions

Releases 0.3.0 and earlier are archived in [CHANGELOG-ARCHIVE.md](./CHANGELOG-ARCHIVE.md).

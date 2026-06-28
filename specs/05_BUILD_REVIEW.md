# 05 — Build Review

> **Populated during the build**, after each meaningful pass (`04 §7`). Record what actually
> happened — honestly. Empty until P0 begins.

For each build pass, append an entry:

## Pass <n> — <date> — <phase, e.g. P0 Scaffold>
- **Implemented:** what now works (with the verbatim verification output that proves it).
- **Skipped / deferred:** what was intentionally not done, and why.
- **Hallucinated / corrected:** anything the agent assumed that turned out wrong.
- **Broke / regressed:** what stopped working, and the fix.
- **Still risky:** areas that compile/pass but warrant scrutiny.

---

## Pass — 2026-06-28 — 0.2.1 PR4 part 1: event-driven capture

**Branch:** `feat/0.2.1-pr4p1-event-capture`. The 0.2.1 line; 0.2.0 keeps timer/idle capture.

### Implemented
- **Opt-in event-driven capture (default OFF).** New master setting `capture.event_driven_enabled`
  selects Timer vs Event-driven capture. In event mode the capture source fires on the enabled
  user-activity triggers plus a long fallback timer (a static screen is still sampled), a debounce
  (collapse bursts), and a min-interval rate ceiling (no storms).
- **Four triggers:** **foreground/app-switch** (`SetWinEventHook` `EVENT_SYSTEM_FOREGROUND`,
  `WINEVENT_OUTOFCONTEXT`), **clipboard change** (`AddClipboardFormatListener`, change event only),
  **idle**, and **typing-pause** (both derived from `GetLastInputInfo` timing only).
- **Two new capture modules.** `crates/capture/src/trigger.rs` — a pure, `traits`-only debounce /
  rate-ceiling / idle-edge state machine (no Win32), with 11 unit tests. `crates/capture/src/events.rs`
  — the only new `unsafe`: a dedicated message-pump thread owning a message-only `HWND_MESSAGE`
  window + the foreground hook + clipboard listener, with clean `WM_QUIT` / unhook / destroy / join
  teardown on drop. The event source lives inside `WgcCapture`, which stamps each frame's trigger;
  the kernel capture loop and the `CaptureSource` trait are unchanged.
- **`traits::CaptureTrigger` enum** (`Timer|Idle|ForegroundChange|ClipboardChange|TypingPause|Manual`)
  with `as_db_str`/`from_db_str`, persisted to the new nullable `frames.capture_trigger` column via
  forward-only migration **`schema_version` 4→5**, surfaced in `FrameDetail` and the Moment view's
  "Captured via" row. Legacy frames read back as NULL (unknown).
- **Ten new clamped settings keys** (never hardcoded): `capture.event_driven_enabled` (false),
  `capture.event_on_foreground` (true), `capture.event_on_clipboard` (true), `capture.event_on_idle`
  (false), `capture.event_on_typing_pause` (false), `capture.event_debounce_ms` (500, 100–10000),
  `capture.event_min_interval_ms` (1000, 250–60000), `capture.event_typing_pause_ms` (1500,
  500–10000), `capture.event_idle_threshold_ms` (5000, 1000–60000),
  `capture.event_fallback_interval_ms` (30000, 1000–3600000).
- **Hot-apply with no `src-tauri` change.** `CaptureConfig` `PartialEq` now includes the event
  fields, so the existing `set_settings`→`reload_capture` path restarts a running capture loop when
  any event setting changes.
- **New `windows` features** in `crates/capture/Cargo.toml`: `Win32_UI_Accessibility`,
  `Win32_System_DataExchange`, `Win32_System_LibraryLoader`.

### Skipped / deferred
- **Click + scroll-stop triggers deferred to ≥0.2.2** — both would require a low-level mouse hook
  (`WH_MOUSE_LL`), which the roadmap deliberately steers away from. Recorded in `07` #47.
- No UIA text and no smart enrichment throttle (the other 0.2.1 deferrals) — out of scope for this
  pass; still tracked in `07` #48/#49.
- The attention filter is intentionally left **trigger-agnostic**: the `CaptureTrigger` is a
  provenance label, not a classifier input (`07` #57).

### Privacy posture
- Opt-in, default off. **No keystrokes and no clipboard contents are ever read or stored** — only
  change/idle-timing signals (`GetLastInputInfo` exposes a timestamp only). All existing privacy
  gates still apply (self-exclude own window, excluded-apps, pause-on-lock, diff gate).

### Still risky
- `events.rs` carries the only new `unsafe` (Win32 hook + message pump on a dedicated thread). It has
  a `#[ignore]` hardware lifecycle test that starts/drops the hook source 50× asserting no leak/hang
  (`cargo test -p capture -- --ignored`); the pure trigger logic is covered by the CI unit tests.
- Hook install failure is non-fatal: capture falls back to the event-mode fallback timer + idle
  polling (the machine still runs), logged via `tracing::warn!`.

### Verification
Full suite green: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -D warnings`,
`cargo build --workspace`, `cargo test --workspace` (11 `trigger.rs` unit tests, extended settings
round-trip + sanitize tests), the UI lint + build, and the `git diff --exit-code -- ui/src/bindings`
binding guard. Raw command output is pasted in the session response. (Docs-only pass does not re-run
the gates.)

### Review follow-up — rate ceiling delays rather than drops
An adversarial review of the trigger machine confirmed one real defect: when a discrete event's
debounce window settled but the global `min_interval_ms` rate ceiling blocked the emit, `poll`
cleared `pending` (and set `fired_idle`/`fired_typing_pause`) **before** checking the emit result, so
the trigger was silently dropped until the next event or the fallback timer. Fixed by clearing
`pending` / setting the `fired_*` edge flags **only on a successful `try_emit`**, so the rate ceiling
now **delays** a capture (retried on the next poll) instead of consuming it. Two TDD regression tests
added (`pending_event_retries_after_min_interval_block`, `idle_retries_after_min_interval_block`);
`trigger.rs` now has 11 unit tests, `cargo test -p capture` → 24 passed, fmt + clippy clean.

### Review follow-up — PR #44 automated review (3 fixes)
Three findings from the PR #44 bot reviewers (Claude / Gemini / Codex) were confirmed real and applied
(v5 is unreleased, so the migration could still be corrected without schema drift):
1. **CHECK constraint on `frames.capture_trigger`** (`store/src/schema.rs` `MIGRATION_V5`). It was the
   only closed-set TEXT column without a `CHECK`, unlike `primary_source`/`role`/`suppress_reason`.
   Added `CHECK (capture_trigger IS NULL OR capture_trigger IN (…six tokens…))` so an invalid token
   from a future bug fails loudly at write time instead of silently mapping to `None` (lost
   provenance). SQLite enforces it on new writes only, so existing `NULL` rows need no data migration.
2. **Busy-wait spin on a closed hook channel** (`capture/src/lib.rs` `next_event_trigger`). The old
   comment claimed `recv()` returns `None` *only* when there is no hook source; in fact a `tokio` mpsc
   `recv()` also returns `None` when all senders drop — i.e. the hook thread exits post-startup
   (`GetMessageW` error). That made the `select!` event arm ready every iteration, hot-looping until
   the fallback. Fixed by clearing the local `events` handle (→ `recv_event(None)` is `pending`
   forever) and `tracing::warn!`-ing once, matching the documented "degrade to fallback timer + idle".
3. **Honor disabled event sources before installing them** (`capture/src/events.rs`). The Win32 layer
   installed *both* the foreground hook and the clipboard listener unconditionally. Now `start` takes
   the per-trigger flags and installs only the enabled source(s): a disabled clipboard no longer pushes
   `WM_CLIPBOARDUPDATE` into the 64-slot queue (where churn could crowd out an enabled foreground
   event), and a clipboard-listener setup failure no longer disables the foreground hook. Teardown
   releases only what was registered. The `#[ignore]` lifecycle test passes both flags.

All gates re-run green: fmt, clippy `-D warnings`, `cargo test --workspace` (capture 24, store 49+7),
bindings guard clean, UI lint + build. The events.rs lifecycle leak test (`-- --ignored`, 50× start/
drop with both hooks) passed on real hardware.

---

## Pass — 2026-06-27 — PR7 audit follow-ups

**Branch:** `codex/pr7-audit-followups`.

### Implemented
- Relabeled Recall Ask source-frame tiles from `Cited frames` to `Frames checked`, matching the
  existing backend semantics: those frame ids are context/provenance supplied to the answer model,
  not model-authored evidence for a positive claim.
- Updated nearby Ask comments in the UI, Tauri command, and inference provider so future readers do
  not reintroduce the PR7 confusion.
- Reconciled PR7 audit docs: the static-chrome search finding is now recorded as resolved by the
  later PR3 self-exclude/backfill fix (`07` #66) with residual rect-None / secondary-monitor risk
  left in `07` #58; the no-evidence Ask finding (`07` #41/#63) is resolved by the relabel approach;
  the PR8 stale-bitmap follow-up is renumbered to `07` #69 to remove the duplicate #66.
- Updated `docs/ARCHITECTURE.md` for the current backend search cap (`1..=2,000`, candidate pool
  capped at 2,000) and updated `docs/TESTING.md` to make PR7 audit artifacts local-only ignored
  evidence.

### Skipped / deferred
- No schema, migration, typed IPC, binding, or prompt/protocol change. True model-authored
  claim-level citations remain deferred until the app has a structured citation protocol; this pass
  only makes the current reviewed-context UI truthful.

### Verification
Automated gates passed on 2026-06-27; raw command output is pasted in the final session response:
- `cd ui && npm ci && npm run lint && npm run build`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --workspace`
- `cargo test --workspace`
- `git diff --exit-code -- ui/src/bindings`

Manual dev-exe verification passed with `npm run tauri dev`, launching
`target/debug/screensearch.exe`; logs are stored under
`.playwright-mcp/pr7-followups-2026-06-27/` and remain ignored local evidence.
- Recall Ask no-evidence query `PR7_NO_EVIDENCE_UNIQUE_TOKEN_20260627_X9Q` rendered an honest
  refusal and displayed retrieved tiles under `FRAMES CHECKED`; the old `Cited frames` label was not
  present.
- Daily report generation progress displayed the existing range-neutral copy:
  `Reports summarize active periods in bounded passes, so larger ranges can take a little longer.`
- Default Recall search for `chrome` returned result rows with the `CONTENT TEXT ONLY` control and
  static-toolbar filter copy visible, including Chrome hits plus backfilled non-Chrome rows, with no
  self-capture/static-chrome regression observed in the sampled dev app state.

---

## Audit — 2026-06-26 — 0.2.0 PR3 attention-first filtering

**Branch:** `codex/0.2.0-pr3-audit`. Runtime: `npm run tauri dev` launching
`target/debug/screensearch.exe`. DB policy: existing
`%APPDATA%\app.screensearchv2c.desktop\screensearch.db`, online backup to
`.playwright-mcp/pr3-2026-06-26/screensearch-pr3-before.sqlite`, no reset/backfill/destructive SQL.

### Implemented / audited
- Added the audit artifact `docs/AUDIT_0.2.0_PR3_2026-06-26.md`.
- Verified PR3's storage/retrieval plumbing: raw text is preserved, filtered content/spans/filter
  version are written, embeddings read `content_text`, default search uses content FTS, and
  `include_chrome=true` keeps raw/static recovery available.
- Verified Settings text-filter thresholds and per-app suppression readout load and match grouped
  SQL for the audited corpus.

### Broke / regressed / release blocker
- **Release blocker:** strict PR3 acceptance is not met. Default content search still has content FTS
  hits for static/app chrome terms (`Firefox` 24, `Steam` 24, `Deck` 68, `Recall` 42,
  `GPU Memory` 15) on the baseline DB. A fresh Notepad capture preserved the deliberate foreground
  content, but also indexed `Firefox`, `Deck`, `Recall`, and `COMMAND` in default `content_text`.
  See `docs/AUDIT_0.2.0_PR3_2026-06-26.md`, `06` patch #8, and `07` gap #64.

### Verbatim verification
Raw logs are preserved under `.playwright-mcp/pr3-2026-06-26/29-verify-ui-npm-ci-lint-build.txt`
through `34-verify-bindings-diff.txt`; the audit report includes the command output summary and
the exact evidence paths. All required commands exited 0:
`cd ui && npm ci && npm run lint && npm run build`, `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace`,
`cargo test --workspace`, and `git diff --exit-code -- ui/src/bindings`.

---


> Pre-0.2.x (v0.1.0) history archived in specs/archive/05_BUILD_REVIEW.v0.1.0.md.

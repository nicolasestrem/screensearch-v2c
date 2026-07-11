# Testing — quick guide

Run from the repo root. Each command stands alone. Green = good.

## ✅ The one command

```sh
cargo test --workspace
```

That runs every test. **0 failed = pass.** (GPU/model tests are skipped automatically — that's normal.)

## 🔁 Before you push (CI-order gates)

Run these. All must be clean:

```sh
(cd ui && npm ci && npm run lint && npm run build)
node scripts/stage-mcp.mjs   # once per clone — src-tauri's externalBin sidecar; bare cargo fails without it (0.3.0 PR8)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
git diff --exit-code -- ui/src/bindings
```

Tip: copy-paste them one at a time. Don't move on until one passes.

## 🩹 If something fails

| You see | Do this |
|---|---|
| `fmt` diff | `cargo fmt --all` (auto-fixes), then re-check |
| `clippy` error | read the `--> file:line`, fix that one spot, re-run |
| a test `FAILED` | scroll up to the test **name** + its `assert` message; that's the clue |
| `npm` error | `cd ui && npm ci` once, then `npm run build` |

## ⏭️ What gets skipped (and that's fine)

Tests marked `ignored` need a GPU, a downloaded model, or hardware. They are **not** run by `cargo test`. Only run these on purpose:

```sh
# real llama-server: downloads models + uses the GPU (slow, big)
cargo test -p inference --test smoke -- --ignored --nocapture

# event-driven capture hooks: start/drop the Win32 hook source 50× — no leak/hang (real desktop)
cargo test -p capture -- --ignored
```

The event-driven trigger logic itself runs in plain CI: the 9 `crates/capture/src/trigger.rs` unit
tests cover foreground debounce / rate-ceiling / idle-edge behavior (pure, no Win32), and the kernel
settings round-trip + sanitize + retired-key-drop tests cover the `capture.event_*` keys. Only the
hardware hook lifecycle test in `events.rs` is `#[ignore]`d. *(0.3.0 PR2 trimmed the six triggers to
foreground + idle, deleting the clipboard, click, scroll-stop, and typing-pause tests with their
triggers — `docs/0.3.0.md`.)*

## 🎯 Just one crate (faster)

```sh
cargo test -p inference     # the sidecar
cargo test -p kernel        # workers + scheduler
cargo test -p store         # database
```

## 🟢 The no-orphan gate (P4's must-pass)

Proves the sidecar can't outlive the app:

```sh
cargo test -p inference --test no_orphan
```

Want it to be `ok`. If it's not — stop and ask.

## Archived — manual acceptance checklists (pre-0.4.0)

The per-PR manual acceptance sections for the 0.2.x through 0.3.2 arcs (Manual PR7 audit,
event-driven capture, model tiers, image-lane removal, Flow overlay, where-was-i + marks,
local API + export, MCP server, polish bundle, auto-update) have shipped and moved to
[`specs/archive/TESTING.pre-0.4.0.md`](../specs/archive/TESTING.pre-0.4.0.md). The sections below
cover the current 0.4.0 sessions arc.


## Dev-only harness — segmentation ground truth + validation (0.4.0 PR2)

The `crates/harness` binary is a **dev-only, read-only** referee for the sessions arc. It is a
workspace crate, so it is built and tested by the normal `cargo` gates above, but it is **never
bundled** by the NSIS installer (only `src-tauri` + the `screensearch-mcp.exe` externalBin ship).
Run it with `cargo run -p harness -- <subcommand>`.

**Read-only guarantee.** Every query path opens the DB with `SQLITE_OPEN_READ_ONLY` + `PRAGMA
query_only`; the harness's unit tests assert a write is rejected on that connection. The only file
it writes to the DB side is the `backup` target (a `VACUUM INTO` snapshot to a fresh file). Exports
and hand labels are personal screen history and live under the **git-ignored** `harness-data/`.

**Automated tests (CI-safe, no real data).** `cargo test -p harness` runs the pure segmenter,
taxonomy, label-parsing, scoring (typed DP-optimal boundary matching), digest, and read-only export
tests against synthetic fixtures + a tempfile SQLite DB. No test touches `%APPDATA%` or a real path.

**Manual end-to-end (Phase A/B/C, maintainer-in-the-loop).**
1. **D5 backup FIRST** (release-blocker-class; before any other live-DB command):
   `cargo run -p harness -- backup --to <a dir OUTSIDE the repo and OUTSIDE %APPDATA%\app.screensearchv2c.desktop\>`.
   It writes `screensearch-YYYY-MM-DD.db`, refuses to overwrite, refuses a destination inside the
   repo tree or the app data dir, and prints `PRAGMA integrity_check` + source/copy row counts as
   the attestation. WAL note: if the app was force-killed and left an unrecovered `-wal`, a
   read-only open fails with an actionable message; start the app once (or point `--db` at the
   backup) and retry.
2. `cargo run -p harness -- suggest-days` prints a per-day survey (frames, distinct apps, coarse
   AI/meeting window-title signals, marks). Pick 5-10 representative days (a meeting-heavy day, a
   Claude Code day, a Codex day, a browser-AI day, a mixed/fragmented day, plus one contiguous
   2-3-day stretch for the stability check). June-July days avoid the DST-transition guard.
3. `cargo run -p harness -- export --days 2026-06-15,2026-06-16,...` writes each day to
   `harness-data/<day>/` (`day.json`, `frames.jsonl`, `marks.jsonl`, `digest.md`, `labels.toml`).
   Re-exporting a day refreshes the data files but preserves an existing hand-edited
   `labels.toml` (it prints `(kept existing labels.toml)`), so it is safe to re-run.
4. Hand-label each day's `labels.toml` from its `digest.md` (the readable context-run timeline;
   marks appear as anchors). Under an evening for the whole sample.
5. `cargo run -p harness -- score` (optionally `replay`, `sweep`, `stability`) scores boundary
   precision/recall/F1 (+/- tolerance) and tool-recognition accuracy against the labels.
   `sweep`/`stability` write markdown to `harness-data/reports/`.

**The segmenters — `--algo micro | grouped | concurrent | shipped`.** Pass 1 (`segment_micro`) produces
unfloored app-run micro-spans; pass 2 groups them. Three algorithms:
- `concurrent` (**default**, `06` #28 / `07` #114): the **per-identity-track** model. Sessions of
  different identities may overlap in wall-clock time (an AI track spans a meeting; two AI tools run
  at once) while a frame belongs to exactly one session. A foreign AI id opens its own track; a short
  unrecognized run absorbs into the last-touched open AI track, a leading one ramps into an opening
  track, a run over `absorb_max` is focus; an AI track emits only if its summed recognized presence
  reaches `IDENTITY_QUALIFY_MS`; meeting bands are no longer barriers and overlapping meetings are
  not merged.
- `grouped` (`06` #27): the serial two-pass segmenter (one open group, meeting bands as barriers) —
  kept as the A/B baseline.
- `micro`: the ungrouped `§7b` app-context baseline.
- `shipped` (PR4): calls the production `crates/sessions` concurrent engine with the frozen
  `merge_gap=2700s`, `absorb_max=1800s`, `meeting_gap=480s`, focus floor/density, qualification,
  and W constants. The harness `--gap-close`/`--min-len` flags still exercise the two final settings.

**`labels.toml` is v2 (`06` #28):** non-overlap is enforced **per identity track**, not globally —
`ai` sessions may not overlap another `ai` with the same `tool`; `focus`/`other` may not overlap
another of the same kind; `meeting` labels may overlap; the file is globally sorted by start.
Different identities may overlap. Serial (pre-v2) label files stay valid.

- `score` reports the **identity-partitioned** typed boundary P/R/F1 as the primary metric plus the
  pooled position-only `posF1` comparability column, at BOTH 120 s and 180 s tolerance (the D9
  evidence pair); `--tolerance <s>` overrides with a single window. Labels are snapped to the nearest
  captured frame (a disclosed policy for boundaries inside no-frame idle gaps).
- `replay` prints each session's context key, kind, tool, host, frame count, close reason, and marks
  overlapping sessions with `~` (the concurrency indicator).
- `sweep` runs the Stage-A `merge_gap x absorb_max` grid plus Stage-B 1-D knob sweeps (each FLAT ->
  named constant, or SENSITIVE -> keep as a setting), with BOTH a `micro` and a serial-`grouped`
  baseline line and predicted-session-count honesty columns.
- `stability` re-proves the freeze-lookback window (identity-partitioned boundaries: an identity swap
  at the same instant counts as drift).
- Group flags (proposed `sessions.*` names; PR4 owns the finals): `--merge-gap` `--absorb-max`
  `--meeting-gap` `--focus-min-len` `--focus-density`. Seg flags: `--gap-close` `--min-len`.
  Scoring: `--tolerance`. An unknown subcommand exits non-zero.

The approved D9 thresholds + chosen parameters land in `specs/05`/`06` (they are PR4's binding merge
gate). The exported sample and labels are never committed; specs/PR carry aggregate numbers only.

### PR4 production gate and live checks

1. **CI-runnable parity:** `cargo test -p harness --test shipped_parity`. The table-driven fixtures
   cover interleaved tools, short None absorption, meeting overlap, merge-gap equality, focus density,
   AI qualification, open projection, renamed `app_hint=chatgpt,title=Codex`, and excluded
   `ChatGPT Classic`. Boundaries/identity/host and first/last metadata match harness-concurrent;
   production additionally owns pass-1-consumed excursion frame ids, which the frozen referee only
   counted as absorbed time.
2. **Binding D9 re-run:** prepare an input directory containing only `2026-07-07`, `2026-07-08`,
   and held-out `2026-07-09` (do not include the 07-10 capture-limit demonstrator), then run:
   `cargo run -p harness -- score --algo shipped --data <three-day-dir>`, followed by the same command
   with `--algo micro` and `--algo grouped`. With no explicit tolerance, each command prints both
   ±120 s and ±180 s. Gate criteria are verbatim in `06` #26; a miss stops the PR with no retuning.
3. **D5 backup before live launch:**
   `cargo run -p harness -- backup --to <outside-repo-and-app-data-dir>`. Confirm the printed
   integrity check and frame/mark count parity, then print the backup's full path, byte size, and
   mtime. Only after this gate may `npm run dev` open the live schema-11 DB. Never launch the debug
   executable directly.
4. **Historical/incremental observation:** keep capture running while logs show
   `sessions historical backfill advanced` (`cursor_ms` increasing toward `target_ms`). Query
   `sessions`, `frames.session_id`, and `session_artifacts` to confirm old rows appear, open/recent ids
   reconcile stably, and new frames continue arriving. A segmenter error must log and leave capture
   active.
5. **Recognition + D8 qualitative check (maintainer in the loop):** foreground a real Claude Code
   session, Codex desktop session, browser-AI page, and meeting-titled window long enough to pass the
   frozen floors; inspect `kind/tool/host/context_key`. For an AI row, inspect `kind='exchange'`
   artifacts: explicit user/agent markers may produce rows; no marker must produce none, never an
   invented role.
6. **D10 regression spot-check:** exercise where-was-i, frame search, Ask, Timeline, and marks while
   capture continues. PR4 adds no commands, no NavRail route, no audio, and no notification surface.

## Native acceptance — sessions UI (0.4.0 PR5)

This is a **real Tauri/WebView2 acceptance run**, not a Vite-browser substitute. Start only from the
repo root with `npm run dev`; never launch the debug executable or use `cargo tauri dev`. Before
opening the PR5 build against the live database, confirm the D5 backup described above still exists
outside both the repo and the app-data directory. Use real schema-11 session data. A local answer
model must be ready for the Recap checks.

Record the date, Windows build, WebView2 version, monitor resolution/DPI, database frame/session
counts, and screenshots or screen recording for every observed result. Do not mark a row accepted
when the required real-data state is absent; record that state as **not available** and carry it to
the PR7 audit.

### Navigation and data truth

1. Open Timeline on a range containing multiple session kinds and overlapping sessions. Confirm the
   density ribbon remains visible, session bands use the existing neutral/ok/warn token vocabulary,
   every visible band has a usable native-button hit target, and the band layer is exactly four lanes.
   Use a fixture/range with at least five simultaneous collisions: confirm no fifth row appears, the
   omitted bands are counted by the neutral “more sessions — narrow the range” keyboard control, and
   activating that control focuses the existing range presets. Repeat during initial route skeleton,
   loading, error, empty, and populated; all five states must reserve identical four-lane geometry
   with no clipping, nested scrolling, horizontal scrolling, or layout shift.
2. With pointer input, follow **band → session drill-in → representative Moment → back**. Confirm
   Back returns to the same session drill-in and the drill-in remains on the same Timeline context.
   Open a Moment directly and use **Part of session**; confirm the link is omitted for a frame with
   no session. Also open the session URL directly and confirm its back action returns to Timeline.
3. In the drill-in, compare the displayed absolute span, kind/tool/host, confidence, frame total,
   representative first/last frames, and exchanges against the real session row/artifacts. Confirm
   exchange links open the cited Moment and no exchange roles are invented when artifacts are absent.
4. Generate **Recap** with the answer model ready. Confirm progress is visible, the result uses the
   existing report rendering/footer, every citation opens a Moment owned by this exact session, and
   no frame from an overlapping session leaks into the citations. Run it again and press **Cancel**;
   confirm generation stops and no late result replaces the idle state. Repeat once while navigating
   away mid-generation to confirm route teardown also cancels backend work.
5. Open Deck and confirm the existing Jump back action still opens its Moment while the optional
   session span truthfully frames it. In Advanced Settings, confirm the collapsed **Sessions** group
   contains only `sessions.min_len_secs` (30–3600) and `sessions.gap_close_secs` (60–3600), saves
   normally, and adds no NavRail item.

### Keyboard, state, and layout matrix

6. Keyboard-only: focus the Timeline slider and use its existing arrow/Enter behavior; then Tab to
   session bands, move through every band in DOM order, press Enter/Space to open one, traverse every
   drill-in action/link, open a Moment, and return. Confirm visible focus throughout and no interactive
   button is nested inside the ARIA slider.
7. Observe each real-data state that is available: loading skeletons; list/detail error + Retry;
   frames with **no sessions** (density ribbon remains usable); session with **no exchanges**; **open**
   session (still running/non-final boundary); **low-confidence** session (numeric confidence shown);
   missing/deleted session; summary failure/retry; Recap failure/retry. Do not synthesize database
   rows or mock IPC responses merely to make a state appear tested.
8. At 1280×720, 1920×1080, and 3440×1440, check 100%, 125%, and 150% Windows display scaling where
   the hardware permits. Cover Timeline, session drill-in, affected Moment, and Deck. At each point
   confirm fixed rails, exactly one vertical route scroll context, no route-level horizontal
   scrollbar, no nested vertical scroller, wrapped frame/citation/exchange strips, no clipped band
   lanes, and no cumulative layout shift when session/summary/Recap data arrives. Repeat with Windows
   reduced motion enabled and confirm session-band/scan ambient motion is absent.
9. Keep Timeline mounted while a real scheduler pass commits new/reconciled session rows. Confirm the
   Tauri `sessions_changed` event payload is `null`, the mounted session list refetches without route
   navigation or manual reload, and the new bands/ownership become visible. Confirm the signal creates
   no toast, notification, badge, nudge, or score. Repeat an unchanged pass and confirm it emits no
   event; repeat with a failed scheduler pass and confirm no false success is claimed (a failed
   multi-write pass may conservatively invalidate because an earlier write may already have
   committed).

### PR5 evidence status

**Observed 2026-07-10 — native acceptance passed, with the open-session variant unavailable in the
current live dataset.** The run used `npm run dev` against the live schema-11 database, with the
worktree's `target/debug/screensearch.exe` process and a real Tauri WebView
(`window.__TAURI_INTERNALS__ === true`), not browser mocks. The live status showed 515 captures / 514
tagged. Evidence:

- Deck rendered `JUMP BACK ... until 17:08 session 16:44–17:14`, preserving the existing action while
  adding truthful session framing.
- Timeline rendered eight real overlapping sessions in two lanes. At 1280×720 DPR 1, 1920×1080 DPR
  1, and emulated 3440×1440 DPR 1.5, document width equalled viewport width; every band was at least
  32×32; band overlaps, nested vertical scroll contexts, and horizontal overflow were all zero.
  Accessible band labels included kind, full date/time, and tool/host.
- Keyboard focus on the first band followed by CDP native `rawKeyDown` / `char` / `keyUp` Enter
  opened `/timeline/session/3`. The real round trip was AI band → session 3 → representative frame
  `/timeline/2651`; Moment showed back label `SESSION` and `PART OF SESSION ScreenSearch Workflow`,
  and SESSION returned exactly to `/timeline/session/3`.
- Session 3 showed AI · codex/desktop · Confidence 78.5% · Jul 10 01:21–02:50 · 41 frames / 24
  representatives, plus the honest `No exchanges captured for this session.` state. It had one
  vertical scroller and no horizontal overflow.
- Recap cancellation was observed after `Summarizing 1 of 4 · 1/4`: clicking CANCEL returned to
  GENERATE RECAP and no stale result appeared. A clean Recap completed in 15 seconds with 5 passes,
  1/1 periods, 39/39 frames summarized, and the truthful trimmed footer. All 39
  `report.cited_frame_ids` were read back through the live `get_frame` command; every frame resolved
  with `session.id === 3`.
- Live session 21 provided the low-confidence state: neutral Confidence 47.2%, 50 frames, no invented
  exchanges, one scroll context, and no horizontal overflow. A live `list_sessions` query returned
  21 sessions and open IDs `[]`; therefore the open-session visual variant was **unavailable in this
  dataset**, not passed or failed. Its implementation/test coverage remains, but no live observation
  is claimed.
- With CDP emulating `prefers-reduced-motion: reduce`, the media query matched and computed animated
  elements were `[]`. The Sessions expander showed Minimum session length 120 (min 30 / max 3600)
  and Close gap 300 (min 60 / max 3600), with labels explaining the next session pass.
- The only console/network noise was the existing `favicon.ico` 404 and an informational WebView
  lazy-image intervention; no app runtime errors were observed. The screenshot remains outside the
  repo at `%TEMP%\screensearch-pr5-timeline.png` and is not committed.

**Observed 2026-07-11 — final review-fix acceptance at code `02e5cad` / docs `67b76ce`.** The real
`npm run dev` process was again this worktree's `target/debug/screensearch.exe`, with
`window.__TAURI_INTERNALS__ === true`. Through the real typed listener in
`/src/lib/ipc/events.ts`, a startup scheduler pass produced `sessions_changed` probe `{count:1}` and
no toast or notification. Fixed geometry was then observed at 1280 px:

- Forced initial loading through the existing dev-state seam: grid
  `32px 32px 32px 32px`, grid height 140, session outer height 192, and five skeleton elements.
- Live empty Today: the same 140/192 grid/outer geometry and no horizontal overflow.
- Live populated 7-day: the same 140/192 geometry, 21 visible bands, zero band overlaps, and document
  width 1280 equal to viewport width 1280.
- Live populated dense 30-day: the same 140/192 geometry, 12 visible bands, neutral
  `9 more sessions — narrow the range`, zero overlaps, and document width 1280 equal to viewport
  width 1280. The overflow button was keyboard-focusable. CDP native
  `rawKeyDown` / `char` / `keyUp` Enter moved focus to `TODAY`, whose parent `aria-label` was
  `Time range`.

These observations close the focused dense-overflow, fixed-state-geometry, keyboard-focus, and live
scheduler-refresh steps above. No schema, API/MCP, or frame behavior changed.

**Final clean suite (Pass 12, 2026-07-11):** the required UI-first sequence passed at tip `8629e0c`,
with the additional `npm run test` session-band regression gate between `npm ci` and lint. Full
color-disabled raw output—including the 1,084-line workspace test log and empty fmt/binding-guard
outputs—is preserved verbatim in `specs/05_BUILD_REVIEW.md` Pass 12. The npm allow-scripts warning
was non-failing.

**PR #104 review follow-up (Pass 13, 2026-07-11):** all four unresolved inline threads were
applicable and are fixed in code `c360d7e`; two store threads described the same unbounded content
scan. Focused RED/GREEN evidence is preserved in `specs/05_BUILD_REVIEW.md` Pass 13. The store query
now stops in SQLite and exactly preserves Rust Unicode trim behavior; Timeline loading/error/empty/
populated all reserve four lanes (the forced error/loading render measured four 31.9965 px rows,
139.9653 px total, 1704 px document/viewport parity, and no nested scrollers); and a repeated
unchanged scheduler pass emits no `sessions_changed` event or redundant ownership/artifact writes.
This is focused review verification only; the post-review full UI-first suite is recorded separately
after it runs.

**Post-review final clean suite (Pass 14, 2026-07-11):** the required color-disabled UI-first
sequence passed after code `c360d7e` and review documentation `3f62479`: `npm ci` → `npm run test`
→ lint → UI build → MCP staging → fmt → workspace clippy/build/test → generated-binding guard. All
ten commands exited 0; the npm allow-scripts warning was non-failing. The ten fenced blocks in
`specs/05_BUILD_REVIEW.md` Pass 14 were compared directly with their captured logs; all matched,
including the complete 1,091-line workspace-test log and the empty fmt/binding-guard logs.

The automated UI-first verification below this acceptance record remains the build/test evidence;
the native observations above complete `03 §13c-5` without substituting mocks for the live app.

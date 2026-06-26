# 0.2.0 PR7 Integration Audit - 2026-06-25

Branch: `codex/0.2.0-pr7-integration-audit`

Runtime audited: `npm run tauri dev`, driving the real Tauri debug executable
`target/debug/screensearch.exe` against the existing populated app-data store at
`%APPDATA%/app.screensearchv2c.desktop`. The database was not reset or backfilled.

Local screenshot evidence is stored under ignored local storage:
`.playwright-mcp/pr7-2026-06-25/`. These PNGs are intentionally not tracked or embedded in this doc.

## Baseline

- Initial Deck evidence: `01-deck-initial.png` showed capture off, database size 95 MB, readiness
  ready, 432 captures today, 410 tagged, and enrichment queue `running 0 / pending 0 / done 1192`.
- The UI was reachable through the `npm run tauri dev` window and Computer Use snapshots.
- Read-only SQLite check after the live capture step showed `433` rows in `frames` and `433` rows in
  `frame_text`.

## Search Audit

Screenshots:

- `03-default-search-firefox.png` - default content-text search for `Firefox`.
- `04-default-search-steam.png` - default content-text search for `Steam`.
- `05-default-search-deck.png` - default content-text search for `Deck`.
- `06-default-search-recall.png` - default content-text search for `Recall`.
- `07-default-search-gpu-memory.png` - default content-text search for `GPU Memory`.
- `08-content-search-calendar-grid.png` - content search for `Calendar-Grid`.
- `09-content-search-cargo-test.png` - content search for `cargo test`.
- `10-include-chrome-search-firefox.png` - `include app chrome + raw text` enabled, searching
  `Firefox`.

Observed:

- Content searches for real work terms were useful. `Calendar-Grid` and `cargo test` returned PR,
  terminal, and documentation captures tied to the populated corpus.
- `GPU Memory` results were mostly legitimate app/content captures.
- The `include app chrome + raw text` toggle worked and recovered raw/static labels.
- Default content-only search still returned static-label-heavy rows for `Firefox`, `Steam`, and
  especially `Deck` in the existing corpus. Read-only counts after audit:
  - `Firefox` in `content_text`: explorer 4, screensearch 2, SnippingTool 1, Codex 1.
  - `Steam` in `content_text`: screensearch 3, explorer 3, SnippingTool 1, Codex 1.
  - `Deck` in `content_text`: screensearch 150, chrome 6, explorer 2, plus one each from notepad++,
    Codex, and Code.
- The live capture taken during PR7 also stored ScreenSearch UI labels in `content_text` for the new
  frame 433, including `COMMAND DECK`, `CAPTURE`, `TODAY`, `Timeline`, `Insights`, and `Settings`.
  That frame still suppressed many background spans (`suppressed_count = 464`), so the issue is not
  a total classifier failure; it is an acceptance gap where captured app chrome / screenshot-recents
  can still become searchable content.

Status: default retrieval is improved but PR7's strict "static chrome should not dominate default
search" acceptance is not fully met on the populated corpus. Per user direction, this audit did not
rewrite or backfill the DB.

## Ask Audit

Screenshots:

- `12-ask-initial.png` - Ask tab idle state.
- `13-ask-calendar-progress.png` and `14-ask-calendar-final.png` - grounded question about
  Calendar-Grid Coverage Map-Reduce.
- `15-ask-calendar-citations.png` - cited frame tiles for the grounded answer.
- `16-ask-no-evidence-progress.png` and `17-ask-no-evidence-final.png` - no-evidence question using
  unique token `SSV2C_PR7_NEVER_CAPTURED_20260625_QZX`.

Observed:

- Positive Ask grounded correctly on content text and produced cited frame tiles. The answer described
  Calendar-Grid Coverage Map-Reduce as the Daily/Weekly/Custom report engine using bounded passes over
  attention-filtered content text.
- The no-evidence prompt did not fabricate a claim about the unique token; it said no specific
  information was present.
- However, the no-evidence answer still displayed `CITED FRAMES` with unrelated context frames. This
  is technically consistent with the backend's "frames the model saw" citation model, but the UI label
  reads as support for the refused claim. This remains an audit finding.
- The Thinking panel is visible when `answer.thinking` is enabled. That is currently a documented
  setting, not a PR7 code change.

## Reports Audit

Screenshots:

- `18-reports-initial.png` - Reports idle state.
- `19-daily-report-progress.png` and `20-daily-report-final.png` - Daily report generation and result.
- `21-daily-report-source-frames.png` - Daily report source-frame/footer evidence.
- `22-weekly-report-progress.png` and `23-weekly-report-final.png` - Weekly report generation and
  result.

Observed:

- Daily generated a user-facing report with source frames and footer metadata. The observed footer
  reported `6 passes`, `39/40 frames summarized`, and `range trimmed to fit - more was captured than
  summarized`.
- Weekly generated a report for the last seven days, explicitly stating that available activity was
  only on day 7. It also reported `6 passes`, `39/40 frames summarized`, and the same trim notice.
- During the audit, the generating helper text said "Weekly reports..." even when Daily was selected.
  This was fixed in `ui/src/routes/Recall.tsx` by changing the copy to range-neutral bounded-pass
  language.

## Live Capture Audit

Screenshots:

- `24-deck-before-capture.png` - capture off, 432 captures today, queue done 1192.
- `25-capture-running.png` - capture started.
- `26-capture-tick.png` - capture tick observed: 433 captures today, ScreenSearch app count 162, queue
  done 1194, new `20:26` capture card visible.
- `27-capture-stopped.png` - capture stopped and UI returned to off state.

Observed:

- Start/stop capture works in the dev executable.
- The live step appended one visible capture row and advanced enrichment queue counts without any
  destructive DB operation.

## Findings

1. Default content search still exposes some static/app chrome from the populated corpus and from a
   fresh ScreenSearch capture. No DB rewrite/backfill was attempted.
2. Ask refuses a no-evidence unique token honestly, but still displays retrieved context frames under
   `CITED FRAMES`; the citation semantics are ambiguous for refusals.
3. Daily report progress copy was mislabeled as Weekly-only. Fixed with range-neutral copy.

## Verification Notes

The audit began from a clean worktree and kept PR7 screenshots in `.playwright-mcp/`. Final
verification commands and raw output are recorded in the session response; screenshot files are not
tracked.

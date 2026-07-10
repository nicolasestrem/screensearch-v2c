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

## 🧭 Manual PR7 audit

PR7 is a live UI audit over the user's populated app-data store. Run the app with:

```sh
npm run dev
```

Use the existing `%APPDATA%\app.screensearchv2c.desktop` DB. Do not reset or backfill it. Store local
evidence under ignored paths such as `.playwright-mcp/pr7-YYYY-MM-DD/` and, if needed,
`docs/AUDIT_0.2.0_PR7_YYYY-MM-DD.md`; do not put PR7 images in `screenshots/` or commit audit
artifacts.

Audit coverage:

- Recall Search default content text vs. `include app chrome + raw text`.
- Real content terms from the corpus.
- Ask positive grounding and no-evidence behavior. For no-evidence refusals, source-frame tiles may
  appear only as reviewed context labeled `Frames checked`, not as `Cited frames`.
- Daily and Weekly reports, including pass/frame footer metadata.
- A short start/stop capture tick.

The 2026-06-25 run is a local ignored artifact at `docs/AUDIT_0.2.0_PR7_2026-06-25.md`; tracked
summaries live in `CHANGELOG.md` and `specs/05_BUILD_REVIEW.md` / `07_KNOWN_GAPS.md`.

## 🎛️ Manual acceptance — event-driven capture (0.2.1; trimmed 0.3.0 PR2)

Opt-in event-driven capture (`07` #47) needs a quick live check on a real Windows desktop with
`npm run dev`. **0.3.0 PR2** trimmed the six triggers to **foreground + idle** (clipboard,
click, scroll-stop, and typing-pause removed — `docs/0.3.0.md`); the Settings panel now shows only
the master toggle, app-switch, idle, and the three surviving thresholds (debounce, min-interval,
idle-threshold — plus the fallback interval).

- **Default off.** On a fresh profile, Settings shows event-driven capture **off** and capture uses
  the timer cadence (no input hooks installed). The clipboard / click / scroll-stop / typing-pause
  toggles are gone.
- **Foreground fires on activity.** Turn **Event-driven capture** on, start capture, then **alt-tab**
  to another app → a new frame appears; the Moment view's **"Captured via"** row shows **App switch**.
- **Idle fires after the threshold.** Leave the machine untouched past the idle threshold (default
  5 s) → one **Idle** frame appears; a static screen is still sampled at least every fallback
  interval (default 30 s, tagged **Timer**).
- **Legacy tokens still render.** Open a frame captured before the trim whose trigger was
  `click` / `scroll_stop` / `clipboard_change` / `typing_pause` (from an earlier 0.2.1 run) — the
  Moment "Captured via" row still renders **Click** / **Scroll stop** / etc.; new frames never emit
  those tokens again.
- **Retired keys drop on load.** A profile whose settings DB still holds the retired
  `capture.event_on_clipboard` / `…_typing_pause` / `…_on_click` / `…_on_scroll_stop` /
  `…_typing_pause_ms` keys loads cleanly: the app logs **one** warn (`settings: dropped retired keys`)
  on first launch and nothing on the next (the keys are gone).
- **Timer mode unchanged.** Turn event mode back off → capture returns to the fixed-interval cadence,
  with no behavior regression.
- **Privacy.** Nothing typed is stored — only the timing/change signal. Existing gates (self-exclude
  own window, excluded apps, pause-on-lock) still apply in event mode.

## 🎛️ Manual acceptance — model tiers (0.3.0 PR3)

**0.3.0 PR3** retired the **Beta** model tier: each lane (Vision, Answer) now offers **Default** and
**Quality** only (`docs/0.3.0.md`, D3/D4). Quick live check on a real Windows desktop with
`npm run dev`.

- **Two tiers per lane.** Settings → Models shows the tier picker with exactly **Default** and
  **Quality** for both the Vision and Answer models — no **Beta** button. Hovering a tier shows the
  resolved model name (e.g. Vision Quality → *Qwen3-VL-8B-Instruct*).
- **Persisted `beta` loads as Quality.** With the app closed, set a lane's persisted tier to the
  retired value (`UPDATE settings SET value='"beta"' WHERE key IN ('models.vision_tier',
  'models.answer_tier')` in `<app-data>\screensearch.db`), then launch. The app logs **one** warn per
  lane (`settings: retired \`beta\` tier mapped to \`quality\``) on first launch and **nothing** on
  the next; Settings shows **Quality** on that lane; the DB row is rewritten to `"quality"`.
- **On-disk Beta files untouched.** Any previously downloaded Beta GGUF stays on disk (no automatic
  cleanup); it can be removed from the existing model-management surface.
- **Both surviving tiers resolve.** Switching a lane between **Default** and **Quality** hot-applies
  (toast) and the sidecar reloads that lane's model; the first use of an as-yet-undownloaded tier
  fetches it per `MODEL_REGISTRY §4` (a multi-GB download — allow time). *(Downloading both Quality
  GGUFs end-to-end is the PR9 pass; the tier→repo resolution for all four `(lane, tier)` pairs is
  pinned by the `repo_mapping_matches_registry` unit test.)*

## 🎛️ Manual acceptance — image-embedding lane removal (0.3.0 PR4)

**0.3.0 PR4** removed the dark-launched, flag-off nomic-embed-vision **image-embedding lane**: the
`enrich.image_embeddings` setting, the second vec0 table, and the `embed_image` job kind are gone,
proven by a forward-only **v8 → v9** migration that drops *only derived, re-derivable* vectors
(`docs/0.3.0.md` PR4, D5/D15). Quick live check on a real Windows desktop with `npm run dev`,
against a **backed-up copy** of a populated 0.2.x/PR3 profile (`<app-data>\screensearch.db`).

> **PR9 audit note (2026-07-04):** the live populated-0.2.x-profile pass below was **waived by
> user decision** — the app has zero installed users, so no real pre-v9 databases exist in the
> wild. The migration chain stays proven by the automated populated-DB tests
> (`migration_v9_drops_image_lane_and_embed_image_jobs`, `fresh_and_migrated_schemas_agree_at_latest`,
> and PR6's `migration_v10_adds_marks_with_cascade`). The steps remain documented for any future
> upgrade-path re-verification. The settings-tolerance checks (retired-key drop, `beta` remap)
> need no 0.2.x profile and stay in the PR2/PR3 sections.

- **Seed the legacy state (app closed).** In the backed-up DB:
  `INSERT INTO settings VALUES('enrich.image_embeddings','true');` and one leftover job —
  `INSERT INTO jobs (kind, frame_id, state) VALUES ('embed_image', <an existing frame id>, 'pending');`
- **Migration on boot.** Launch → the log reaches `applied store migration … schema_version=9` and
  the app boots normally. Verify with sqlite3:
  `SELECT name FROM sqlite_master WHERE name LIKE 'image_embedding%' OR name='image_embeddings_ad';`
  → **empty**; `SELECT count(*) FROM jobs WHERE kind='embed_image';` → **0**; the `frames` and
  `embeddings` (text) counts are **unchanged** from before launch.
- **Retired key drops once.** First launch logs **one** `settings: dropped retired keys` warn naming
  `enrich.image_embeddings`; the next launch logs **none**.
- **Settings UI.** Settings → Enrichment shows only **Embed OCR text** (no **Embed images** toggle);
  the Performance-throttle hint no longer mentions image embeds.
- **Text embeddings still work post-migration.** Start capture: `embed_text` jobs drain (job stats),
  `embed_model` readiness reaches **Ready**, and a semantic search over freshly captured content
  returns hits — confirming the trimmed fastembed build still loads and runs the text model.

## Manual acceptance — Flow overlay (0.3.0 PR5)

**0.3.0 PR5** adds the global-hotkey Flow overlay: a hidden, capture-protected second Tauri window
for quick Search/Ask over the current foreground context (`docs/0.3.0.md` PR5, D6/D7). Run on a
real Windows desktop with `npm run dev`, capture enabled, and at least a few searchable frames
in the profile.

- **Default hotkey and keyboard loop.** Focus an unrelated app, press `Ctrl+Alt+Z`: the overlay
  appears over that app, the input is focused, and the taskbar does not gain a second ScreenSearch
  entry. Type a query, use `ArrowDown`/`ArrowUp` to move the active row, press `Enter`: the overlay
  hides and the main window opens the selected Moment. Reopen, press `Tab`: Search/Ask mode toggles.
  Reopen, type `? what was I reading`, press `Enter`: Ask streams in the overlay. Press `Esc`: it
  hides and any in-flight Ask stream is cancelled.
- **Settings hotkey controls.** Settings -> Hotkeys -> Flow overlay records a modifier chord, ignores
  pure modifiers, saves through `set_settings`, and `get_hotkey_status` reports the active chord as
  registered. Reset restores `Ctrl+Alt+Z`. Settings -> Overlay results clamps direct entry to
  `1..=50`.
- **Registration conflict is loud (D6).** Hold a chord with another local app or a tiny test program,
  then set the overlay hotkey to that same chord. Expected: a warning toast appears and Settings
  shows the failed chord with its error; releasing the conflict and saving the same chord again
  registers successfully.
- **Placement and focus.** Trigger the overlay from apps on each monitor. The window appears on the
  foreground monitor when a foreground window rect is available; otherwise it falls back to the
  primary monitor. It does not appear until the hotkey is pressed and hides on blur.
- **Capture self-exclusion (D7).** With capture running, open the overlay for at least one capture
  interval, search for visible overlay-only text such as `Search your screen history`, and inspect the
  newest captured frames around that time. The overlay must not appear in its own capture history.
- **Exclusive fullscreen canary.** Put a game or video app into exclusive fullscreen and press the
  overlay hotkey. Some apps may suppress global overlays; this is accepted by the 0.3.0 contract. The
  required behavior is that ScreenSearch does not steal focus or silently change settings.
- **Latency note.** From hotkey press to focused input should feel effectively instant on a warm app.
  For a repeatable local measurement, compare `overlay_perf` log timestamps around `summon_overlay`,
  `overlay_shown`, and `overlay_shown_ack` on a warm profile.

## Manual acceptance — where-was-i + marks (0.3.0 PR6)

**0.3.0 PR6** adds two pull-based flow-recall features (`docs/0.3.0.md` Part II, D8/D9/D10/D14): the
**where-was-i** heuristic (overlay empty state + Deck card) and **mark-this-moment** (global hotkey
`Ctrl+Alt+M` → a `capture_now` past the diff gate + a mark, with a non-focus-stealing toast). Run on a
real Windows desktop with `npm run dev`, capture enabled, and a few minutes of history.

- **Where-was-i offers the work context (the core flow).** Spend ≥ 2 minutes (the default
  `resume.min_dwell_secs = 120`) in one app (e.g. VS Code), then switch to a browser for a short detour
  and stay there. Press the overlay hotkey (`Ctrl+Alt+Z`) with the query empty: the strip reads
  **"Jump back: <window> — <app>, until HH:MM"** for the work app, not the browser. Press `Enter` (or
  click): the main window opens that Moment. The Deck's **"Where was I?"** card shows the same.
- **Honest empty.** On a fresh profile, or when you never left one app, the strip/card reads **"Nothing
  to resume yet"** — never a fabricated suggestion.
- **Mark a static screen (diff-gate bypass).** Open a third-party app and leave the screen completely
  still for several capture intervals (so the diff gate would drop a normal frame). Press `Ctrl+Alt+M`:
  a **"Marked ✓"** toast appears over the app, and a new frame is captured **at press time** (verify in
  Timeline / the Intentions strip that the marked frame's time is *now*, not an older frame).
- **The toast does not steal focus (D1).** Immediately after pressing `Ctrl+Alt+M`, keep typing in the
  underlying app — your keystrokes land there, not in the toast. Then click the toast's note field: the
  overlay takes focus, type a note, press `Enter` — the note saves and the toast dismisses. Ignoring the
  toast entirely lets it fade after ~6 seconds.
- **Intentions strip.** Open the Deck: the **Intentions** strip lists the mark (thumbnail or "text kept"
  reconstruction, the note or a title fallback, and its age), newest first. **Open** navigates to the
  Moment; **Done** and **Dismiss** both resolve it (it leaves the strip). Confirm there is **no badge
  count** anywhere in the app.
- **Capture off is honest (not silent).** Stop capture, then press `Ctrl+Alt+M`: the toast reads
  **"Capture is off — mark not saved"** and no mark appears in the strip.
- **Excluded app is refused.** Focus an app on the excluded-apps list (e.g. a password manager) and
  press `Ctrl+Alt+M`: the toast explains the app is excluded, and no mark is created.
- **Multi-monitor determinism.** With the foreground window on a secondary monitor, mark it: the marked
  frame's monitor is the one holding the foreground window. Minimize everything (no resolvable
  foreground) and mark: the frame is captured on the primary monitor.
- **Hotkey conflict is loud (D6).** Pre-register `Ctrl+Alt+M` in another app (or set both ScreenSearch
  hotkeys to the same chord), then set the mark hotkey to it in Settings → Hotkeys: a warning toast
  fires and Settings shows the failed chord. Releasing the conflict and saving again registers cleanly.
- **Retention keeps a mark reachable.** For a marked frame whose screenshot has expired
  (`storage.retention_days`), the Intentions row still renders (the "text kept" state) and Open shows
  the text reconstruction — the mark is never orphaned.
- **Five overlay states.** Exercise the overlay empty-state strip's loading / error / null / populated
  paths (e.g. via `?__devState=…` where supported, or by toggling capture/data).

## Manual acceptance — local API + export (0.3.0 PR7)

**0.3.0 PR7** adds the opt-in local HTTP API and JSON export (`docs/0.3.0.md` Part III, D11/D12;
`03 §7c`). Run on a real Windows desktop with `npm run dev`, capture enabled, and a few minutes
of history. Use a terminal with `curl` (and `jq` for readability). The token is shown in
Settings → **Local API** after you enable it.

- **Off by default (fresh profile: nothing listens).** On a profile that never enabled the API, open
  Settings → Local API: the panel reads **"API disabled — nothing is listening"** and shows the
  threat-model line. Confirm nothing is bound: `netstat -ano | findstr 43210` returns no LISTENING row.
- **Enable → listening + token.** Toggle **Enable local HTTP API** on. The panel flips to **"Listening
  on 127.0.0.1:43210"** with a masked token (Reveal / Copy / Regenerate). `netstat -ano | findstr
  43210` now shows a LISTENING row on `127.0.0.1`.
- **401 without / with a wrong token.** With the API on:
  - `curl -s -o NUL -w "%{http_code}" http://127.0.0.1:43210/v1/health` → **401**.
  - `curl -s -o NUL -w "%{http_code}" -H "Authorization: Bearer wrong" http://127.0.0.1:43210/v1/health`
    → **401**.
  - `curl -s -H "Authorization: Bearer <token>" http://127.0.0.1:43210/v1/health` → **200** with
    `{"version":…,"uptime_secs":…,"capturing":…}`.
- **Every endpoint round-trips.** With the token set (e.g. `set TOKEN=<token>` then
  `-H "Authorization: Bearer %TOKEN%"`):
  - `GET /v1/search?q=<term>` returns matching hits (the same as the UI search).
  - `GET /v1/frames/<id>` returns metadata + text; `GET /v1/frames/<id>?image=1 --output f.webp` writes
    the screenshot (or `404 image_purged` if it expired).
  - `GET /v1/context/where-was-i` returns the resume context (or `null`).
  - `GET /v1/marks` lists marks; `POST /v1/marks` with `{"frame_id":<id>}` creates one (`201`);
    `{"now":true}` captures + marks now; `POST /v1/marks/<id>/resolve` resolves it. Both `frame_id` and
    `now` together → `400`; an unknown frame/mark → `404`.
- **SSE ask, and disconnect cancels generation.** With an answer model loaded,
  `curl -N -H "Authorization: Bearer %TOKEN%" -H "Content-Type: application/json" -d "{\"query\":\"what
  did I read about X\"}" http://127.0.0.1:43210/v1/ask` streams `data:` events (`token`, then `citation`,
  then `done`). Start another and press `Ctrl+C` mid-answer: the log shows **"answer stream cancelled by
  consumer; aborting sidecar generation"** and the sidecar stops (GPU/CPU settle) rather than finishing
  into a closed socket.
- **Port conflict is loud + guided.** Occupy the port first (in another terminal:
  `python -m http.server 43210` or any listener), then toggle the API on: a **warning toast** fires and
  the panel shows **"port in use"** with an inline **Retry** and the error under the port field. Change
  the port field to a free port and Retry (or disable the other listener): it binds and reads
  "Listening…".
- **Regenerate token — no restart.** While listening, click **Regenerate**. The old token now gets
  `401`, the new token `200`, on the **same** running server (no toggle-off/on needed).
- **Change the port while running (PR #76 fix).** While listening, edit the port field to a different
  free port: a **"Restart on {port}"** button appears with the "differs from the running port" note.
  Click it — the server rebinds and reads "Listening on 127.0.0.1:{new port}", and `netstat` shows the
  new port LISTENING (the old one freed).
- **Malformed request → JSON 400 (PR #76 fix).** With the API on,
  `curl -s -H "Authorization: Bearer <token>" "http://127.0.0.1:43210/v1/search?limit=10"` (no `q`) and
  `.../v1/frames/not-a-number` each return `{"error":"bad_request","message":…}` with
  `content-type: application/json` — not a plaintext framework rejection.
- **Export works with the API disabled.** Turn the API **off**. Click Settings → Data export →
  **Export…**: a success toast shows the written path in your Downloads folder
  (`screensearch-export-<stamp>-<rand>.json` — the random suffix keeps same-second exports from
  colliding). Validate it: `jq . "%USERPROFILE%\Downloads\screensearch-export-*.json"`
  parses, with `schema:"screensearch.export.v1"`, a `frames` array (metadata + `content_text`, **no**
  image bytes), and a `marks` array.
- **Exit frees the port.** With the API listening, quit the app. `netstat -ano | findstr 43210` shows
  no LISTENING row afterward (the server is stopped on exit; an in-flight `curl -N` ask does not wedge
  the quit).
- **Five panel states.** Exercise the Local API panel's loading / load-error / off / enabling /
  listening paths (toggle, retry, and the port-conflict path above).

## Manual acceptance — MCP server (0.3.0 PR8)

**0.3.0 PR8** adds `screensearch-mcp.exe`, a stdio MCP server wrapping the PR7 local API
(`docs/0.3.0.md` Part III, D13; `03 §7c`/`§13b.7`). Full config in `docs/MCP.md`. Run on a real
Windows desktop. First: `node scripts/stage-mcp.mjs` (once per clone — see "Before you push"), then
`npm run dev`, enable **Settings → Local API**, and copy the token.

- **Handshake + tool listing over stdio.** Save a session file `mcp-session.jsonl` with one request
  per line:
  ```
  {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"manual","version":"0"}}}
  {"jsonrpc":"2.0","method":"notifications/initialized"}
  {"jsonrpc":"2.0","id":2,"method":"tools/list"}
  {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"search_screen_history","arguments":{"query":"<a term you saw today>","limit":5}}}
  {"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_moment","arguments":{"frame_id":<id from #3>,"include_image":true}}}
  {"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"where_was_i","arguments":{}}}
  {"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"add_mark","arguments":{"note":"from MCP acceptance"}}}
  {"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"list_marks","arguments":{}}}
  {"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"ask_screen_history","arguments":{"query":"what did I read about <topic>?"}}}
  ```
  Run (PowerShell): `$env:SCREENSEARCH_API_TOKEN="<token>"; Get-Content mcp-session.jsonl | .\target\release\screensearch-mcp.exe`
  — expect eight in-order responses: `initialize` reports `serverInfo.name = "screensearch-mcp"`;
  `tools/list` lists the six tools; `#3` returns hits; `#4` carries a base64 **image** block; `#6`
  creates a mark that appears in the Deck **Intentions** strip (with the "Marked ✓" toast); `#8`
  returns the aggregated cited answer.
- **API-off error path is clean.** Toggle the API **off** in Settings and rerun the session: ids `1`–`2`
  still succeed, and every `tools/call` returns `isError: true` with text containing
  **"enable the API in ScreenSearch Settings"** — never a crash or a hang.
- **Wrong token.** With the API on but `$env:SCREENSEARCH_API_TOKEN="wrong"`, a tool call returns
  `isError: true` mentioning the **401** and to copy the current token from Settings.
- **A real client end-to-end.** `claude mcp add screensearch --env SCREENSEARCH_API_TOKEN=<token> -- "<abs path>\screensearch-mcp.exe"`,
  then in a Claude Code session: *"search my screen history for X"*, *"what was I doing before this?"* —
  the tool calls round-trip.
- **Installer includes the binary.** `npm run tauri build`, then
  `7z l "target\release\bundle\nsis\ScreenSearch_<ver>_x64-setup.exe" | findstr screensearch-mcp`
  lists `screensearch-mcp.exe`. Install silently (`ScreenSearch_<ver>_x64-setup.exe /S`), confirm
  `screensearch-mcp.exe` sits next to `ScreenSearch.exe` in the install dir and runs
  `screensearch-mcp.exe --version`; uninstalling removes it.

## Manual acceptance — polish bundle (0.3.1 PR3)

**0.3.1 PR3** ships three small, user-visible polish items (`docs/0.3.1.md` PR3; decisions
D1/D2/D3/D4). Run `npm run dev` (or a release build); all three are pure UI/command work,
no schema or settings change.

- **#59 — Moment recognized-text has no nested scrollbar (D1).** Open a **terminal-heavy** or
  otherwise long-text capture in the Timeline → Moment view. The **Recognized text** panel (and the
  **Raw text** disclosure) grow to their full height with the rest of the page; there is **no inner
  scrollbar** — the only scroll context is the main content area (`AppShell` `<main>`). Confirm the
  page scrolls as one column and text is never trapped in a 320 px box.

- **#65 — report filename + footer (D2/D3).** Recall → **Reports** → pick a range with captures →
  **Generate**. On the finished report:
  - The on-screen **footer** block states, in one line: `ScreenSearch v<version>` · the model id ·
    the covered date(s) · the filter (kind, plus `(focus: …)` for a Custom report with a prompt) ·
    `N passes` · `covered/total periods` · `summarized/sampled frames summarized` (and, if trimmed,
    the "range trimmed to fit" note).
  - **Download .md** saves to your **Downloads** folder as
    `screensearch-report-YYYY-MM-DD-HHmm.md` (local time); a success toast shows the full path.
    Click **Download .md** a second time **within the same minute** → the file is
    `…-HHmm-2.md` (then `-3`, …) — the first file is never overwritten.
  - Open the saved `.md`: the **same footer block** appears at the bottom (after a `---` rule), so
    the exported file is self-describing. **Copy** also carries the footer.
  - A **no-evidence** range (empty window) renders its honest message with **no footer** and no
    save-time footer.

- **#57 partial — NavRail version link (D4).** The bottom of the left NavRail shows a quiet
  `v<version>` line (mono, faint). It is **keyboard-focusable** (Tab to it, visible focus ring) and
  clicking or pressing Enter opens **https://github.com/nicolasestrem/screensearch-v2c** in your
  **default browser** (a new browser window/tab — *not* inside the app WebView). Confirm the app UI
  itself does not navigate away.

## Manual acceptance — auto-update (0.3.2 PR2, #69)

**0.3.2 PR2** wires `tauri-plugin-updater` against a **minisign-signed** `latest.json` manifest on
GitHub Releases (`03 §11b`; `docs/0.3.2.md` §3 PR2, D1/D2). The runbook below reproduces the whole
flow locally, including the load-bearing **signature-rejection** negative test. Nothing here is
committed as production config — all test material lives in a scratch dir.

**Version note.** The app's `--version` prints the compile-time crate version
(`env!("CARGO_PKG_VERSION")` = the workspace `Cargo.toml` version), while the updater compares the
`tauri.conf.json` `version`. In production these are hand-synced at release, so they always agree;
for a *test* build both must be set to the test version — the runbook edits the workspace
`Cargo.toml` version (reverted with `git checkout` afterward) **and** overrides the config version
with a `--config` overlay. A release build is `windows_subsystem = "windows"` (no attached console),
but **shell redirection still works**, so capture `--version` with
`cmd /c "…\ScreenSearch.exe --version > out.txt"` — do not "fix" it with `AttachConsole`.

**Prep (scratch dir; test key or the real key — the runbook below uses the real production key so
the artifact verifies against the pubkey baked into `tauri.conf.json`).**
1. Close any running ScreenSearch (dev *and* installed builds share the bundle identifier → the same
   single-instance mutex and the same app-data dir `%APPDATA%\app.screensearchv2c.desktop`). Back up
   `…\screensearch.db*` (zero schema changes this arc — D10 — but evidence runs deserve a net).
2. Overlays in the scratch dir (deep-merged over `tauri.conf.json`):
   - `e2e-new.json`: `{ "version": "0.3.2" }` — the update *target*.
   - `e2e-old.json`: `{ "version": "0.3.2-pre", "plugins": { "updater": { "endpoints":
     ["http://127.0.0.1:8765/latest.json"], "dangerousInsecureTransportProtocol": true } } }` — the
     installed build that will *receive* the update, pointed at a localhost manifest over http.
3. With `TAURI_SIGNING_PRIVATE_KEY` (+ `_PASSWORD` if the key has one) set from the offline backup:
   build the **new** target first (workspace `Cargo.toml` version → `0.3.2`):
   `npm run build -- --config <scratch>\e2e-new.json` → copy
   `ScreenSearch_0.3.2_x64-setup.exe` **and** its `.sig` into `<scratch>\serve\`.
   Then build the **old** receiver (workspace `Cargo.toml` version → `0.3.2-pre`):
   `npm run build -- --config <scratch>\e2e-old.json`. Revert the `Cargo.toml` edit afterward.
4. Manifest + server: pass `--expected-version 0.3.2` so the tag matches the overlay-stamped
   version (written when the committed `tauri.conf.json` was still `0.3.1`; adjust the versions to
   the current release when re-running. The flag is the documented test-only
   override — the release workflow never uses it, so its strict conf-drift guard is untouched):
   `node scripts/make-latest-json.mjs --tag v0.3.2 --expected-version 0.3.2 --bundle-dir <scratch>\serve
   --url-base http://127.0.0.1:8765/ --out <scratch>\serve\latest.json`, then serve it:
   `npx http-server <scratch>\serve -p 8765`.

**Negative test first (signature is load-bearing).**
5. Install the receiver: `ScreenSearch_0.3.2-pre_x64-setup.exe /S`.
   `cmd /c ""C:\Program Files\ScreenSearch\ScreenSearch.exe" --version > before.txt"` →
   `ScreenSearch 0.3.2-pre`.
6. **Corrupt** `serve\latest.json` — flip characters inside the `signature` string — and launch the
   app. Expected: the launch check finds `0.3.2`, then the **download is rejected at signature
   verification**. Evidence: the verbatim `WARN … update check/download failed` line in
   `%APPDATA%\app.screensearchv2c.desktop\logs\`; Settings → App shows the quiet
   **"Couldn't check for updates"** line + a retry, and there is **no modal and no toast**. The
   NavRail shows a transient dot during `available`/`downloading`, then nothing on `error`.
   (Variant: sign the artifact with a *different* key instead of corrupting — same rejection.)

**Positive path.**
7. Restore the good `latest.json`. Launch (or Settings → App → **Check for updates**). Expected log
   lines: `update available; starting background download` → `update downloaded + signature-verified`
   (with the byte count). The NavRail dot appears and Settings → App reads
   **"v0.3.2 available — restart to update."**
8. Click **Restart to update**. The app runs its graceful shutdown (capture stopped, sidecar torn
   down via the Job Object — no orphaned `llama-server`) and hands off to the **passive** NSIS
   installer, which upgrades in place and the app exits.
   `cmd /c ""C:\Program Files\ScreenSearch\ScreenSearch.exe" --version > after.txt"` →
   `ScreenSearch 0.3.2`. Relaunch → the check now returns nothing newer → **zero updater UI
   presence** (the NavRail shows only Command + version; Settings → App shows "No update available").

**Cleanup.** Uninstall the test build, reinstall a build of the current tree (so the machine isn't
left claiming a version that isn't released), and restore the DB backup if desired.

**Release-time reminder (endpoint reachability).** The updater endpoint is
`releases/latest/download/latest.json`, which GitHub resolves **only for a published,
non-prerelease release**. Every historical release (v0.1.0..v0.3.1) was a *prerelease*; from v0.3.2
on, releases must be **published as full releases** or installed copies never see the update.

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

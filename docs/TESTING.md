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
npm run tauri dev
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
`npm run tauri dev`. **0.3.0 PR2** trimmed the six triggers to **foreground + idle** (clipboard,
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
`npm run tauri dev`.

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
(`docs/0.3.0.md` PR4, D5/D15). Quick live check on a real Windows desktop with `npm run tauri dev`,
against a **backed-up copy** of a populated 0.2.x/PR3 profile (`<app-data>\screensearch.db`).

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
real Windows desktop with `npm run tauri dev`, capture enabled, and at least a few searchable frames
in the profile.

- **Default hotkey and keyboard loop.** Focus an unrelated app, press `Ctrl+Alt+Space`: the overlay
  appears over that app, the input is focused, and the taskbar does not gain a second ScreenSearch
  entry. Type a query, use `ArrowDown`/`ArrowUp` to move the active row, press `Enter`: the overlay
  hides and the main window opens the selected Moment. Reopen, press `Tab`: Search/Ask mode toggles.
  Reopen, type `? what was I reading`, press `Enter`: Ask streams in the overlay. Press `Esc`: it
  hides and any in-flight Ask stream is cancelled.
- **Settings hotkey controls.** Settings -> Hotkeys -> Flow overlay records a modifier chord, ignores
  pure modifiers, saves through `set_settings`, and `get_hotkey_status` reports the active chord as
  registered. Reset restores `Ctrl+Alt+Space`. Settings -> Overlay results clamps direct entry to
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
real Windows desktop with `npm run tauri dev`, capture enabled, and a few minutes of history.

- **Where-was-i offers the work context (the core flow).** Spend ≥ 2 minutes (the default
  `resume.min_dwell_secs = 120`) in one app (e.g. VS Code), then switch to a browser for a short detour
  and stay there. Press the overlay hotkey (`Ctrl+Alt+Space`) with the query empty: the strip reads
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
`03 §7c`). Run on a real Windows desktop with `npm run tauri dev`, capture enabled, and a few minutes
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
  (`screensearch-export-<stamp>.json`). Validate it: `jq . "%USERPROFILE%\Downloads\screensearch-export-*.json"`
  parses, with `schema:"screensearch.export.v1"`, a `frames` array (metadata + `content_text`, **no**
  image bytes), and a `marks` array.
- **Exit frees the port.** With the API listening, quit the app. `netstat -ano | findstr 43210` shows
  no LISTENING row afterward (the server is stopped on exit; an in-flight `curl -N` ask does not wedge
  the quit).
- **Five panel states.** Exercise the Local API panel's loading / load-error / off / enabling /
  listening paths (toggle, retry, and the port-conflict path above).

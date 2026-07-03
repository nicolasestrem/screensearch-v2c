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

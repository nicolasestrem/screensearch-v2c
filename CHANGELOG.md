# Changelog

All notable changes to ScreenSearch V2c are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Detailed AI build records live in `specs/08_CHANGELOG_AI.md`; this file is the
> human-facing summary.

## [Unreleased]

### Added — Local HTTP API + export (0.3.0 PR7)
ScreenSearch can now serve your screen history to local scripts and agents through an opt-in **local
HTTP API** — the open-source ask since Rewind shut down. It is **off by default**. When you enable it
in Settings it binds `127.0.0.1` only (never your network — that is hard-coded, not a setting) and
mints a **bearer token**: every request must carry it, and anything without it — or with the wrong
one — gets a `401`. The token is shown in Settings to reveal, copy, and regenerate (regenerating takes
effect immediately, no restart). The threat model is stated plainly next to the switch: any local
process holding the token can read your entire screen history, so enabling it is an explicit trust
decision. If the port (default `43210`) is already in use, enabling fails **loudly** — a warning, and
an inline "pick another port" retry — never a silent no-op.

The v1 API mirrors the app: `GET /v1/health`, `GET /v1/search`, `POST /v1/ask` (a streamed, grounded
answer over Server-Sent Events — and disconnecting the client now actually stops the model generating,
freeing the GPU), `GET /v1/frames/{id}` (with `?image=1` for the screenshot),
`GET /v1/context/where-was-i`, and marks (`GET`/`POST` and resolve — the only write surface). Full
docs, with copy-paste examples, are in `docs/API.md`.

A new **Export…** button in Settings writes your frames, content text, and marks to a JSON file in your
Downloads folder (screenshots are not included). It streams to disk, so exporting months of history
stays memory-flat, and it shares the API's export code path — so it works even with the API disabled.
There is no schema change in this release. *Live-verified on Windows: off by default (nothing listens),
the 401 posture, the loud port-conflict retry, token regeneration without a restart, SSE cancellation
on disconnect, and a valid export produced with the API off.*

Review hardening (PR #76): malformed requests (a bad query string, body, or path segment) — and now
unknown endpoint paths and an inverted `from > to` window — return the same `{ "error", "message" }`
error as every other response instead of a framework plaintext body; the port can be changed while the
API is running via a "Restart on {port}" button (previously the edited port was inert); the port field
clamps to `1024–65535`; a failed export never leaves a `.partial` file behind (fixed for Windows, where
the open file handle blocked cleanup); a screenshot missing on disk returns a clean `404` rather than a
`500`; overlapping enable/disable calls are serialized so a race can't leave the API running against a
disabled intent; and a client that disconnects mid-answer now stops the stream immediately instead of
draining the model's backlog first.

### Added — "Where was I?" + Mark this moment (0.3.0 PR6)
Two pull-based recall features for picking work back up. **Where was I?** answers "what was I doing
before this detour?" — open the Flow overlay with an empty query (or look at the Deck) and, when it can,
it offers the last context you stayed in for a while before your current detour: the app, the window,
and when you were last there, one click from reopening that Moment. It never nags — if there's nothing
worth resuming, it says so honestly.

**Mark this moment** captures the current screen the instant you press `Ctrl+Alt+M` — even on a static
screen that hasn't changed — and flags it as an intention to come back to. A quiet toast confirms
("Marked ✓") **without stealing focus**, so you keep typing; click it to add an optional one-line note.
Your open intentions live in a new **Intentions** strip on the Deck (newest first, with a thumbnail or
text reconstruction and its age) where you can open, resolve, or dismiss them. There are no badge
counts anywhere — nothing to shame you into acting.

Both hotkeys are configurable in Settings alongside the overlay hotkey, with the same loud
registration-conflict warning. The where-was-i sensitivity ("how long a context must persist to count")
is a setting too. If capture is off, the mark hotkey tells you plainly instead of silently doing
nothing, and a mark taken in an excluded app is refused with a reason. Existing databases migrate
forward (schema v9 → v10) by adding a `marks` table; nothing else is touched, and a marked frame keeps
its text reconstruction reachable even after its screenshot expires under retention.

### Added — Flow overlay (0.3.0 PR5)
ScreenSearch now has a global-hotkey **Flow overlay** for quick recall without leaving the app you
are working in. Press `Ctrl+Alt+Space` to open a second, pre-created Tauri window over the foreground
monitor, type to search, press `Tab` or prefix the query with `?` to switch into Ask, and press
`Enter` to open the selected Moment in the main Command Deck. `Esc` hides the overlay.

The overlay is treated as a privacy boundary: it is hidden by default, capture-protected at the
window level, skipped from the taskbar, and covered by the same own-process capture gate used for the
main window so it does not appear in ScreenSearch's own history. Settings exposes the summon hotkey
and top-N result count (`overlay.max_results`, clamped to `1..=50`). A hotkey registration conflict
is loud: the app emits a warning toast and Settings shows the failed chord instead of silently doing
nothing. Exclusive-fullscreen applications may still suppress global overlays; that limitation is
documented in the manual acceptance checklist.

### Removed — Image-embedding lane (0.3.0 PR4)
ScreenSearch no longer has an optional **image-embedding** lane. It was dark-launched and off by
default (the `enrich.image_embeddings` "Embed images" toggle), so almost nobody turned it on — and a
flag-off feature is pure carrying cost: a second on-disk vector table, a second model download
(nomic-embed-vision-v1.5), and code that quietly rots because nothing exercises it. Text embeddings
plus vision tags already cover semantic reach, so the lane is **gone**. (Git remembers it if it is
ever wanted back.)

Your data is safe. On first launch after this update the database migrates itself forward (schema
v8 → v9) and **drops only the derived image vectors** — your screenshots, recognised text, and text
embeddings are never touched, and the image vectors were re-derivable from stored frames in any case.
Any queued image-embedding jobs are cleared. If your settings still held the **Embed images** toggle,
it is dropped cleanly on the next launch (logged once, no error), and the Settings → Enrichment panel
now shows just **Embed OCR text**. Search behaviour is unchanged — the image lane was never fused into
hybrid search, verified by identical result rankings across a 10,000-frame fixture before and after.
Live-verified on a real desktop: a populated profile migrates 8 → 9 on boot (image tables and the
`embed_image` jobs gone, frames and text embeddings intact), the retired key drops once, and text
semantic search still returns hits after the migration.

### Removed — Beta model tier retired; Default / Quality only (0.3.0 PR3)
Each model lane (Vision and Answer) now offers **two** tiers instead of three: **Default** and
**Quality**. The **Beta** tier is **gone** on both lanes, which removes the two models behind it —
the vision `Qwen3.5-9B-VLM` and the answer `NVIDIA-Nemotron-3-Nano-4B`. Nemotron was the only
non-Apache-licensed model (NVIDIA OML) and the only unproven hybrid Mamba-Transformer architecture
in the set, so the remaining line-up is **uniformly Apache-2.0 and vanilla-arch** — a smaller,
better-tested matrix and a cleaner licensing story.

If you had **Beta** selected for a lane, it now loads as **Quality** automatically — mapped once on
the next launch, logged, and saved (no error, no reset to Default). Any Beta model files already
downloaded are **left on disk**; you can remove them from the existing model-management surface if
you want the space back. Live-verified on a real desktop: a profile persisted with `beta` for both
lanes boots straight to Quality (one log line per lane, none on relaunch), the change is persisted
to the database, and the app runs on the remapped tiers with inference attached.

### Removed — Event-capture triggers trimmed to foreground + idle (0.3.0 PR2)
Opt-in event-driven capture now offers **two** triggers instead of six: **foreground/app-switch**
and **idle**. Four triggers are **gone**:
- **Capture on click** and **Capture when scrolling stops** — these were the only users of a
  *system-wide low-level mouse hook* (`WH_MOUSE_LL`), the one input-latency/invasiveness risk the
  0.2.0 design had deliberately avoided; removing them deletes that hook and a whole `unsafe` code
  path.
- **Capture on clipboard change** — a clipboard listener is a privacy-optics liability in an app
  built for the privacy-conscious, and app-switch already fires at nearly the same moments (people
  switch windows around copy/paste). Contents were never read; now nothing listens at all.
- **Capture on typing pause** — redundant with **idle** (both derive from the same idle-time poll,
  differing only in threshold).

Everything that carried the value stays: foreground, idle, the fallback interval, the debounce, and
the min-interval rate ceiling. Event mode remains **opt-in, default off**; timer capture is
unchanged. **No database change** — screenshots you captured before this update that were tagged
Click / Scroll stop / Clipboard change / Typing pause **still show that label** in the Moment view;
new captures simply never use those tags again. If your settings still held the removed toggles, they
are **dropped cleanly on the next launch** (logged once, no error). Live-verified: the retired keys
drop on load (one log line, none on relaunch); the app boots clean; the foreground hook thread
starts/stops cleanly 50× on a real desktop; the v8 schema still accepts a legacy `click` frame.

### Docs — 0.3.0 arc specs contract (PR1, specs-only; no code / schema / UI)
The 0.3.0 roadmap (`docs/0.3.0.md`) is normalized into the specs so every later PR (PR2–PR9) is
implementable from the specs alone, without reopening the roadmap. **This change touches only specs
and docs.** The contract now locks in the arc's **subtractions** — the six event-capture triggers
trim to foreground + idle (retiring the global `WH_MOUSE_LL` mouse hook, the clipboard listener, and
typing-pause), the model **Beta** tier is retired (Default/Quality only, uniformly Apache — Nemotron
and Qwen3.5-9B-VLM leave the registry), and the flag-off image-embedding lane is removed (with a
forward-only migration that drops only derived vectors) — and its **additions**: a global-hotkey
**Flow overlay** for instant recall, a **where-was-i + mark-this-moment** workflow, and an **opt-in
localhost HTTP API + export** with a thin stdio **MCP** wrapper. Deferrals (audio capture, custom
GGUF, proactive nudges, marks-in-reports, wider API write scopes) are recorded in known-gaps.
Automated PR review then tightened the contract (still specs-only): the "where was I" resume picks up
the app you were actually working in — not the overlay you just opened or a two-second detour;
"mark this moment" is pinned to the screen you're looking at on a multi-monitor setup; the local API
stops generating an answer the instant a caller disconnects and streams exports so a months-long
history can't exhaust memory; and the marks list has one defined order (open items first, newest
first).

### Changed — Faster UIA text capture on heavy windows (gap #71)
Reading a foreground window's accessibility text now fetches each element's properties in a **single
batched call** instead of ~5 separate cross-process calls, cutting the per-walk overhead ~2.5× on the
heavy Chromium/Electron windows the capture targets — while keeping the same small, interruptible steps
that bound the walk to its latency budget (two bulkier single-fetch designs were tried and dropped for
overrunning the budget on very large windows). Editable-field **values** are deliberately kept off the
batch and read only after the password/offscreen privacy guard passes, so a masked or hidden field's
text is never fetched. Live-verified capturing real Chrome pages. The earlier 0.2.1 hang mitigation is
unchanged.

### Changed — Expired screenshots now shrink the database too, not just disk (gap #73a)
When a screenshot expires under retention, the app keeps a text + layout reconstruction of it. That
reconstruction is drawn from per-word text positions, which are the biggest remaining database cost
for old frames. Rather than delete them (which would blank out the reconstruction), the app now
**merges each expired frame's words into per-line entries** — reclaiming roughly 80% of those rows
while the reconstruction still reads correctly at the line level. Merging happens as frames expire,
and a one-time pass on first launch cleans up frames that expired before this shipped. Search is
unaffected. (Full-text and semantic search read separate stored text/vectors, not these positions.)
Following PR review, an expired frame is now retired in a single atomic step, so a momentary
database hiccup can never leave a frame half-degraded (marked expired but still carrying its
per-word rows); if it fails, the frame is simply retried on the next sweep.

### Fixed — Semantic search could miss in-range results on tight time windows (gap #8)
When a search combined a query with a narrow time filter, the vector (meaning-based) arm fetched a
fixed pool of nearest matches and *then* dropped those outside the window — so an in-window match that
happened to rank just past the pool was silently missed. The vector arm now **widens its search
adaptively** when a time range is set: it re-runs with a larger nearest-neighbour count until the pool
fills with in-window matches or the index is exhausted. Unfiltered searches are unchanged. (sqlite-vec
can't filter inside its nearest-neighbour query, so widening the pool is the fix.) Search latency stays
well within budget (10k-frame fixture p95 ~66 ms, bar is 200 ms).
Following review (PR #66, P2): the widening now stops as soon as it has gathered every embedded frame
the window actually **holds**, bounded by a cheap, index-served count of in-window embedded frames.
Without that cap, a *sparse* window — one with fewer captures than the pool — on a database larger than
the widening ceiling would have run the maximum 20 000-neighbour pass on every query even after already
finding all its matches. An empty window now skips the vector query entirely.
Following the follow-up review (PR #66, P2): that count itself is now hard-bounded. A `LIMIT` on
*matches* can't stop early when a window has many captured frames but few embedded ones (an embedding
backlog, or a wide multi-day range), so the count would have walked the whole frame range — O(window),
not O(pool). The count now examines at most a fixed budget of frames; a window too large to prove
sparse within that budget falls back to assuming it is dense (widen up to the pool), which can only
*raise* the target and never drops an in-window match. The count is now O(pool) even on a wide,
sparsely-embedded window.

### Changed — Packaging spec re-scoped to NSIS (Inno / portable ZIP / MSI dropped)
ScreenSearch ships an unsigned **NSIS** installer (Tauri 2 native, since v0.1.0). The specs had still
called for an "Inno Setup installer + portable ZIP"; all nine live references (project intake,
context, plan, master-spec DoD §13.9, architecture doc, CI note, README) are rewritten to NSIS, and
DoD §13.9 is re-scoped to "NSIS installer builds successfully" and met. **Code-signing** is now the
lone remaining packaging follow-up (known-gap #26 closed).

### Fixed — Second launch restores a hidden window
The single-instance handler now shows the existing window before unminimizing and focusing it, so a
hidden / tray-minimized window is properly restored (not just unminimized) on a second launch.

### Accessibility — Keyboard & focus pass (gap #42)
- **Navigation rail** is a single Tab stop with a roving tabindex: Arrow Up/Down (wrapping) and
  Home/End move focus between the five destinations, and the active route carries `aria-current="page"`.
- **Command palette** returns focus to the control that opened it when it closes.
- **Recall → Ask** moves focus to the answer once streaming finishes, so keyboard and screen-reader
  users land on the result.
- **Settings** exposes each section's controls as a labelled group for assistive technology.

### Fixed — Model-downloader resume hardening (two corruption/waste edge cases)
Two narrow but real durability gaps in the parallel chunked model downloader
(`crates/inference/src/download.rs`), neither previously test-covered:
- **A truncated `.part` no longer publishes zeros (gap #69).** The resume bitmap was only discarded
  when the downloader *created* a brand-new `.part`. If an external cleanup tool truncated an
  existing `.part` without deleting it, `set_len` re-grew the file with zeros while a header-matching
  bitmap still marked those (now-zero) chunks "done" — so the zero-filled ranges were published (the
  length check passes; sha256 is skipped when the CDN advertises no `X-Linked-ETag`).
  `open_preallocated` now reports a part as untrustworthy whenever its on-disk length is **not
  exactly** `total` (brand-new, truncated, *or* corruption-grown larger), forcing a full refetch.
  Safe for normal resumes: a legitimate interrupted `.part` is always preallocated to exactly
  `total`, so it is never falsely discarded.
- **A locked download re-checks the cache before re-downloading (PR #27 Codex-P2).** When another
  live downloader holds hf-hub's per-blob advisory lock, the single-stream fallback backs off and
  retries; it now re-checks the clean-layout and HF-cache fast paths **after each backoff**, so if
  the holder finishes during the sleep the loser copies the finished blob into place instead of
  re-downloading it or colliding on publish. The bounded backoff was extended (per-attempt cap + more
  attempts → ~5 min total) so a real multi-GB download by the holder is outlasted rather than
  abandoned after ~20 s, while the cache re-check exits the instant the holder is done.

### Fixed — Full-resolution frames overflowed the vision context (tagging failed + slow)
Native full-resolution captures (e.g. a 3440×1440 ultra-wide frame → ~4148 vision tokens) exceeded
the vision lane's pinned **4096**-token context, so `llama-server` rejected every tag with HTTP 400
`exceed_context_size_error`; jobs retried 3× and dead-lettered (DB showed `vision_tag` 72 dead / 0
done). The deceptively low RAM/VRAM that looked like a "memory miracle" was the model **rejecting
requests in ~0.1 s without running inference** — the GPU sat near-idle while the weights stayed
resident (~6.4 GB VRAM). Fix is two-part, plus the diagnosability gaps that hid it:
- **Downscale the VLM request image** to a 1568 px longest edge before JPEG-encoding
  (`crates/inference/src/vision.rs`). Captures/timeline keep full resolution — only the tag request
  shrinks — so it fixes the overflow *and* cuts prefill time (addresses the slowdown). The 1568 px
  cap bounds even a worst-case square frame to ~2.5 K prompt tokens, comfortably under the
  spec-contracted **4096**-token vision default — so the auto-context is left unchanged (per PR #61
  review: bumping it would have contradicted `03 §8` "not bumped by default" and raised KV-cache
  VRAM on weak GPUs for headroom the downscale makes unnecessary).
- **Record the real cause**: `vision_tag` failures now log the full anyhow chain (`{e:#}`) into
  `jobs.last_error` (`crates/kernel/src/worker_pool.rs`) instead of the collapsed `"vision
  completion"`, and the sidecar's stdout/stderr are captured to `<sidecar dir>/llama-server.log`
  (`crates/inference/src/process.rs`) — previously the sidecar's own error was discarded.

### Docs — Restore the archive-on-release convention across the build-loop logs
Brought the lean-logs convention current. v0.1.0 history was archived on 2026-06-27, but the shipped
**0.2.0, 0.2.1, and 0.2.2** history had piled back up in the live logs. Moved it out **verbatim**
(byte-identical to `git HEAD`; original `#N` ids preserved so cross-references stay valid):
- `specs/07_KNOWN_GAPS.md` — 18 resolved 0.2.x rows + 5 accepted-as-is rows + the ~85-line
  resolved-engineering-decisions list → `specs/archive/07_KNOWN_GAPS.v0.2.x.md`. The live table now
  holds only still-open + current-arc rows (35 → 12).
- `specs/05_BUILD_REVIEW.md`, `specs/08_CHANGELOG_AI.md`, `specs/06_PATCH_PLAN.md` — shipped 0.2.x
  entries → matching `*.v0.2.x.md` archives (06 keeps its one open upstream-leak row #15).
- This file — the 0.2.0/0.2.1/0.2.2 sections (which had accumulated under `[Unreleased]`) →
  `CHANGELOG-ARCHIVE.md` as proper versioned sections.

### Added — Deterministic dev-only route-state triggers (closes `07` #43)
A dev-only `?__devState=loading|error` URL param forces any P5 route into its loading or error state
for audit verification, applied centrally at the TanStack Query seam and **stripped from production
builds** (`import.meta.env.DEV`). Empty/partial/populated stay driven by real data — no mocks in the
production path. Documented in `docs/DEV_STATE_OVERRIDE.md`. Per PR #60 review, the helper reads
`window.location.search` directly instead of `useSearchParams()`, so it calls no React hook: the
production path truly folds to `return result` (no router-history subscription on the 17 query
consumers, no `<Router>`-context coupling) rather than merely stripping the `__devState` string.

### Added — Privacy-safe VLM request logging (closes `07` #44)
The kernel now logs `frame_id` + the relative capture path at `info` immediately before each vision
request, so the VLM image input is visible in `screensearch.log` (previously a log scan found nothing).
No screen content, OCR text, or image bytes are logged.

## Older versions

Releases 0.2.2 and earlier are archived in [CHANGELOG-ARCHIVE.md](./CHANGELOG-ARCHIVE.md).

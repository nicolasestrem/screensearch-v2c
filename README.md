# ScreenSearch V2c

A local-first **Windows** desktop app that continuously captures your screen, makes it
searchable by **text and meaning**, and answers questions about what you've seen — fully
on-device, no cloud.

> **Status: v0.4.0 shipped (the sessions arc).** Capture → OCR/UIA text → deferred enrichment →
> **hybrid search**, the **llama.cpp inference sidecar** (vision tagging + grounded streaming `ask`),
> the full **Command-Deck UI**, and the global-hotkey **Flow overlay** all run on the live app.
> The shipped 0.2.x arc added attention-first text filtering, Recall reports, opt-in event-driven
> capture, and a smart enrichment throttle; the 0.3.0 arc trimmed invasive surfaces (event
> triggers, Beta tier, image embeddings) and added flow recall — where-was-i + marks — plus an
> opt-in **local HTTP API** and the bundled **`screensearch-mcp` MCP server**; the 0.3.1 patch
> restored vision-tagging throughput to the pre-WebP baseline (#64) and added recall polish.
> The 0.3.2 arc gave the app its product shell: **auto-update** (signed manifest on GitHub
> Releases, background download, install only on your restart — v0.3.2 is the last manual
> download), a **system tray** with a passive capture-state icon and quick actions
> (close-to-tray on by default, run-at-startup off by default), a hardened one-scroll-context
> layout, and a **two-tier Settings** page. The **0.3.3** hotfix (auto-delivered by the 0.3.2
> updater — the first automatic release) skips Chromium/Electron windows in the UIA text source to
> stop browser freezes. The **0.4.0 sessions arc** (shipped) groups frames into sessions
> additively (zero frame-level behavior change) via a pure heuristic engine with no model calls,
> behind its one schema migration (v10 → v11); sessions are reachable in the Timeline and the
> read-only local API and MCP, and v0.4.0 is the first release auto-delivered to 0.3.2+ installs.
> The **NSIS installer** is unsigned (SmartScreen will
> warn — "More info → Run anyway"); **Authenticode code-signing** is the lone remaining packaging
> follow-up (the updater's minisign signature is separate and already live). Design lives
> in [`specs/`](./specs); the as-built architecture is in
> [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md). A standalone, clean-slate project — no shared
> code or data with any prior version.

## Screenshots

The **Command Deck** is six on-device screens over your screen history — Deck, Recall (grounded
**Ask** with cited frames), Insights (activity analytics), Moment (frame detail + on-demand vision
tagging), Timeline, and Settings. Nothing here touches the network: every frame, query, and answer
stays on the machine.

![Deck — capture status, today's activity, where to jump back in](screenshots/deck.png)

> **Deck** — the at-a-glance home: capture status, today's activity and top apps, where-was-I resume,
> intentions (marks), and recent captures.

![Timeline — a scanline of the day with session bands](screenshots/timeline.png)

> **Timeline** — scrub a day / week / month of captures on a scanline, with additive **session bands**
> (focus, meeting, and concurrent AI-tool sessions) layered over the density. `Enter` opens the Moment.

![Recall — hybrid search over screen history with highlighted matches](screenshots/recall.png)

> **Recall** — hybrid text + semantic search over everything on screen (grounded **Ask** and recall
> **Reports** share the screen). Matches are highlighted and link straight to the captured frame.

![Insights — capture density, top apps, and activity breakdown](screenshots/insights.png)

> **Insights** — truthful aggregates over a range: captures over time, top foreground apps, and the
> activity-type breakdown.

![Moment — one captured frame with context, recognized text, and vision tags](screenshots/moment.png)

> **Moment** — a single capture in full: the image, its session, recognized text, vision tags, and the
> neighbouring captures.

> **Note:** every screenshot above is rendered against **synthetic seed data** — invented frames,
> sessions, and text with no personal content (see [`docs/SCREENSHOTS.md`](docs/SCREENSHOTS.md)).

## Build progress

| Phase | Scope | Status |
|---|---|---|
| **P0** | Scaffold — Cargo workspace, `traits` contracts, Tauri 2 shell, React/TS UI, `ts-rs` IPC, CI, `doctor` | ✅ Complete |
| **P1** | Data spine — SQLite (WAL) + FTS5 + sqlite-vec, forward-only migrations, durable job queue, hybrid search | ✅ Complete |
| **P2** | Capture happy path — WGC capture + diff/privacy gates, WinRT OCR, kernel event bus, minimal live timeline | ✅ Complete |
| **P3** | Deferred enrichment — fastembed embedding worker pool, vector arm live, `search` command, perf-verified | ✅ Complete |
| **P4** | Inference sidecar — llama.cpp (Job-Object-bound, no-orphan), vision tagging, grounded streaming `ask` | ✅ Complete |
| **P5** | Command-Deck UI (Deck, Recall, Timeline, Moment, Insights, Settings) + typed IPC | 🚧 Feature-complete; live-verified (full keyboard/state/a11y matrix pending) |
| **Pkg** | Unsigned **NSIS** installer shipped (v0.1.0); Inno/MSI/portable ZIP dropped; `onnxruntime.dll` static-linked (not bundled); **code-signing** the lone follow-up (DoD §13.9, `07` #26) | 🚧 Signing pending |

The **0.2.x arc** builds on that v1.0 base — an attention-first text signal plus recall and
capture refinements:

| Feature | What it adds | Status |
|---|---|---|
| **Attention-first text** | Span-aware classifier so search/Ask/embeddings rank on content, not chrome (raw text still opt-in) | ✅ Shipped |
| **Recall reports** | On-device Daily / Weekly / Custom summaries that cite the frames they used | ✅ Shipped |
| **Event-driven capture** | Opt-in triggers — foreground + idle (timer stays the default) | ✅ Shipped |
| **UIA text source** | Foreground-window text via UI Automation, with automatic OCR fallback | ✅ Shipped |
| **Smart enrichment throttle** | Opt-in CPU/GPU backpressure that eases off background work under load — capture/OCR/storage never pause | ✅ Shipped |

The **0.3.0 arc** (shipped) was the surface-reduction + flow-recall pass:

| Feature | What it changes | Status |
|---|---|---|
| **Surface reduction** | Removes click/scroll/clipboard/typing triggers, the Beta model tier, and the unused image-embedding lane | ✅ Shipped |
| **Flow overlay** | `Ctrl+Alt+Z` opens a protected always-on-top Search/Ask overlay over your current app | ✅ Shipped |
| **Where-was-i + marks** | Resume context (`where_was_i`) and mark-this-moment (`Ctrl+Alt+M`, diff-gate-bypassing `capture_now`) | ✅ Shipped |
| **Local API + MCP wrapper** | Opt-in localhost API (127.0.0.1 + bearer token), JSON export, and the `screensearch-mcp` stdio wrapper | ✅ Shipped |

The **0.3.2 arc** (shipped) is the product-shell pass — lifecycle + interface:

| Feature | What it adds | Status |
|---|---|---|
| **Auto-update** | Signed `latest.json` on GitHub Releases; check on launch + manual check; background download; install only on user-initiated restart — no modal, no nag | ✅ Shipped (v0.3.2 is the updater's genesis — the last manual download) |
| **System tray + quick actions** | Passive capture-state icon (capturing / paused / error), six-item menu (open, pause/resume, load/unload model, start/stop vision, check for updates, quit); close-to-tray default on; run-at-startup default off | ✅ Shipped |
| **Shell layout hardening** | One scroll context per route, no nested scrollbars, no layout shift on load; WebView2 ghost-rail mitigation | ✅ Shipped |
| **Settings two-tier IA** | Essentials always visible; Advanced collapsed into seven groups; live hotkey-conflict warning; two dead settings retired (old configs still load) | ✅ Shipped |

The **0.3.3 hotfix** (shipped, first auto-delivered release): the UIA text source skips
Chromium/Electron windows to stop browser freezes.

The **0.4.0 sessions arc** (shipped as `v0.4.0`, 2026-07-11) reframes frames into **sessions**,
additively, with no frame-level behavior change:

| Feature | What it adds | Status |
|---|---|---|
| **Sessions schema** | Migration v10 → v11: `sessions` + `session_artifacts` tables and a nullable `frames.session_id`, structure-only (no backfill) | ✅ Shipped (PR3) |
| **Segmentation engine** | Pure heuristic `crates/sessions` — no model calls; per-identity-track segmentation + tool recognition from a seed taxonomy | ✅ Shipped (PR4) |
| **Sessions in the UI** | Typed session commands and a sessions surface (pull-based, non-shaming; no new NavRail route) | ✅ Shipped (PR5) |
| **Sessions API / MCP** | `list_sessions` / `get_session` / `ask_session` over the local API and MCP wrapper | ✅ Shipped (PR6) |

> Detailed point-in-time PR audits live as local-only artifacts under `docs/audits/` (git-ignored,
> not pushed).

### Working today
Start capture → each changed frame's text is read (foreground-window **UIA**, falling back to native
**WinRT OCR**), stored, and archived as lossless WebP → an attention-first filter keeps content text over chrome
→ an `embed_text` job is enqueued → a background worker pool embeds it with **fastembed**
(EmbeddingGemma-300M, 768-dim) → **hybrid search** (FTS5 keyword + sqlite-vec semantic, fused with
Reciprocal Rank Fusion) returns the right frames in **~33 ms p95 on a 10 000-frame database**.
Capture runs on a timer by default, with **opt-in event-driven triggers** (foreground + idle).
**Vision tagging** (on-demand / timer / idle — structured
output with an honest confidence, never a fabricated score), **grounded streaming answers** with
citations, and **Recall reports** (Daily / Weekly / Custom, citing their source frames) run on the
local **llama.cpp sidecar**; the full Command-Deck UI surfaces all of it. An optional **enrichment
throttle** eases off background work under sustained CPU/GPU pressure without ever pausing capture,
OCR, or storage. Retention purges run at startup and hourly when enabled, and the StatusRail shows
real DB/frame storage usage. `Ctrl+Alt+Z` opens the **Flow overlay**: a second, capture-protected
Tauri window for quick Search/Ask without leaving the foreground app; `Esc` hides it and `Enter`
opens the selected Moment in the main Command Deck. An **opt-in local HTTP API** (off by default,
`127.0.0.1` + bearer token) exposes search/ask/frames/marks to local scripts and agents, and
`screensearch-mcp.exe` — bundled in the installer — wraps it as a stdio **MCP server** for Claude
Desktop / Claude Code (`docs/API.md`, `docs/MCP.md`). The app lives in the **system tray**
(closing the window keeps capture running by default — a one-time toast explains it), and
**updates itself** from v0.3.2 on: a signed manifest is checked at launch, the new installer
downloads in the background, and it installs only when you choose to restart.

## What it does (v1.0 target)

- **Always-on, cheap capture** — screen capture (Windows.Graphics.Capture) + native WinRT OCR,
  written straight to a local SQLite store. *(P2 — done)*
- **Deferred, user-controlled enrichment** — embeddings run as durable jobs in a SQLite-backed
  queue, drained by a background worker pool; vision tagging is **on-demand / timed / idle** only.
  *(embeddings P3 — done; vision P4 — done)*
- **Hybrid search** — FTS5 keyword + vector (sqlite-vec) semantic, fused with Reciprocal Rank
  Fusion. *(P3 — done)*
- **Grounded, reasoning answers** — RAG over your screen history via a local llama.cpp model with
  a *thinking* mode. *(P4 — done)*
- **Fast recall overlay** — a protected global-hotkey window for Search and Ask over the current
  working context. *(0.3.0 — done)*

## Architecture (summary)

- **Shell:** Tauri 2 + WebView2; React 18 + TypeScript UI; typed IPC via `ts-rs`; a main Command
  Deck window plus a pre-created protected Flow overlay summoned by a global shortcut.
- **Core:** a modular Rust **kernel** — trait-bounded modules over a typed event bus; `src-tauri`
  is the composition root that wires concrete impls in.
- **Processing:** *capture-cheap, enrich-deferred* — a durable SQLite **job queue** drained by a
  bounded worker pool (with retry/backoff, dead-lettering, and stale-job recovery). An optional
  CPU/GPU **pressure throttle** reduces background enrichment under load; capture/OCR/storage never pause.
- **Text source:** foreground-window text via **UI Automation** with automatic fallback to native
  **WinRT OCR**, then an attention-first filter that keeps content over chrome.
- **Data:** SQLite (WAL) + FTS5 + sqlite-vec (768-dim, cosine); forward-only migrations.
- **Embeddings:** **fastembed** (in-process ONNX) — EmbeddingGemma-300M text.
  **No Python in the runtime.**
- **Inference (P4):** a single supervised, model-agnostic **llama.cpp sidecar** (Vulkan GPU + CPU
  fallback), **bound to the app via a Windows Job Object** so it can never orphan after a crash;
  advanced users can list/select llama.cpp devices when the default Vulkan device is wrong.

See [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) for the as-built design and data flow.

### Models (user-selectable, 2 tiers per lane)

| Lane | Default | Quality |
|---|---|---|
| **Vision** (P4) | Qwen3-VL-4B-Instruct | Qwen3-VL-8B-Instruct |
| **Answer** (P4) | Ministral-3-3B-Reasoning-2512 | Qwen3-4B-Thinking-2507 |
| **Embeddings** | EmbeddingGemma-300M (text) | |

Exact HF repos / quants are pinned in [`specs/MODEL_REGISTRY.md`](./specs/MODEL_REGISTRY.md).
Embedding models auto-download on first use into `<app-data>/models/fastembed`.

## Repository layout

```
CLAUDE.md          agent entry point (Claude Code) — mandatory reading order + hard rules
AGENTS.md          agent entry point (Codex) — same contract, Codex-flavored
README.md          this file
CHANGELOG.md       human-facing changelog (Keep a Changelog)
Cargo.toml         Cargo workspace (centralized dependency versions)
docs/
  ARCHITECTURE.md          as-built system design + data flow
  audits/                  point-in-time PR audit evidence (local-only, git-ignored)
screenshots/       Command-Deck UI screenshots used by this README
crates/
  traits/          module contracts + shared domain/IPC/job types (no impls)
  kernel/          orchestrator: event bus, capture loop, worker pool, settings
  store/           data spine: SQLite + sqlite-vec + FTS5, job queue, hybrid search
  capture/         CaptureSource (WGC) + diff/privacy gates + event-driven triggers
  ocr/             OcrProvider (WinRT Media.Ocr, STA worker)
  uia/             UI Automation foreground-window text source (OCR fallback)
  textfilter/      pure, deterministic span-aware text classifier (attention-first filtering)
  embeddings/      EmbeddingProvider (fastembed, in-process ONNX)
  inference/       VisionProvider + AnswerProvider + llama.cpp supervisor (Job-Object lifecycle)
  sysmon/          CPU/GPU pressure probe driving the enrichment throttle
  doctor/          WebView2 / Vulkan / llama-server environment smoke-check
  api/             opt-in localhost HTTP API + JSON export (axum, 127.0.0.1 + bearer token)
  mcp/             screensearch-mcp.exe — stdio MCP wrapper over the local API
  sessions/        0.4.0 — pure heuristic session segmentation + tool recognition (no model calls)
  harness/         0.4.0 — dev-only, read-only segmentation validation harness (not shipped)
src-tauri/         Tauri 2 shell + composition root + command handlers + main()
ui/                React 18 + TS + Vite — the full "Command Deck" (6 screens, typed IPC)
specs/             spec-engineering pipeline (00 intake → 04 build prompt → 05–08 build/review)
                   + UI_REFERENCE.md   (frontend identity, tokens, screens, state matrix)
                   + MODEL_REGISTRY.md (exact HF repos / quants / mmproj per model tier)
```

## Build & run

Prerequisites: **Windows 10/11**, a recent Rust toolchain (workspace MSRV **1.82**), Node.js +
npm, and WebView2 (preinstalled on current Windows). First run downloads the embedding model
(~hundreds of MB) and an ONNX Runtime build at compile time.

```powershell
# 1. UI first — src-tauri's `generate_context!` embeds `ui/dist` (git-ignored), so the
#    Rust build fails if the UI hasn't been built yet. `npm run lint` is the Rules-of-Hooks gate.
cd ui && npm ci && npm run lint && npm run build && cd ..

# 2. Stage the MCP sidecar — src-tauri declares `screensearch-mcp.exe` as a `bundle.externalBin`
#    that `tauri-build` resolves on every compile, so a fresh clone must stage it once before any
#    cargo command (npm run dev / build do this automatically; a bare cargo build does not).
node scripts/stage-mcp.mjs

# 3. Rust workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace            # GPU/WinRT/model/perf tests are #[ignore]d

# 4. Binding guard — `cargo test` regenerates the ts-rs IPC bindings; they must stay clean
#    (commit the regenerated files, or CI fails).
git diff --exit-code -- ui/src/bindings

# Run the app (debug) — the Tauri CLI ships as the npm dev-dependency `@tauri-apps/cli`,
# so launch via the root npm script (use `cargo tauri dev` only if you `cargo install tauri-cli`).
npm run dev
```

Model-backed and hardware tests are gated behind `#[ignore]` (they download models or need a real
display/GPU). Run them locally:

```powershell
cargo test -p embeddings -- --ignored                       # loads the real EmbeddingGemma model
cargo test -p store --test perf -- --ignored --nocapture    # hybrid-search latency on 10k frames
cargo test -p ocr -- --ignored                              # WinRT OCR smoke (needs a language pack)
cargo test -p inference --test smoke -- --ignored --nocapture  # real llama-server: vision tag + grounded ask (GPU)
```

## Environment check

```powershell
cargo run -p doctor            # WebView2 / Vulkan / llama-server readiness (diagnostic; add -- --json)
```

## Platform

Windows 10/11 only (uses Windows-native capture, OCR, and WebView2 APIs). Cross-platform
abstractions are intentionally **not** added (see `CLAUDE.md`).

## License

[MIT](./LICENSE) © 2026 Nicolas Estrem

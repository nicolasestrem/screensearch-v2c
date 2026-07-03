# ScreenSearch V2c

A local-first **Windows** desktop app that continuously captures your screen, makes it
searchable by **text and meaning**, and answers questions about what you've seen — fully
on-device, no cloud.

> **Status — 0.3.0 arc in progress.** Capture → OCR/UIA text → deferred enrichment →
> **hybrid search**, the **llama.cpp inference sidecar** (vision tagging + grounded streaming `ask`),
> the full **Command-Deck UI**, and the global-hotkey **Flow overlay** all run on the live app.
> The shipped 0.2.x arc added attention-first text filtering, Recall reports, opt-in event-driven
> capture, and a smart enrichment throttle; the active 0.3.0 arc trims invasive surfaces (event
> triggers, Beta tier, image embeddings) and adds faster recall surfaces. The unsigned **NSIS
> installer** ships today; **code-signing** is the lone remaining packaging follow-up. Design lives
> in [`specs/`](./specs); the as-built architecture is in
> [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md). A standalone, clean-slate project — no shared
> code or data with any prior version.

## Screenshots

The **Command Deck** — six on-device screens over your screen history (five shown below; Settings
omitted). Nothing here touches the network: every frame, query, and answer stays on the machine.

![ScreenSearch Deck — capture toggle, today's activity, live enrichment queue, and recent frames](screenshots/deck.png)

> **Deck** — start/stop capture, today's capture count with a per-app breakdown, the live enrichment
> queue, and a "jump back in" strip of recent frames.

| Recall — grounded **Ask** | Insights — activity analytics | Moment — frame detail |
|:--:|:--:|:--:|
| [![Recall screen — natural-language Ask with cited frames](screenshots/recall-ask.png)](screenshots/recall-ask.png) | [![Insights screen — captures over time, top apps, activities](screenshots/insights.png)](screenshots/insights.png) | [![Moment screen — recognized text and vision tagging](screenshots/moment.png)](screenshots/moment.png) |
| Ask in plain language; answers **cite the exact frames** they came from. | Captures over time, top apps, and inferred activities from vision tags. | One moment's recognized text and context, with on-demand **vision tagging**. |

![Timeline — scrub the day's captures on a scanline](screenshots/timeline.png)

> **Timeline** — scrub a day / week / month of captures on a scanline; `Enter` opens the Moment.

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

The **0.3.0 arc** is the current surface-reduction + flow-recall pass:

| Feature | What it changes | Status |
|---|---|---|
| **Surface reduction** | Removes click/scroll/clipboard/typing triggers, the Beta model tier, and the unused image-embedding lane | ✅ Shipped |
| **Flow overlay** | `Ctrl+Alt+Space` opens a protected always-on-top Search/Ask overlay over your current app | ✅ Implemented |
| **Where-was-i + marks** | Resume context and mark-this-moment workflows | 🚧 Next |
| **Local API + MCP wrapper** | Opt-in localhost API, export path, and thin MCP wrapper | 🚧 Planned |

> Detailed point-in-time PR audits live as local-only artifacts under `docs/audits/` (git-ignored,
> not pushed).

### Working today
Start capture → each changed frame's text is read (foreground-window **UIA**, falling back to native
**WinRT OCR**), stored, and JPEG-archived → an attention-first filter keeps content text over chrome
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
real DB/frame storage usage. `Ctrl+Alt+Space` opens the **Flow overlay**: a second, capture-protected
Tauri window for quick Search/Ask without leaving the foreground app; `Esc` hides it and `Enter`
opens the selected Moment in the main Command Deck.

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
  working context. *(0.3.0 PR5 — implemented)*

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
screenshots/       Command-Deck UI screenshots (used by this README)
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

# 2. Rust workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace            # GPU/WinRT/model/perf tests are #[ignore]d

# 3. Binding guard — `cargo test` regenerates the ts-rs IPC bindings; they must stay clean
#    (commit the regenerated files, or CI fails).
git diff --exit-code -- ui/src/bindings

# Run the app (debug) — the Tauri CLI ships as the npm dev-dependency `@tauri-apps/cli`,
# so launch via the root npm script (use `cargo tauri dev` only if you `cargo install tauri-cli`).
npm run tauri dev
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

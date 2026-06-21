# 00 — Project Intake

> **Purpose (per the methodology):** "the form you fill out once." Raw inputs that seed Layers
> 01–04. Not the spec — the source material.
>
> **Status:** ✅ **Finalized** from the design session. The one remaining open item is **license**
> (defaulted to **MIT** — change anytime). Items resolved this session are marked **`✓ decided`**.

---

## A. Identity

| Field | Value |
|---|---|
| Working name | **ScreenSearch V2c** (repo: `screensearch-v2c`) — `✓ decided` |
| One-line pitch | A Windows-native app that continuously captures your screen, makes it searchable by text *and* meaning, and answers questions about what you've seen — fully local. — `✓ decided` |
| Project type | **Open-source product app** — shipped with an installer + website **screensearch.app**. — `✓ decided` |
| License | **MIT** — `✓ decided` |
| Relationship to V1 | **Standalone.** No code link, **no data import**, nothing shared with V1. V1 is reference knowledge only. — `✓ decided` |

## B. Why V2c (motivation)

- **Pain 1 — modularity:** V1's subsystems were coupled through ad-hoc tokio channels. — `✓`
- **Pain 2 — fault isolation & resource use:** V1's tight *real-time* pipeline was resource-hungry,
  and a crash in a blocking/unsafe subsystem could take down the whole process. — `✓`
- **Expanded feature set:**
  - **v1.0 scope decision:** **No automation** yet. — `✓ decided`
  - **Nice-to-haves (later phases):** multi-model routing, timeline analytics, export, sharing. — `✓ decided`

## C. Users & value

| Field | Value |
|---|---|
| Primary user | Privacy-conscious Windows power users (open-source audience). — `✓ proposed` |
| Core job-to-be-done | "Recall anything I've seen on my screen and ask questions about it." — `✓` |
| Success metrics (proposed) | Crash-free sessions; recall latency (search < ~200 ms); answer usefulness; low idle resource use (a V1 pain). Refine later. — `✓ proposed` |
| Privacy posture | **Local-first, no cloud uploads, no telemetry by default.** All data in a local SQLite file; models on-device. — `✓` |

## D. Platform & scope

| Field | Value |
|---|---|
| Target OS | **Windows 10/11 only.** — `✓ decided` |
| In scope (v1.0) | Always-on cheap capture + OCR + store; **deferred** enrichment (embeddings + **on-demand/timed** vision); hybrid search (FTS5 + vector + RRF); RAG answers; Tauri UI. — `✓ decided` |
| Out of scope / non-goals | macOS/Linux; **OS automation** (deferred); cloud upload / default telemetry; mobile; multi-user/tenancy; accounts; **real-time vision**; V1 data import. — `✓ decided` |
| Feature flags | New/risky features default-off (image embeddings, reranker, scheduled-vision cadence). — `✓` |

## E. Tech stack (decided)

| Layer | Choice | |
|---|---|---|
| Desktop shell | **Tauri 2** (WebView2) | `✓ decided` |
| Frontend | **React 18 + TypeScript + Vite** (reuse the "Command Deck" design language) | `✓ decided` |
| UI ↔ Core IPC | Tauri commands + events, typed via **`ts-rs`** | `✓ decided` |
| Core | **Rust 2021 + tokio**, modular *kernel* + trait-bounded module crates + typed event bus | `✓ decided` |
| **Processing model** | **Capture-cheap, enrich-deferred** — durable **SQLite job queue** + worker pools (on-demand / timer / idle) | `✓ decided` |
| Capture | **Windows.Graphics.Capture (WGC)** behind `CaptureSource` trait | `✓ decided` |
| OCR | **WinRT `Media.Ocr`** on a COM STA worker, behind `OcrProvider` | `✓ decided` |
| Embeddings | **fastembed** (ONNX), in-process — **no Python runtime dep** | `✓ decided` |
| Inference (vision + answer) | **single supervised, model-agnostic llama.cpp sidecar** (Vulkan GPU + CPU fallback), OpenAI-compatible; loads the **selected tier** on demand, swaps between vision/answer models, idle-evicts; **child bound to parent via Windows Job Object (KILL_ON_JOB_CLOSE) → no orphans on crash** | `✓ decided` |
| Database | **SQLite (WAL) + sqlite-vec (`vec0`, cosine) + FTS5 (porter)** | `✓ decided` |
| Automation | **None in v1.0** (deferred) | `✓ decided` |

### Models (researched June 2026) — uniform 3-tier scheme per lane

Two independent model **lanes**, each user-selectable across **Default / Quality / Beta**.
**Default** = lightest / most resource-friendly · **Quality** = best output · **Beta** =
cutting-edge with caveats. The model-agnostic sidecar loads whichever tier is selected on demand.

**Vision lane** (on-demand / timed screenshot understanding, GGUF + mmproj):
| Tier | Model | License | Notes |
|---|---|---|---|
| Default | **Qwen3-VL-4B-Instruct** | Apache-2.0 | Light, fast on-demand tagging. `✓ decided` |
| Quality | **Qwen3-VL-8B-Instruct** | Apache-2.0 | Higher-fidelity descriptions. `✓ decided` |
| Beta | **Qwen3.5-9B-VLM** | Apache-2.0 | Newest generation; experimental. `✓ decided` (swappable) |

**Answer lane** (RAG, *thinking* enabled, GGUF):
| Tier | Model | Ctx | License | Notes |
|---|---|---|---|---|
| Default | **Ministral-3-3B-Reasoning-2512** | 256K | Apache-2.0 | Lightest (3B), vanilla arch (rock-solid llama.cpp), proven lineage. `✓ decided` |
| Quality | **Qwen3-4B-Thinking-2507** | 256K | Apache-2.0 | Top small reasoner; same family as vision. `✓ decided` |
| Beta | **NVIDIA-Nemotron-3-Nano-4B** | ~49K | ⚠️ NVIDIA OML | Strongest reasoner-per-param; **hybrid Mamba-Transformer** + non-Apache → experimental. `✓ decided` |

**Embeddings (in-process fastembed, not the sidecar):**
- **Text:** **EmbeddingGemma-300M** (768-dim). *Embed one input at a time* (quantized; no batch). `✓ decided`
- **Image (optional visual recall):** **nomic-embed-vision-v1.5** (768-dim). `✓ decided`
- **Avoid:** Qwen3-VL-Embedding on llama.cpp — ignores images (V1 POC, cos≈1.0). `✓ fact`

**GPU:** Vulkan assumed for ~99% of users; CPU fallback retained. `✓ decided`
**Constraint:** sidecar mmproj must be same-family as the active vision model. `✓ fact`

## F. Data & migration

| Field | Value |
|---|---|
| Store | Single embedded SQLite file (relational + FTS5 + sqlite-vec + job queue), WAL. — `✓` |
| Import V1 history? | **No.** Start with an empty DB. — `✓ decided` |
| Retention/cleanup | Configurable retention with periodic cleanup. — `✓ proposed` |

## G. Delivery & operations

| Field | Value |
|---|---|
| Distribution | **Inno Setup installer (major releases) + portable ZIP.** **Auto-update** is a nice-to-have (later). — `✓ decided` |
| Build/runtime deps | WebView2 runtime (ships on Win11), Vulkan-capable GPU (optional, CPU fallback), models downloaded on first use. **No Python in the end-user runtime** (Python is fine for build/dev tooling). — `✓ decided` |
| CI/CD | **From scratch** (GitHub Actions, new). — `✓ decided` |
| Timeline / effort budget | **Open-ended** — no deadline. Phasing optimizes for correctness, not speed. — `✓ decided` |

## H. Constraints carried as reference knowledge (hard-won; do not re-discover)

- **EmbeddingGemma-Q cannot batch** — embed one input at a time. — `✓ fact`
- **mmproj projector must be same-family** as the vision model. — `✓ fact`
- **WinRT OCR / WGC capture are COM STA-bound** — calls on the COM-init thread. — `✓ fact`
- **llama.cpp sidecar must not orphan** — Job Object KILL_ON_JOB_CLOSE + startup reap + heartbeat. — `✓ decided`
- **OCR runs on full-res frame before any JPEG resize.** — `✓ fact`

## I. Known risks / unknowns to track

- Tauri 2 + WebView2 packaging on Windows (first-time setup cost). — `✓ noted`
- Sidecar lifecycle correctness (kill-on-close, reap, restart). — `✓ noted`
- WGC capture integration (new code path vs. V1's `screenshots` crate). — `✓ noted`
- On-demand/timed vision UX — how users trigger/schedule enrichment without surprise resource spikes. — `✓ noted`

---

### Status
- **All intake items decided.** License **MIT**; vision Beta **Qwen3.5-9B-VLM**; model tiers locked.

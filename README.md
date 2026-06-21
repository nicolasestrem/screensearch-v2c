# ScreenSearch V2c

A local-first **Windows** desktop app that continuously captures your screen, makes it
searchable by **text and meaning**, and answers questions about what you've seen — fully
on-device, no cloud.

> **Status: specification phase.** No application code yet. This repository is being designed
> with a spec-first methodology; the design lives in [`specs/`](./specs). A standalone,
> clean-slate project (not linked to, and importing no data from, any prior version).

## What it does (planned v1.0)

- **Always-on, cheap capture** — screen capture (Windows.Graphics.Capture) + native WinRT OCR,
  written straight to a local SQLite store.
- **Deferred, user-controlled enrichment** — embeddings and **on-demand / timed** vision tagging
  run as durable jobs in a SQLite-backed queue (you control when the heavy AI work happens).
- **Hybrid search** — FTS5 keyword + vector (sqlite-vec) semantic, fused with Reciprocal Rank
  Fusion.
- **Grounded, reasoning answers** — RAG over your screen history via a local llama.cpp model
  with a *thinking* mode.

## Architecture (summary)

- **Shell:** Tauri 2 + WebView2; React 18 + TypeScript UI; typed IPC via `ts-rs`.
- **Core:** a modular Rust **kernel** — trait-bounded modules over a typed event bus.
- **Processing:** *capture-cheap, enrich-deferred* — a durable SQLite **job queue** + worker
  pools (on-demand / timer / idle).
- **Data:** SQLite (WAL) + FTS5 + sqlite-vec (768-dim, cosine).
- **Inference:** a single supervised, model-agnostic **llama.cpp sidecar** (Vulkan GPU + CPU
  fallback), **bound to the app via a Windows Job Object** so it can never orphan after a crash.
- **ML runtime:** Rust-only (fastembed for embeddings) — **no Python**.

### Models (user-selectable, 3 tiers per lane)

| Lane | Default | Quality | Beta |
|---|---|---|---|
| **Vision** | Qwen3-VL-4B-Instruct | Qwen3-VL-8B-Instruct | Qwen3.5-9B-VLM |
| **Answer** | Ministral-3-3B-Reasoning-2512 | Qwen3-4B-Thinking-2507 | NVIDIA-Nemotron-3-Nano-4B |
| **Embeddings** | EmbeddingGemma-300M (text) · nomic-embed-vision-v1.5 (image) | | |

## Repository layout

```
specs/   spec-engineering pipeline (00 intake → 04 build prompt → 05+ build/review)
```

## Platform

Windows 10/11 only (uses Windows-native capture, OCR, and WebView2 APIs).

## License

[MIT](./LICENSE) © 2026 Nicolas Estrem

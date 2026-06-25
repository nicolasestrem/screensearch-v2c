# 02 — Strategic Plan

> **Question this file answers:** *"What should change, and why?"* — the strategy and the phased
> path. **Not** every table/endpoint/env var (that's `03_MASTER_PRODUCTION_SPEC.md`). Builds on
> `01_PROJECT_CONTEXT.md`.

---

## 1. Goal & user value

**Goal:** ship a local-first Windows app that turns your screen history into an instantly
searchable, question-answerable memory — **without** the resource drain and fragility that sank
the prior attempt.

**User value:**
- *Recall:* find any moment by **text or meaning** (hybrid search).
- *Answers:* ask questions about what you saw and get **grounded, reasoned** answers.
- *Control:* heavy AI work runs **when you choose** (on-demand / timed / idle), not constantly.
- *Trust:* everything is **local**; no cloud, no default telemetry.

## 2. The core strategic change (the "why")

V1's engines were fine; its **shape** was the problem — a tight *real-time* streaming pipeline
(capture→OCR→embed→vision→index, all live) made it resource-hungry and brittle (one crash took
everything down). **V2c changes the shape, not the engines:**

1. **Capture-cheap, enrich-deferred.** Capture + OCR are the only always-on work and are cheap.
   Everything expensive — embeddings, vision tagging, answers — becomes **durable jobs in a
   SQLite-backed queue**, executed by workers **on-demand / on-timer / when-idle**. This is the
   single most important change: it delivers user resource-control *and* fault isolation *and*
   modularity in one move.
2. **Fault isolation by construction.** The only crash-prone, out-of-process component (the
   llama.cpp inference sidecar) is **bound to the app via a Windows Job Object** so it can never
   orphan; a failed enrichment **job retries** instead of crashing capture.
3. **Modular kernel over ad-hoc channels.** Trait-bounded modules communicate over a typed event
   bus — any module is swappable and testable in isolation.
4. **Tiered models, user-selectable.** Vision and answer each offer **Default / Quality / Beta**,
   so users trade footprint vs. quality explicitly (see `00 §E`).

## 3. What stays vs. changes (vs. the V1 reference baseline)

| Area | V1 (reference) | V2c |
|---|---|---|
| Engines | SQLite+FTS5+sqlite-vec, WinRT OCR, fastembed, llama.cpp | **Keep** (proven) |
| Processing | real-time streaming pipeline | **Change → enrich-deferred job queue** |
| Vision | per-frame, real-time | **Change → on-demand / timed** |
| Shell/IPC | Axum localhost HTTP + rust-embed | **Change → Tauri 2 typed IPC** |
| Structure | monolith, ad-hoc channels | **Change → modular kernel + event bus** |
| ML runtime | flirted with Python ML sidecar (failed) | **Rust-only runtime** (no Python *ML sidecar*; Python OK for tooling) |
| Models | single defaults | **Change → 3-tier per lane** |
| Automation | present | **Drop for v1.0** (later) |

## 4. Future-state architecture (high level)

```
Tauri 2 app  ──typed IPC──  Rust kernel (event bus + trait modules)
   │                              │
   │   always-on (cheap):  WGC capture → WinRT OCR → Store
   │   deferred (controlled):  SQLite JobQueue → workers:
   │        • fastembed (text+image vectors, in-process)
   │        • llama.cpp sidecar (vision tag / RAG answer, Job-Object-bound)
   │   query:  FTS5 + vec KNN → RRF → (sidecar answer, thinking) → stream to UI
```

Detailed schema, traits, command/event contracts, and sidecar protocol live in `03`.

## 5. Delivery phases (correctness-first; no deadline)

- **P0 — Scaffold:** Tauri 2 + Cargo workspace (`kernel`, `traits`, module crates), `ts-rs`
  binding gen, CI skeleton, WebView2/Vulkan/llama smoke-check.
- **P1 — Data spine:** `Store` + `JobQueue` on SQLite + sqlite-vec + FTS5; schema + migrations;
  RRF retrieval. *Everything writes here.*
- **P2 — Capture happy path:** WGC capture + WinRT OCR (STA) + event bus → frames+text stored;
  live timeline in the UI. *Proves the kernel.*
- **P3 — Deferred enrichment:** embedding worker (fastembed) + job scheduling
  (on-demand/timer/idle); hybrid search end-to-end.
- **P4 — Inference sidecar:** Job-Object lifecycle (spawn/reap/heartbeat/evict), model-agnostic
  tiered loader; on-demand/timed **vision tagging** + **RAG answers** (thinking).
- **P5 — Product:** Command-Deck UI polish; settings (model tiers, schedules, retention);
  packaging (Inno installer + portable ZIP); first release.
- **Later (nice-to-haves):** multi-model routing UI, timeline analytics, export, sharing,
  auto-update, OS automation.

## 5b. Post-1.0 arc — 0.2.x (attention-first text signal + recall workflows)

P0–P5 (v1.0) are complete and merged; the 0.2.x line is a **separate arc** layered on the shipped
app, **not** a retrofit of the v1.0 phases above. It is tracked in detail in `docs/0.2.0.md`
(roadmap) and `03` (contract); this section states only the strategic *what/why*.

- **The problem.** Capture indexes **raw full-screen OCR with no filtering**, so search, Ask, and
  embeddings get dominated by static chrome — taskbars, desktop icons, browser toolbars, even the
  app's own sidebar labels. Searching "Firefox" / "Steam" / "Deck" surfaces frames purely because
  those labels were on screen, not because they were the user's actual work.
- **P6 — Attention-first text signal + recall workflows.** Preserve raw text, but derive a default
  **content-text** layer (filtered OCR/UIA text — *not* vision descriptions) and make search, Ask,
  embeddings, and reports use it by default. Raw / app-chrome text stays searchable **opt-in**
  (`include_chrome`); default search stays **hybrid (FTS + vector) over content text** and the FTS
  fallback is never removed. Adds Recall **reports** (daily/weekly/custom) and premade Ask cards on
  top of the cleaned signal.
- **Ships in 0.2.0** (clean DB, no backfill): PR1 specs → PR2 data model + OCR spans → PR3
  attention-first filtering → PR6 reports → PR7 audit.
- **Deferred to 0.2.1** (highest-risk, most-invasive, not needed for the retrieval fix):
  event-driven capture, UIA text, and a smart enrichment throttle — each its own gated PR, recorded
  in `07`. **0.2.0 keeps timer/idle capture; no raw keystrokes or clipboard text are ever stored.**

## 6. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Tauri 2 + WebView2 packaging friction (new) | Spike in P0; keep the shell thin; the kernel is shell-agnostic behind traits. |
| WGC capture integration (new code path) | `CaptureSource` trait + an early P2 spike; fall back to a simpler capture if WGC misbehaves. |
| Sidecar orphan/hang | Job Object KILL_ON_JOB_CLOSE + startup reap + heartbeat/restart (hard requirement). |
| Beta answer model (Nemotron) hybrid arch on llama.cpp/Vulkan | It's **Beta only**, never the default; Default/Quality are vanilla-arch + Apache. |
| Resource spikes surprising users | On-demand/timed by default; explicit schedule UI; idle-only option; per-job budgets. |
| Scope creep | Automation + nice-to-haves are explicitly **out of v1.0**. |

## 7. Non-goals (reaffirmed)
macOS/Linux · OS automation (v1.0) · cloud/telemetry · accounts/multi-user · real-time vision ·
V1 data import.

## 8. Status
- **License decided: MIT.** No open strategic questions.
- **v1.0 (P0–P5) shipped** (`v0.1.0`, 2026-06-24). Active arc: **0.2.x — attention-first text
  signal + recall workflows** (see `§5b` and `docs/0.2.0.md`).

---

*Next layer:* `03_MASTER_PRODUCTION_SPEC.md` — the engineering truth: schema, traits, event/command
contracts, job-queue + sidecar protocols, config, logging, testing, CI/CD, definition of done.

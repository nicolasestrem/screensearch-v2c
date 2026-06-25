# UI Reference (companion spec)

> **Scope:** the frontend contract — identity, design tokens, screen inventory, per-screen state
> matrix, components, typed-data rules, accessibility, performance, and voice. Same authority model
> as `03` but narrowed to the UI. The build (P5) is held to this file. When it is silent or
> contradictory, **stop and ask** (`04 §5`).

---

## 1. Aesthetic direction (the point of view)

**Thesis — a forensic console for your own screen-time.** This is not a productivity dashboard;
it's an instrument that *reads back a recording* of what you saw. The product literally captures
screens, so the interface is built from that material: a dark telemetry console where **time is a
physical filmstrip** you scrub, and a single signal-orange scan-head marks "now / here." Everything
is quiet and disciplined except that one living element.

This honors the pinned **Command Deck** identity (warm-graphite + a single signal-orange accent,
Windows-native fonts, dark-only, WCAG-AA) and gives it a reason to exist beyond "dark + accent."

**The one aesthetic risk (justified):** treat the timeline as a real instrument — a continuous
**Scanline Timeline** with a sweeping scan-head and a faint scanline texture on surfaces. The
scanline is the subject's *native material* (screen capture / CRT / monitoring), not decoration. We
spend all boldness here; every other surface stays calm.

### Palette (warm graphite + one bold accent)
| Token | Hex | Use |
|---|---|---|
| `--bg-base` | `#15120D` | app background (warm near-black, not pure black) |
| `--bg-surface` | `#1E1A13` | panels, cards |
| `--bg-overlay` | `#262017` | popovers, command palette, modals |
| `--line` | `#332B20` | hairline dividers, borders |
| `--ink` | `#ECE4D5` | primary text (warm off-white) |
| `--ink-muted` | `#9A8F7A` | secondary text, labels |
| `--ink-faint` | `#6B6253` | tertiary, disabled |
| `--accent` | `#FF6A1A` | **the only bold color** — scan-head, active state, primary action |
| `--accent-wash` | `rgba(255,106,26,0.14)` | scanline glow, selection, active row |

**Functional-only** (never brand decoration, used solely in status contexts, kept desaturated):
`--danger #C5524B` · `--warn #D9912F` · `--ok #7FA88B`. If a screen reaches for a fourth hue, stop.

### Typography (Windows-native — a constraint *and* a personality)
No web-font downloads (privacy/offline/portable). The native stack is the character:
- `--font-display`: **Bahnschrift** (condensed industrial / DIN-like) → labels, eyebrows,
  section heads, set in **uppercase with tracking** for an instrument-readout feel.
- `--font-body`: **Segoe UI** → prose, answers, descriptions.
- `--font-mono`: **Consolas** → all data: timestamps, counts, durations, model/job state.

**Type scale:** display 28/600 · title 20/600 · subtitle 16/600 · body 14/400 · caption 12/500 ·
data 13 mono. Long-form RAG answers use `@tailwindcss/typography` (body face).

### Form
- **Radius:** 4px panels, 2px data chips, **0 on the timeline ribbon** (it's an instrument, not a
  card). Not zero everywhere — that's the broadsheet default.
- **Elevation:** surface color steps + hairline, minimal shadow (dark UI).
- **Motion:** 120–200ms ease for UI; the scan-head moves **linearly**; an ambient scanline drifts
  *very* slowly. All ambient motion disabled under `prefers-reduced-motion`.

### Signature: the **Scanline Timeline**
A horizontal time-ribbon: frames appear as density ticks (busier time = denser), thumbnails on
hover, a sweeping **signal-orange scan-head** at the focused moment, faint scanline texture. It
encodes real information (when things happened, how much) — never decorative tick-marks. Reused as
a thin "minimap" strip at the top of every recall view.

## 2. Design tokens (single source)
Tokens live in `ui/src/styles/tokens.css` as CSS custom properties; Tailwind theme maps to them.
**No component hardcodes a hex, px font, or magic spacing** — everything references a token.
Spacing scale: 4 · 8 · 12 · 16 · 24 · 32 · 48. Z-layers: base / rail / overlay / toast.

## 3. Information architecture (every screen → a real job)
```
AppShell
 ├─ StatusRail (top): capture state · DB size · queue depth · sidecar/model · readiness   [telemetry]
 ├─ NavRail (left): Deck · Recall · Timeline · Insights · Settings
 ├─ CommandPalette (⌘K): jump-to + actions (search, ask, tag, settings)
 └─ Routes:
     /            Deck      — at-a-glance: capture status, today's activity, jump back in
     /recall      Recall    — Search · Ask · Reports (0.2.x); content text default, opt-in raw/chrome
     /timeline    Timeline  — the Scanline Timeline browser
     /timeline/:id Moment   — one frame: image, OCR text, vision tags, context, actions
     /insights    Insights  — activity analytics (nice-to-have; ships as real or honest-empty)
     /settings    Settings  — capture · models (tiers) · enrichment schedule · privacy · retention
     *            NotFound
```
Rules: one primary action per screen; every route is reachable from NavRail or a link; no orphan
screens; deep-linkable (real routes, `/timeline/:id` shareable within the app).

## 4. Per-screen state matrix (the comprehensiveness guarantee)
**Every view defines all of: `loading` · `empty` · `error` · `partial` · `populated`.** No screen
ships with only the happy path; no mock data; no "Coming Soon."

| Screen | empty | error | partial | notes |
|---|---|---|---|---|
| Deck | "Capture is off / no frames yet — start capture" | readiness probe failed → retry | capturing but no enrichment yet | drives onboarding |
| Recall (search) | "No matches — try different words / widen the range, or include app chrome" | search cmd failed → retry | vectors still indexing → "searching text only for now" banner | content text by default + "include app chrome / raw text" toggle; never a zero-result dead end |
| Recall (ask) | prompt invites a question (or a premade card) | sidecar unavailable → "answer model not loaded; load it?" | streaming (tokens arriving) | cite frames; premade cards prefill + submit |
| Recall (reports) | range picked; prompt invites "Generate" | generation failed → retry, keep range | generating (single-pass / map-reduce in progress) | markdown + clickable source-frame chips + Copy + `.md` download + model/tokens footer; honest empty on no-evidence ranges |
| Timeline | "No captures in this range" | load failed → retry | thumbnails still resolving | scrub never blank |
| Moment | — | frame missing/deleted → explain + back | vision not yet tagged → "queue vision for this frame" | on-demand vision entry point |
| Insights | "Not enough history yet" (honest) | compute failed → retry | partial windows labeled | no fabricated charts |
| Settings | — | save failed → keep form, explain | model downloading (progress) | optimistic + reconcile |

Loading uses skeletons that match final layout (no spinner-only screens). Empty states are
**invitations to act**, not mood.

## 5. Component inventory (built once, reused)
Shell: `AppShell`, `StatusRail`, `NavRail`, `CommandPalette`, `ReadinessBanner`.
Primitives: `Panel`, `Button`, `IconButton`, `Field`, `Select`, `Toggle`, `Chip`, `Toast`,
`EmptyState`, `ErrorState`, `Skeleton`, `Tooltip`.
Domain: `ScanlineTimeline`, `FrameTile`, `FrameImage` (lazy), `AnswerStream` (markdown + citations),
`SearchResult`, `MomentDetail`, `JobQueueMeter`, `ModelTierPicker` (Default/Quality/Beta),
`ScheduleControl` (on-demand/timer/idle), `RetentionControl`.
Domain (0.2.x): `RecallModeTabs` (Search/Ask/Reports), `TextSourceToggle` (content / include-chrome),
`ReportBuilder` (daily/weekly/custom range → Generate), `ReportView` (markdown + clickable
source-frame chips + Copy + `.md` download + model/tokens footer), `PromptCardGrid` (premade Ask
cards: Day Recap, Standup Update, Time Breakdown, Top of Mind, AI Habits — click fills + submits).
Each component owns one job; a label labels, an example demonstrates — nothing does double duty.

## 6. Data & state (reliability by construction)
- **Typed IPC only.** Every command/event payload is a Rust struct exported via **`ts-rs`** and
  imported by the UI. The UI **never** hand-writes an API type. Contract drift is impossible.
- **TanStack Query** owns all server-state (commands): one place for cache/loading/error/refetch.
  No bespoke `useEffect` fetch-and-setState.
- **Zustand** only for ephemeral UI state (palette open, selected range, active Recall mode).
- **Content vs raw text is a typed query param (0.2.x):** the Recall toggle sets
  `SearchQuery.include_chrome` (default `false`); reports call `generate_report`. Both flow through
  TanStack Query — no ad-hoc `useEffect` fetches; premade Ask cards just prefill + submit the
  existing `ask` flow.
- Streaming (`ask`) consumes `answer_delta` events into a reducer; `readiness_changed` /
  `sidecar_status` / `job_progress` drive the StatusRail live.
- **Rules of Hooks are inviolable** — all hooks before any early return; JSX conditionals, not
  conditional hooks. (This is a known scar; see `04` guardrails.)
- Error boundaries per route; a thrown render never blanks the whole app.

## 7. Accessibility (WCAG-AA, non-negotiable)
AA contrast for all text/controls (palette chosen for it); visible keyboard focus on every
interactive element; full keyboard nav incl. the timeline (arrow scrub, Enter to open); ARIA on
custom widgets (timeline = slider semantics); `prefers-reduced-motion` disables scan/ambient;
respects OS dark (app is dark-only by design); hit targets ≥ 32px.

## 8. Performance budgets
Initial JS ≤ 250 KB gzip; route-split per page; virtualized frame grids/timeline (no full-list
DOM); `FrameImage` lazy + decoded async; search results render < 100 ms after data; interaction
latency < 100 ms; no layout shift on data arrival (skeletons reserve space).

## 9. Voice & copy (interface speaks plainly, from the user's side)
Name things by what the user controls ("Pause capture", not "halt pipeline"). Actions keep their
name through the flow ("Tag with vision" → toast "Vision tagged"). Errors explain what happened and
how to fix it, in the interface's voice, never apologizing or vague. Sentence case, active voice,
no filler. Timestamps human-relative with absolute on hover.

## 10. Acceptance criteria (definition of done — UI)
1. Every route renders **real API data** or an explicit loading/empty/error/partial state — **zero
   mock data, zero dead ends, zero "Coming Soon."**
2. All payload types are `ts-rs`-generated; no hand-written API types in the UI.
3. `eslint-plugin-react-hooks` passes as an error-level gate; routes have error boundaries.
4. Tokens are the single source — no hardcoded hex/font/spacing in components.
5. AA contrast verified; keyboard-only operation of every screen incl. the timeline; reduced-motion
   honored.
6. Performance budgets (§8) met on a realistic DB.
7. The Scanline Timeline scrubs smoothly and reflects real capture density.
8. Verified by **running the app and capturing screenshots** of each screen in each state — not by
   "it compiles" (`04 §6`).

---

*Companion to `00`–`04`. Aesthetic direction is intentional and subject-grounded; if a future change
makes the UI read as a templated dark dashboard, that's a regression against §1.*

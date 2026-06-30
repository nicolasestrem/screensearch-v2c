# Design — Readable native-WebP captures + degrade-to-text retention

**Date:** 2026-06-30 · **Branch:** `feat/degrade-to-text-retention` · **Status:** implemented

## Problem

Two complaints, both confirmed on disk (813 frames, ~3 h):

1. **Captures are unreadable on an ultra-wide.** Screenshots were downscaled to
   `storage.max_width = 1280`, squashing a 3440/5120 px monitor 3–4× and crushing text. (OCR/search
   were unaffected — OCR runs on the full-res frame before the downscale.)
2. **Storage grows unbounded** (`storage.retention_days = 0` = keep forever): `frames/` JPEGs 91 MB
   (60%) + `screensearch.db` 59 MB (40% — `text_spans`, two FTS indexes, embeddings; **not** images).

## Decisions (from the brainstorm)

- **Proof = text + layout is enough** — no need to keep literal pixels long-term.
- **Keep on-demand Vision usable** — so recent frames must keep real pixels.
- **Scope = just enable retention** — do *not* prune the DB in this change.
- **Retention = drop the screenshot, keep the text.**
- **Readability = native resolution + WebP.**

Synthesis: keep readable real pixels for the recent window (Vision works); past the window, degrade a
capture to a tiny, searchable **text + layout reconstruction** drawn from the spans we already store.

## Design

### 1. Readable, native-resolution WebP captures
- `storage.max_width == 0` ⇒ native (no downscale); new default `0`. (`maybe_resize` already no-ops at
  `0`; `sanitize_settings` now admits `0` as a sentinel, like the sidecar `0 = auto` knobs.)
- Encoder JPEG → **lossless WebP** (`image::codecs::webp::WebPEncoder::new_lossless`), `.webp` files.
  No new C dependency — `image-webp` (lossless encoder) is already in the tree. `storage.jpeg_quality`
  is inert for the lossless encoder (kept for forward-compat / a future lossy codec).
- **Honesty:** native res makes each *kept* file bigger than the old 1280 JPEG; the space win comes
  entirely from retention degrading old pixels to text — so the two ship together and retention
  defaults non-zero.

### 2. Degrade-to-text retention
- Schema **v7**: `frames.image_purged` (`0` present / `1` degraded; CHECK-bounded; additive ADD COLUMN).
- `frames_with_image_older_than(cutoff, limit)` (excludes `image_purged = 1`) + `purge_frame_image`.
- Sweeper (`run_retention_once`): delete the image file → `purge_frame_image` (keeps row + text + spans
  + embeddings). `delete_frame` stays for the one-time self-capture purge. Default
  `storage.retention_days = 30`; `0` keeps screenshots forever.

### 3. Proof when the picture is gone
- `image_purged` on `FrameMeta` / `FrameDetail` / `SearchHit`.
- `get_frame_spans` IPC command over the existing `SqliteStore::frame_spans`; `TextSpan` exported via ts-rs.
- `FrameReconstruction` component draws each span at its normalized position, scaled to its line height,
  with role-based emphasis — shown in the Moment view for degraded frames. `FrameImage` shows a
  "Text kept" state in tile/search/citation surfaces. Settings: native-width affordance + retention copy.

## Out of scope (deferred — `07` #73)
DB-side growth pruning; lossy WebP / AVIF (new C dep); Vision / `embed_image` on already-degraded frames.

## Verification
UI lint + `vite build`; `cargo fmt --check`; `clippy -D warnings`; `cargo test --workspace` (incl.
migration v7 + purge + native/webp resize tests); binding guard clean. **Live:** `e2e_capture
--ignored` stored a real **3440×1440 native WebP** (decoded dims == captured dims).

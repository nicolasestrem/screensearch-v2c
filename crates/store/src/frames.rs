//! Lightweight frame browsing: frames within a time window, the frame nearest a
//! timestamp, and the captures bracketing one. Backs the `get_frames` /
//! `get_nearest_frame` / `get_frame_context` IPC commands (P5 Timeline hover
//! thumbnails, Deck "jump back in" recents, Moment neighbour context + prev/next).
//! These are read-only browsing helpers returning [`FrameMeta`] — the
//! heavier [`SqliteStore::get_frame`] hydrates the full per-frame detail once a
//! frame id is chosen. Both seek on `idx_frames_captured_at`.

use rusqlite::{params, OptionalExtension, Row};
use traits::{FrameMeta, Result};

use crate::SqliteStore;

/// Maps a `(id, captured_at, image_path, app_hint)` row to a [`FrameMeta`]. Shared
/// by both queries so the column order is defined in exactly one place.
fn row_to_meta(r: &Row<'_>) -> rusqlite::Result<FrameMeta> {
    Ok(FrameMeta {
        frame_id: r.get(0)?,
        captured_at: r.get(1)?,
        image_path: r.get(2)?,
        app_hint: r.get(3)?,
    })
}

impl SqliteStore {
    /// Frames captured in the half-open window `[start, end)`, **most recent first**,
    /// capped at `limit`. Returns the lightweight [`FrameMeta`] (no OCR/vision/tags) —
    /// enough to render a tile/thumbnail and open the frame. An empty/invalid window
    /// (`end <= start`) or `limit == 0` yields no rows.
    pub async fn frames_in_range(
        &self,
        start: i64,
        end: i64,
        limit: u32,
    ) -> Result<Vec<FrameMeta>> {
        if end <= start || limit == 0 {
            return Ok(Vec::new());
        }
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, captured_at, image_path, app_hint
                 FROM frames
                 WHERE captured_at >= ?1 AND captured_at < ?2
                 ORDER BY captured_at DESC
                 LIMIT ?3",
            )?;
            let rows = stmt
                .query_map(params![start, end, i64::from(limit)], row_to_meta)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .await
    }

    /// The single frame whose `captured_at` is closest to `at` (unix ms), or `None`
    /// if the DB has no frames at all. Resolves the Timeline scan-head's continuous
    /// position to a concrete frame id — "Enter opens the moment under the head" —
    /// without loading the whole window. Evaluates the nearest frame on each side of
    /// `at` (each a single index seek) and returns the closer; an exact tie prefers
    /// the at-or-after frame (the more recent), matching the DESC tie-break elsewhere.
    pub async fn nearest_frame(&self, at: i64) -> Result<Option<FrameMeta>> {
        self.with_conn(move |conn| {
            let after = conn
                .query_row(
                    "SELECT id, captured_at, image_path, app_hint
                     FROM frames WHERE captured_at >= ?1
                     ORDER BY captured_at ASC LIMIT 1",
                    params![at],
                    row_to_meta,
                )
                .optional()?;
            let before = conn
                .query_row(
                    "SELECT id, captured_at, image_path, app_hint
                     FROM frames WHERE captured_at < ?1
                     ORDER BY captured_at DESC LIMIT 1",
                    params![at],
                    row_to_meta,
                )
                .optional()?;
            Ok(nearer(at, before, after))
        })
        .await
    }

    /// The single frame whose `captured_at` is closest to `at`, constrained to the
    /// half-open window `[start, end)`. Returns `None` when the visible window has no
    /// frames, even if the database has captures outside it.
    pub async fn nearest_frame_in_range(
        &self,
        at: i64,
        start: i64,
        end: i64,
    ) -> Result<Option<FrameMeta>> {
        if end <= start {
            return Ok(None);
        }
        self.with_conn(move |conn| {
            let after = conn
                .query_row(
                    "SELECT id, captured_at, image_path, app_hint
                     FROM frames
                     WHERE captured_at >= ?1 AND captured_at >= ?2 AND captured_at < ?3
                     ORDER BY captured_at ASC LIMIT 1",
                    params![at, start, end],
                    row_to_meta,
                )
                .optional()?;
            let before = conn
                .query_row(
                    "SELECT id, captured_at, image_path, app_hint
                     FROM frames
                     WHERE captured_at < ?1 AND captured_at >= ?2 AND captured_at < ?3
                     ORDER BY captured_at DESC LIMIT 1",
                    params![at, start, end],
                    row_to_meta,
                )
                .optional()?;
            Ok(nearer(at, before, after))
        })
        .await
    }

    /// Bounded retention candidates: frames older than `cutoff`, oldest first. The
    /// caller deletes the returned rows and their JPEG files in small batches.
    pub async fn frames_older_than(&self, cutoff: i64, limit: u32) -> Result<Vec<FrameMeta>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, captured_at, image_path, app_hint
                 FROM frames
                 WHERE captured_at < ?1
                 ORDER BY captured_at ASC
                 LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![cutoff, i64::from(limit)], row_to_meta)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .await
    }

    /// The captures immediately **bracketing** `at` (unix ms): up to `limit_each`
    /// frames just *before* it and up to `limit_each` just *after* it, within
    /// `±half_window_ms`, returned ascending by `captured_at`. The anchor's own row
    /// (`captured_at == at`) is excluded — the caller already holds it via
    /// [`SqliteStore::get_frame`]. Backs the Moment screen's prev/next + context strip,
    /// which need the captures *adjacent* to the viewed frame. This cannot be expressed
    /// with [`frames_in_range`]: its newest-first cap returns only the latest frames in
    /// the window, so in a busy session (a frame every few seconds) the 30-minute
    /// forward window fills with frames near its far edge and the true neighbours — and
    /// the anchor — are silently dropped. Here each side is ordered toward the anchor
    /// (before: DESC, after: ASC) before capping, so the closest neighbours always win.
    /// A degenerate window (`half_window_ms <= 0`) or `limit_each == 0` yields no rows.
    pub async fn neighbour_frames(
        &self,
        at: i64,
        half_window_ms: i64,
        limit_each: u32,
    ) -> Result<Vec<FrameMeta>> {
        if half_window_ms <= 0 || limit_each == 0 {
            return Ok(Vec::new());
        }
        // Saturating so an extreme `at`/window can't overflow the bound (mirrors the
        // i128 care in `nearer` and the overflow-safe `timeline_buckets`).
        let lo = at.saturating_sub(half_window_ms);
        let hi = at.saturating_add(half_window_ms);
        self.with_conn(move |conn| {
            let n = i64::from(limit_each);
            // Closest BEFORE the anchor: largest `captured_at` strictly below `at`.
            let mut before_stmt = conn.prepare(
                "SELECT id, captured_at, image_path, app_hint
                 FROM frames
                 WHERE captured_at >= ?1 AND captured_at < ?2
                 ORDER BY captured_at DESC
                 LIMIT ?3",
            )?;
            let mut out = before_stmt
                .query_map(params![lo, at, n], row_to_meta)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            // Closest AFTER the anchor: smallest `captured_at` strictly above `at`.
            let mut after_stmt = conn.prepare(
                "SELECT id, captured_at, image_path, app_hint
                 FROM frames
                 WHERE captured_at > ?1 AND captured_at <= ?2
                 ORDER BY captured_at ASC
                 LIMIT ?3",
            )?;
            let after = after_stmt
                .query_map(params![at, hi, n], row_to_meta)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            out.extend(after);
            // The before-side came back DESC; merge to one ascending-by-time list.
            out.sort_by_key(|f| f.captured_at);
            Ok(out)
        })
        .await
    }
}

/// Picks whichever of `before` / `after` is closer to `at`. `before.captured_at < at`
/// and `after.captured_at >= at`, so distances are non-negative; an exact tie prefers
/// `after`. Distances are computed in `i128` so hostile/extreme timestamps can't
/// overflow `i64`.
fn nearer(at: i64, before: Option<FrameMeta>, after: Option<FrameMeta>) -> Option<FrameMeta> {
    match (before, after) {
        (None, None) => None,
        (Some(b), None) => Some(b),
        (None, Some(a)) => Some(a),
        (Some(b), Some(a)) => {
            let d_before = (i128::from(at) - i128::from(b.captured_at)).abs();
            let d_after = (i128::from(a.captured_at) - i128::from(at)).abs();
            if d_after <= d_before {
                Some(a)
            } else {
                Some(b)
            }
        }
    }
}

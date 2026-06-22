//! Lightweight frame browsing: frames within a time window, and the frame nearest
//! a timestamp. Backs the `get_frames` / `get_nearest_frame` IPC commands (P5
//! Timeline hover thumbnails, Deck "jump back in" recents, Moment neighbour
//! context). These are read-only browsing helpers returning [`FrameMeta`] — the
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

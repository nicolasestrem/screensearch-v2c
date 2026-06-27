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

    /// Up to `limit` frames sampled **evenly across** `[start, end)` by `captured_at`
    /// ascending (chronological) — for temporal-coverage report sampling (`03 §8b`).
    /// Unlike [`frames_in_range`]'s newest-first cap, this partitions the in-range frames
    /// into `limit` equally-sized rank buckets and keeps the first frame of each, so it
    /// returns the **full** requested quota spread across the whole window — a report
    /// covers the period, not just its tail. (A plain `ceil(total/limit)` stride collapses
    /// to ~half the quota the moment `total` just exceeds `limit`: 41 frames with
    /// `limit = 40` would yield 21, not 40.) When the window holds `<= limit` frames every
    /// frame is returned. An empty/invalid window (`end <= start`) or `limit == 0` yields
    /// no rows. The caller passes one period's bounds at a time, so the selection is
    /// per-period (coverage within each period), never a single global stride.
    pub async fn sample_frames_in_range(
        &self,
        start: i64,
        end: i64,
        limit: u32,
    ) -> Result<Vec<FrameMeta>> {
        if end <= start || limit == 0 {
            return Ok(Vec::new());
        }
        self.with_conn(move |conn| {
            let n = i64::from(limit);
            // Number the in-range frames 0..total by time, then assign each to one of
            // `limit` even buckets (`rn * limit / total`) and keep the first row of each
            // bucket. This yields exactly `min(total, limit)` rows evenly spread across the
            // window, rather than an integer stride that halves the sample once `total`
            // edges past `limit`. `LIMIT` is a defensive cap. (Codex review, PR #33.)
            let mut stmt = conn.prepare(
                "SELECT id, captured_at, image_path, app_hint FROM (
                     SELECT id, captured_at, image_path, app_hint,
                            row_number() OVER (ORDER BY captured_at ASC, id ASC) - 1 AS rn,
                            count(*) OVER () AS total
                     FROM frames
                     WHERE captured_at >= ?1 AND captured_at < ?2
                 )
                 WHERE rn = 0 OR (rn * ?3) / total <> ((rn - 1) * ?3) / total
                 ORDER BY captured_at ASC, id ASC
                 LIMIT ?3",
            )?;
            let rows = stmt
                .query_map(params![start, end, n], row_to_meta)?
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

    /// Frames whose foreground `app_hint` equals `hint` (case-insensitive), oldest
    /// first, capped at `limit`. Backs the one-time self-capture purge: the app never
    /// indexes its own window after the PR3 audit
    /// (`docs/AUDIT_0.2.0_PR3_2026-06-26.md`), so any pre-existing own-window frames are
    /// swept out. The caller deletes the returned rows and their JPEG files in batches.
    pub async fn frames_with_app_hint(&self, hint: &str, limit: u32) -> Result<Vec<FrameMeta>> {
        if hint.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let hint = hint.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, captured_at, image_path, app_hint
                 FROM frames
                 WHERE app_hint = ?1 COLLATE NOCASE
                 ORDER BY captured_at ASC
                 LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![hint, i64::from(limit)], row_to_meta)?
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

#[cfg(test)]
mod tests {
    use crate::SqliteStore;
    use traits::NewFrame;

    fn frame(at: i64) -> NewFrame {
        NewFrame {
            captured_at: at,
            monitor_index: 0,
            width: 1920,
            height: 1080,
            image_path: format!("frames/{at}.jpg"),
            content_hash: format!("h{at}"),
            app_hint: None,
            window_title: None,
            browser_url: None,
        }
    }

    async fn seed(store: &SqliteStore, count: i64, step: i64) {
        for i in 0..count {
            store.insert_frame(frame(i * step)).await.unwrap();
        }
    }

    /// Sampling spreads evenly across the window (covers the EARLY end, not just the
    /// newest tail) — the property `frames_in_range`'s DESC-LIMIT cannot give.
    #[tokio::test]
    async fn sample_spreads_evenly_and_includes_the_earliest_frame() {
        let store = SqliteStore::open_in_memory().unwrap();
        seed(&store, 12, 10).await; // captured_at = 0,10,…,110
        let got = store.sample_frames_in_range(0, 120, 4).await.unwrap();
        // 12 frames into 4 even buckets (first of each) → rn 0,3,6,9 → times 0,30,60,90.
        let times: Vec<i64> = got.iter().map(|f| f.captured_at).collect();
        assert_eq!(times, vec![0, 30, 60, 90]);
        // Earliest frame present (not a newest-first tail) and ascending order.
        assert_eq!(got.first().unwrap().captured_at, 0);
        assert!(times.windows(2).all(|w| w[0] < w[1]));
    }

    /// When the window holds `<= limit` frames the stride is 1 → all are returned,
    /// ascending, with none dropped.
    #[tokio::test]
    async fn sample_returns_all_when_count_under_limit() {
        let store = SqliteStore::open_in_memory().unwrap();
        seed(&store, 3, 100).await; // 0,100,200
        let got = store.sample_frames_in_range(0, 1_000, 10).await.unwrap();
        let times: Vec<i64> = got.iter().map(|f| f.captured_at).collect();
        assert_eq!(times, vec![0, 100, 200]);
    }

    /// Just over the quota must still return the FULL quota, not collapse to ~half — the
    /// regression for the `ceil(total/limit)` stride that doubled at `total > limit`
    /// (41 frames, limit 40 → 21 rows). (Codex review, PR #33.)
    #[tokio::test]
    async fn sample_returns_full_quota_when_just_over_limit() {
        let store = SqliteStore::open_in_memory().unwrap();
        seed(&store, 41, 10).await; // 41 frames at 0,10,…,400
        let got = store.sample_frames_in_range(0, 410, 40).await.unwrap();
        assert_eq!(
            got.len(),
            40,
            "must return the full requested quota, not ~half"
        );
        // Still evenly spread & chronological: earliest frame kept, strictly ascending.
        let times: Vec<i64> = got.iter().map(|f| f.captured_at).collect();
        assert_eq!(times.first(), Some(&0));
        assert!(times.windows(2).all(|w| w[0] < w[1]));
    }

    /// The sample never exceeds `limit` and stays within the half-open window.
    #[tokio::test]
    async fn sample_caps_at_limit_within_window() {
        let store = SqliteStore::open_in_memory().unwrap();
        seed(&store, 100, 10).await; // 0,10,…,990
        let got = store.sample_frames_in_range(0, 500, 7).await.unwrap();
        assert!(got.len() <= 7, "got {} > limit", got.len());
        assert!(got
            .iter()
            .all(|f| f.captured_at >= 0 && f.captured_at < 500));
        assert!(!got.is_empty());
    }

    /// Degenerate inputs yield no rows (mirrors `frames_in_range`).
    #[tokio::test]
    async fn sample_degenerate_windows_are_empty() {
        let store = SqliteStore::open_in_memory().unwrap();
        seed(&store, 5, 10).await;
        assert!(store
            .sample_frames_in_range(50, 50, 4)
            .await
            .unwrap()
            .is_empty());
        assert!(store
            .sample_frames_in_range(100, 50, 4)
            .await
            .unwrap()
            .is_empty());
        assert!(store
            .sample_frames_in_range(0, 100, 0)
            .await
            .unwrap()
            .is_empty());
    }
}

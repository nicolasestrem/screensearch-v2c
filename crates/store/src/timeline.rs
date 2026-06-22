//! Timeline density buckets: how many frames fall in each slice of a time window.
//! Backs the `get_timeline` command (`03 §7`) and is reused by the Insights
//! capture-over-time aggregate (`insights.rs`).

use rusqlite::params;
use traits::{Result, TimelineBucket};

use crate::SqliteStore;

impl SqliteStore {
    /// Counts frames per fixed-width bucket across the half-open window
    /// `[start, end)`, split into at most `bucket_count` buckets. Returns **sparse**
    /// buckets — only those that contain at least one frame, ascending by time — so
    /// the payload stays small over wide ranges; the caller fills the gaps with
    /// zero-count buckets when rendering.
    ///
    /// Bucket width is `ceil((end - start) / bucket_count)` ms (floored at 1 ms), so
    /// the final bucket covers `end`. An empty/invalid range (`end <= start`) or
    /// `bucket_count == 0` yields no buckets. Uses `idx_frames_captured_at`.
    pub async fn timeline_buckets(
        &self,
        start: i64,
        end: i64,
        bucket_count: u32,
    ) -> Result<Vec<TimelineBucket>> {
        if end <= start || bucket_count == 0 {
            return Ok(Vec::new());
        }
        let span = end - start;
        let n = i64::from(bucket_count);
        // Ceil division so the last bucket reaches `end`; floored at 1 ms so the
        // bucket width (and the SQL divisor) is always positive.
        let width = ((span + n - 1) / n).max(1);
        self.with_conn(move |conn| {
            // `?1`/`?2`/`?3` are reused positional params (start/width/end). The
            // integer bucket index `(captured_at - start) / width` groups frames into
            // fixed-width slots; sqlite integer division floors, matching the map below.
            let mut stmt = conn.prepare(
                "SELECT (captured_at - ?1) / ?2 AS bucket, COUNT(*) AS n
                 FROM frames
                 WHERE captured_at >= ?1 AND captured_at < ?3
                 GROUP BY bucket
                 ORDER BY bucket",
            )?;
            let rows = stmt
                .query_map(params![start, width, end], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let buckets = rows
                .into_iter()
                .map(|(b, count)| {
                    let b_start = start + b * width;
                    TimelineBucket {
                        start: b_start,
                        end: (b_start + width).min(end),
                        count: u32::try_from(count).unwrap_or(u32::MAX),
                    }
                })
                .collect();
            Ok(buckets)
        })
        .await
    }
}

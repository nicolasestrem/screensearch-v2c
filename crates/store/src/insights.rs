//! Activity insights: truthful aggregates over a time window for the Insights
//! screen (`get_insights`, P5). The spec defines no Insights contract — the shape
//! and these aggregates are the chosen default (logged in `07_KNOWN_GAPS.md`):
//! total + vision-tagged counts, capture density over time (reusing
//! [`SqliteStore::timeline_buckets`]), the top foreground apps, and the vision
//! activity-type breakdown. Every number is a real DB aggregate — never fabricated
//! (`UI_REFERENCE §4` honest-empty).

use rusqlite::params;
use traits::{ActivityCount, AppCount, InsightsSummary, Result};

use crate::SqliteStore;

/// Rows returned in the top-apps / activity breakdowns (a readable Insights list,
/// not the full long tail).
const TOP_N: i64 = 12;
impl SqliteStore {
    /// Aggregates frame activity over the half-open window `[start, end)`. Returns an
    /// empty summary (all zeros / empty lists) when the window holds no frames.
    pub async fn insights_summary(
        &self,
        start: i64,
        end: i64,
        bucket_count: u32,
    ) -> Result<InsightsSummary> {
        // Invalid or unrepresentable window → honest-empty summary up front, skipping
        // four queries (and a `timeline_buckets` call) that would all return
        // zero/empty anyway.
        if end <= start || end.checked_sub(start).is_none() || bucket_count == 0 {
            return Ok(InsightsSummary::default());
        }
        let captures = self.timeline_buckets(start, end, bucket_count).await?;
        self.with_conn(move |conn| {
            let total_frames: i64 = conn.query_row(
                "SELECT COUNT(*) FROM frames WHERE captured_at >= ?1 AND captured_at < ?2",
                params![start, end],
                |r| r.get(0),
            )?;
            let tagged_frames: i64 = conn.query_row(
                "SELECT COUNT(*) FROM frames
                 WHERE captured_at >= ?1 AND captured_at < ?2 AND activity_type IS NOT NULL",
                params![start, end],
                |r| r.get(0),
            )?;

            // Top foreground apps by frame count. A NULL `app_hint` groups into one
            // `app: None` row (frames with no resolved foreground window) — truthful,
            // labelled by the UI; ties break by name for a stable order.
            let mut app_stmt = conn.prepare(
                "SELECT app_hint, COUNT(*) AS n FROM frames
                 WHERE captured_at >= ?1 AND captured_at < ?2
                 GROUP BY app_hint ORDER BY n DESC, app_hint LIMIT ?3",
            )?;
            let top_apps = app_stmt
                .query_map(params![start, end, TOP_N], |r| {
                    Ok(AppCount {
                        app: r.get::<_, Option<String>>(0)?,
                        count: u32::try_from(r.get::<_, i64>(1)?).unwrap_or(u32::MAX),
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            // Vision activity-type breakdown — only frames that have been tagged
            // (untagged frames are excluded so the breakdown isn't dominated by NULL).
            let mut act_stmt = conn.prepare(
                "SELECT activity_type, COUNT(*) AS n FROM frames
                 WHERE captured_at >= ?1 AND captured_at < ?2 AND activity_type IS NOT NULL
                 GROUP BY activity_type ORDER BY n DESC, activity_type LIMIT ?3",
            )?;
            let activity_breakdown = act_stmt
                .query_map(params![start, end, TOP_N], |r| {
                    Ok(ActivityCount {
                        activity: r.get::<_, Option<String>>(0)?,
                        count: u32::try_from(r.get::<_, i64>(1)?).unwrap_or(u32::MAX),
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            Ok(InsightsSummary {
                total_frames,
                tagged_frames,
                captures,
                top_apps,
                activity_breakdown,
            })
        })
        .await
    }
}

//! Frame, OCR, and vision records — the always-on capture path's write targets
//! plus the assembled per-frame read used by the `get_frame` IPC command
//! (`03 §3/§4/§7`).

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};
use traits::{
    FrameDetail, FrameEnrichmentInput, NewFrame, OcrResult, Result, SuppressReason, TextRole,
    TextSource, TextSpan, VisionAnalysis,
};

use crate::schema::UNFILTERED_FILTER_VERSION;
use crate::SqliteStore;

impl SqliteStore {
    /// Inserts a captured frame, returning its new id (`03 §3`).
    pub async fn insert_frame(&self, f: NewFrame) -> Result<i64> {
        self.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO frames
                   (captured_at, monitor_index, width, height, image_path, content_hash,
                    app_hint, window_title, browser_url)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    f.captured_at,
                    f.monitor_index,
                    f.width,
                    f.height,
                    f.image_path,
                    f.content_hash,
                    f.app_hint,
                    f.window_title,
                    f.browser_url,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
        .await
    }

    /// Stores (or replaces) the text signal for a frame (`03 §3b`/`§4`): the
    /// `frame_text` row (raw + content text, both FTS-mirrored by the schema
    /// triggers) and the per-word `text_spans`. Atomic — all writes share one
    /// transaction, mirroring [`Self::insert_vision`].
    ///
    /// **Interim populator (PR2, `07` #51):** PR3's classifier isn't wired yet, so
    /// `content_text` is a passthrough copy of `raw_text` (the column is `NOT NULL`),
    /// `filter_version` is the unfiltered marker, and `suppressed_count` is 0. The
    /// foreground-window context already on the `frames` row (gap #12) is copied into
    /// `target_window_title` / `target_app_hint`. Re-OCR replaces spans wholesale
    /// (delete-then-insert) so it is idempotent.
    pub async fn insert_ocr(&self, frame_id: i64, ocr: OcrResult) -> Result<()> {
        self.with_conn(move |conn| {
            let tx = conn.unchecked_transaction()?;

            // Foreground-window metadata (gap #12); the frame row was inserted just
            // before this call (`03 §5`). A missing row leaves both NULL.
            let (target_app_hint, target_window_title): (Option<String>, Option<String>) = tx
                .query_row(
                    "SELECT app_hint, window_title FROM frames WHERE id = ?1",
                    params![frame_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?
                .unwrap_or((None, None));

            // `?2` (raw_text) is bound once and reused for content_text (passthrough).
            tx.execute(
                "INSERT INTO frame_text
                   (frame_id, raw_text, content_text, primary_source, filter_version,
                    suppressed_count, target_window_title, target_app_hint)
                 VALUES (?1, ?2, ?2, ?3, ?4, 0, ?5, ?6)
                 ON CONFLICT(frame_id) DO UPDATE SET
                   raw_text = excluded.raw_text,
                   content_text = excluded.content_text,
                   primary_source = excluded.primary_source,
                   filter_version = excluded.filter_version,
                   suppressed_count = excluded.suppressed_count,
                   target_window_title = excluded.target_window_title,
                   target_app_hint = excluded.target_app_hint",
                params![
                    frame_id,
                    ocr.text,
                    TextSource::Ocr.as_db_str(),
                    UNFILTERED_FILTER_VERSION,
                    target_window_title,
                    target_app_hint,
                ],
            )?;

            // Replace spans wholesale (idempotent re-OCR). Enum→DB token via the
            // shared `as_db_str` helpers; the CHECK constraints catch any drift.
            tx.execute(
                "DELETE FROM text_spans WHERE frame_id = ?1",
                params![frame_id],
            )?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO text_spans
                       (frame_id, span_index, text, normalized_text, source, role,
                        x, y, w, h, is_searchable, suppress_reason)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                )?;
                for (i, span) in ocr.spans.iter().enumerate() {
                    stmt.execute(params![
                        frame_id,
                        i as i64,
                        span.text,
                        span.normalized_text,
                        span.source.as_db_str(),
                        span.role.as_db_str(),
                        // Bind as f64 (the REAL column type) — portable across
                        // rusqlite versions; f32→f64→f32 is lossless.
                        span.x as f64,
                        span.y as f64,
                        span.w as f64,
                        span.h as f64,
                        span.is_searchable as i32,
                        span.suppress_reason.map(|r| r.as_db_str()),
                    ])?;
                }
            }

            tx.commit()?;
            Ok(())
        })
        .await
    }

    /// Stores (or replaces) the deferred vision analysis for a frame (`03 §5`).
    pub async fn insert_vision(&self, frame_id: i64, v: VisionAnalysis) -> Result<()> {
        self.with_conn(move |conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute(
                "INSERT INTO vision_analysis
                   (frame_id, description, activity_type, app_hint, confidence, model)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(frame_id) DO UPDATE SET
                   description = excluded.description,
                   activity_type = excluded.activity_type,
                   app_hint = excluded.app_hint,
                   confidence = excluded.confidence,
                   model = excluded.model",
                params![
                    frame_id,
                    v.description,
                    v.activity_type,
                    v.app_hint,
                    v.confidence,
                    v.model,
                ],
            )?;
            // 03 §4: frames.activity_type is "filled by vision" — mirror the
            // classification onto the frame so the timeline can filter by activity
            // without joining vision_analysis.
            tx.execute(
                "UPDATE frames SET activity_type = ?2 WHERE id = ?1",
                params![frame_id, v.activity_type],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    /// The minimal inputs the embedding worker needs to enrich a frame — the stored
    /// JPEG's relative path and the **content text** (if any) — in one round-trip, or
    /// `None` if the frame no longer exists (`03 §5`). Lighter than [`Self::get_frame`]
    /// (no vision/tags), so the worker doesn't pay for context it won't embed.
    ///
    /// Embeddings run over `content_text` (`03 §3b`); in PR2 that equals `raw_text`
    /// (passthrough), so behavior is unchanged until PR3's filter lands. The field is
    /// still called `ocr_text` on [`FrameEnrichmentInput`] to keep the worker stable.
    pub async fn frame_enrichment_input(
        &self,
        frame_id: i64,
    ) -> Result<Option<FrameEnrichmentInput>> {
        self.with_conn(move |conn| {
            let row = conn
                .query_row(
                    "SELECT f.image_path, ft.content_text
                     FROM frames f LEFT JOIN frame_text ft ON ft.frame_id = f.id
                     WHERE f.id = ?1",
                    params![frame_id],
                    |r| {
                        Ok(FrameEnrichmentInput {
                            image_path: r.get(0)?,
                            ocr_text: r.get(1)?,
                        })
                    },
                )
                .optional()?;
            Ok(row)
        })
        .await
    }

    /// Frame ids with no `vision_analysis` row yet **and** no `vision_tag` job in a
    /// `pending`/`running`/`dead` state, oldest first, capped at `limit`, optionally
    /// within `[start, end)` capture time. Feeds the timer/idle vision batch and the
    /// `enqueue_vision` range target (`03 §5`). Excluding `pending`/`running` stops a
    /// slow batch from being re-enqueued on the next tick and the timer/idle lanes from
    /// double-queuing the same frames; excluding `dead` stops a poisoned frame (its job
    /// exhausted retries without ever writing a `vision_analysis` row) from being
    /// re-enqueued every tick forever. A `done` job does **not** exclude a frame (a job
    /// that finished without persisting analysis is eligible to retry); on-demand
    /// single-frame re-tagging bypasses this query, so a dead frame can still be forced.
    pub async fn untagged_frame_ids(
        &self,
        limit: u32,
        range: Option<(i64, i64)>,
    ) -> Result<Vec<i64>> {
        self.with_conn(move |conn| {
            // Build the query once, appending the optional time-window predicate and
            // binding every value as an anonymous positional `?` (in push order). The
            // NOT EXISTS guard skips frames whose `vision_tag` job is still in flight.
            let mut sql = String::from(
                "SELECT f.id FROM frames f
                 LEFT JOIN vision_analysis v ON v.frame_id = f.id
                 WHERE v.frame_id IS NULL
                   AND NOT EXISTS (
                       SELECT 1 FROM jobs j
                       WHERE j.frame_id = f.id
                         AND j.kind = 'vision_tag'
                         AND j.state IN ('pending', 'running', 'dead')
                   )",
            );
            let mut args: Vec<i64> = Vec::new();
            if let Some((start, end)) = range {
                sql.push_str(" AND f.captured_at >= ? AND f.captured_at < ?");
                args.push(start);
                args.push(end);
            }
            sql.push_str(" ORDER BY f.captured_at ASC LIMIT ?");
            args.push(i64::from(limit));

            let mut stmt = conn.prepare(&sql)?;
            let ids = stmt
                .query_map(rusqlite::params_from_iter(args), |r| r.get::<_, i64>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(ids)
        })
        .await
    }

    /// Bulk-fetches **content text** for many frames in one `IN (…)` query (the `ask`
    /// grounding hydrate, `03 §7/§13.5`). Grounding uses content text (`03 §3b`); only
    /// frames with non-empty content are returned.
    pub async fn ocr_texts(&self, frame_ids: &[i64]) -> Result<HashMap<i64, String>> {
        if frame_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let ids = frame_ids.to_vec();
        self.with_conn(move |conn| {
            let placeholders = vec!["?"; ids.len()].join(",");
            let sql = format!(
                "SELECT frame_id, content_text FROM frame_text
                 WHERE frame_id IN ({placeholders}) AND content_text <> ''"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?;
            let mut map = HashMap::new();
            for row in rows {
                let (id, text) = row?;
                map.insert(id, text);
            }
            Ok(map)
        })
        .await
    }

    /// Assembles the full per-frame detail (frame context + raw/content text +
    /// vision + tags), or `None` if the frame does not exist. Backs the `get_frame`
    /// command (`03 §7`). Raw text stays viewable here even though retrieval defaults
    /// to content text (`03 §3b`).
    pub async fn get_frame(&self, frame_id: i64) -> Result<Option<FrameDetail>> {
        self.with_conn(move |conn| {
            let base = conn
                .query_row(
                    "SELECT captured_at, monitor_index, width, height, image_path,
                            app_hint, window_title, browser_url, activity_type
                     FROM frames WHERE id = ?1",
                    params![frame_id],
                    |r| {
                        Ok(FrameDetail {
                            frame_id,
                            captured_at: r.get(0)?,
                            monitor_index: r.get(1)?,
                            width: r.get(2)?,
                            height: r.get(3)?,
                            image_path: r.get(4)?,
                            app_hint: r.get(5)?,
                            window_title: r.get(6)?,
                            browser_url: r.get(7)?,
                            activity_type: r.get(8)?,
                            raw_text: None,
                            content_text: None,
                            text_source: TextSource::Ocr,
                            suppressed_text_count: 0,
                            vision: None,
                            tags: Vec::new(),
                        })
                    },
                )
                .optional()?;

            let Some(mut detail) = base else {
                return Ok(None);
            };

            if let Some((raw, content, source, suppressed)) = conn
                .query_row(
                    "SELECT raw_text, content_text, primary_source, suppressed_count
                     FROM frame_text WHERE frame_id = ?1",
                    params![frame_id],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, i64>(3)?,
                        ))
                    },
                )
                .optional()?
            {
                detail.raw_text = Some(raw);
                detail.content_text = Some(content);
                detail.text_source = TextSource::from_db_str(&source).unwrap_or(TextSource::Ocr);
                detail.suppressed_text_count = suppressed.max(0) as u32;
            }

            detail.vision = conn
                .query_row(
                    "SELECT description, activity_type, app_hint, confidence, model
                     FROM vision_analysis WHERE frame_id = ?1",
                    params![frame_id],
                    |r| {
                        Ok(VisionAnalysis {
                            description: r.get(0)?,
                            activity_type: r.get(1)?,
                            app_hint: r.get(2)?,
                            confidence: r.get(3)?,
                            model: r.get(4)?,
                        })
                    },
                )
                .optional()?;

            let mut stmt = conn.prepare(
                "SELECT t.name FROM frame_tags ft
                 JOIN tags t ON t.id = ft.tag_id
                 WHERE ft.frame_id = ?1 ORDER BY t.name",
            )?;
            detail.tags = stmt
                .query_map(params![frame_id], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            Ok(Some(detail))
        })
        .await
    }

    /// All text spans for a frame, ordered by `span_index` (`03 §3b`/`§4`). An
    /// inherent read that makes the `text_spans` write path observable — like
    /// [`Self::get_frame`] / `delete_frame` (`07` engineering note) — and the read
    /// PR3's classifier uses to recompute roles. Empty when the frame has no spans.
    pub async fn frame_spans(&self, frame_id: i64) -> Result<Vec<TextSpan>> {
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT text, normalized_text, source, role, x, y, w, h,
                        is_searchable, suppress_reason
                 FROM text_spans WHERE frame_id = ?1 ORDER BY span_index",
            )?;
            let spans = stmt
                .query_map(params![frame_id], |r| {
                    let source: String = r.get(2)?;
                    let role: String = r.get(3)?;
                    let suppress: Option<String> = r.get(9)?;
                    Ok(TextSpan {
                        text: r.get(0)?,
                        normalized_text: r.get(1)?,
                        source: TextSource::from_db_str(&source).unwrap_or(TextSource::Ocr),
                        role: TextRole::from_db_str(&role).unwrap_or(TextRole::Unknown),
                        x: r.get::<_, f64>(4)? as f32,
                        y: r.get::<_, f64>(5)? as f32,
                        w: r.get::<_, f64>(6)? as f32,
                        h: r.get::<_, f64>(7)? as f32,
                        is_searchable: r.get::<_, i64>(8)? != 0,
                        suppress_reason: suppress.as_deref().and_then(SuppressReason::from_db_str),
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(spans)
        })
        .await
    }
}

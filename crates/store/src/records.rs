//! Frame, OCR, and vision records — the always-on capture path's write targets
//! plus the assembled per-frame read used by the `get_frame` IPC command
//! (`03 §3/§4/§7`).

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};
use textfilter::{classify, reconcile, ClassifyInput, FilterConfig};
use traits::{
    AppSuppression, FrameDetail, FrameEnrichmentInput, NewFrame, OcrResult, Result, SuppressReason,
    TextFilterContext, TextRole, TextSource, TextSpan, VisionAnalysis,
};

use crate::schema::{FILTER_VERSION, UNFILTERED_FILTER_VERSION};
use crate::SqliteStore;

/// The `settings` key holding the active attention `filter_version` watermark, used by
/// [`SqliteStore::backfill_filter_version`]. Internal bookkeeping — deliberately not a
/// user-facing [`traits::Settings`] field.
const CATALOG_FILTER_VERSION_KEY: &str = "text.catalog_filter_version";

/// Frames re-cleaned per backfill transaction. Small enough to keep the write lock short
/// (a concurrent capture isn't blocked for long) and the rollback bounded; large enough
/// to amortize per-statement overhead. Internal tuning constant.
const BACKFILL_BATCH: usize = 64;

/// Loads the chrome catalog's prior `seen_count`s for the foreground `app_hint` in a
/// single query, returning the `signature -> seen_count` map the pure classifier reads
/// via [`textfilter::ChromeCatalog`] (`HashMap` already implements it). One bulk read
/// instead of one point-lookup per candidate line on the OCR hot path. Scoped to the
/// current app because every signature this frame can query is built with this
/// `app_hint` as its prefix (`app_hint IS ?1` matches the NULL app correctly, unlike
/// `=`). Reads see committed state (the per-frame bump happens after classify); a read
/// error → empty map (treat as all-unseen — never over-suppress on a transient
/// failure, the top risk).
fn load_chrome_catalog(
    conn: &rusqlite::Connection,
    app_hint: Option<&str>,
) -> rusqlite::Result<HashMap<String, u32>> {
    let mut catalog = HashMap::new();
    let mut stmt =
        conn.prepare("SELECT signature, seen_count FROM chrome_text_catalog WHERE app_hint IS ?1")?;
    let mut rows = stmt.query(params![app_hint])?;
    while let Some(row) = rows.next()? {
        catalog.insert(
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?.max(0) as u32,
        );
    }
    Ok(catalog)
}

/// Foreground-window context already on the `frames` row (`07` #12): `(app_hint,
/// window_title)`, both `None` if the row is missing.
fn frame_target_context(
    conn: &rusqlite::Connection,
    frame_id: i64,
) -> rusqlite::Result<(Option<String>, Option<String>)> {
    Ok(conn
        .query_row(
            "SELECT app_hint, window_title FROM frames WHERE id = ?1",
            params![frame_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .unwrap_or((None, None)))
}

/// Reads a frame's `text_spans` in `span_index` (reading) order, reconstructing each
/// [`TextSpan`] with its stored role/geometry. Sync (takes a borrowed connection or
/// transaction) so it can run inside the filter-version backfill transaction; the async
/// [`SqliteStore::frame_spans`] wraps it. Unknown role/source/suppress tokens fall back
/// conservatively (never lose a span).
fn read_text_spans(conn: &rusqlite::Connection, frame_id: i64) -> rusqlite::Result<Vec<TextSpan>> {
    let mut stmt = conn.prepare(
        "SELECT text, normalized_text, source, role, x, y, w, h,
                line_index, is_searchable, suppress_reason
         FROM text_spans WHERE frame_id = ?1 ORDER BY span_index",
    )?;
    let spans = stmt
        .query_map(params![frame_id], |r| {
            let source: String = r.get(2)?;
            let role: String = r.get(3)?;
            let suppress: Option<String> = r.get(10)?;
            Ok(TextSpan {
                text: r.get(0)?,
                normalized_text: r.get(1)?,
                source: TextSource::from_db_str(&source).unwrap_or(TextSource::Ocr),
                role: TextRole::from_db_str(&role).unwrap_or(TextRole::Unknown),
                x: r.get::<_, f64>(4)? as f32,
                y: r.get::<_, f64>(5)? as f32,
                w: r.get::<_, f64>(6)? as f32,
                h: r.get::<_, f64>(7)? as f32,
                line_index: r.get::<_, i64>(8)? as u32,
                is_searchable: r.get::<_, i64>(9)? != 0,
                suppress_reason: suppress.as_deref().and_then(SuppressReason::from_db_str),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(spans)
}

/// Replaces a frame's `text_spans` wholesale (idempotent re-OCR / re-filter). Shared by
/// [`SqliteStore::insert_ocr`] (passthrough, roles `unknown`) and
/// [`SqliteStore::insert_ocr_filtered`] (classified roles). Carries `line_index` so PR3
/// can reconstruct lines exactly (`03 §3b`).
fn replace_text_spans(
    tx: &rusqlite::Transaction<'_>,
    frame_id: i64,
    spans: &[TextSpan],
) -> rusqlite::Result<()> {
    tx.execute(
        "DELETE FROM text_spans WHERE frame_id = ?1",
        params![frame_id],
    )?;
    let mut stmt = tx.prepare(
        "INSERT INTO text_spans
           (frame_id, span_index, text, normalized_text, source, role,
            x, y, w, h, line_index, is_searchable, suppress_reason)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
    )?;
    for (i, span) in spans.iter().enumerate() {
        stmt.execute(params![
            frame_id,
            i as i64,
            span.text,
            span.normalized_text,
            span.source.as_db_str(),
            span.role.as_db_str(),
            // Bind as f64 (the REAL column type); f32→f64→f32 is lossless.
            span.x as f64,
            span.y as f64,
            span.w as f64,
            span.h as f64,
            span.line_index as i64,
            span.is_searchable as i32,
            span.suppress_reason.map(|r| r.as_db_str()),
        ])?;
    }
    Ok(())
}

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
            let (target_app_hint, target_window_title) = frame_target_context(conn, frame_id)?;

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

            // Replace spans wholesale (idempotent re-OCR); PR2 roles stay `unknown`.
            replace_text_spans(&tx, frame_id, &ocr.spans)?;

            tx.commit()?;
            Ok(())
        })
        .await
    }

    /// Inserts OCR **and** applies PR3's attention filter in one transaction
    /// (`03 §3b`, `docs/0.2.0.md` PR3). Classifies spans into roles via the pure
    /// [`textfilter`] crate, writes the **filtered** `content_text` directly (so the
    /// content FTS index is written once — no transient unfiltered window a concurrent
    /// search could match), stores the classified `text_spans`, and bumps the
    /// `chrome_text_catalog` for the signatures observed this frame. `raw_text` is
    /// always preserved; the foreground title/app come from the `frames` row (the rect
    /// and thresholds come from `ctx`). Because the embed worker reads `content_text`
    /// and is enqueued only after this commits, embeddings run over filtered text.
    pub async fn insert_ocr_filtered(
        &self,
        frame_id: i64,
        ocr: OcrResult,
        ctx: TextFilterContext,
    ) -> Result<()> {
        self.with_conn(move |conn| {
            let tx = conn.unchecked_transaction()?;
            let (app_hint, window_title) = frame_target_context(conn, frame_id)?;

            let config = FilterConfig {
                chrome_suppress_min_seen: ctx.chrome_suppress_min_seen,
                chrome_protect_min_chars: ctx.chrome_protect_min_chars,
                chrome_region_buckets: ctx.chrome_region_buckets,
            };
            let catalog = load_chrome_catalog(conn, app_hint.as_deref())?;
            let input = ClassifyInput {
                spans: &ocr.spans,
                target_rect: ctx.target_rect,
                target_window_title: window_title.as_deref(),
                target_app_hint: app_hint.as_deref(),
            };
            let out = classify(&input, &catalog, &config);

            // Single filtered write — content FTS mirror is written exactly once.
            tx.execute(
                "INSERT INTO frame_text
                   (frame_id, raw_text, content_text, primary_source, filter_version,
                    suppressed_count, target_window_title, target_app_hint)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
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
                    out.content_text,
                    TextSource::Ocr.as_db_str(),
                    FILTER_VERSION,
                    out.suppressed_count as i64,
                    window_title,
                    app_hint,
                ],
            )?;

            replace_text_spans(&tx, frame_id, &out.spans)?;

            // Bump the catalog for each candidate signature observed this frame. The
            // current appearance reaches `seen_count`, and `suppressed` is recomputed
            // against the configured threshold for observability / stats.
            {
                let min_seen = ctx.chrome_suppress_min_seen.max(1) as i64;
                let mut stmt = tx.prepare(
                    "INSERT INTO chrome_text_catalog
                       (signature, app_hint, region_bucket, normalized_text,
                        seen_count, first_seen_at, last_seen_at, suppressed)
                     VALUES (?1, ?2, ?3, ?4, 1, unixepoch()*1000, unixepoch()*1000,
                             CASE WHEN 1 >= ?5 THEN 1 ELSE 0 END)
                     ON CONFLICT(signature) DO UPDATE SET
                       seen_count   = seen_count + 1,
                       last_seen_at = unixepoch()*1000,
                       suppressed   = CASE WHEN seen_count + 1 >= ?5 THEN 1 ELSE 0 END",
                )?;
                for obs in &out.observed {
                    stmt.execute(params![
                        obs.signature,
                        obs.app_hint,
                        obs.region_bucket,
                        obs.normalized_text,
                        min_seen,
                    ])?;
                }
            }

            tx.commit()?;
            Ok(())
        })
        .await
    }

    /// Per-app text-filter suppression metric over frames classified by
    /// `filter_version` (`03 §3b`). `rate = suppressed_spans / total_spans` grouped by
    /// the **target** (foreground) app. Filtering on `filter_version` excludes interim
    /// passthrough frames so they can't dilute the rate to a misleading 0%.
    pub async fn text_filter_stats(&self, filter_version: i32) -> Result<Vec<AppSuppression>> {
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT ft.target_app_hint,
                        COUNT(*) AS total,
                        SUM(CASE WHEN ts.role IN ('chrome','system','background')
                                 THEN 1 ELSE 0 END) AS suppressed
                 FROM text_spans ts
                 JOIN frame_text ft ON ft.frame_id = ts.frame_id
                 WHERE ft.filter_version = ?1
                 GROUP BY ft.target_app_hint
                 ORDER BY suppressed DESC, total DESC",
            )?;
            let rows = stmt.query_map(params![filter_version], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })?;
            let mut out = Vec::new();
            for row in rows {
                let (app, total_spans, suppressed_spans) = row?;
                let rate = if total_spans > 0 {
                    suppressed_spans as f32 / total_spans as f32
                } else {
                    0.0
                };
                out.push(AppSuppression {
                    app,
                    total_spans,
                    suppressed_spans,
                    rate,
                });
            }
            Ok(out)
        })
        .await
    }

    /// Backfills the active attention `filter_version` (`03 §3b`,
    /// `docs/AUDIT_0.2.0_PR3_2026-06-26.md`). When the stored watermark differs from
    /// `current`, re-cleans every frame whose `filter_version < current` against the
    /// now-warm `chrome_text_catalog` via [`textfilter::reconcile`]: positional roles
    /// decided at capture are preserved, but a short repeated edge label that was kept
    /// during the catalog's cold-start window (before its signature crossed
    /// `chrome_suppress_min_seen`) is retroactively demoted to chrome and dropped from
    /// `content_text` (the content FTS re-syncs via its trigger). For frames whose
    /// `content_text` actually changed, an `embed_text` job is enqueued (when `reembed`)
    /// so the vector arm re-embeds from clean text; the embeddings `content_hash` makes
    /// an unchanged frame a no-op. Monotonic and idempotent (only ever suppresses more):
    /// each frame's `filter_version` advances as it is processed and the watermark is
    /// recorded only after the whole pass, so an interrupted run safely resumes. Runs in
    /// batched transactions ([`BACKFILL_BATCH`]) to bound the write lock. Returns the
    /// number of frames whose `content_text` changed; a no-op returning `0` when the
    /// watermark already equals `current`. Run once at startup.
    pub async fn backfill_filter_version(
        &self,
        current: i32,
        chrome_suppress_min_seen: u32,
        chrome_protect_min_chars: u32,
        chrome_region_buckets: u32,
        reembed: bool,
    ) -> Result<u64> {
        self.with_conn(move |conn| {
            let stored: Option<i32> = conn
                .query_row(
                    "SELECT value FROM settings WHERE key = ?1",
                    params![CATALOG_FILTER_VERSION_KEY],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .and_then(|v| v.parse().ok());
            if stored == Some(current) {
                return Ok(0u64);
            }

            let config = FilterConfig {
                chrome_suppress_min_seen,
                chrome_protect_min_chars,
                chrome_region_buckets,
            };

            // Frames still below the current filter version, oldest first.
            let frame_ids: Vec<i64> = {
                let mut stmt = conn.prepare(
                    "SELECT frame_id FROM frame_text WHERE filter_version < ?1 ORDER BY frame_id",
                )?;
                let ids = stmt
                    .query_map(params![current], |r| r.get::<_, i64>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                ids
            };

            let mut changed_total: u64 = 0;
            for batch in frame_ids.chunks(BACKFILL_BATCH) {
                let tx = conn.unchecked_transaction()?;
                for &fid in batch {
                    let (app_hint, old_content): (Option<String>, String) = tx.query_row(
                        "SELECT target_app_hint, content_text FROM frame_text WHERE frame_id = ?1",
                        params![fid],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )?;
                    let catalog = load_chrome_catalog(&tx, app_hint.as_deref())?;
                    let spans = read_text_spans(&tx, fid)?;
                    let out = reconcile(&spans, app_hint.as_deref(), &catalog, &config);

                    if out.content_text != old_content {
                        tx.execute(
                            "UPDATE frame_text
                             SET content_text = ?2, suppressed_count = ?3, filter_version = ?4
                             WHERE frame_id = ?1",
                            params![fid, out.content_text, out.suppressed_count as i64, current],
                        )?;
                        replace_text_spans(&tx, fid, &out.spans)?;
                        changed_total += 1;
                        if reembed {
                            tx.execute(
                                "INSERT INTO jobs (kind, frame_id, priority, max_attempts, not_before)
                                 VALUES ('embed_text', ?1, 0, 3, 0)",
                                params![fid],
                            )?;
                        }
                    } else {
                        // No chrome to drop — just advance the version so it isn't rescanned.
                        tx.execute(
                            "UPDATE frame_text SET filter_version = ?2 WHERE frame_id = ?1",
                            params![fid, current],
                        )?;
                    }
                }
                tx.commit()?;
            }

            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![CATALOG_FILTER_VERSION_KEY, current.to_string()],
            )?;
            Ok(changed_total)
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
        self.with_conn(move |conn| Ok(read_text_spans(conn, frame_id)?))
            .await
    }
}

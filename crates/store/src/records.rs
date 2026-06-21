//! Frame, OCR, and vision records — the always-on capture path's write targets
//! plus the assembled per-frame read used by the `get_frame` IPC command
//! (`03 §3/§4/§7`).

use rusqlite::{params, OptionalExtension};
use traits::{FrameDetail, NewFrame, OcrResult, Result, VisionAnalysis};

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

    /// Stores (or replaces) the OCR text for a frame. The FTS5 mirror is kept in
    /// sync by the schema triggers (`03 §4`).
    pub async fn insert_ocr(&self, frame_id: i64, ocr: OcrResult) -> Result<()> {
        self.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO ocr_text (frame_id, text, mean_confidence, engine)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(frame_id) DO UPDATE SET
                   text = excluded.text,
                   mean_confidence = excluded.mean_confidence,
                   engine = excluded.engine",
                params![frame_id, ocr.text, ocr.mean_confidence, ocr.engine],
            )?;
            Ok(())
        })
        .await
    }

    /// Stores (or replaces) the deferred vision analysis for a frame (`03 §5`).
    pub async fn insert_vision(&self, frame_id: i64, v: VisionAnalysis) -> Result<()> {
        self.with_conn(move |conn| {
            conn.execute(
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
            Ok(())
        })
        .await
    }

    /// Assembles the full per-frame detail (frame context + OCR text + vision +
    /// tags), or `None` if the frame does not exist. Backs the `get_frame`
    /// command (`03 §7`).
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
                            text: None,
                            vision: None,
                            tags: Vec::new(),
                        })
                    },
                )
                .optional()?;

            let Some(mut detail) = base else {
                return Ok(None);
            };

            detail.text = conn
                .query_row(
                    "SELECT text FROM ocr_text WHERE frame_id = ?1",
                    params![frame_id],
                    |r| r.get::<_, String>(0),
                )
                .optional()?;

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
}

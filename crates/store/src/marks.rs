//! Marks — user-flagged moments with an optional intention note (`03 §4`/`§7b`,
//! 0.3.0 PR6). Backs the `add_mark` / `list_marks` / `resolve_mark` / `set_mark_note`
//! IPC commands. A mark is a child of one frame (`ON DELETE CASCADE`), so listing
//! marks joins `frames` for the thumbnail + context the Intentions strip renders (no
//! N+1 `get_frame`); the join is total while the frame row exists, and a
//! retention-degraded frame keeps its row (image_purged), so a mark survives image
//! expiry (`03 §4`, D10).

use rusqlite::{params, Row};
use traits::{Mark, Result};

use crate::SqliteStore;

/// Maps a `list_marks` join row to a [`Mark`]. Column order is defined here once.
fn row_to_mark(r: &Row<'_>) -> rusqlite::Result<Mark> {
    Ok(Mark {
        mark_id: r.get(0)?,
        frame_id: r.get(1)?,
        created_at: r.get(2)?,
        note: r.get(3)?,
        resolved_at: r.get(4)?,
        captured_at: r.get(5)?,
        image_path: r.get(6)?,
        image_purged: r.get::<_, i64>(7)? != 0,
        app_hint: r.get(8)?,
        window_title: r.get(9)?,
    })
}

impl SqliteStore {
    /// Inserts a mark on `frame_id` and returns the new `mark_id` (`03 §4`/`§7b`).
    /// `created_at` is the caller's clock (unix epoch ms). A `frame_id` that doesn't
    /// exist trips the foreign key and maps to a clear "frame not found" error rather
    /// than a raw SQLite constraint string (FK enforcement is ON per connection —
    /// `open_connection`).
    pub async fn insert_mark(
        &self,
        frame_id: i64,
        created_at: i64,
        note: Option<String>,
    ) -> Result<i64> {
        self.with_conn(move |conn| {
            let res = conn.execute(
                "INSERT INTO marks (frame_id, created_at, note) VALUES (?1, ?2, ?3)",
                params![frame_id, created_at, note],
            );
            match res {
                Ok(_) => Ok(conn.last_insert_rowid()),
                Err(rusqlite::Error::SqliteFailure(e, _))
                    if e.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    Err(anyhow::anyhow!("frame {frame_id} not found"))
                }
                Err(e) => Err(e.into()),
            }
        })
        .await
    }

    /// All marks, **unresolved first then newest-first within each group** (`03 §7`,
    /// index-served by `idx_marks_open`). Joins `frames` for the display fields; the
    /// join drops a mark only if its frame is hard-deleted (which cascades the mark
    /// away too), so it never hides a live mark.
    pub async fn list_marks(&self) -> Result<Vec<Mark>> {
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT m.id, m.frame_id, m.created_at, m.note, m.resolved_at,
                        f.captured_at, f.image_path, f.image_purged, f.app_hint, f.window_title
                 FROM marks m
                 JOIN frames f ON f.id = m.frame_id
                 ORDER BY (m.resolved_at IS NOT NULL) ASC, m.created_at DESC, m.id DESC",
            )?;
            let rows = stmt
                .query_map([], row_to_mark)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .await
    }

    /// Resolves a mark at `resolved_at` (resolve = done, dismiss = resolve-with-no-
    /// action — same op, `03 §7b`). **Idempotent:** resolving an already-resolved mark
    /// is a no-op (`Ok`), so a double-click or a stale UI never errors; an unknown
    /// `mark_id` is a real error (nothing to resolve).
    pub async fn resolve_mark(&self, mark_id: i64, resolved_at: i64) -> Result<()> {
        self.with_conn(move |conn| {
            let changed = conn.execute(
                "UPDATE marks SET resolved_at = ?2 WHERE id = ?1 AND resolved_at IS NULL",
                params![mark_id, resolved_at],
            )?;
            if changed == 0 {
                // Either the mark is already resolved (idempotent no-op) or it doesn't
                // exist (a real error). Disambiguate with an existence check.
                let exists: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM marks WHERE id = ?1)",
                    params![mark_id],
                    |r| r.get(0),
                )?;
                if !exists {
                    return Err(anyhow::anyhow!("mark {mark_id} not found"));
                }
            }
            Ok(())
        })
        .await
    }

    /// Sets (or overwrites) the optional one-line note on an existing mark
    /// (`03 §7`). The mark is created at hotkey-press time; the note arrives seconds
    /// later from the confirmation toast. An unknown `mark_id` errors.
    pub async fn set_mark_note(&self, mark_id: i64, note: &str) -> Result<()> {
        let note = note.to_string();
        self.with_conn(move |conn| {
            let changed = conn.execute(
                "UPDATE marks SET note = ?2 WHERE id = ?1",
                params![mark_id, note],
            )?;
            if changed == 0 {
                return Err(anyhow::anyhow!("mark {mark_id} not found"));
            }
            Ok(())
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use crate::SqliteStore;
    use traits::NewFrame;

    fn frame(at: i64, app: &str, title: &str) -> NewFrame {
        NewFrame {
            captured_at: at,
            monitor_index: 0,
            width: 1920,
            height: 1080,
            image_path: format!("frames/{at}.webp"),
            content_hash: format!("h{at}"),
            app_hint: Some(app.to_string()),
            window_title: Some(title.to_string()),
            browser_url: None,
            capture_trigger: Some(traits::CaptureTrigger::Manual),
        }
    }

    #[tokio::test]
    async fn insert_rejects_unknown_frame_with_clear_error() {
        let store = SqliteStore::open_in_memory().unwrap();
        let err = store.insert_mark(999, 100, None).await.unwrap_err();
        assert!(
            err.to_string().contains("frame 999 not found"),
            "unknown frame_id must map to a clear error, got: {err}"
        );
    }

    #[tokio::test]
    async fn list_orders_unresolved_first_then_newest_first() {
        let store = SqliteStore::open_in_memory().unwrap();
        let f1 = store
            .insert_frame(frame(10, "VS Code", "repo"))
            .await
            .unwrap();
        let f2 = store
            .insert_frame(frame(20, "Firefox", "docs"))
            .await
            .unwrap();
        let f3 = store
            .insert_frame(frame(30, "Slack", "team"))
            .await
            .unwrap();

        // Two unresolved (created at 100 then 200) and one resolved.
        let m_old = store.insert_mark(f1, 100, None).await.unwrap();
        let m_new = store.insert_mark(f2, 200, None).await.unwrap();
        let m_res = store.insert_mark(f3, 150, None).await.unwrap();
        store.resolve_mark(m_res, 300).await.unwrap();

        let marks = store.list_marks().await.unwrap();
        let ids: Vec<i64> = marks.iter().map(|m| m.mark_id).collect();
        // Unresolved first (newest-first within: m_new before m_old), then the resolved.
        assert_eq!(ids, vec![m_new, m_old, m_res]);
        // The resolved mark carries its resolved_at; the unresolved ones don't.
        assert_eq!(marks[0].resolved_at, None);
        assert_eq!(marks[2].resolved_at, Some(300));
        // Joined frame fields are present for display.
        assert_eq!(marks[0].app_hint.as_deref(), Some("Firefox"));
        assert_eq!(marks[0].window_title.as_deref(), Some("docs"));
        assert_eq!(marks[0].captured_at, 20);
    }

    #[tokio::test]
    async fn set_note_round_trips_and_rejects_unknown() {
        let store = SqliteStore::open_in_memory().unwrap();
        let f = store
            .insert_frame(frame(10, "VS Code", "repo"))
            .await
            .unwrap();
        let m = store.insert_mark(f, 100, None).await.unwrap();
        store.set_mark_note(m, "call the plumber").await.unwrap();

        let marks = store.list_marks().await.unwrap();
        assert_eq!(marks[0].note.as_deref(), Some("call the plumber"));

        let err = store.set_mark_note(999, "nope").await.unwrap_err();
        assert!(err.to_string().contains("mark 999 not found"));
    }

    #[tokio::test]
    async fn resolve_is_idempotent_but_errors_on_unknown() {
        let store = SqliteStore::open_in_memory().unwrap();
        let f = store
            .insert_frame(frame(10, "VS Code", "repo"))
            .await
            .unwrap();
        let m = store.insert_mark(f, 100, None).await.unwrap();

        store.resolve_mark(m, 200).await.unwrap();
        // Re-resolving is a no-op (Ok), and the first resolved_at is preserved.
        store.resolve_mark(m, 999).await.unwrap();
        let marks = store.list_marks().await.unwrap();
        assert_eq!(marks[0].resolved_at, Some(200));

        // An unknown mark is a real error.
        let err = store.resolve_mark(12345, 200).await.unwrap_err();
        assert!(err.to_string().contains("mark 12345 not found"));
    }

    #[tokio::test]
    async fn mark_survives_image_purge_with_text_kept() {
        let store = SqliteStore::open_in_memory().unwrap();
        let f = store
            .insert_frame(frame(10, "VS Code", "repo"))
            .await
            .unwrap();
        let m = store
            .insert_mark(f, 100, Some("finish PR6".to_string()))
            .await
            .unwrap();

        // Retention degrades the frame's image to text: the row survives (D10).
        store.degrade_frame_to_text(f).await.unwrap();

        let marks = store.list_marks().await.unwrap();
        assert_eq!(marks.len(), 1, "the mark survives image expiry");
        assert_eq!(marks[0].mark_id, m);
        assert!(marks[0].image_purged, "the frame now reads as image-purged");
        assert_eq!(marks[0].note.as_deref(), Some("finish PR6"));
    }
}

//! Read-only export of frame metadata + marks from the live SQLite DB, the `suggest-days`
//! survey, and the D5 `VACUUM INTO` backup.
//!
//! **Read-only discipline.** [`open_readonly`] opens with `SQLITE_OPEN_READ_ONLY` and sets
//! `PRAGMA query_only`; every query path uses it. The only file this module writes is the
//! backup target ([`backup`], a `VACUUM INTO` to a fresh file outside the repo and the app
//! data dir) and the per-day export files under a caller-chosen output directory.
//!
//! **Schema-drift guard.** The crate is a standalone referee (no `store` dependency), so the
//! frames/marks DDL is embedded for the test fixture only (kept in sync with
//! `crates/store/src/schema.rs`, schema version 10). Against a real DB, the export queries
//! name their exact columns, so any future schema change fails loudly ("no such column")
//! rather than silently mis-reading.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use rusqlite::{Connection, OpenFlags};

use crate::digest::{render_digest, render_labels_template};
use crate::model::{DayHeader, ExportFrame, ExportMark};

/// One day's line in the `suggest-days` survey. The AI / meeting columns are coarse
/// window-title `LIKE` signals (not authoritative recognition) — enough to pick a
/// representative sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaySurvey {
    pub day: String,
    pub frames: i64,
    pub distinct_apps: i64,
    pub ai_title_frames: i64,
    pub meeting_title_frames: i64,
    pub marks: i64,
}

/// Local-day bounds resolved from SQLite's timezone math (no chrono).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayBounds {
    pub local_midnight_ms: i64,
    pub utc_offset_min: i64,
    pub day_end_ms: i64,
}

/// The default live DB path: `%APPDATA%\app.screensearchv2c.desktop\screensearch.db`
/// (`src-tauri/src/lib.rs`). `None` if `APPDATA` is unset.
pub fn default_db_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(
        Path::new(&appdata)
            .join("app.screensearchv2c.desktop")
            .join("screensearch.db"),
    )
}

/// Open the DB **read-only** with `query_only` + a bounded busy timeout. Maps the WAL-recovery
/// failure (app killed, `-wal` needs replay, no live `-shm`) to an actionable message rather
/// than a raw SQLite code.
pub fn open_readonly(path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("opening {} read-only", path.display()))?;
    conn.busy_timeout(Duration::from_millis(5000))?;
    conn.pragma_update(None, "query_only", true)?;
    // Probe: a read-only connection cannot replay a WAL that needs recovery; surface the fix.
    if let Err(e) = conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    }) {
        let msg = e.to_string();
        if msg.contains("readonly") || msg.contains("recovery") || msg.contains("locking") {
            bail!(
                "{path}: read-only open cannot recover the write-ahead log \u{2014} the app was \
                 likely killed with an unflushed -wal. Start the app once (or close it cleanly), \
                 or point --db at the D5 backup copy. (underlying: {msg})",
                path = path.display()
            );
        }
        return Err(e).context("probing the database");
    }
    Ok(conn)
}

/// Resolve a `"YYYY-MM-DD"` local day to epoch-ms bounds, asserting a 24 h span (a
/// DST-transition day is 23/25 h, which breaks label `HH:MM` arithmetic — refuse it loudly).
pub fn day_bounds(conn: &Connection, date: &str) -> Result<DayBounds> {
    let local_midnight_ms: i64 = conn
        .query_row(
            "SELECT unixepoch(?1 || ' 00:00:00', 'utc') * 1000",
            [date],
            |r| r.get(0),
        )
        .with_context(|| format!("resolving local midnight for {date}"))?;
    let day_end_ms: i64 = conn.query_row(
        "SELECT unixepoch(date(?1, '+1 day') || ' 00:00:00', 'utc') * 1000",
        [date],
        |r| r.get(0),
    )?;
    let utc_offset_min: i64 = conn.query_row(
        "SELECT (unixepoch(?1 || ' 00:00:00') - unixepoch(?1 || ' 00:00:00', 'utc')) / 60",
        [date],
        |r| r.get(0),
    )?;
    if day_end_ms - local_midnight_ms != 86_400_000 {
        bail!(
            "{date} is not a 24h local day ({} h) \u{2014} a DST-transition day breaks HH:MM \
             label arithmetic. Pick a different sample day (June\u{2013}July days are safe).",
            (day_end_ms - local_midnight_ms) / 3_600_000
        );
    }
    Ok(DayBounds {
        local_midnight_ms,
        utc_offset_min,
        day_end_ms,
    })
}

/// Per-day survey over the whole DB, for picking the 5\u{2013}10 sample days.
pub fn survey(conn: &Connection) -> Result<Vec<DaySurvey>> {
    let mut stmt = conn.prepare(
        "SELECT date(captured_at/1000,'unixepoch','localtime') AS day,
                COUNT(*),
                COUNT(DISTINCT app_hint),
                SUM(CASE WHEN lower(coalesce(window_title,'')) LIKE '%claude%'
                          OR lower(coalesce(window_title,'')) LIKE '%codex%'
                          OR lower(coalesce(window_title,'')) LIKE '%chatgpt%'
                          OR lower(coalesce(window_title,'')) LIKE '%gemini%' THEN 1 ELSE 0 END),
                SUM(CASE WHEN lower(coalesce(window_title,'')) LIKE '%zoom%'
                          OR lower(coalesce(window_title,'')) LIKE '%teams%'
                          OR lower(coalesce(window_title,'')) LIKE '%meet%'
                          OR lower(coalesce(window_title,'')) LIKE '%webex%' THEN 1 ELSE 0 END)
         FROM frames GROUP BY day ORDER BY day",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(DaySurvey {
            day: r.get(0)?,
            frames: r.get(1)?,
            distinct_apps: r.get(2)?,
            ai_title_frames: r.get(3)?,
            meeting_title_frames: r.get(4)?,
            marks: 0,
        })
    })?;
    let mut out: Vec<DaySurvey> = rows.collect::<rusqlite::Result<_>>()?;

    // Marks per local day, merged in.
    let mut mstmt = conn.prepare(
        "SELECT date(created_at/1000,'unixepoch','localtime') AS day, COUNT(*)
         FROM marks GROUP BY day",
    )?;
    let marks: Vec<(String, i64)> = mstmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (day, n) in marks {
        if let Some(d) = out.iter_mut().find(|d| d.day == day) {
            d.marks = n;
        }
    }
    Ok(out)
}

/// The frames of one local day, ordered by `(captured_at, id)`. Uses the `captured_at` index
/// via ms bounds rather than a per-row `date()` call.
pub fn day_frames(conn: &Connection, bounds: DayBounds) -> Result<Vec<ExportFrame>> {
    let mut stmt = conn.prepare(
        "SELECT id, captured_at, app_hint, window_title, browser_url, capture_trigger
         FROM frames WHERE captured_at >= ?1 AND captured_at < ?2
         ORDER BY captured_at, id",
    )?;
    let rows = stmt.query_map([bounds.local_midnight_ms, bounds.day_end_ms], |r| {
        Ok(ExportFrame {
            frame_id: r.get(0)?,
            captured_at: r.get(1)?,
            app_hint: r.get(2)?,
            window_title: r.get(3)?,
            browser_url: r.get(4)?,
            capture_trigger: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The marks of one local day (by `created_at`), ordered by `(created_at, id)`.
pub fn day_marks(conn: &Connection, bounds: DayBounds) -> Result<Vec<ExportMark>> {
    let mut stmt = conn.prepare(
        "SELECT id, frame_id, created_at, note, resolved_at
         FROM marks WHERE created_at >= ?1 AND created_at < ?2
         ORDER BY created_at, id",
    )?;
    let rows = stmt.query_map([bounds.local_midnight_ms, bounds.day_end_ms], |r| {
        Ok(ExportMark {
            mark_id: r.get(0)?,
            frame_id: r.get(1)?,
            created_at: r.get(2)?,
            note: r.get(3)?,
            resolved_at: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Total (frames, marks) row counts, for backup verification.
pub fn table_counts(conn: &Connection) -> Result<(i64, i64)> {
    let frames: i64 = conn.query_row("SELECT COUNT(*) FROM frames", [], |r| r.get(0))?;
    let marks: i64 = conn.query_row("SELECT COUNT(*) FROM marks", [], |r| r.get(0))?;
    Ok((frames, marks))
}

/// Today's local date `"YYYY-MM-DD"` per the DB's clock (avoids a date library in the bin).
pub fn today_local(conn: &Connection) -> Result<String> {
    Ok(
        conn.query_row("SELECT strftime('%Y-%m-%d','now','localtime')", [], |r| {
            r.get(0)
        })?,
    )
}

/// The outcome of writing one day's export directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayExport {
    pub dir: PathBuf,
    /// True when an existing hand-edited `labels.toml` was kept instead of being
    /// overwritten with a fresh template (a re-export of an already-labeled day).
    pub labels_preserved: bool,
}

/// Export one day's frames + marks + digest + labels template into `out_dir/<date>/`.
///
/// Regenerated data (`day.json`, `frames.jsonl`, `marks.jsonl`, `digest.md`) is always
/// rewritten, but an existing `labels.toml` is preserved: re-exporting a day to refresh
/// its frames or digest must never clobber the hand-edited ground truth the scoring and
/// sweep steps depend on. The template is written only when no labels file is present.
pub fn write_day(
    out_dir: &Path,
    date: &str,
    bounds: DayBounds,
    conn: &Connection,
) -> Result<DayExport> {
    let frames = day_frames(conn, bounds)?;
    let marks = day_marks(conn, bounds)?;
    let header = DayHeader {
        schema: "harness-day-v1".into(),
        date: date.to_string(),
        local_midnight_ms: bounds.local_midnight_ms,
        utc_offset_min: bounds.utc_offset_min,
        frame_count: frames.len() as i64,
        mark_count: marks.len() as i64,
    };
    let dir = out_dir.join(date);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    std::fs::write(dir.join("day.json"), serde_json::to_string_pretty(&header)?)?;
    write_jsonl(&dir.join("frames.jsonl"), &frames)?;
    write_jsonl(&dir.join("marks.jsonl"), &marks)?;
    std::fs::write(
        dir.join("digest.md"),
        render_digest(&header, &frames, &marks),
    )?;
    let labels_path = dir.join("labels.toml");
    let labels_preserved = labels_path.exists();
    if !labels_preserved {
        std::fs::write(
            &labels_path,
            render_labels_template(date, &marks, bounds.local_midnight_ms),
        )?;
    }
    Ok(DayExport {
        dir,
        labels_preserved,
    })
}

fn write_jsonl<T: serde::Serialize>(path: &Path, rows: &[T]) -> Result<()> {
    let mut s = String::new();
    for row in rows {
        s.push_str(&serde_json::to_string(row)?);
        s.push('\n');
    }
    std::fs::write(path, s).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// The result of a D5 backup, carrying the attestation evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupReport {
    pub dest: PathBuf,
    pub src_frames: i64,
    pub dst_frames: i64,
    pub src_marks: i64,
    pub dst_marks: i64,
    pub integrity_ok: bool,
}

/// Walk up from `start` to the git working-tree root (the first ancestor holding a `.git`
/// entry; `.git` is a dir in a normal clone and a file in a worktree, so `exists()` covers
/// both). None when `start` is not inside a git tree.
fn git_root(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    while let Some(dir) = cur {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    None
}

/// Take a consistent `VACUUM INTO` snapshot of `src` into `to_dir/screensearch-<ymd>.db`.
/// Refuses to overwrite, and refuses a destination inside the repo tree or the app data dir.
/// Reads the source read-only; the source file is never modified.
pub fn backup(src: &Path, to_dir: &Path, ymd: &str) -> Result<BackupReport> {
    std::fs::create_dir_all(to_dir).with_context(|| format!("creating {}", to_dir.display()))?;
    let to_abs = to_dir
        .canonicalize()
        .with_context(|| format!("resolving {}", to_dir.display()))?;

    if let Ok(cwd) = std::env::current_dir() {
        if let Ok(cwd) = cwd.canonicalize() {
            // Guard the whole repo tree, not just the current directory: running from a
            // subdirectory (e.g. crates/harness) must still reject a `--to` that lands anywhere
            // under the repo root (e.g. ../../backups). Fall back to CWD when not in a git tree.
            let repo_root = git_root(&cwd).unwrap_or(cwd);
            if to_abs.starts_with(&repo_root) {
                bail!(
                    "backup destination {} is inside the repo tree ({}) \u{2014} the backup must \
                     live outside the repo (it is personal data and a safety copy)",
                    to_abs.display(),
                    repo_root.display()
                );
            }
        }
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let app_dir = Path::new(&appdata).join("app.screensearchv2c.desktop");
        if let Ok(app_dir) = app_dir.canonicalize() {
            if to_abs.starts_with(&app_dir) {
                bail!(
                    "backup destination {} is inside the app data dir \u{2014} the D5 backup must \
                     live OUTSIDE {} so it survives a bad migration",
                    to_abs.display(),
                    app_dir.display()
                );
            }
        }
    }

    let dest = to_abs.join(format!("screensearch-{ymd}.db"));
    if dest.exists() {
        bail!(
            "backup destination {} already exists \u{2014} refusing to overwrite",
            dest.display()
        );
    }

    // Read-only source; VACUUM INTO only reads the source and writes the fresh target.
    let conn = Connection::open_with_flags(src, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("opening {} read-only for backup", src.display()))?;
    let (src_frames, src_marks) = table_counts(&conn)?;
    conn.execute("VACUUM INTO ?1", [dest.to_string_lossy().as_ref()])
        .context("VACUUM INTO backup target")?;
    drop(conn);

    // Verify the copy independently.
    let dst = open_readonly(&dest)?;
    let integrity_ok: String = dst
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap_or_default();
    let (dst_frames, dst_marks) = table_counts(&dst)?;
    if dst_frames != src_frames || dst_marks != src_marks {
        bail!(
            "backup verification failed: source has {src_frames} frames / {src_marks} marks, \
             copy has {dst_frames} / {dst_marks}"
        );
    }
    Ok(BackupReport {
        dest,
        src_frames,
        dst_frames,
        src_marks,
        dst_marks,
        integrity_ok: integrity_ok == "ok",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Frames + marks DDL, schema version 10, copied verbatim (subset) from
    // crates/store/src/schema.rs (frames final shape after the capture_trigger migration,
    // and the marks table). Keep in sync: if PR3+ changes these columns, this fixture and the
    // export queries must move together (the export queries name their columns, so a real-DB
    // drift fails loudly regardless).
    const FIXTURE_DDL: &str = r#"
CREATE TABLE frames (
  id            INTEGER PRIMARY KEY,
  captured_at   INTEGER NOT NULL,
  monitor_index INTEGER NOT NULL,
  width         INTEGER NOT NULL,
  height        INTEGER NOT NULL,
  image_path    TEXT    NOT NULL,
  content_hash  TEXT    NOT NULL,
  app_hint      TEXT, window_title TEXT, browser_url TEXT,
  activity_type TEXT,
  created_at    INTEGER NOT NULL DEFAULT (unixepoch()*1000),
  capture_trigger TEXT
    CHECK (capture_trigger IS NULL
           OR capture_trigger IN ('timer','idle','foreground_change','clipboard_change',
                                  'typing_pause','click','scroll_stop','manual'))
);
CREATE INDEX idx_frames_captured_at ON frames(captured_at);
CREATE TABLE marks (
  id          INTEGER PRIMARY KEY,
  frame_id    INTEGER NOT NULL REFERENCES frames(id) ON DELETE CASCADE,
  created_at  INTEGER NOT NULL,
  note        TEXT,
  resolved_at INTEGER
);
"#;

    // Local-midnight epoch (ms) for the fixture day, derived at runtime from the process
    // timezone via the same query `day_bounds` uses. NOT hardcoded: `cargo test` must pass on
    // any runner (CI is UTC, dev may be Europe/Paris), so frames placed at `day_mid() + N`
    // always fall inside the queried day regardless of the active timezone. July 1 is never a
    // DST-transition day, so the 24h invariant holds everywhere.
    fn day_mid() -> i64 {
        let c = Connection::open_in_memory().unwrap();
        day_bounds(&c, "2026-07-01").unwrap().local_midnight_ms
    }

    fn insert_frame(
        c: &Connection,
        id: i64,
        min: i64,
        monitor: i64,
        app: Option<&str>,
        title: Option<&str>,
    ) {
        c.execute(
            "INSERT INTO frames
             (id, captured_at, monitor_index, width, height, image_path, content_hash,
              app_hint, window_title, browser_url, capture_trigger)
             VALUES (?1, ?2, ?3, 1920, 1080, 'f.webp', 'h', ?4, ?5, NULL, 'timer')",
            rusqlite::params![id, day_mid() + min * 60_000, monitor, app, title],
        )
        .unwrap();
    }

    /// A writable fixture DB on disk (tempfile), seeded with one day of frames + a mark.
    fn fixture(dir: &Path) -> PathBuf {
        let path = dir.join("screensearch.db");
        let c = Connection::open(&path).unwrap();
        c.execute_batch(FIXTURE_DDL).unwrap();
        // Two monitors share captured_at at t=0 (a dual-monitor cycle).
        insert_frame(&c, 1, 0, 0, Some("Code"), Some("a.rs - VS Code"));
        insert_frame(&c, 2, 0, 1, Some("Code"), Some("a.rs - VS Code"));
        insert_frame(&c, 3, 3, 0, Some("Code"), Some("b.rs - VS Code"));
        insert_frame(
            &c,
            4,
            10,
            0,
            Some("chrome"),
            Some("ChatGPT - Google Chrome"),
        );
        insert_frame(&c, 5, 20, 0, None, None); // keyless
        c.execute(
            "INSERT INTO marks (id, frame_id, created_at, note, resolved_at)
             VALUES (10, 3, ?1, 'fix the CI', NULL)",
            [day_mid() + 3 * 60_000],
        )
        .unwrap();
        path
    }

    #[test]
    fn day_bounds_are_24h_and_offset_is_consistent() {
        let tmp = tempfile::tempdir().unwrap();
        let path = fixture(tmp.path());
        let c = open_readonly(&path).unwrap();
        let b = day_bounds(&c, "2026-07-01").unwrap();
        assert_eq!(b.local_midnight_ms, day_mid());
        assert_eq!(b.day_end_ms - b.local_midnight_ms, 86_400_000);
        // Timezone-agnostic: local midnight shifted by the reported offset lands on a UTC
        // midnight (divisible by a whole day). Holds for Paris (+120), UTC (0), or any runner.
        assert_eq!(
            (b.local_midnight_ms + b.utc_offset_min * 60_000).rem_euclid(86_400_000),
            0
        );
    }

    #[test]
    fn day_frames_ordered_with_multimonitor_and_keyless() {
        let tmp = tempfile::tempdir().unwrap();
        let path = fixture(tmp.path());
        let c = open_readonly(&path).unwrap();
        let b = day_bounds(&c, "2026-07-01").unwrap();
        let frames = day_frames(&c, b).unwrap();
        assert_eq!(frames.len(), 5);
        // ordered by (captured_at, id): the two t=0 monitors first, by id.
        assert_eq!(frames[0].frame_id, 1);
        assert_eq!(frames[1].frame_id, 2);
        assert_eq!(frames[0].captured_at, frames[1].captured_at);
        assert_eq!(frames[4].app_hint, None); // keyless last
    }

    #[test]
    fn day_marks_read() {
        let tmp = tempfile::tempdir().unwrap();
        let path = fixture(tmp.path());
        let c = open_readonly(&path).unwrap();
        let b = day_bounds(&c, "2026-07-01").unwrap();
        let marks = day_marks(&c, b).unwrap();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].mark_id, 10);
        assert_eq!(marks[0].note.as_deref(), Some("fix the CI"));
    }

    #[test]
    fn read_only_connection_rejects_writes() {
        let tmp = tempfile::tempdir().unwrap();
        let path = fixture(tmp.path());
        let c = open_readonly(&path).unwrap();
        // query_only + read-only open: any write must fail (proves the DB is untouched).
        let err = c
            .execute("INSERT INTO marks (frame_id, created_at) VALUES (1, 0)", [])
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("readonly") || err.contains("read-only") || err.contains("read only"),
            "write should be rejected: {err}"
        );
    }

    #[test]
    fn survey_counts_signals() {
        let tmp = tempfile::tempdir().unwrap();
        let path = fixture(tmp.path());
        let c = open_readonly(&path).unwrap();
        let s = survey(&c).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].day, "2026-07-01");
        assert_eq!(s[0].frames, 5);
        assert_eq!(s[0].distinct_apps, 2); // Code, chrome (NULL not counted by COUNT DISTINCT)
        assert_eq!(s[0].ai_title_frames, 1); // the ChatGPT frame
        assert_eq!(s[0].marks, 1);
    }

    #[test]
    fn write_day_emits_all_files() {
        let tmp = tempfile::tempdir().unwrap();
        let path = fixture(tmp.path());
        let out = tmp.path().join("out");
        let c = open_readonly(&path).unwrap();
        let b = day_bounds(&c, "2026-07-01").unwrap();
        let done = write_day(&out, "2026-07-01", b, &c).unwrap();
        assert!(!done.labels_preserved); // fresh export writes the template
        let dir = done.dir;
        for f in [
            "day.json",
            "frames.jsonl",
            "marks.jsonl",
            "digest.md",
            "labels.toml",
        ] {
            assert!(dir.join(f).exists(), "missing {f}");
        }
        let day: DayHeader =
            serde_json::from_str(&std::fs::read_to_string(dir.join("day.json")).unwrap()).unwrap();
        assert_eq!(day.local_midnight_ms, day_mid());
        assert_eq!(day.frame_count, 5);
        let frames_jsonl = std::fs::read_to_string(dir.join("frames.jsonl")).unwrap();
        assert_eq!(frames_jsonl.lines().count(), 5);
        // Verify a JSONL line round-trips to ExportFrame with the expected field names.
        let first: ExportFrame =
            serde_json::from_str(frames_jsonl.lines().next().unwrap()).unwrap();
        assert_eq!(first.frame_id, 1);
    }

    #[test]
    fn re_export_preserves_hand_edited_labels() {
        let tmp = tempfile::tempdir().unwrap();
        let path = fixture(tmp.path());
        let out = tmp.path().join("out");
        let c = open_readonly(&path).unwrap();
        let b = day_bounds(&c, "2026-07-01").unwrap();

        // First export writes the template.
        let first = write_day(&out, "2026-07-01", b, &c).unwrap();
        assert!(!first.labels_preserved);
        let labels_path = first.dir.join("labels.toml");
        std::fs::write(&labels_path, "# hand-edited ground truth\n").unwrap();

        // Re-export (e.g. to refresh frames.jsonl) must NOT clobber the labels.
        let second = write_day(&out, "2026-07-01", b, &c).unwrap();
        assert!(second.labels_preserved);
        assert_eq!(
            std::fs::read_to_string(&labels_path).unwrap(),
            "# hand-edited ground truth\n"
        );
        // Regenerated data is still refreshed.
        assert!(second.dir.join("frames.jsonl").exists());
    }

    #[test]
    fn backup_snapshots_and_verifies() {
        let tmp = tempfile::tempdir().unwrap();
        let src = fixture(tmp.path());
        // Destination outside the repo tree: use the tempdir (not under CWD).
        let dst_dir = tmp.path().join("backups");
        let report = backup(&src, &dst_dir, "2026-07-07").unwrap();
        assert!(report.dest.ends_with("screensearch-2026-07-07.db"));
        assert_eq!(report.src_frames, 5);
        assert_eq!(report.dst_frames, 5);
        assert_eq!(report.src_marks, 1);
        assert_eq!(report.dst_marks, 1);
        assert!(report.integrity_ok);
        // Refuses to overwrite an existing dated file.
        assert!(backup(&src, &dst_dir, "2026-07-07")
            .unwrap_err()
            .to_string()
            .contains("refusing to overwrite"));
    }

    #[test]
    fn backup_refuses_destination_in_repo_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let src = fixture(tmp.path());
        // The current working dir (repo root during tests) is off-limits.
        let inside = std::env::current_dir().unwrap().join("target");
        let err = backup(&src, &inside, "2026-07-07").unwrap_err().to_string();
        assert!(err.contains("inside the repo tree"), "{err}");
    }

    #[test]
    fn git_root_walks_above_the_crate_dir() {
        let cwd = std::env::current_dir().unwrap();
        let root = git_root(&cwd).expect("tests run inside a git tree");
        // The crate dir (CWD during tests) is strictly below the repo root, so the old
        // CWD-only guard would have missed a dest under the root but outside the crate dir.
        assert!(cwd.starts_with(&root));
        assert_ne!(cwd, root, "crate dir should be below the git root");
        assert!(root.join(".git").exists());
    }
}

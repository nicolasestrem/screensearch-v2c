//! Load exported days back from `harness-data/<day>/` for replay/score/sweep/stability.
//!
//! Reads `frames.jsonl` (segmentation input), `day.json` (the `local_midnight_ms` anchor for
//! resolving label times), and `labels.toml` when present (absent/empty = an unlabeled day,
//! fine for `replay` but skipped by `score`). Personal data never leaves this local tree.

use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::labels::{parse_labels, resolve_day};
use crate::model::{DayHeader, ExportFrame};
use crate::score::LoadedDay;
use crate::segmenter::FrameRow;

/// Load one exported day directory into a [`LoadedDay`]. Requires `day.json` + `frames.jsonl`;
/// `labels.toml` is optional (missing or session-less = no labels).
pub fn load_day(dir: &Path) -> Result<LoadedDay> {
    let header: DayHeader = serde_json::from_str(
        &std::fs::read_to_string(dir.join("day.json"))
            .with_context(|| format!("reading {}", dir.join("day.json").display()))?,
    )
    .context("parsing day.json")?;

    let frames_txt = std::fs::read_to_string(dir.join("frames.jsonl"))
        .with_context(|| format!("reading {}", dir.join("frames.jsonl").display()))?;
    let frames: Vec<FrameRow> = frames_txt
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<ExportFrame>(l).map(|f| FrameRow::from(&f)))
        .collect::<serde_json::Result<_>>()
        .context("parsing frames.jsonl")?;

    let labels_path = dir.join("labels.toml");
    let labels = if labels_path.exists() {
        let day = parse_labels(&std::fs::read_to_string(&labels_path)?)?;
        // Fail fast if a labels file was copied between day dirs or its date header went stale:
        // its HH:MM times would otherwise resolve silently against THIS day's midnight and
        // corrupt the acceptance evidence.
        if day.date != header.date {
            bail!(
                "{}: labels date {:?} does not match the exported day {:?} \
                 (wrong day's labels.toml?)",
                labels_path.display(),
                day.date,
                header.date
            );
        }
        resolve_day(&day, header.local_midnight_ms)
            .with_context(|| format!("validating {}", labels_path.display()))?
    } else {
        Vec::new()
    };

    let (day_start_ms, day_end_ms) = match (frames.first(), frames.last()) {
        (Some(a), Some(b)) => (a.captured_at, b.captured_at),
        _ => (header.local_midnight_ms, header.local_midnight_ms),
    };
    Ok(LoadedDay {
        date: header.date,
        frames,
        day_start_ms,
        day_end_ms,
        labels,
    })
}

/// Load every exported day under `data_dir` (any subdir containing a `day.json`), sorted by
/// date. The `reports/` subdir and anything without a `day.json` are skipped.
pub fn load_all(data_dir: &Path) -> Result<Vec<LoadedDay>> {
    let mut days = Vec::new();
    let entries = std::fs::read_dir(data_dir)
        .with_context(|| format!("reading {} (run `export` first?)", data_dir.display()))?;
    for e in entries {
        let path = e?.path();
        if path.is_dir() && path.join("day.json").exists() {
            days.push(load_day(&path)?);
        }
    }
    days.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(days)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ExportFrame;

    #[test]
    fn round_trips_an_exported_day() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("2026-07-01");
        std::fs::create_dir_all(&dir).unwrap();
        let mid = 1_782_856_800_000i64;
        let header = DayHeader {
            schema: "harness-day-v1".into(),
            date: "2026-07-01".into(),
            local_midnight_ms: mid,
            utc_offset_min: 120,
            frame_count: 2,
            mark_count: 0,
        };
        std::fs::write(
            dir.join("day.json"),
            serde_json::to_string(&header).unwrap(),
        )
        .unwrap();
        let frames = [
            ExportFrame {
                frame_id: 1,
                captured_at: mid + 60_000,
                app_hint: Some("Code".into()),
                window_title: Some("a.rs".into()),
                browser_url: None,
                capture_trigger: Some("timer".into()),
            },
            ExportFrame {
                frame_id: 2,
                captured_at: mid + 120_000,
                app_hint: Some("Code".into()),
                window_title: Some("a.rs".into()),
                browser_url: None,
                capture_trigger: Some("timer".into()),
            },
        ];
        let jsonl: String = frames
            .iter()
            .map(|f| serde_json::to_string(f).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(dir.join("frames.jsonl"), jsonl).unwrap();
        std::fs::write(
            dir.join("labels.toml"),
            "date = \"2026-07-01\"\n[[session]]\nstart=\"00:01\"\nend=\"00:02\"\nkind=\"focus\"\n",
        )
        .unwrap();

        let day = load_day(&dir).unwrap();
        assert_eq!(day.date, "2026-07-01");
        assert_eq!(day.frames.len(), 2);
        assert_eq!(day.day_start_ms, mid + 60_000);
        assert_eq!(day.labels.len(), 1);
        assert_eq!(day.labels[0].start_ms, mid + 60_000);

        // load_all finds the day and skips a reports/ dir.
        std::fs::create_dir_all(tmp.path().join("reports")).unwrap();
        let all = load_all(tmp.path()).unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn rejects_labels_whose_date_mismatches_the_day() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("2026-07-01");
        std::fs::create_dir_all(&dir).unwrap();
        let mid = 1_782_856_800_000i64;
        let header = DayHeader {
            schema: "harness-day-v1".into(),
            date: "2026-07-01".into(),
            local_midnight_ms: mid,
            utc_offset_min: 120,
            frame_count: 1,
            mark_count: 0,
        };
        std::fs::write(
            dir.join("day.json"),
            serde_json::to_string(&header).unwrap(),
        )
        .unwrap();
        let frame = ExportFrame {
            frame_id: 1,
            captured_at: mid + 60_000,
            app_hint: Some("Code".into()),
            window_title: Some("a.rs".into()),
            browser_url: None,
            capture_trigger: Some("timer".into()),
        };
        std::fs::write(
            dir.join("frames.jsonl"),
            serde_json::to_string(&frame).unwrap(),
        )
        .unwrap();
        // A labels file carrying a different day's date (copied/stale header).
        std::fs::write(
            dir.join("labels.toml"),
            "date = \"2026-07-02\"\n[[session]]\nstart=\"00:01\"\nend=\"00:02\"\nkind=\"focus\"\n",
        )
        .unwrap();

        let err = load_day(&dir).unwrap_err().to_string();
        assert!(err.contains("does not match the exported day"), "{err}");
    }
}

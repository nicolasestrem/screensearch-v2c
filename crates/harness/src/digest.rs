//! The human-readable per-day digest and the label template.
//!
//! The digest collapses consecutive same-`app_hint` frames into a readable context-run
//! timeline the maintainer labels from. It is **deliberately not** the candidate segmenter:
//! no gap/close/dwell logic, no taxonomy tool-splitting. That keeps the labels anchor-free
//! (unbiased) — the segmenter must not get to suggest its own answer key. Keyless frames
//! (no `app_hint`) are transparent: they extend the current run rather than starting one,
//! matching the segmenter's treatment so the run boundaries stay comparable.

use std::collections::BTreeMap;

use crate::model::{local_hhmm, DayHeader, ExportFrame, ExportMark};

/// Normalize an `app_hint` for run grouping: trimmed, lowercased; `None`/empty is keyless.
fn norm_app(app: Option<&str>) -> Option<String> {
    let a = app?.trim();
    (!a.is_empty()).then(|| a.to_ascii_lowercase())
}

struct Run {
    app: String,
    start_ms: i64,
    end_ms: i64,
    frames: usize,
    titles: BTreeMap<String, usize>,
}

/// Collapse ordered frames into consecutive same-app runs (keyless frames absorbed into the
/// current run). Leading keyless frames (before any keyed frame) are counted separately.
fn collapse(frames: &[ExportFrame]) -> (Vec<Run>, usize) {
    let mut runs: Vec<Run> = Vec::new();
    let mut lead_keyless = 0usize;
    for f in frames {
        match norm_app(f.app_hint.as_deref()) {
            Some(app) => match runs.last_mut() {
                Some(r) if r.app == app => {
                    r.end_ms = f.captured_at;
                    r.frames += 1;
                    if let Some(t) = f.window_title.as_deref() {
                        if !t.trim().is_empty() {
                            *r.titles.entry(t.to_string()).or_insert(0) += 1;
                        }
                    }
                }
                _ => {
                    let mut titles = BTreeMap::new();
                    if let Some(t) = f.window_title.as_deref() {
                        if !t.trim().is_empty() {
                            titles.insert(t.to_string(), 1);
                        }
                    }
                    runs.push(Run {
                        app,
                        start_ms: f.captured_at,
                        end_ms: f.captured_at,
                        frames: 1,
                        titles,
                    });
                }
            },
            None => match runs.last_mut() {
                Some(r) => {
                    r.end_ms = f.captured_at;
                    r.frames += 1;
                }
                None => lead_keyless += 1,
            },
        }
    }
    (runs, lead_keyless)
}

/// Up to `k` most frequent titles for a run, "title (n)" joined by " · ".
fn top_titles(titles: &BTreeMap<String, usize>, k: usize) -> String {
    let mut v: Vec<(&String, &usize)> = titles.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    v.into_iter()
        .take(k)
        .map(|(t, n)| format!("{t} ({n})"))
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
}

/// Render the per-day digest markdown the maintainer labels from.
pub fn render_digest(header: &DayHeader, frames: &[ExportFrame], marks: &[ExportMark]) -> String {
    let mid = header.local_midnight_ms;
    let (runs, lead_keyless) = collapse(frames);
    let mut out = String::new();
    out.push_str(&format!(
        "# {} \u{2014} context runs (raw app runs, NOT sessions; label from this)\n\n",
        header.date
    ));
    out.push_str(&format!(
        "{} frames, {} marks, {} context runs. Times are local {}.\n",
        header.frame_count,
        header.mark_count,
        runs.len(),
        header.date
    ));
    if lead_keyless > 0 {
        out.push_str(&format!(
            "({lead_keyless} leading frames had no resolved app and are not shown as a run.)\n"
        ));
    }
    out.push_str("\n| start | end | app | frames | top window titles |\n");
    out.push_str("|-------|-----|-----|--------|-------------------|\n");
    for r in &runs {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            local_hhmm(r.start_ms, mid),
            local_hhmm(r.end_ms, mid),
            r.app,
            r.frames,
            top_titles(&r.titles, 3),
        ));
    }
    if !marks.is_empty() {
        out.push_str("\nMarks (labeling anchors):\n");
        for m in marks {
            let note = m.note.as_deref().unwrap_or("");
            out.push_str(&format!(
                "- {} mark #{}{}\n",
                local_hhmm(m.created_at, mid),
                m.mark_id,
                if note.is_empty() {
                    String::new()
                } else {
                    format!(": {note:?}")
                },
            ));
        }
    }
    out
}

/// Render the `labels.toml` template for a day, with marks listed as comment anchors.
pub fn render_labels_template(date: &str, marks: &[ExportMark], local_midnight_ms: i64) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# ScreenSearch 0.4.0 PR2 ground-truth labels for {date}. Local 24h \"HH:MM\".\n\
         # Label every session you can identify (>= ~2 min). Unlabeled time = no session.\n\
         # kind: focus | meeting | ai | other\n\
         # tool (required iff kind = \"ai\"): claude-code | codex | claude-desktop | cursor | vscode | browser-ai\n\
         # host (optional): terminal | desktop | browser | ide\n\
         date = \"{date}\"\n\n\
         # [[session]]\n\
         # start = \"09:14\"\n\
         # end   = \"11:32\"\n\
         # kind  = \"ai\"\n\
         # tool  = \"claude-code\"\n\
         # host  = \"terminal\"\n"
    ));
    if marks.is_empty() {
        out.push_str("\n# (no marks this day)\n");
    } else {
        out.push_str("\n# Anchors from your marks (comments only, delete freely):\n");
        for m in marks {
            let note = m.note.as_deref().unwrap_or("");
            out.push_str(&format!(
                "#   {}  {:?}\n",
                local_hhmm(m.created_at, local_midnight_ms),
                note
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MID: i64 = 1_782_856_800_000; // 2026-07-01 local midnight (Europe/Paris)

    fn frame(id: i64, min: i64, app: Option<&str>, title: Option<&str>) -> ExportFrame {
        ExportFrame {
            frame_id: id,
            captured_at: MID + min * 60_000,
            app_hint: app.map(str::to_string),
            window_title: title.map(str::to_string),
            browser_url: None,
            capture_trigger: Some("timer".into()),
        }
    }

    fn header(frames: i64, marks: i64) -> DayHeader {
        DayHeader {
            schema: "harness-day-v1".into(),
            date: "2026-07-01".into(),
            local_midnight_ms: MID,
            utc_offset_min: 120,
            frame_count: frames,
            mark_count: marks,
        }
    }

    #[test]
    fn collapses_consecutive_same_app_runs() {
        let frames = vec![
            frame(1, 0, Some("Code"), Some("a.rs - VS Code")),
            frame(2, 1, Some("Code"), Some("a.rs - VS Code")),
            frame(3, 2, Some("Code"), Some("b.rs - VS Code")),
            frame(4, 5, Some("chrome"), Some("GitHub")),
        ];
        let (runs, lead) = collapse(&frames);
        assert_eq!(lead, 0);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].app, "code");
        assert_eq!(runs[0].frames, 3);
        assert_eq!(runs[0].start_ms, MID);
        assert_eq!(runs[0].end_ms, MID + 2 * 60_000);
        assert_eq!(runs[1].app, "chrome");
    }

    #[test]
    fn keyless_frames_are_absorbed_not_split() {
        // Code, a keyless blip, Code again -> one run (keyless transparent).
        let frames = vec![
            frame(1, 0, Some("Code"), Some("a.rs")),
            frame(2, 1, None, None),
            frame(3, 2, Some("Code"), Some("a.rs")),
        ];
        let (runs, lead) = collapse(&frames);
        assert_eq!(lead, 0);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].frames, 3);
        assert_eq!(runs[0].end_ms, MID + 2 * 60_000);
    }

    #[test]
    fn leading_keyless_counted_separately() {
        let frames = vec![
            frame(1, 0, None, None),
            frame(2, 1, None, None),
            frame(3, 2, Some("Code"), Some("a.rs")),
        ];
        let (runs, lead) = collapse(&frames);
        assert_eq!(lead, 2);
        assert_eq!(runs.len(), 1);
    }

    #[test]
    fn digest_renders_table_marks_and_top_titles() {
        let frames = vec![
            frame(1, 0, Some("Code"), Some("a.rs")),
            frame(2, 1, Some("Code"), Some("a.rs")),
            frame(3, 2, Some("Code"), Some("b.rs")),
        ];
        let marks = vec![ExportMark {
            mark_id: 7,
            frame_id: 3,
            created_at: MID + 2 * 60_000,
            note: Some("fix the CI".into()),
            resolved_at: None,
        }];
        let md = render_digest(&header(3, 1), &frames, &marks);
        assert!(md.contains("| 00:00 | 00:02 | code | 3 |"));
        assert!(md.contains("a.rs (2)"), "most frequent title first: {md}");
        assert!(md.contains("00:02 mark #7: \"fix the CI\""));
    }

    #[test]
    fn labels_template_lists_marks_as_comments() {
        let marks = vec![ExportMark {
            mark_id: 1,
            frame_id: 1,
            created_at: MID + 14 * 60_000,
            note: Some("ship it".into()),
            resolved_at: None,
        }];
        let t = render_labels_template("2026-07-01", &marks, MID);
        assert!(t.contains("date = \"2026-07-01\""));
        assert!(t.contains("#   00:14  \"ship it\""));
    }
}

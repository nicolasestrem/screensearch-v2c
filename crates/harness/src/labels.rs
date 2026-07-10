//! Hand-label parsing + validation for the per-day `labels.toml` files.
//!
//! The maintainer writes local `"HH:MM"` times; [`resolve_day`] validates each `[[session]]`
//! and converts it to an epoch-ms span against the day's `local_midnight_ms`. Validation
//! rules (`docs/0.4.0.md` §3 PR2; `06` #28 / `07` #114 concurrent labels **v2**):
//! - `end > start`; times are `HH:MM` 24-hour, `end` may be `"24:00"` (local midnight next day).
//! - the file is **globally sorted by start time** (readability; each session's start is
//!   `>=` the previous session's start).
//! - **non-overlap is enforced per identity track, not globally** (`07` #114 concurrent model):
//!   `kind = "ai"` sessions may not overlap another `ai` session *with the same `tool`*;
//!   `focus`/`other` may not overlap another session of the *same kind*; **`meeting` sessions
//!   are exempt** (labels carry no meeting id, so concurrent meetings may overlap). Different
//!   identities (e.g. `claude-code` vs `codex`, or any `ai` vs a `meeting`) **may** overlap.
//!   **Touching** (`end == next start`) is always allowed. Serial (non-overlapping) label files
//!   from before v2 stay valid unchanged.
//! - `kind = "ai"` requires a non-empty `tool`; any other kind must omit `tool`.
//! - enum membership (`kind`/`host`) is enforced by the TOML deserializer.
//!
//! Errors name the offending 1-based `[[session]]` index (and its identity track on overlaps).

use std::collections::HashMap;

use anyhow::{bail, Context, Result};

use crate::model::{DayLabels, Host, Kind};

/// The non-overlap **identity track** key for a labeled session, or `None` when the kind is
/// exempt from non-overlap. Under the concurrent model (`07` #114 / `06` #28) only *same-identity*
/// sessions must stay non-overlapping: `ai` per `tool`, `focus`/`other` pooled per kind. `meeting`
/// labels carry no id and so may overlap each other — they are exempt (`None`). `ai` always has a
/// tool here (the kind/tool check runs first), but `unwrap_or("")` keeps this total.
fn track_key(kind: Kind, tool: Option<&str>) -> Option<String> {
    match kind {
        Kind::Ai => Some(format!("ai:{}", tool.unwrap_or(""))),
        Kind::Focus => Some("focus".to_string()),
        Kind::Other => Some("other".to_string()),
        Kind::Meeting => None,
    }
}

/// A validated, time-resolved label. `start_ms`/`end_ms` are unix epoch ms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLabel {
    pub start_ms: i64,
    pub end_ms: i64,
    pub kind: Kind,
    pub tool: Option<String>,
    pub host: Option<Host>,
}

/// Parse a `labels.toml` document into [`DayLabels`] (no time resolution or cross-session
/// validation yet — see [`resolve_day`]).
pub fn parse_labels(doc: &str) -> Result<DayLabels> {
    toml::from_str(doc).context("parsing labels.toml")
}

/// Minutes-since-local-midnight for an `"HH:MM"` string (0..=1440; `"24:00"` = 1440).
fn parse_hhmm(s: &str) -> Result<i64> {
    let (h, m) = s
        .split_once(':')
        .with_context(|| format!("time {s:?} is not HH:MM"))?;
    let h: i64 = h.parse().with_context(|| format!("time {s:?}: bad hour"))?;
    let m: i64 = m
        .parse()
        .with_context(|| format!("time {s:?}: bad minute"))?;
    if !(0..60).contains(&m) {
        bail!("time {s:?}: minute out of range 0..59");
    }
    let total = h * 60 + m;
    if !(0..=1440).contains(&total) {
        bail!("time {s:?}: out of range 00:00..24:00");
    }
    Ok(total)
}

/// Validate + resolve every session in a day's labels to epoch-ms spans. `local_midnight_ms`
/// comes from the day's `day.json` header.
pub fn resolve_day(labels: &DayLabels, local_midnight_ms: i64) -> Result<Vec<ResolvedLabel>> {
    let mut out: Vec<ResolvedLabel> = Vec::with_capacity(labels.sessions.len());
    // Global chronological ordering (by start), for readability. NOT the non-overlap check.
    let mut prev_start_min: Option<i64> = None;
    // Per-identity-track last end-minute, for non-overlap within a track (`07` #114 / `06` #28).
    let mut track_end: HashMap<String, i64> = HashMap::new();

    for (i, s) in labels.sessions.iter().enumerate() {
        let idx = i + 1; // 1-based, matches how the file reads
        let start_min = parse_hhmm(&s.start).with_context(|| format!("session #{idx} start"))?;
        let end_min = parse_hhmm(&s.end).with_context(|| format!("session #{idx} end"))?;

        if start_min >= 1440 {
            bail!(
                "session #{idx}: start {} cannot be at or past 24:00",
                s.start
            );
        }
        if end_min <= start_min {
            bail!(
                "session #{idx}: end {} must be after start {}",
                s.end,
                s.start
            );
        }
        match s.kind {
            Kind::Ai => {
                if s.tool.as_deref().is_none_or(|t| t.trim().is_empty()) {
                    bail!("session #{idx}: kind = \"ai\" requires a non-empty tool");
                }
            }
            _ => {
                if s.tool.is_some() {
                    bail!(
                        "session #{idx}: tool is only allowed when kind = \"ai\" (kind is {:?})",
                        s.kind
                    );
                }
            }
        }
        // Global ordering: the file must be written sorted by start time.
        if let Some(ps) = prev_start_min {
            if start_min < ps {
                bail!(
                    "session #{idx}: starts at {} out of order \u{2014} labels.toml must be sorted by start time ({} min precedes it); concurrent sessions are still listed in start order",
                    s.start,
                    ps
                );
            }
        }
        prev_start_min = Some(start_min);

        // Per-identity non-overlap: only same-identity sessions must not overlap (meetings exempt).
        if let Some(key) = track_key(s.kind, s.tool.as_deref()) {
            if let Some(&pe) = track_end.get(&key) {
                if start_min < pe {
                    bail!(
                        "session #{idx}: starts at {} but overlaps the previous \"{}\" session (ends {} min) \u{2014} same-identity sessions cannot overlap; only different identities may run concurrently (`07` #114)",
                        s.start,
                        key,
                        pe
                    );
                }
            }
            track_end.insert(key, end_min);
        }

        out.push(ResolvedLabel {
            start_ms: local_midnight_ms + start_min * 60_000,
            end_ms: local_midnight_ms + end_min * 60_000,
            kind: s.kind,
            tool: s.tool.clone(),
            host: s.host,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Local midnight for 2026-07-01 in Europe/Paris (verified live via node:sqlite:
    // `SELECT unixepoch('2026-07-01 00:00:00','utc')*1000` -> 1782856800000, offset 120 min).
    const MIDNIGHT: i64 = 1_782_856_800_000;

    const TEMPLATE: &str = r#"
date = "2026-07-01"

[[session]]
start = "09:14"
end   = "11:32"
kind  = "ai"
tool  = "claude-code"
host  = "terminal"

[[session]]
start = "13:05"
end   = "13:41"
kind  = "meeting"
"#;

    #[test]
    fn parses_and_resolves_template() {
        let day = parse_labels(TEMPLATE).expect("parse");
        assert_eq!(day.date, "2026-07-01");
        assert_eq!(day.sessions.len(), 2);
        let r = resolve_day(&day, MIDNIGHT).expect("resolve");
        // 09:14 = 554 min past midnight.
        assert_eq!(r[0].start_ms, MIDNIGHT + 554 * 60_000);
        assert_eq!(r[0].start_ms, 1_782_890_040_000);
        assert_eq!(r[0].end_ms, MIDNIGHT + 692 * 60_000);
        assert_eq!(r[0].kind, Kind::Ai);
        assert_eq!(r[0].tool.as_deref(), Some("claude-code"));
        assert_eq!(r[0].host, Some(Host::Terminal));
        assert_eq!(r[1].kind, Kind::Meeting);
        assert_eq!(r[1].tool, None);
    }

    fn day_of(body: &str) -> DayLabels {
        parse_labels(&format!("date = \"2026-07-01\"\n{body}")).expect("parse")
    }

    #[test]
    fn rejects_true_overlap() {
        let d = day_of(
            r#"
[[session]]
start = "09:00"
end   = "10:00"
kind  = "focus"

[[session]]
start = "09:30"
end   = "10:30"
kind  = "focus"
"#,
        );
        let err = resolve_day(&d, MIDNIGHT).unwrap_err().to_string();
        assert!(err.contains("#2"), "error should name session #2: {err}");
    }

    #[test]
    fn accepts_touching_sessions() {
        // end == next start: meetings butting against the next block. Must pass.
        let d = day_of(
            r#"
[[session]]
start = "09:00"
end   = "10:00"
kind  = "meeting"

[[session]]
start = "10:00"
end   = "10:45"
kind  = "focus"
"#,
        );
        let r = resolve_day(&d, MIDNIGHT).expect("touching sessions are allowed");
        assert_eq!(r[0].end_ms, r[1].start_ms);
    }

    #[test]
    fn accepts_cross_identity_overlap() {
        // v2 (`07` #114): different identities may overlap. Here: codex over claude-code,
        // an AI session over a meeting, and two meetings over each other. All must pass.
        let d = day_of(
            r#"
[[session]]
start = "09:00"
end   = "11:00"
kind  = "ai"
tool  = "claude-code"
host  = "terminal"

[[session]]
start = "09:30"
end   = "10:30"
kind  = "meeting"

[[session]]
start = "10:00"
end   = "12:00"
kind  = "ai"
tool  = "codex"
host  = "terminal"

[[session]]
start = "10:15"
end   = "11:15"
kind  = "meeting"
"#,
        );
        let r = resolve_day(&d, MIDNIGHT).expect("cross-identity overlap is allowed under v2");
        assert_eq!(r.len(), 4);
        // claude-code (0) and codex (2) overlap in wall-clock but are distinct tracks.
        assert!(r[2].start_ms < r[0].end_ms, "the two AI tracks overlap");
        assert_eq!(r[0].tool.as_deref(), Some("claude-code"));
        assert_eq!(r[2].tool.as_deref(), Some("codex"));
        // The two meetings (1, 3) overlap each other and are exempt.
        assert!(r[3].start_ms < r[1].end_ms, "the two meetings overlap");
    }

    #[test]
    fn rejects_same_tool_and_focus_overlap() {
        // Same tool overlapping itself is still rejected (one track cannot overlap itself).
        let same_ai = day_of(
            r#"
[[session]]
start = "09:00"
end   = "11:00"
kind  = "ai"
tool  = "claude-code"
host  = "terminal"

[[session]]
start = "10:00"
end   = "12:00"
kind  = "ai"
tool  = "claude-code"
host  = "terminal"
"#,
        );
        let err = resolve_day(&same_ai, MIDNIGHT).unwrap_err().to_string();
        assert!(err.contains("#2"), "error should name session #2: {err}");
        assert!(
            err.contains("ai:claude-code"),
            "error should name the overlapping identity track: {err}"
        );

        // Two focus sessions of the same (pooled) kind overlapping is also rejected.
        let same_focus = day_of(
            r#"
[[session]]
start = "09:00"
end   = "10:00"
kind  = "focus"

[[session]]
start = "09:30"
end   = "10:30"
kind  = "focus"
"#,
        );
        let err = resolve_day(&same_focus, MIDNIGHT).unwrap_err().to_string();
        assert!(err.contains("#2") && err.contains("focus"), "{err}");
    }

    #[test]
    fn serial_label_files_still_validate() {
        // A pre-v2 serial (non-overlapping, mixed-kind) file must parse + resolve unchanged.
        let d = day_of(
            r#"
[[session]]
start = "09:14"
end   = "11:32"
kind  = "ai"
tool  = "claude-code"
host  = "terminal"

[[session]]
start = "11:32"
end   = "12:05"
kind  = "meeting"

[[session]]
start = "13:05"
end   = "14:40"
kind  = "ai"
tool  = "codex"
host  = "terminal"

[[session]]
start = "15:00"
end   = "16:20"
kind  = "focus"
"#,
        );
        let r = resolve_day(&d, MIDNIGHT).expect("serial files stay valid under v2");
        assert_eq!(r.len(), 4);
        assert_eq!(
            r[0].end_ms, r[1].start_ms,
            "touching AI->meeting still allowed"
        );
    }

    #[test]
    fn rejects_out_of_start_order() {
        // The file must be globally sorted by start, even for concurrent labels.
        let d = day_of(
            r#"
[[session]]
start = "10:00"
end   = "11:00"
kind  = "ai"
tool  = "codex"
host  = "terminal"

[[session]]
start = "09:00"
end   = "12:00"
kind  = "ai"
tool  = "claude-code"
host  = "terminal"
"#,
        );
        let err = resolve_day(&d, MIDNIGHT).unwrap_err().to_string();
        assert!(err.contains("#2") && err.contains("out of order"), "{err}");
    }

    #[test]
    fn rejects_end_at_or_before_start() {
        let d = day_of(
            r#"
[[session]]
start = "10:00"
end   = "10:00"
kind  = "focus"
"#,
        );
        assert!(resolve_day(&d, MIDNIGHT)
            .unwrap_err()
            .to_string()
            .contains("must be after start"));
    }

    #[test]
    fn rejects_bad_enum() {
        let d = parse_labels(
            r#"
date = "2026-07-01"
[[session]]
start = "10:00"
end   = "11:00"
kind  = "nope"
"#,
        );
        assert!(d.is_err(), "unknown kind must fail to parse");
    }

    #[test]
    fn rejects_ai_without_tool() {
        let d = day_of(
            r#"
[[session]]
start = "10:00"
end   = "11:00"
kind  = "ai"
"#,
        );
        assert!(resolve_day(&d, MIDNIGHT)
            .unwrap_err()
            .to_string()
            .contains("requires a non-empty tool"));
    }

    #[test]
    fn rejects_tool_when_not_ai() {
        let d = day_of(
            r#"
[[session]]
start = "10:00"
end   = "11:00"
kind  = "focus"
tool  = "vscode"
"#,
        );
        assert!(resolve_day(&d, MIDNIGHT)
            .unwrap_err()
            .to_string()
            .contains("only allowed when kind"));
    }

    #[test]
    fn end_at_2400_is_local_midnight_next_day() {
        let d = day_of(
            r#"
[[session]]
start = "23:00"
end   = "24:00"
kind  = "focus"
"#,
        );
        let r = resolve_day(&d, MIDNIGHT).expect("24:00 end allowed");
        assert_eq!(r[0].end_ms, MIDNIGHT + 1440 * 60_000);
    }

    #[test]
    fn rejects_start_at_2400() {
        let d = day_of(
            r#"
[[session]]
start = "24:00"
end   = "24:00"
kind  = "focus"
"#,
        );
        assert!(resolve_day(&d, MIDNIGHT).is_err());
    }

    #[test]
    fn rejects_malformed_time() {
        for bad in ["9h14", "09:60", "25:00", "09:1:2", ""] {
            assert!(parse_hhmm(bad).is_err(), "{bad:?} should be rejected");
        }
        assert_eq!(parse_hhmm("00:00").unwrap(), 0);
        assert_eq!(parse_hhmm("24:00").unwrap(), 1440);
    }
}

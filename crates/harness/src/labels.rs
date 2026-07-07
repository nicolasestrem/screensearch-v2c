//! Hand-label parsing + validation for the per-day `labels.toml` files.
//!
//! The maintainer writes local `"HH:MM"` times; [`resolve_day`] validates each `[[session]]`
//! and converts it to an epoch-ms span against the day's `local_midnight_ms`. Validation
//! rules (`docs/0.4.0.md` §3 PR2, plan Task 3):
//! - `end > start`; times are `HH:MM` 24-hour, `end` may be `"24:00"` (local midnight next day).
//! - sessions are chronological and non-overlapping, but **touching is allowed**
//!   (`end == next start` — meetings frequently butt against the next block).
//! - `kind = "ai"` requires a non-empty `tool`; any other kind must omit `tool`.
//! - enum membership (`kind`/`host`) is enforced by the TOML deserializer.
//! Errors name the offending 1-based `[[session]]` index.

use anyhow::{bail, Context, Result};

use crate::model::{DayLabels, Host, Kind};

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
    let mut prev_end_min: Option<i64> = None;

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
                if !s.tool.as_deref().is_some_and(|t| !t.trim().is_empty()) {
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
        if let Some(pe) = prev_end_min {
            if start_min < pe {
                bail!(
                    "session #{idx}: starts at {} before the previous session ends ({} min) \u{2014} sessions must be chronological and non-overlapping (touching is allowed)",
                    s.start,
                    pe
                );
            }
        }
        prev_end_min = Some(end_min);

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

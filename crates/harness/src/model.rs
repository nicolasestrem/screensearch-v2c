//! Shared data types for the harness.
//!
//! - Export records ([`ExportFrame`], [`ExportMark`], [`DayHeader`]) — the read-only export
//!   shape written to `harness-data/<day>/` (`export.rs`).
//! - Recognition enums ([`Kind`], [`Host`]) — mirror the `sessions.kind` / `sessions.host`
//!   CHECK sets in `specs/03 §4`.
//! - Segmenter I/O ([`SegParams`], [`SessionSpan`]) — the pure candidate segmenter's
//!   parameters and output span (`segmenter.rs`). `SessionSpan` is the referee's unit: PR4
//!   adapts its shipped segmenter's output to this type for the D9 re-run.
//! - Hand-label types ([`LabeledSession`], [`DayLabels`]) — the parsed per-day TOML
//!   (`labels.rs`).

use serde::{Deserialize, Serialize};

/// A `focus`/`meeting`/`ai`/`other` session kind — the `sessions.kind` CHECK set (`03 §4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Focus,
    Meeting,
    Ai,
    Other,
}

/// Where a recognized tool runs — the `sessions.host` CHECK set (`03 §4`), and the second
/// D7 recognition dimension (tool identity × host).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Host {
    Terminal,
    Desktop,
    Browser,
    Ide,
}

/// One exported frame's segmentation-relevant metadata — one JSONL line in
/// `harness-data/<day>/frames.jsonl`. Field names match the `frames` columns (`03 §4`,
/// `crates/store/src/schema.rs`). Only the metadata segmentation needs: never OCR/content
/// text, never the image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportFrame {
    pub frame_id: i64,
    /// Capture time, unix epoch **milliseconds** (the frame clock unit).
    pub captured_at: i64,
    pub app_hint: Option<String>,
    pub window_title: Option<String>,
    /// Dormant in production capture (always `None`, gap #109) — carried for completeness.
    pub browser_url: Option<String>,
    pub capture_trigger: Option<String>,
}

/// One exported mark — one JSONL line in `harness-data/<day>/marks.jsonl`. Marks are
/// labeling **anchors**, not labels (`docs/0.4.0.md` §3 PR2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportMark {
    pub mark_id: i64,
    pub frame_id: i64,
    pub created_at: i64,
    pub note: Option<String>,
    pub resolved_at: Option<i64>,
}

/// `harness-data/<day>/day.json` — the per-day export header. `local_midnight_ms` anchors
/// label `HH:MM` → epoch arithmetic; `utc_offset_min` records the day's offset for audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayHeader {
    /// Format tag, always `"harness-day-v1"`.
    pub schema: String,
    /// Local calendar day, `"YYYY-MM-DD"`.
    pub date: String,
    /// Unix ms at local midnight starting this day.
    pub local_midnight_ms: i64,
    /// Local UTC offset in minutes on this day (e.g. 120 for Europe/Paris summer).
    pub utc_offset_min: i64,
    pub frame_count: i64,
    pub mark_count: i64,
}

/// Segmenter tunables, in the frame clock unit (ms). `min_len_ms` doubles as the single
/// dwell knob for the sustained-interrupter rule (`03 §7e` — "reusing the same dwell logic
/// rather than inventing a second knob").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegParams {
    /// `sessions.gap_close_secs` × 1000 (default 300_000). Idle-gap / context-switch close.
    pub gap_close_ms: i64,
    /// `sessions.min_len_secs` × 1000 (default 120_000). Floor + sustained-interrupter dwell.
    pub min_len_ms: i64,
}

impl Default for SegParams {
    fn default() -> Self {
        Self {
            gap_close_ms: 300_000,
            min_len_ms: 120_000,
        }
    }
}

/// One emitted candidate session. The referee's unit ([`score`](crate::score)). `tool` is
/// `Some` only when `kind == Ai`; `host` is set when recognition resolved one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSpan {
    pub start_ms: i64,
    pub end_ms: i64,
    /// The segmenter's context key: `"app"`, `"app|domain"`, or `"app|domain|tool"`
    /// (`03 §7e` stored-key shape; domain dormant, gap #109).
    pub context_key: String,
    pub kind: Kind,
    pub tool: Option<String>,
    pub host: Option<Host>,
    pub frame_count: usize,
    pub first_frame_id: i64,
    pub last_frame_id: i64,
}

/// One hand-labeled session from a per-day `labels.toml`. Times are local `"HH:MM"` as
/// written by the maintainer; [`labels`](crate::labels) validates + converts them to epoch ms
/// against the day's `local_midnight_ms`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LabeledSession {
    pub start: String,
    pub end: String,
    pub kind: Kind,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub host: Option<Host>,
    #[serde(default)]
    pub note: Option<String>,
}

/// A parsed per-day label file (`harness-data/<day>/labels.toml`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DayLabels {
    pub date: String,
    #[serde(default, rename = "session")]
    pub sessions: Vec<LabeledSession>,
}

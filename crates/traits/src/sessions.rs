//! Shared session-domain types and the frozen PR2 segmentation parameters (`03 §4`/`§7e`).
//! These are internal Rust contracts in PR4; PR5 owns the `ts-rs` IPC surface.

use serde::{Deserialize, Serialize};

/// Frozen concurrent-model parameters approved by the D9 gate (`06` #26/#28).
pub const SESSION_MERGE_GAP_MS: i64 = 2_700_000;
pub const SESSION_ABSORB_MAX_MS: i64 = 1_800_000;
pub const SESSION_MEETING_GAP_MS: i64 = 480_000;
pub const SESSION_FOCUS_MIN_LEN_MS: i64 = 600_000;
pub const SESSION_FOCUS_MIN_DENSITY_FPH: i64 = 90;
pub const SESSION_IDENTITY_QUALIFY_MS: i64 = 120_000;
/// Stable-id freeze lookback W: 24 hours. Correctness parameter, never a setting.
pub const SESSION_FREEZE_LOOKBACK_MS: i64 = 86_400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionKind {
    Focus,
    Meeting,
    Ai,
    Other,
}

impl SessionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::Meeting => "meeting",
            Self::Ai => "ai",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionHost {
    Terminal,
    Desktop,
    Browser,
    Ide,
}

impl SessionHost {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Desktop => "desktop",
            Self::Browser => "browser",
            Self::Ide => "ide",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionArtifactKind {
    Exchange,
    Transcript,
    Note,
}

impl SessionArtifactKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exchange => "exchange",
            Self::Transcript => "transcript",
            Self::Note => "note",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionArtifactRole {
    User,
    Agent,
}

impl SessionArtifactRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Agent => "agent",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub kind: SessionKind,
    pub tool: Option<String>,
    pub host: Option<SessionHost>,
    pub context_key: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub summary_model: Option<String>,
    pub confidence: f64,
    pub frozen: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewSession {
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub kind: SessionKind,
    pub tool: Option<String>,
    pub host: Option<SessionHost>,
    pub context_key: String,
    pub confidence: f64,
    pub frozen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionArtifact {
    pub id: i64,
    pub session_id: i64,
    pub kind: SessionArtifactKind,
    pub role: Option<SessionArtifactRole>,
    pub frame_id: Option<i64>,
    pub content: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSessionArtifact {
    pub kind: SessionArtifactKind,
    pub role: Option<SessionArtifactRole>,
    pub frame_id: Option<i64>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmenterFrame {
    pub id: i64,
    pub captured_at: i64,
    pub app_hint: Option<String>,
    pub window_title: Option<String>,
    pub browser_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionContent {
    pub frame_id: i64,
    pub captured_at: i64,
    pub content_text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionDraft {
    pub started_at: i64,
    /// Last owned frame time even while `open`; persistence maps an open draft to `ended_at=NULL`.
    pub ended_at: i64,
    pub kind: SessionKind,
    pub tool: Option<String>,
    pub host: Option<SessionHost>,
    pub context_key: String,
    pub confidence: f64,
    /// Exclusive frame ownership. Cross-track spans may overlap, frame ids never do.
    pub frame_ids: Vec<i64>,
    /// True while this identity track can still accrete at `params.now_ms`.
    pub open: bool,
}

impl SessionDraft {
    pub fn as_new_session(&self, frozen: bool) -> NewSession {
        NewSession {
            started_at: self.started_at,
            ended_at: (!self.open).then_some(self.ended_at),
            kind: self.kind,
            tool: self.tool.clone(),
            host: self.host,
            context_key: self.context_key.clone(),
            confidence: self.confidence,
            frozen,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedExchange {
    pub role: SessionArtifactRole,
    pub content: String,
    pub frame_id: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentationParams {
    pub now_ms: i64,
    pub gap_close_ms: i64,
    pub min_len_ms: i64,
    pub merge_gap_ms: i64,
    pub absorb_max_ms: i64,
    pub meeting_gap_ms: i64,
    pub focus_min_len_ms: i64,
    pub focus_min_density_fph: i64,
    pub identity_qualify_ms: i64,
    pub freeze_lookback_ms: i64,
}

impl SegmentationParams {
    /// Production parameters: only the two typed settings are supplied by the caller.
    pub const fn shipped(now_ms: i64, gap_close_ms: i64, min_len_ms: i64) -> Self {
        Self {
            now_ms,
            gap_close_ms,
            min_len_ms,
            merge_gap_ms: SESSION_MERGE_GAP_MS,
            absorb_max_ms: SESSION_ABSORB_MAX_MS,
            meeting_gap_ms: SESSION_MEETING_GAP_MS,
            focus_min_len_ms: SESSION_FOCUS_MIN_LEN_MS,
            focus_min_density_fph: SESSION_FOCUS_MIN_DENSITY_FPH,
            identity_qualify_ms: SESSION_IDENTITY_QUALIFY_MS,
            freeze_lookback_ms: SESSION_FREEZE_LOOKBACK_MS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionFilter {
    pub from_ms: Option<i64>,
    pub to_ms: Option<i64>,
    pub kind: Option<SessionKind>,
    pub tool: Option<String>,
    pub limit: Option<u32>,
    /// Request-time clock used to treat open (`ended_at=NULL`) sessions in overlap predicates.
    pub now_ms: i64,
}

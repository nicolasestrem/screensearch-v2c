//! Typed UI ↔ core contract: command inputs/outputs and event payloads (`03 §7`).
//!
//! Every type here derives [`ts_rs::TS`] and exports to `ui/src/bindings/`, so the
//! UI consumes generated types only — never a hand-written duplicate (no contract
//! drift, `04` UI guardrails).
//!
//! **Convention:** every `i64`/`u64` field carries `#[ts(type = "number")]`. Tauri
//! serializes over serde_json where 64-bit ints become JS `number`, so the bindings
//! must say `number`, not ts-rs's default `bigint`. Frame ids and unix-ms timestamps
//! stay well under 2^53, so there is no precision loss. (`03 §7`.)

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::domain::VisionAnalysis;
use crate::jobs::JobStats;

/// Half-open time window, unix epoch milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct TimeRange {
    #[ts(type = "number")]
    pub start: i64,
    #[ts(type = "number")]
    pub end: i64,
}

/// Input to the `search` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct SearchQuery {
    pub text: String,
    pub limit: u32,
    pub time_range: Option<TimeRange>,
}

/// One hybrid-search result row (`search` output).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct SearchHit {
    #[ts(type = "number")]
    pub frame_id: i64,
    #[ts(type = "number")]
    pub captured_at: i64,
    pub snippet: String,
    pub score: f32,
    pub image_path: String,
    pub app_hint: Option<String>,
}

/// Input to the `ask` command. The answer streams back via `answer_delta` events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct AskRequest {
    pub query: String,
    pub thinking: bool,
    pub max_tokens: u32,
}

/// One bucket of the timeline histogram (`get_timeline` output).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct TimelineBucket {
    #[ts(type = "number")]
    pub start: i64,
    #[ts(type = "number")]
    pub end: i64,
    pub count: u32,
}

/// Full detail for a single frame (`get_frame` output).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct FrameDetail {
    #[ts(type = "number")]
    pub frame_id: i64,
    #[ts(type = "number")]
    pub captured_at: i64,
    pub monitor_index: u32,
    pub width: u32,
    pub height: u32,
    pub image_path: String,
    pub app_hint: Option<String>,
    pub window_title: Option<String>,
    pub browser_url: Option<String>,
    pub activity_type: Option<String>,
    pub text: Option<String>,
    pub vision: Option<VisionAnalysis>,
    pub tags: Vec<String>,
}

/// Target of an `enqueue_vision` request: a single frame or a time range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum VisionTarget {
    Frame {
        #[ts(type = "number")]
        frame_id: i64,
    },
    Range {
        #[ts(type = "number")]
        start: i64,
        #[ts(type = "number")]
        end: i64,
    },
}

/// One of the two inference lanes (`03 §6`, `MODEL_REGISTRY`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum ModelLane {
    Vision,
    Answer,
}

/// User-selectable model tier per lane (`00 §E`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum ModelTier {
    Default,
    Quality,
    Beta,
}

/// Input to `set_model_tier`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct SetModelTier {
    pub lane: ModelLane,
    pub tier: ModelTier,
}

/// Input to `capture_control`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum CaptureControl {
    Start,
    Stop,
}

/// When deferred vision tagging is allowed to run (`03 §5/§8`).
/// Never real-time — only on-demand, on a timer, or when the user is idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum VisionMode {
    OnDemand,
    Timer,
    Idle,
}

/// User-facing settings (`get_settings`/`set_settings`). Field defaults mirror
/// `03 §8`; persisted as key/value rows in the `settings` table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct Settings {
    pub capture_interval_ms: u32,
    /// Empty = all monitors.
    pub capture_monitors: Vec<u32>,
    pub capture_diff_threshold: f32,
    pub storage_jpeg_quality: u8,
    pub storage_max_width: u32,
    /// 0 = keep forever.
    pub storage_retention_days: u32,
    pub enrich_embed_text: bool,
    pub enrich_image_embeddings: bool,
    pub enrich_vision_mode: VisionMode,
    pub enrich_vision_timer_interval_ms: u32,
    pub enrich_vision_idle_secs: u32,
    pub enrich_worker_concurrency: u32,
    pub models_vision_tier: ModelTier,
    pub models_answer_tier: ModelTier,
    pub answer_thinking: bool,
    pub sidecar_idle_ttl_secs: u32,
    pub sidecar_ngl: u32,
    pub privacy_excluded_apps: Vec<String>,
    pub privacy_pause_on_lock: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            capture_interval_ms: 3000,
            capture_monitors: Vec::new(),
            capture_diff_threshold: 0.006,
            storage_jpeg_quality: 80,
            storage_max_width: 1280,
            storage_retention_days: 0,
            enrich_embed_text: true,
            enrich_image_embeddings: false,
            enrich_vision_mode: VisionMode::OnDemand,
            // NOTE: `03 §8` lists these two keys without a default value — provisional,
            // confirm before P3/P4 vision scheduling. Tracked in specs/07_KNOWN_GAPS.md.
            enrich_vision_timer_interval_ms: 300_000, // 5 min
            enrich_vision_idle_secs: 300,             // 5 min
            enrich_worker_concurrency: 2,
            models_vision_tier: ModelTier::Default,
            models_answer_tier: ModelTier::Default,
            answer_thinking: true,
            sidecar_idle_ttl_secs: 180,
            sidecar_ngl: 99,
            privacy_excluded_apps: vec![
                "1Password".to_string(),
                "KeePass".to_string(),
                "Bitwarden".to_string(),
            ],
            privacy_pause_on_lock: true,
        }
    }
}

/// Readiness of a single subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum ComponentStatus {
    Unknown,
    Initializing,
    Ready,
    Unavailable,
    Error,
}

/// Aggregate readiness (`get_readiness` output / `readiness_changed` event).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct Readiness {
    pub capture: ComponentStatus,
    pub db: ComponentStatus,
    pub embed_model: ComponentStatus,
    pub sidecar: ComponentStatus,
}

/// A streamed chunk of an answer (`answer_delta` event / `AnswerProvider` channel).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum AnswerDelta {
    /// A token of the model's *thinking* trace.
    Thinking { text: String },
    /// A token of the final answer.
    Token { text: String },
    /// A grounding citation to a source frame.
    Citation {
        #[ts(type = "number")]
        frame_id: i64,
    },
    /// The answer is complete.
    Done,
    /// The answer failed.
    Error { message: String },
}

/// Emitted once per stored capture (`capture_tick` event).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct CaptureTick {
    #[ts(type = "number")]
    pub frame_id: i64,
    #[ts(type = "number")]
    pub captured_at: i64,
    pub monitor_index: u32,
}

/// Job-queue progress snapshot (`job_progress` event).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct JobProgress {
    pub stats: JobStats,
}

/// Lifecycle state of the inference sidecar (`03 §6`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum SidecarState {
    Stopped,
    Starting,
    Ready,
    Evicted,
    Crashed,
}

/// Sidecar status update (`sidecar_status` event).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct SidecarStatus {
    pub state: SidecarState,
    pub model: Option<String>,
}

/// Severity of a UI toast (`toast` event).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// A transient user-facing notification (`toast` event).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct Toast {
    pub level: ToastLevel,
    pub message: String,
}

#[cfg(test)]
mod ts_number_guard {
    use super::*;
    use crate::jobs::JobStats;
    use ts_rs::TS;

    /// Every i64/u64 IPC field must export as TS `number`, not `bigint` — Tauri's
    /// JSON wire delivers 64-bit ints as JS numbers. This guards the
    /// `#[ts(type = "number")]` convention against regressions (deterministic, no
    /// file IO). When adding a 64-bit field to an IPC type, list that type here.
    #[test]
    fn no_bigint_in_ipc_types() {
        let decls = [
            ("TimeRange", TimeRange::inline()),
            ("SearchHit", SearchHit::inline()),
            ("TimelineBucket", TimelineBucket::inline()),
            ("FrameDetail", FrameDetail::inline()),
            ("VisionTarget", VisionTarget::inline()),
            ("CaptureTick", CaptureTick::inline()),
            ("AnswerDelta", AnswerDelta::inline()),
            ("JobStats", JobStats::inline()),
        ];
        for (name, decl) in decls {
            assert!(
                !decl.contains("bigint"),
                "{name} exports a `bigint` field — add #[ts(type = \"number\")]: {decl}"
            );
        }
    }
}

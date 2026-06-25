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

use crate::domain::{TextSource, VisionAnalysis};
use crate::jobs::{JobKind, JobStats};

/// Half-open `[start, end)` time window (start inclusive, end exclusive), unix
/// epoch milliseconds. `Store::hybrid_search` filters with `captured_at >= start
/// AND captured_at < end`, so callers must pass an exclusive upper bound — a frame
/// captured exactly at `end` is *not* included.
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
    /// Also search raw/app-chrome text, not just `content_text` (`03 §3b`). Default
    /// `false` → retrieval over content text only. `#[serde(default)]` so a client
    /// that omits it gets the safe default.
    #[serde(default)]
    pub include_chrome: bool,
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

/// A lightweight frame reference for browsing — the `get_frames` and
/// `get_nearest_frame` outputs (P5). Carries only what a tile/thumbnail needs
/// (frame id, capture time, the stored JPEG's relative path, and the foreground
/// app hint), without the OCR text / vision / tags that [`FrameDetail`] hydrates.
/// Drives the Timeline hover thumbnails, the Deck "jump back in" recents, and a
/// Moment's neighbour context; open one with `get_frame(frame_id)` for full detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct FrameMeta {
    #[ts(type = "number")]
    pub frame_id: i64,
    #[ts(type = "number")]
    pub captured_at: i64,
    pub image_path: String,
    pub app_hint: Option<String>,
}

/// Input to the `ask` command. The answer streams back via request-scoped `answer_delta` events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct AskRequest {
    pub request_id: Option<String>,
    pub query: String,
    pub thinking: bool,
    pub max_tokens: u32,
}

/// Request-scoped streamed answer event (`answer_delta` payload).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct AnswerEvent {
    pub request_id: String,
    pub delta: AnswerDelta,
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

/// Aggregate activity summary over a time window (`get_insights` output).
///
/// The spec defines no Insights contract (silent gap, logged in `07`). This is the
/// chosen shape: real DB aggregates only — totals, capture density over time
/// (reusing [`TimelineBucket`]), the top foreground apps, and the vision
/// activity-type breakdown. [`Default`] is the honest-empty summary.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct InsightsSummary {
    /// Frames captured in the window.
    #[ts(type = "number")]
    pub total_frames: i64,
    /// Frames in the window that carry a vision `activity_type`.
    #[ts(type = "number")]
    pub tagged_frames: i64,
    /// Capture density across the window (sparse, ascending by time).
    pub captures: Vec<TimelineBucket>,
    /// Most-captured foreground apps, descending by frame count.
    pub top_apps: Vec<AppCount>,
    /// Vision activity-type breakdown, descending by frame count.
    pub activity_breakdown: Vec<ActivityCount>,
}

/// Storage footprint shown in the StatusRail.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct StorageStats {
    #[ts(type = "number")]
    pub db_bytes: u64,
    #[ts(type = "number")]
    pub frame_bytes: u64,
    #[ts(type = "number")]
    pub total_bytes: u64,
}

/// One row of the per-app text-filter suppression metric (`get_text_filter_stats`
/// output, `03 §3b`). The guardrail that makes silent over-suppression observable:
/// `rate` = `suppressed_spans / total_spans` over frames classified by the live
/// `filter_version`. `app` (the foreground/target app) is `None` for frames with no
/// resolved foreground app. False suppression is the top risk, so this is surfaced
/// in the UI and recoverable via `include_chrome` + preserved `raw_text`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct AppSuppression {
    pub app: Option<String>,
    /// Total classified spans for this app.
    #[ts(type = "number")]
    pub total_spans: i64,
    /// Spans dropped from `content_text` (role `chrome`/`system`/`background`).
    #[ts(type = "number")]
    pub suppressed_spans: i64,
    /// `suppressed_spans / total_spans`, in `[0,1]` (`0` when `total_spans == 0`).
    pub rate: f32,
}

/// One row of the [`InsightsSummary`] top-apps breakdown. `app` is `None` for
/// frames with no resolved foreground app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct AppCount {
    pub app: Option<String>,
    pub count: u32,
}

/// One row of the [`InsightsSummary`] activity-type breakdown. `activity` is the
/// vision-assigned label (only tagged frames are counted).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct ActivityCount {
    pub activity: Option<String>,
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
    /// Full, unfiltered OCR/UIA text — always preserved (`03 §3b`). `None` when no
    /// `frame_text` row exists yet.
    pub raw_text: Option<String>,
    /// Filtered default-retrieval text (`03 §3b`). In 0.2.0 this is a passthrough
    /// copy of `raw_text` until PR3's classifier lands (`07` #51).
    pub content_text: Option<String>,
    /// Which engine produced the primary text (`ocr` in 0.2.0; `uia` from 0.2.1).
    pub text_source: TextSource,
    /// Spans dropped from `content_text` (the suppression-rate metric, `03 §3b`).
    /// Always `0` in PR2 (no filtering yet).
    pub suppressed_text_count: u32,
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

/// KV-cache element type for the llama.cpp sidecar (`--cache-type-k`/`--cache-type-v`).
/// Lower precision shrinks the on-GPU KV cache (less VRAM) for a small quality cost.
/// `Q8_0` is a near-lossless default; `F16` is the no-compromise escape hatch; `Q4_0`
/// is the smallest. The `#[serde(rename)]`s pin the wire/TS strings to the exact tokens
/// `llama-server` expects, so a stored value is also the launch-arg value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum KvCacheType {
    #[serde(rename = "f16")]
    F16,
    #[serde(rename = "q8_0")]
    Q8_0,
    #[serde(rename = "q4_0")]
    Q4_0,
}

impl KvCacheType {
    /// The `--cache-type-k`/`--cache-type-v` argument value llama.cpp expects.
    pub fn as_arg(self) -> &'static str {
        match self {
            KvCacheType::F16 => "f16",
            KvCacheType::Q8_0 => "q8_0",
            KvCacheType::Q4_0 => "q4_0",
        }
    }

    /// Whether this is a quantized (non-`f16`) cache type. Quantized KV requires flash
    /// attention, so the sidecar only emits `--cache-type-*` for a quantized type when
    /// flash attention is active.
    pub fn is_quantized(self) -> bool {
        !matches!(self, KvCacheType::F16)
    }
}

/// Flash-attention mode for the llama.cpp sidecar (`--flash-attn`). `Auto` follows what
/// the bundled binary supports (it resolves to on when the flag exists, off otherwise);
/// `On`/`Off` force the choice. Flash attention reduces the attention compute buffer and
/// is a prerequisite for quantized KV cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum FlashAttnSetting {
    Auto,
    On,
    Off,
}

/// The llama.cpp launch knobs derived from [`Settings`], threaded to both inference
/// providers and on into the sidecar's argument list. Bundled into one struct so adding
/// a knob does not ripple through every provider/`resolve_spec` signature. Not an IPC
/// type (constructed in the core from [`Settings`]), so it carries no `ts-rs` derive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarParams {
    pub ngl: u32,
    pub device: Option<String>,
    /// `0` = automatic (a per-lane default chosen at model resolution); otherwise the
    /// shared context-window override for both lanes.
    pub ctx_size: u32,
    pub kv_cache_type: KvCacheType,
    pub flash_attn: FlashAttnSetting,
}

impl From<&Settings> for SidecarParams {
    fn from(s: &Settings) -> Self {
        Self {
            ngl: s.sidecar_ngl,
            device: s.sidecar_device.clone(),
            ctx_size: s.sidecar_ctx_size,
            kv_cache_type: s.sidecar_kv_cache_type,
            flash_attn: s.sidecar_flash_attn,
        }
    }
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

/// User-facing settings (`get_settings`/`set_settings`). Field defaults mirror
/// `03 §8`; persisted as key/value rows in the `settings` table.
///
/// Deferred vision tagging is never real-time (`03 §5`). On-demand tagging
/// (UI-triggered) is always available; **timed** and **idle** enrichment are each
/// independent opt-in toggles, off by default, with a user-set threshold. (This
/// replaces `03 §8`'s single `enrich.vision_mode` enum — see specs/06_PATCH_PLAN.)
///
/// `sidecar_device` is the optional llama.cpp `--device` selector (for example,
/// `Vulkan0`); `None` lets llama.cpp choose its default device.
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
    /// Opt-in: tag up to a batch of untagged frames every `vision_timer_interval_ms`.
    pub enrich_vision_timer_enabled: bool,
    pub enrich_vision_timer_interval_ms: u32,
    /// Opt-in: tag while the user has been idle for at least `vision_idle_secs`.
    pub enrich_vision_idle_enabled: bool,
    pub enrich_vision_idle_secs: u32,
    /// Max still-untagged frames a timer/idle tick enqueues per run (the scheduler
    /// batch size). Already-queued frames are skipped, so this caps fresh work per run.
    pub enrich_vision_batch_size: u32,
    pub enrich_worker_concurrency: u32,
    pub models_vision_tier: ModelTier,
    pub models_answer_tier: ModelTier,
    pub answer_thinking: bool,
    pub sidecar_idle_ttl_secs: u32,
    pub sidecar_ngl: u32,
    pub sidecar_device: Option<String>,
    /// Sidecar context window in tokens (`--ctx-size`). `0` = automatic: a small
    /// per-lane default (vision 4096, answer 8192). A non-zero value overrides both
    /// lanes. Lower = less VRAM (smaller KV cache); too low can truncate long answers.
    pub sidecar_ctx_size: u32,
    /// KV-cache precision (`--cache-type-k`/`--cache-type-v`). A quantized cache uses
    /// less VRAM and is applied only when flash attention is active.
    pub sidecar_kv_cache_type: KvCacheType,
    /// Flash-attention mode (`--flash-attn`). Reduces attention memory and unlocks KV
    /// quantization; `Auto` enables it when the bundled binary supports it.
    pub sidecar_flash_attn: FlashAttnSetting,
    pub privacy_excluded_apps: Vec<String>,
    pub privacy_pause_on_lock: bool,
    /// Default value of the Recall search "include app chrome / raw text" toggle
    /// (`03 §8` `text.include_chrome_default`). `false` → default search uses
    /// `content_text` only; the per-query `SearchQuery.include_chrome` can still opt in.
    pub text_include_chrome_default: bool,
    /// Appearances of a span signature before it is marked static chrome and dropped
    /// from `content_text` (`03 §8` `text.chrome_suppress_min_seen`). A threshold, never
    /// hardcoded (`03 §3b`).
    pub text_chrome_suppress_min_seen: u32,
    /// Lines at least this many characters are never suppressed for merely repeating
    /// (`03 §8` `text.chrome_protect_min_chars`) — protects long, information-rich text.
    pub text_chrome_protect_min_chars: u32,
    /// Grid resolution for a span's `region_bucket` in the chrome signature
    /// (`03 §8` `text.chrome_region_buckets`); an N×N grid over the normalized frame.
    pub text_chrome_region_buckets: u32,
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
            // Timed/idle vision enrichment are opt-in (off by default); on-demand is
            // always available. Thresholds chosen with the user (07 gap #1), used only
            // when the matching toggle is enabled. All user-adjustable in settings.
            enrich_vision_timer_enabled: false,
            enrich_vision_timer_interval_ms: 3_600_000, // 60 min
            enrich_vision_idle_enabled: false,
            enrich_vision_idle_secs: 300, // 5 min
            enrich_vision_batch_size: 20, // frames per scheduled tick
            enrich_worker_concurrency: 2,
            models_vision_tier: ModelTier::Default,
            models_answer_tier: ModelTier::Default,
            answer_thinking: true,
            sidecar_idle_ttl_secs: 180,
            sidecar_ngl: 99,
            sidecar_device: None,
            // Memory-tuning defaults (balanced): pin a small per-lane context (0 =
            // auto → vision 4096 / answer 8192), quantize the KV cache to q8_0, and let
            // flash attention turn on where the bundled llama.cpp build supports it.
            // Together these cut VRAM well below the uncontrolled-context default with
            // no expected quality loss; the f16 / larger-context escape hatches let a
            // user trade memory back for quality.
            sidecar_ctx_size: 0,
            sidecar_kv_cache_type: KvCacheType::Q8_0,
            sidecar_flash_attn: FlashAttnSetting::Auto,
            privacy_excluded_apps: vec![
                "1Password".to_string(),
                "KeePass".to_string(),
                "Bitwarden".to_string(),
            ],
            privacy_pause_on_lock: true,
            // 0.2.x attention-first text signal (03 §8). Thresholds are settings, never
            // hardcoded (03 §3b); these defaults match the spec and are tuned in PR3.
            text_include_chrome_default: false,
            text_chrome_suppress_min_seen: 12,
            text_chrome_protect_min_chars: 48,
            text_chrome_region_buckets: 8,
        }
    }
}

/// Readiness state of a single subsystem.
///
/// `03 §7` returns a `Readiness` but does not define this enum (07 gap #3). The
/// states below are a closed set chosen so the UI's readiness panel can show a
/// truthful, actionable status for every subsystem without inventing per-screen
/// vocabulary:
/// - `Unknown` — not yet probed (the honest pre-init value).
/// - `Disabled` — intentionally off via settings (e.g. capture stopped, image
///   embeddings disabled, vision in `on_demand` and idle). Not an error.
/// - `Initializing` — coming up (DB migrating, model downloading/loading, sidecar
///   spawning).
/// - `Ready` — operational (or, for the lazily-evicted sidecar, able to serve on
///   demand).
/// - `Unavailable` — a prerequisite is missing (model not downloaded, sidecar
///   binary absent, no capturable monitor). Actionable by the user.
/// - `Error` — a failure occurred; see `detail`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum ComponentStatus {
    #[default]
    Unknown,
    Disabled,
    Initializing,
    Ready,
    Unavailable,
    Error,
}

/// Readiness of one subsystem: a [`ComponentStatus`] plus optional human-readable
/// `detail` (e.g. "model downloading 40%", "sidecar evicted (idle)", "WebView2
/// runtime missing") so the UI can explain *why* without a separate lookup.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct ComponentReadiness {
    pub status: ComponentStatus,
    pub detail: Option<String>,
}

impl ComponentReadiness {
    /// A status with no extra detail.
    pub fn of(status: ComponentStatus) -> Self {
        Self {
            status,
            detail: None,
        }
    }

    /// A status with a human-readable explanation.
    pub fn with_detail(status: ComponentStatus, detail: impl Into<String>) -> Self {
        Self {
            status,
            detail: Some(detail.into()),
        }
    }
}

/// Aggregate readiness of the four subsystems (`get_readiness` output /
/// `readiness_changed` event, `03 §7`). [`Default`] is every component `Unknown`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct Readiness {
    pub capture: ComponentReadiness,
    pub db: ComponentReadiness,
    pub embed_model: ComponentReadiness,
    pub sidecar: ComponentReadiness,
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

/// Data-changing enrichment job completion (`job_completed` event). Carries enough
/// identity for the UI to refresh frame/search/insights data surgically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct JobCompleted {
    #[ts(type = "number")]
    pub job_id: i64,
    pub kind: JobKind,
    #[ts(type = "number")]
    pub frame_id: i64,
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

/// Sidecar status update (`sidecar_status` event). `lane` says which model is (or was
/// last) resident — vision vs. answer — so the UI can label the engine truthfully instead
/// of guessing from the filename. `None` when no model has loaded yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct SidecarStatus {
    pub state: SidecarState,
    pub model: Option<String>,
    pub lane: Option<ModelLane>,
}

/// Phase of a model download (`model_download` event). Drives the progress UI so a
/// multi-GB model fetch communicates progress + completion/error instead of just opaque
/// network activity (`03 §6/§7`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum ModelDownloadPhase {
    Downloading,
    Done,
    Failed,
}

/// Progress of a model download for one lane. `total_bytes` is `None` when the size could
/// not be probed (the UI then shows bytes-downloaded without a percentage). `error` is set
/// only on the `Failed` phase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct ModelDownloadStatus {
    pub lane: ModelLane,
    pub model: Option<String>,
    pub phase: ModelDownloadPhase,
    // `#[ts(type = "number")]`: Tauri's JSON wire delivers 64-bit ints as JS numbers, and
    // byte counts stay well under 2^53 — same convention as `JobStats`.
    #[ts(type = "number")]
    pub downloaded_bytes: u64,
    #[ts(type = "number | null")]
    pub total_bytes: Option<u64>,
    pub error: Option<String>,
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
            ("FrameMeta", FrameMeta::inline()),
            ("TimelineBucket", TimelineBucket::inline()),
            ("InsightsSummary", InsightsSummary::inline()),
            ("StorageStats", StorageStats::inline()),
            ("AppSuppression", AppSuppression::inline()),
            ("FrameDetail", FrameDetail::inline()),
            ("VisionTarget", VisionTarget::inline()),
            ("CaptureTick", CaptureTick::inline()),
            ("AnswerEvent", AnswerEvent::inline()),
            ("AnswerDelta", AnswerDelta::inline()),
            ("JobCompleted", JobCompleted::inline()),
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

//! Core domain types shared across modules.
//!
//! Types that hold raw pixel data ([`CapturedFrame`]) are **internal only** — they
//! never cross the typed IPC boundary. Types that the UI needs derive [`ts_rs::TS`]
//! and live in [`crate::ipc`]; a few plain records here ([`VisionAnalysis`],
//! [`MonitorInfo`]) are exported because the IPC layer embeds them.

use std::sync::Arc;

use image::RgbaImage;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A connected monitor the capture source can see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct MonitorInfo {
    pub index: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

/// A single captured (already diff-gated as *changed*) frame.
///
/// Internal: holds the full-resolution RGBA pixels behind an [`Arc`] and never
/// crosses IPC. OCR runs on these pixels **before** any JPEG resize (`03 §8`).
///
/// `app_hint` / `window_title` carry the foreground-window context the capture
/// source already reads for the `privacy.excluded_apps` gate (`03 §8`); the kernel
/// copies them onto the stored [`NewFrame`] so the timeline has context without a
/// second OS call. `None` when the foreground window can't be resolved.
#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub monitor_index: u32,
    pub width: u32,
    pub height: u32,
    /// Capture time, unix epoch milliseconds.
    pub captured_at: i64,
    pub pixels: Arc<RgbaImage>,
    pub content_hash: String,
    /// Foreground app/process name at capture time (privacy gate by-product).
    pub app_hint: Option<String>,
    /// Foreground window title at capture time.
    pub window_title: Option<String>,
}

/// Origin of a text span / the primary text of a frame (`03 §3b`). Serializes to
/// the DB `source` / `primary_source` columns (`'ocr' | 'uia'`, `03 §4`). UIA is
/// modelled now but only produced from 0.2.1 (`07` #48); 0.2.0 OCR is always `Ocr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum TextSource {
    Ocr,
    Uia,
}

impl TextSource {
    /// The DB token for the `source` / `primary_source` columns (`03 §4` CHECK).
    pub fn as_db_str(self) -> &'static str {
        match self {
            TextSource::Ocr => "ocr",
            TextSource::Uia => "uia",
        }
    }

    /// Parses the DB token back into a [`TextSource`]; `None` on an unknown token.
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "ocr" => Some(TextSource::Ocr),
            "uia" => Some(TextSource::Uia),
            _ => None,
        }
    }
}

/// Classified role of a text span (`03 §3b`). Serializes to the DB `role` column
/// (`03 §4` CHECK). PR2 emits every span as [`TextRole::Unknown`]; PR3's classifier
/// assigns the real roles and drops non-`content` spans from `content_text`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum TextRole {
    Content,
    Chrome,
    Background,
    System,
    Unknown,
}

impl TextRole {
    /// The DB token for the `role` column (`03 §4` CHECK).
    pub fn as_db_str(self) -> &'static str {
        match self {
            TextRole::Content => "content",
            TextRole::Chrome => "chrome",
            TextRole::Background => "background",
            TextRole::System => "system",
            TextRole::Unknown => "unknown",
        }
    }

    /// Parses the DB token back into a [`TextRole`]; `None` on an unknown token.
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "content" => Some(TextRole::Content),
            "chrome" => Some(TextRole::Chrome),
            "background" => Some(TextRole::Background),
            "system" => Some(TextRole::System),
            "unknown" => Some(TextRole::Unknown),
            _ => None,
        }
    }
}

/// Why a span was excluded from `content_text` (`03 §3b`). `Option<SuppressReason>`
/// maps to the nullable `text_spans.suppress_reason` column — `None` = a searchable,
/// non-suppressed span (no redundant in-enum `None` variant, `03 §4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum SuppressReason {
    StaticChrome,
    SystemUi,
    BackgroundWindow,
}

impl SuppressReason {
    /// The DB token for the `suppress_reason` column (`03 §4` CHECK).
    pub fn as_db_str(self) -> &'static str {
        match self {
            SuppressReason::StaticChrome => "static_chrome",
            SuppressReason::SystemUi => "system_ui",
            SuppressReason::BackgroundWindow => "background_window",
        }
    }

    /// Parses the DB token back into a [`SuppressReason`]; `None` on an unknown token
    /// (including the SQL `NULL` → "not suppressed" case the caller handles).
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "static_chrome" => Some(SuppressReason::StaticChrome),
            "system_ui" => Some(SuppressReason::SystemUi),
            "background_window" => Some(SuppressReason::BackgroundWindow),
            _ => None,
        }
    }
}

/// One OCR/UIA text span with normalized `[0,1]` geometry (`03 §3b`). Carried on
/// [`OcrResult::spans`] and persisted to `text_spans` (`03 §4`). Internal — it never
/// crosses the typed IPC boundary (`FrameDetail` surfaces only raw/content text).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextSpan {
    /// The recognized text, verbatim.
    pub text: String,
    /// Normalized form used for chrome-signature matching (`03 §3b`,
    /// [`normalize_text`]).
    pub normalized_text: String,
    pub source: TextSource,
    pub role: TextRole,
    /// Normalized `[0,1]` bounding box (origin top-left), relative to the
    /// full-resolution frame the OCR ran on.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Whether the span is included in searchable text. PR2 marks every span
    /// searchable; PR3 sets this from the classified role.
    pub is_searchable: bool,
    pub suppress_reason: Option<SuppressReason>,
}

/// Normalizes span text for chrome-signature matching and dedup (`03 §3b`):
/// lowercased, internal whitespace collapsed to single spaces, ends trimmed. Shared
/// so the OCR producer and PR3's classifier derive identical signatures.
pub fn normalize_text(s: &str) -> String {
    // Single output allocation (capacity-hinted): collapse internal whitespace by
    // joining words with single spaces, then lowercase. Hot path — one span per OCR
    // word — so avoid the intermediate Vec + join allocations.
    let mut result = String::with_capacity(s.len());
    let mut words = s.split_whitespace();
    if let Some(first) = words.next() {
        result.push_str(first);
        for word in words {
            result.push(' ');
            result.push_str(word);
        }
    }
    result.to_lowercase()
}

/// Result of running OCR over a [`CapturedFrame`]. `spans` carry per-word geometry
/// for the 0.2.x text-signal pipeline (`03 §3/§3b`); empty when the engine produced
/// no words.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    pub text: String,
    pub mean_confidence: f32,
    pub engine: String,
    pub spans: Vec<TextSpan>,
}

/// A dense embedding vector. Length always equals the provider's
/// [`EmbeddingProvider::dim`](crate::EmbeddingProvider::dim) (768).
#[derive(Debug, Clone, PartialEq)]
pub struct Embedding(pub Vec<f32>);

impl Embedding {
    /// Number of dimensions in the vector.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the vector has no dimensions (an invalid/empty embedding).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Deferred vision-tagging output for a frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct VisionAnalysis {
    pub description: String,
    pub activity_type: Option<String>,
    pub app_hint: Option<String>,
    pub confidence: f32,
    pub model: String,
}

/// A new frame row to insert into the [`Store`](crate::Store).
#[derive(Debug, Clone)]
pub struct NewFrame {
    /// Capture time, unix epoch milliseconds.
    pub captured_at: i64,
    pub monitor_index: u32,
    pub width: u32,
    pub height: u32,
    /// Relative path to the stored JPEG on disk.
    pub image_path: String,
    pub content_hash: String,
    pub app_hint: Option<String>,
    pub window_title: Option<String>,
    pub browser_url: Option<String>,
}

/// The minimal frame inputs an enrichment worker needs to embed a frame: the
/// stored JPEG's relative path and the OCR text (if any was recognized). One cheap
/// read for the embedding worker (`03 §5`), vs the full [`FrameDetail`] the
/// `get_frame` command assembles. `ocr_text` is `None` when no OCR row exists yet.
#[derive(Debug, Clone)]
pub struct FrameEnrichmentInput {
    pub image_path: String,
    pub ocr_text: Option<String>,
}

/// Configuration handed to a [`CaptureSource`](crate::CaptureSource) at
/// construction. The kernel derives it from [`Settings`](crate::Settings) when
/// capture starts (`03 §8`); it is shared here so the kernel can build it and the
/// capture impl can consume it without either depending on the other (`03 §2`).
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// Delay between capture cycles, ms (`capture.interval_ms`).
    pub interval_ms: u32,
    /// Monitor indices to capture; empty = all (`capture.monitors`).
    pub monitors: Vec<u32>,
    /// Normalized [0,1] change ratio above which a frame is kept
    /// (`capture.diff_threshold`).
    pub diff_threshold: f32,
    /// Foreground app/process names whose frames are skipped — case-insensitive
    /// substring match (`privacy.excluded_apps`).
    pub excluded_apps: Vec<String>,
    /// Pause capture entirely while the workstation is locked
    /// (`privacy.pause_on_lock`).
    pub pause_on_lock: bool,
}

/// Origin of an embedded text chunk. Serializes to the DB `source` column
/// (`'ocr' | 'vision_description'`, `03 §4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkSource {
    Ocr,
    VisionDescription,
}

/// A retrieved context chunk handed to the answer provider for grounding.
#[derive(Debug, Clone)]
pub struct RetrievedChunk {
    pub frame_id: i64,
    pub text: String,
    pub score: f32,
    /// Capture time of the source frame, unix epoch milliseconds.
    pub captured_at: i64,
}

/// Options controlling a single `answer` call.
#[derive(Debug, Clone, Copy)]
pub struct AnswerOpts {
    pub thinking: bool,
    pub max_tokens: u32,
}

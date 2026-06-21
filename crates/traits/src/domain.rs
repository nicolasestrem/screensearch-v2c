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

/// Result of running OCR over a [`CapturedFrame`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    pub text: String,
    pub mean_confidence: f32,
    pub engine: String,
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

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
#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub monitor_index: u32,
    pub width: u32,
    pub height: u32,
    /// Capture time, unix epoch milliseconds.
    pub captured_at: i64,
    pub pixels: Arc<RgbaImage>,
    pub content_hash: String,
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

//! [`VisionProvider`] over the llama.cpp sidecar (`03 §3/§6`). On-demand/timer/idle
//! vision tagging: a captured frame's JPEG is sent to the vision model as a base64
//! data URL, and the model's reply is parsed into a [`VisionAnalysis`].
//!
//! The model is asked for compact JSON (`description`/`activity_type`/`app_hint`/
//! `confidence`); if it answers in prose instead, the raw text becomes the description
//! with an "unknown" confidence sentinel — we never fabricate a score (consistent with
//! the OCR-confidence decision in `06_PATCH_PLAN`).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::Engine as _;
use serde::Deserialize;
use traits::{ModelTier, VisionAnalysis, VisionProvider};

use crate::client::ChatMessage;
use crate::download;
use crate::models::{self, ModelLane, ModelSpec};
use crate::supervisor::ModelSupervisor;

/// "Unknown" confidence sentinel (the model gave no score) — mirrors the OCR-confidence
/// convention so the UI can treat negatives as "unknown" uniformly.
pub const CONFIDENCE_UNKNOWN: f32 = -1.0;

const VISION_MAX_TOKENS: u32 = 512;
const JPEG_QUALITY: u8 = 80;
const VISION_PROMPT: &str = "You are analyzing a single screenshot of a user's screen. \
Reply with ONLY a compact JSON object and nothing else, in this exact shape: \
{\"description\": \"one or two sentences describing what is on screen\", \
\"activity_type\": \"a short label such as coding, browsing, email, reading, chat, terminal, design, video\", \
\"app_hint\": \"the application name if identifiable, else null\", \
\"confidence\": 0.0}. The confidence is your certainty from 0.0 to 1.0.";

/// The vision lane provider. Holds the current tier (changed via `set_tier`) and lazily
/// downloads the model on first use, then drives it through the supervisor.
pub struct VisionSidecar {
    supervisor: Arc<ModelSupervisor>,
    models_root: PathBuf,
    tier: RwLock<ModelTier>,
    ngl: u32,
}

impl VisionSidecar {
    pub fn new(
        supervisor: Arc<ModelSupervisor>,
        models_root: PathBuf,
        tier: ModelTier,
        ngl: u32,
    ) -> Self {
        Self {
            supervisor,
            models_root,
            tier: RwLock::new(tier),
            ngl,
        }
    }

    /// Updates the active vision tier; the supervisor switches the sidecar model on the
    /// next request (the resolved GGUF differs, so it restarts).
    pub fn set_tier(&self, tier: ModelTier) {
        *self.tier.write().expect("vision tier lock") = tier;
    }

    /// Resolves the current tier's [`ModelSpec`], downloading the model on first use.
    async fn ensure_spec(&self) -> Result<ModelSpec> {
        let tier = *self.tier.read().expect("vision tier lock");
        if let Some(spec) =
            models::resolve_spec(&self.models_root, ModelLane::Vision, tier, self.ngl)
        {
            return Ok(spec);
        }
        download::ensure_model(&self.models_root, ModelLane::Vision, tier)
            .await
            .context("download vision model")?;
        models::resolve_spec(&self.models_root, ModelLane::Vision, tier, self.ngl)
            .context("vision model files missing after download")
    }
}

#[async_trait]
impl VisionProvider for VisionSidecar {
    async fn analyze(&self, image: &image::RgbaImage) -> Result<VisionAnalysis> {
        let spec = self.ensure_spec().await?;
        let model = model_label(&spec);
        let lease = self.supervisor.acquire(spec).await?;
        let data_url = encode_data_url(image)?;
        let content = lease
            .client()
            .complete(
                vec![ChatMessage::image(VISION_PROMPT, data_url)],
                VISION_MAX_TOKENS,
            )
            .await
            .context("vision completion")?;
        Ok(parse_vision(&content, &model))
    }
}

/// JSON the vision model is asked to return.
#[derive(Deserialize)]
struct VisionJson {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    activity_type: Option<String>,
    #[serde(default)]
    app_hint: Option<String>,
    #[serde(default)]
    confidence: Option<f32>,
}

/// Parses the model reply into a [`VisionAnalysis`], falling back to raw text as the
/// description (with an unknown-confidence sentinel) if it isn't valid JSON.
fn parse_vision(content: &str, model: &str) -> VisionAnalysis {
    if let Some(json) = extract_json(content) {
        if let Ok(v) = serde_json::from_str::<VisionJson>(&json) {
            if let Some(desc) = v.description.filter(|s| !s.trim().is_empty()) {
                return VisionAnalysis {
                    description: desc,
                    activity_type: v.activity_type.filter(|s| !s.trim().is_empty()),
                    app_hint: v.app_hint.filter(|s| !s.trim().is_empty() && s != "null"),
                    confidence: v.confidence.unwrap_or(CONFIDENCE_UNKNOWN),
                    model: model.to_string(),
                };
            }
        }
    }
    VisionAnalysis {
        description: content.trim().to_string(),
        activity_type: None,
        app_hint: None,
        confidence: CONFIDENCE_UNKNOWN,
        model: model.to_string(),
    }
}

/// Extracts the outermost `{ … }` JSON object from a reply (tolerates code fences and
/// stray prose around it).
fn extract_json(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    (end > start).then(|| s[start..=end].to_string())
}

/// Encodes an RGBA frame as a `data:image/jpeg;base64,…` URL for the OpenAI image API.
fn encode_data_url(img: &image::RgbaImage) -> Result<String> {
    let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, JPEG_QUALITY)
        .encode_image(&rgb)
        .context("encode frame as jpeg")?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&buf)
    ))
}

/// The GGUF filename, used as the `model` field recorded with the analysis.
fn model_label(spec: &ModelSpec) -> String {
    spec.gguf_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("vision")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_well_formed_json() {
        let reply = r#"{"description":"A code editor with Rust","activity_type":"coding","app_hint":"VS Code","confidence":0.82}"#;
        let v = parse_vision(reply, "qwen3-vl-4b-q4_k_m.gguf");
        assert_eq!(v.description, "A code editor with Rust");
        assert_eq!(v.activity_type.as_deref(), Some("coding"));
        assert_eq!(v.app_hint.as_deref(), Some("VS Code"));
        assert!((v.confidence - 0.82).abs() < 1e-6);
        assert_eq!(v.model, "qwen3-vl-4b-q4_k_m.gguf");
    }

    #[test]
    fn tolerates_code_fences_and_prose() {
        let reply = "Here is the result:\n```json\n{\"description\": \"A browser\", \"confidence\": 0.5}\n```";
        let v = parse_vision(reply, "m");
        assert_eq!(v.description, "A browser");
        assert!((v.confidence - 0.5).abs() < 1e-6);
    }

    #[test]
    fn falls_back_to_raw_text_on_non_json() {
        let v = parse_vision("The screen shows a spreadsheet.", "m");
        assert_eq!(v.description, "The screen shows a spreadsheet.");
        assert_eq!(v.confidence, CONFIDENCE_UNKNOWN);
        assert!(v.activity_type.is_none());
    }

    #[test]
    fn null_app_hint_string_becomes_none() {
        let reply = r#"{"description":"x","app_hint":"null","confidence":0.1}"#;
        let v = parse_vision(reply, "m");
        assert!(v.app_hint.is_none());
    }
}

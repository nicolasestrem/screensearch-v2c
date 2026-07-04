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
use traits::{ModelTier, SidecarParams, VisionAnalysis, VisionProvider};

use crate::client::ChatMessage;
use crate::models::{self, ModelLane, ModelSpec};
use crate::supervisor::ModelSupervisor;

/// "Unknown" confidence sentinel (the model gave no score) — mirrors the OCR-confidence
/// convention so the UI can treat negatives as "unknown" uniformly.
pub const CONFIDENCE_UNKNOWN: f32 = -1.0;

/// The closed set of activity labels we accept from the vision model. Anything else —
/// a free-form phrase, or the model's own "unknown" — is normalised to `None` rather
/// than stored as a tag, so the Insights activity breakdown stays a small, meaningful
/// enum and we never persist a label we didn't ask for (`07` #20). The `VISION_PROMPT`
/// and the response-format schema enumerate the same set; keep all three in sync.
const ACTIVITY_TYPES: &[&str] = &[
    "coding", "browsing", "email", "reading", "chat", "terminal", "design", "video",
];

const VISION_MAX_TOKENS: u32 = 512;
const JPEG_QUALITY: u8 = 80;
/// Longest edge (px) of the image actually sent to the VLM. A native full-screen capture
/// (e.g. 3440×1440) tokenizes to ~4000 vision tokens and overflowed the sidecar context with
/// `exceed_context_size_error`; capping the longest edge keeps the request well under the
/// window *and* speeds up prefill. The stored capture (timeline/reconstruction) keeps full
/// resolution — only the tag request is downscaled (`encode_data_url`).
///
/// **1280, matching the pre-WebP stored width.** The switch to native-resolution storage
/// silently grew the tag request from 1280×536 (the old stored size) to a 1568 px cap —
/// release-build profiling attributed ~97 % of the resulting ~2.5× vision-throughput
/// regression to the sidecar processing that 1.5×-pixel request (`06` #22). Capping back
/// to 1280 restores request parity with the v0.2.x baseline, trading marginal tag
/// fidelity for throughput.
const VISION_MAX_EDGE: u32 = 1280;
// No literal value is shown for `confidence` — an earlier prompt pinned `0.0` and the
// model dutifully echoed it, recording a fabricated score (`07` #20). The field is
// described, not demonstrated; the response-format grammar and `parse_vision` enforce
// the shape and honesty.
const VISION_PROMPT: &str = "You are analyzing a single screenshot of a user's screen. \
Reply with ONLY a compact JSON object and nothing else, with exactly these fields: \
\"description\": one or two sentences describing what is on screen; \
\"activity_type\": one of coding, browsing, email, reading, chat, terminal, design, video, \
or null if none of those clearly fits (do not guess); \
\"app_hint\": the application name if identifiable, otherwise null; \
\"confidence\": a number between 0.0 and 1.0 giving your certainty in this analysis, \
judged from how clearly the screenshot supports it.";

/// The OpenAI-style `response_format` handed to `llama-server` for vision tagging. The
/// server turns the JSON schema into a sampling grammar, so the model must emit an object
/// whose `activity_type` is one of [`ACTIVITY_TYPES`] *or* `null`, with a numeric
/// `confidence` (`07` #20). This guarantees shape, not meaning — `parse_vision` still
/// validates the values.
///
/// `activity_type` is nullable and **not** required: a low-signal frame (a blank desktop,
/// a lock screen, a synthetic test image) has no identifiable activity, and forcing the
/// grammar to pick one of the eight labels would mirror an arbitrary tag into the Insights
/// breakdown. Allowing `null` lets the model decline, and `normalize_activity` keeps `None`.
fn vision_response_format() -> serde_json::Value {
    let mut activity_enum: Vec<serde_json::Value> = ACTIVITY_TYPES
        .iter()
        .map(|&a| serde_json::Value::from(a))
        .collect();
    activity_enum.push(serde_json::Value::Null);
    serde_json::json!({
        "type": "json_schema",
        "json_schema": {
            "name": "screen_vision",
            "schema": {
                "type": "object",
                "properties": {
                    "description": { "type": "string" },
                    "activity_type": { "type": ["string", "null"], "enum": activity_enum },
                    "app_hint": { "type": ["string", "null"] },
                    "confidence": { "type": "number" }
                },
                "required": ["description", "confidence"]
            }
        }
    })
}

/// The vision lane provider. Holds the current tier (changed via `set_tier`) and lazily
/// downloads the model on first use, then drives it through the supervisor.
pub struct VisionSidecar {
    supervisor: Arc<ModelSupervisor>,
    downloader: Arc<crate::download::ModelDownloader>,
    models_root: PathBuf,
    tier: RwLock<ModelTier>,
    launch: RwLock<SidecarParams>,
}

impl VisionSidecar {
    pub fn new(
        supervisor: Arc<ModelSupervisor>,
        downloader: Arc<crate::download::ModelDownloader>,
        models_root: PathBuf,
        tier: ModelTier,
        params: SidecarParams,
    ) -> Self {
        Self {
            supervisor,
            downloader,
            models_root,
            tier: RwLock::new(tier),
            launch: RwLock::new(params),
        }
    }

    /// Updates the active vision tier; the supervisor switches the sidecar model on the
    /// next request (the resolved GGUF differs, so it restarts).
    pub fn set_tier(&self, tier: ModelTier) {
        *self.tier.write().expect("vision tier lock") = tier;
    }

    /// Updates launch options for the next request (or model restart). A change to any
    /// tuning field makes the next `resolve_spec` differ, so the supervisor relaunches.
    pub fn set_launch_options(&self, params: SidecarParams) {
        *self.launch.write().expect("vision launch lock") = params;
    }

    /// Resolves the current tier's [`ModelSpec`], downloading the model on first use.
    async fn ensure_spec(&self) -> Result<ModelSpec> {
        let tier = *self.tier.read().expect("vision tier lock");
        let params = self.launch.read().expect("vision launch lock").clone();
        if let Some(spec) =
            models::resolve_spec(&self.models_root, ModelLane::Vision, tier, params.clone())
        {
            return Ok(spec);
        }
        self.downloader
            .ensure(ModelLane::Vision, tier)
            .await
            .context("download vision model")?;
        models::resolve_spec(&self.models_root, ModelLane::Vision, tier, params)
            .context("vision model files missing after download")
    }

    /// Eagerly loads the current vision model into the sidecar (the manual "Load" control
    /// for the vision lane). Downloads on first use, then keeps it resident until the
    /// idle-TTL or a manual unload reclaims it.
    pub async fn preload(&self) -> Result<()> {
        let spec = self.ensure_spec().await?;
        self.supervisor.preload(spec).await
    }
}

#[async_trait]
impl VisionProvider for VisionSidecar {
    async fn analyze(&self, image: &image::RgbaImage) -> Result<VisionAnalysis> {
        let spec = self.ensure_spec().await?;
        let model = model_label(&spec);
        let acquire_started = std::time::Instant::now();
        let lease = self.supervisor.acquire(spec).await?;
        let acquire_ms = acquire_started.elapsed().as_millis() as u64;
        let prep_started = std::time::Instant::now();
        let data_url = encode_data_url(image)?;
        let prep_ms = prep_started.elapsed().as_millis() as u64;
        let (req_width, req_height) = vlm_request_dims(image.width(), image.height());
        let complete_started = std::time::Instant::now();
        let content = lease
            .client()
            .complete(
                vec![ChatMessage::image(VISION_PROMPT, data_url)],
                VISION_MAX_TOKENS,
                Some(vision_response_format()),
            )
            .await
            .context("vision completion")?;
        // Per-stage timing: sidecar lease acquisition (includes a cold model spawn),
        // CPU request prep (downscale + JPEG + base64), and the sidecar completion
        // round-trip, with the request image dimensions. Privacy-safe: durations +
        // dimensions only.
        tracing::info!(
            acquire_ms,
            prep_ms,
            complete_ms = complete_started.elapsed().as_millis() as u64,
            req_width,
            req_height,
            "vision analyze timing"
        );
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
/// description (with an unknown-confidence sentinel) if it isn't valid JSON. The
/// `activity_type` and `confidence` are normalised defensively (see helpers below) so a
/// model that ignores the schema can't slip a free-form label or a fabricated score in.
fn parse_vision(content: &str, model: &str) -> VisionAnalysis {
    if let Some(json) = extract_json(content) {
        if let Ok(v) = serde_json::from_str::<VisionJson>(&json) {
            if let Some(desc) = v.description.filter(|s| !s.trim().is_empty()) {
                return VisionAnalysis {
                    description: desc,
                    activity_type: normalize_activity(v.activity_type),
                    app_hint: normalize_app_hint(v.app_hint),
                    confidence: normalize_confidence(v.confidence),
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

/// Maps a model-supplied label to our closed [`ACTIVITY_TYPES`] set (case/space
/// insensitive), or `None`. Off-list or empty values — including the model's "unknown" —
/// collapse to `None` so we never store a label we didn't define (`07` #20).
fn normalize_activity(raw: Option<String>) -> Option<String> {
    let label = raw?.trim().to_ascii_lowercase();
    ACTIVITY_TYPES
        .iter()
        .copied()
        .find(|&a| a == label.as_str())
        .map(str::to_string)
}

/// Keeps a real application name, dropping empties and the literal string `"null"` — the
/// model sometimes emits `null`/`NULL`/`Null` as *text* instead of a JSON null, so the
/// match is case-insensitive (`07` #20). The surviving value is trimmed.
fn normalize_app_hint(raw: Option<String>) -> Option<String> {
    let hint = raw?;
    let trimmed = hint.trim();
    (!trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("null")).then(|| trimmed.to_string())
}

/// Coerces a model-supplied confidence into a real score or the unknown sentinel. Only a
/// finite value in `(0.0, 1.0]` is trusted; `0.0` (the placeholder older prompts echoed),
/// negatives, `NaN` and values above `1.0` all become [`CONFIDENCE_UNKNOWN`] — we record
/// "unknown" rather than a fabricated certainty (`07` #20, mirroring the OCR sentinel).
fn normalize_confidence(raw: Option<f32>) -> f32 {
    match raw {
        Some(c) if c.is_finite() && c > 0.0 && c <= 1.0 => c,
        _ => CONFIDENCE_UNKNOWN,
    }
}

/// Extracts the outermost `{ … }` JSON object from a reply (tolerates code fences and
/// stray prose around it).
fn extract_json(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    (end > start).then(|| s[start..=end].to_string())
}

/// Encodes an RGBA frame as a `data:image/jpeg;base64,…` URL for the OpenAI image API,
/// downscaling oversized frames first (see [`downscale_for_vlm`]).
fn encode_data_url(img: &image::RgbaImage) -> Result<String> {
    let rgb = downscale_for_vlm(img).to_rgb8();
    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, JPEG_QUALITY)
        .encode_image(&rgb)
        .context("encode frame as jpeg")?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&buf)
    ))
}

/// The dimensions [`downscale_for_vlm`] produces for a `width`×`height` source — the size
/// of the image actually sent to the VLM. Split out so the analyze timing log can record
/// the request dimensions without re-deriving them from the encoded payload.
fn vlm_request_dims(width: u32, height: u32) -> (u32, u32) {
    // A zero-area source passes through untouched: scaling it would hand
    // `image::imageops::resize` an empty source buffer, which can panic.
    if width == 0 || height == 0 || width.max(height) <= VISION_MAX_EDGE {
        return (width, height);
    }
    let ratio = f64::from(VISION_MAX_EDGE) / f64::from(width.max(height));
    let nwidth = ((f64::from(width) * ratio).round() as u32).max(1);
    let nheight = ((f64::from(height) * ratio).round() as u32).max(1);
    (nwidth, nheight)
}

/// Downscales a frame so its longest edge is at most [`VISION_MAX_EDGE`], preserving aspect
/// ratio; returns it unscaled when it already fits. A native full-screen capture is otherwise
/// thousands of vision tokens (`exceed_context_size_error`); shrinking the request image keeps
/// it under the sidecar window and cuts prefill time. Triangle filtering is fast and more than
/// enough fidelity for activity/app tagging.
fn downscale_for_vlm(img: &image::RgbaImage) -> image::DynamicImage {
    let (width, height) = (img.width(), img.height());
    let (nwidth, nheight) = vlm_request_dims(width, height);
    if (nwidth, nheight) == (width, height) {
        return image::DynamicImage::ImageRgba8(img.clone());
    }
    // Resize the borrowed frame directly — `DynamicImage::resize` would force a full-resolution
    // clone (~20 MB for a 3440×1440 capture) only to discard it. The fitted dimensions come from
    // `vlm_request_dims` (round-to-nearest scale, ratio = VISION_MAX_EDGE / longest edge), the
    // same math `resize` applies, so the cap behavior is unchanged.
    let resized =
        image::imageops::resize(img, nwidth, nheight, image::imageops::FilterType::Triangle);
    image::DynamicImage::ImageRgba8(resized)
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

    /// Decodes a `data:image/jpeg;base64,…` URL back to an image so a test can inspect the
    /// dimensions actually sent to the VLM.
    fn decode_data_url(url: &str) -> image::DynamicImage {
        let b64 = url
            .strip_prefix("data:image/jpeg;base64,")
            .expect("jpeg data url prefix");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .expect("valid base64");
        image::load_from_memory(&bytes).expect("decode jpeg")
    }

    #[test]
    fn downscales_oversized_frame_to_max_edge() {
        // A native ultra-wide capture (3440×1440) tokenizes to ~4000 vision tokens and
        // overflowed the sidecar context; the request image must be capped to VISION_MAX_EDGE
        // on its longest edge, aspect ratio preserved.
        let img = image::RgbaImage::new(3440, 1440);
        let decoded = decode_data_url(&encode_data_url(&img).unwrap());
        assert_eq!(
            decoded.width().max(decoded.height()),
            VISION_MAX_EDGE,
            "longest edge must be capped"
        );
        // 1280×536 = the pre-WebP stored size; the cap pins request parity with it.
        assert_eq!((decoded.width(), decoded.height()), (1280, 536));
    }

    #[test]
    fn vlm_request_dims_match_encoded_output() {
        // The timing log's request dimensions must agree with what `encode_data_url`
        // actually sends (both capped and pass-through cases). 1280 matches the
        // pre-WebP stored width (see `VISION_MAX_EDGE`).
        assert_eq!(vlm_request_dims(3440, 1440), (1280, 536));
        assert_eq!(vlm_request_dims(800, 600), (800, 600));
        assert_eq!(vlm_request_dims(1440, 3440), (536, 1280), "portrait cap");
        assert_eq!(
            vlm_request_dims(0, 2000),
            (0, 2000),
            "zero width passes through"
        );
        assert_eq!(
            vlm_request_dims(2000, 0),
            (2000, 0),
            "zero height passes through"
        );
    }

    #[test]
    fn small_frame_passes_through_at_native_size() {
        // A frame already within the cap is sent unscaled — no needless quality loss.
        let img = image::RgbaImage::new(800, 600);
        let decoded = decode_data_url(&encode_data_url(&img).unwrap());
        assert_eq!((decoded.width(), decoded.height()), (800, 600));
    }

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
    fn null_app_hint_string_is_dropped_case_insensitively() {
        // The model sometimes emits the *text* "null" instead of a JSON null, in any case
        // and with stray whitespace — none of these should be stored as an app name.
        for raw in ["null", "NULL", "Null", "  null  "] {
            let reply = format!(r#"{{"description":"x","app_hint":"{raw}","confidence":0.1}}"#);
            let v = parse_vision(&reply, "m");
            assert!(v.app_hint.is_none(), "app_hint {raw:?} must be None");
        }
    }

    #[test]
    fn app_hint_is_trimmed_and_kept_when_real() {
        let reply = r#"{"description":"x","app_hint":"  VS Code  ","confidence":0.1}"#;
        let v = parse_vision(reply, "m");
        assert_eq!(v.app_hint.as_deref(), Some("VS Code"));
    }

    #[test]
    fn explicit_null_activity_type_becomes_none() {
        // The schema permits `null` for an unidentifiable activity; it must not be coerced
        // into a label.
        let reply = r#"{"description":"A blank desktop","activity_type":null,"confidence":0.4}"#;
        let v = parse_vision(reply, "m");
        assert!(v.activity_type.is_none());
        assert!((v.confidence - 0.4).abs() < 1e-6);
    }

    #[test]
    fn response_format_allows_null_activity_and_drops_it_from_required() {
        let fmt = vision_response_format();
        let schema = &fmt["json_schema"]["schema"];
        let activity = &schema["properties"]["activity_type"];
        // Nullable type and a `null` member in the enum, so the grammar can decline.
        assert!(activity["type"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t == "null"));
        assert!(activity["enum"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.is_null()));
        // Only description + confidence are forced; activity_type is optional.
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(required, ["description", "confidence"]);
    }

    #[test]
    fn zero_confidence_becomes_unknown_sentinel() {
        // Older prompts pinned `"confidence": 0.0` and the model echoed it; a literal 0.0
        // must be recorded as "unknown", never as a real score (`07` #20).
        let reply = r#"{"description":"A terminal","activity_type":"terminal","confidence":0.0}"#;
        let v = parse_vision(reply, "m");
        assert_eq!(v.confidence, CONFIDENCE_UNKNOWN);
        assert_eq!(v.activity_type.as_deref(), Some("terminal"));
    }

    #[test]
    fn missing_confidence_becomes_unknown_sentinel() {
        let reply = r#"{"description":"x","activity_type":"coding"}"#;
        let v = parse_vision(reply, "m");
        assert_eq!(v.confidence, CONFIDENCE_UNKNOWN);
        assert_eq!(v.activity_type.as_deref(), Some("coding"));
    }

    #[test]
    fn out_of_range_confidence_becomes_unknown_sentinel() {
        for bad in ["-0.5", "1.5", "42"] {
            let reply = format!(r#"{{"description":"x","confidence":{bad}}}"#);
            let v = parse_vision(&reply, "m");
            assert_eq!(
                v.confidence, CONFIDENCE_UNKNOWN,
                "confidence {bad} must be unknown"
            );
        }
    }

    #[test]
    fn off_enum_activity_type_becomes_none() {
        // Free-form labels, the model's own "unknown", and empties are dropped — never
        // stored as a tag (`07` #20).
        for bad in ["unknown", "gaming", "spreadsheet work", ""] {
            let reply =
                format!(r#"{{"description":"x","activity_type":"{bad}","confidence":0.7}}"#);
            let v = parse_vision(&reply, "m");
            assert!(v.activity_type.is_none(), "activity {bad:?} must be None");
        }
    }

    #[test]
    fn activity_type_is_normalised_case_and_space_insensitively() {
        let reply = r#"{"description":"x","activity_type":"  Coding ","confidence":0.7}"#;
        let v = parse_vision(reply, "m");
        assert_eq!(v.activity_type.as_deref(), Some("coding"));
    }
}

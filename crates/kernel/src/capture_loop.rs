//! The always-on capture pipeline (`02 §5`, `03 §5`): pull a *changed* frame from
//! the [`CaptureSource`], OCR it on full resolution, JPEG-encode it for storage,
//! write `frames` + `ocr_text`, enqueue an `embed_text` job, and emit a
//! `capture_tick`. This is the cheap half of the system; everything expensive is
//! deferred to the job queue (consumed in P3+).
//!
//! The loop is shell-agnostic and driven by the traits, so it is exercised
//! end-to-end by fakes (`tests/pipeline.rs`) with no Windows APIs involved.

use std::path::PathBuf;
use std::sync::Arc;

use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use tokio::sync::{broadcast, watch};
use traits::{
    CaptureSource, CaptureTick, CapturedFrame, JobKind, NewFrame, NewJob, OcrProvider, Result,
    Store,
};

use crate::events::KernelEvent;

/// Everything the capture loop needs besides the [`CaptureSource`] itself. Built
/// by the kernel on [`start_capture`](crate::Kernel::start_capture) and reused for
/// the loop's lifetime.
pub struct LoopCtx {
    pub store: Arc<dyn Store>,
    pub ocr: Arc<dyn OcrProvider>,
    /// Absolute directory under which JPEGs are written (`<app-data>/frames`).
    pub frames_dir: PathBuf,
    pub events: broadcast::Sender<KernelEvent>,
    /// `enrich.embed_text` — whether each stored frame enqueues an `embed_text` job.
    pub enrich_embed_text: bool,
    /// `enrich.image_embeddings` — whether each stored frame also enqueues an
    /// `embed_image` job (optional visual recall, off by default).
    pub enrich_image_embeddings: bool,
    /// `storage.jpeg_quality` (0–100).
    pub jpeg_quality: u8,
    /// `storage.max_width` — JPEGs wider than this are downscaled (aspect kept).
    pub max_width: u32,
}

/// Runs the capture loop until the source yields `None` (shutdown) or `stop` fires.
/// Per-frame errors are logged and the frame is skipped — capture keeps going
/// (`03 §12` "corrupt/oversized frame → mark + skip; capture continues"). Dropping
/// `capture` on return releases the underlying OS capture resources.
pub async fn run_capture_loop(
    mut capture: Box<dyn CaptureSource>,
    ctx: LoopCtx,
    mut stop: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            biased;
            _ = stop.changed() => {
                tracing::info!("capture loop: stop requested");
                break;
            }
            res = capture.next_frame() => match res {
                Ok(Some(frame)) => {
                    let monitor = frame.monitor_index;
                    if let Err(e) = process_frame(&ctx, frame).await {
                        // No frame content in the log (privacy, 03 §9) — just the error.
                        tracing::warn!(monitor, error = %e, "capture loop: dropping frame");
                    }
                }
                Ok(None) => {
                    tracing::info!("capture loop: source signaled shutdown");
                    break;
                }
                Err(e) => tracing::warn!(error = %e, "capture loop: source error"),
            }
        }
    }
}

/// OCR → encode → store one frame, then enqueue enrichment and emit the tick.
/// OCR runs on the **full-resolution** pixels before the storage downscale (`03 §8`).
async fn process_frame(ctx: &LoopCtx, frame: CapturedFrame) -> Result<()> {
    let ocr = ctx.ocr.recognize(&frame).await?;

    let (rel_db_path, abs_path) =
        image_paths(&ctx.frames_dir, frame.captured_at, frame.monitor_index);
    write_jpeg(
        frame.pixels.clone(),
        abs_path,
        ctx.max_width,
        ctx.jpeg_quality,
    )
    .await?;

    let frame_id = ctx
        .store
        .insert_frame(NewFrame {
            captured_at: frame.captured_at,
            monitor_index: frame.monitor_index,
            width: frame.width,
            height: frame.height,
            image_path: rel_db_path,
            content_hash: frame.content_hash.clone(),
            app_hint: frame.app_hint.clone(),
            window_title: frame.window_title.clone(),
            browser_url: None, // needs UI Automation — deferred past P2
        })
        .await?;

    ctx.store.insert_ocr(frame_id, ocr).await?;

    // After insert_ocr succeeds → enqueue embed_text (priority normal), and
    // embed_image when image embeddings are enabled. vision_tag is NEVER
    // auto-enqueued here (03 §5, 13.3).
    if ctx.enrich_embed_text {
        ctx.store
            .enqueue_job(NewJob {
                kind: JobKind::EmbedText,
                frame_id: Some(frame_id),
                priority: 0,
                max_attempts: 3,
                not_before: 0,
            })
            .await?;
    }
    if ctx.enrich_image_embeddings {
        ctx.store
            .enqueue_job(NewJob {
                kind: JobKind::EmbedImage,
                frame_id: Some(frame_id),
                priority: 0,
                max_attempts: 3,
                not_before: 0,
            })
            .await?;
    }

    let _ = ctx.events.send(KernelEvent::CaptureTick(CaptureTick {
        frame_id,
        captured_at: frame.captured_at,
        monitor_index: frame.monitor_index,
    }));
    Ok(())
}

/// `(image_path stored in the DB, absolute path to write)`. The DB path is relative
/// to the app-data dir; frames shard into per-day buckets (`frames/day-<n>/…`) by
/// `captured_at` (unix-ms / 86_400_000) — no calendar dependency (recorded in `07`).
fn image_paths(frames_dir: &std::path::Path, captured_at: i64, monitor: u32) -> (String, PathBuf) {
    let day = captured_at.div_euclid(86_400_000);
    let rel_within = format!("day-{day}/{captured_at}-{monitor}.jpg");
    let abs = frames_dir.join(&rel_within);
    (format!("frames/{rel_within}"), abs)
}

/// Downscale to `max_width` (aspect-preserved) and JPEG-encode at `quality`,
/// creating parent dirs. Runs on the blocking pool (CPU + file IO).
async fn write_jpeg(
    pixels: Arc<RgbaImage>,
    path: PathBuf,
    max_width: u32,
    quality: u8,
) -> Result<()> {
    tokio::task::spawn_blocking(move || -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let resized = maybe_resize(&pixels, max_width);
        // JPEG has no alpha channel — drop it.
        let rgb = image::DynamicImage::ImageRgba8(resized).into_rgb8();
        let file = std::io::BufWriter::new(std::fs::File::create(&path)?);
        image::codecs::jpeg::JpegEncoder::new_with_quality(file, quality).write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )?;
        Ok(())
    })
    .await?
}

/// Clone-and-resize to `max_width` if wider; otherwise clone as-is.
fn maybe_resize(pixels: &RgbaImage, max_width: u32) -> RgbaImage {
    if max_width == 0 || pixels.width() <= max_width {
        return pixels.clone();
    }
    let ratio = f64::from(max_width) / f64::from(pixels.width());
    let new_h = ((f64::from(pixels.height()) * ratio).round() as u32).max(1);
    image::imageops::resize(
        pixels,
        max_width,
        new_h,
        image::imageops::FilterType::Triangle,
    )
}

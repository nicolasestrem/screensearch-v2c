//! End-to-end capture-pipeline tests (`03 §10` "capture(stub frames) → OCR → store
//! → embed job"). These drive the real kernel loop with **fake** `CaptureSource` /
//! `OcrProvider` and an in-memory `Store`, so they run on any platform (no Windows
//! APIs) and gate P2's core invariants: frames + OCR are stored, each frame
//! enqueues exactly one `embed_text` job, **no `vision_tag` job is ever enqueued**
//! (`13.3`), and a `capture_tick` is emitted per frame.

use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use image::{Rgba, RgbaImage};
use tokio::sync::{broadcast, watch};

use kernel::{run_capture_loop, CaptureFactory, Kernel, KernelEvent, LoopCtx};
use store::SqliteStore;
use traits::{
    CaptureSource, CapturedFrame, ComponentStatus, JobKind, MonitorInfo, OcrProvider, OcrResult,
    Readiness, Result, Store,
};

/// A capture source that replays a fixed list of frames, then `None` (shutdown).
struct FakeCapture {
    frames: VecDeque<CapturedFrame>,
}

#[async_trait]
impl CaptureSource for FakeCapture {
    fn monitors(&self) -> Vec<MonitorInfo> {
        vec![MonitorInfo {
            index: 0,
            name: "FAKE".to_string(),
            width: 4,
            height: 4,
            is_primary: true,
        }]
    }
    async fn next_frame(&mut self) -> Result<Option<CapturedFrame>> {
        Ok(self.frames.pop_front())
    }
}

/// A deterministic OCR stand-in: text derived from the frame's timestamp.
struct FakeOcr;

#[async_trait]
impl OcrProvider for FakeOcr {
    async fn recognize(&self, frame: &CapturedFrame) -> Result<OcrResult> {
        Ok(OcrResult {
            text: format!("ocr text for frame at {}", frame.captured_at),
            mean_confidence: 0.9,
            engine: "fake".to_string(),
        })
    }
}

fn frame(captured_at: i64) -> CapturedFrame {
    let pixels = RgbaImage::from_pixel(4, 4, Rgba([10, 20, 30, 255]));
    CapturedFrame {
        monitor_index: 0,
        width: 4,
        height: 4,
        captured_at,
        pixels: Arc::new(pixels),
        content_hash: format!("hash-{captured_at}"),
        app_hint: Some("Firefox".to_string()),
        window_title: Some("Inbox".to_string()),
    }
}

#[tokio::test]
async fn capture_loop_stores_frames_ocr_jpegs_and_enqueues_embed_jobs() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().to_path_buf();
    let frames_dir = data_dir.join("frames");

    // keep the concrete handle for reads (`get_frame` is inherent, not on the trait)
    let db = Arc::new(SqliteStore::open_in_memory().unwrap());
    let store: Arc<dyn Store> = db.clone();
    let ocr: Arc<dyn OcrProvider> = Arc::new(FakeOcr);
    let caps: VecDeque<CapturedFrame> = (0..3).map(|i| frame(1_000 + i)).collect();

    let (events, mut rx) = broadcast::channel(64);
    let (_stop_tx, stop_rx) = watch::channel(false);
    let ctx = LoopCtx {
        store: store.clone(),
        ocr,
        frames_dir,
        events,
        enrich_embed_text: true,
        jpeg_quality: 80,
        max_width: 1280,
    };

    run_capture_loop(Box::new(FakeCapture { frames: caps }), ctx, stop_rx).await;

    // every frame: row stored with OCR text + foreground context + a JPEG on disk
    for i in 0..3 {
        let id = i64::from(i) + 1;
        let detail = db.get_frame(id).await.unwrap().expect("frame stored");
        assert_eq!(detail.captured_at, 1_000 + i64::from(i));
        assert_eq!(detail.app_hint.as_deref(), Some("Firefox"));
        assert_eq!(detail.window_title.as_deref(), Some("Inbox"));
        assert!(detail.text.as_deref().unwrap().contains("ocr text"));
        assert!(
            data_dir.join(&detail.image_path).exists(),
            "jpeg should be written at {}",
            detail.image_path
        );
    }

    // exactly one embed_text job per frame, all pending; NO vision_tag jobs (13.3)
    assert_eq!(store.job_stats().await.unwrap().pending, 3);
    assert!(store
        .claim_jobs(&[JobKind::VisionTag], 10, 9_999_999)
        .await
        .unwrap()
        .is_empty());
    let embed = store
        .claim_jobs(&[JobKind::EmbedText], 10, 9_999_999)
        .await
        .unwrap();
    assert_eq!(embed.len(), 3);
    assert!(embed
        .iter()
        .all(|j| j.kind == JobKind::EmbedText && j.frame_id.is_some()));

    // one capture_tick per stored frame
    let mut ticks = 0;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, KernelEvent::CaptureTick(_)) {
            ticks += 1;
        }
    }
    assert_eq!(ticks, 3);
}

#[tokio::test]
async fn capture_loop_skips_embed_jobs_when_disabled() {
    let tmp = tempfile::tempdir().unwrap();
    let db = Arc::new(SqliteStore::open_in_memory().unwrap());
    let store: Arc<dyn Store> = db.clone();
    let ocr: Arc<dyn OcrProvider> = Arc::new(FakeOcr);
    let caps: VecDeque<CapturedFrame> = (0..2).map(|i| frame(5_000 + i)).collect();

    let (events, _rx) = broadcast::channel(64);
    let (_stop_tx, stop_rx) = watch::channel(false);
    let ctx = LoopCtx {
        store: store.clone(),
        ocr,
        frames_dir: tmp.path().join("frames"),
        events,
        enrich_embed_text: false, // enrich.embed_text = false
        jpeg_quality: 80,
        max_width: 1280,
    };

    run_capture_loop(Box::new(FakeCapture { frames: caps }), ctx, stop_rx).await;

    // frames are still stored, but no jobs are enqueued
    assert!(db.get_frame(1).await.unwrap().is_some());
    assert_eq!(store.job_stats().await.unwrap().pending, 0);
}

#[tokio::test]
async fn kernel_start_then_stop_flips_capture_readiness() {
    let tmp = tempfile::tempdir().unwrap();
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().unwrap());
    let ocr: Arc<dyn OcrProvider> = Arc::new(FakeOcr);

    // factory yields a fresh fake source each Start (drains 2 frames, then ends)
    let factory: CaptureFactory = Arc::new(|_cfg| {
        let caps: VecDeque<CapturedFrame> = (0..2).map(|i| frame(2_000 + i)).collect();
        Ok(Box::new(FakeCapture { frames: caps }) as Box<dyn CaptureSource>)
    });

    let kernel = Kernel::new(
        store.clone(),
        ocr,
        factory,
        tmp.path().join("frames"),
        Readiness::default(),
    );

    // off until Start (privacy-first)
    assert_eq!(kernel.readiness().capture.status, ComponentStatus::Disabled);
    assert!(!kernel.is_capturing().await);

    kernel.start_capture().await.unwrap();
    assert_eq!(kernel.readiness().capture.status, ComponentStatus::Ready);
    assert!(kernel.is_capturing().await);

    // start is idempotent — a second call is a no-op, still Ready
    kernel.start_capture().await.unwrap();
    assert_eq!(kernel.readiness().capture.status, ComponentStatus::Ready);

    // stop joins the loop and flips to Disabled; a second stop is a no-op
    kernel.stop_capture().await;
    assert_eq!(kernel.readiness().capture.status, ComponentStatus::Disabled);
    assert!(!kernel.is_capturing().await);
    kernel.stop_capture().await;
    assert_eq!(kernel.readiness().capture.status, ComponentStatus::Disabled);
}

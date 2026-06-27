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

#[derive(Debug, Clone, Copy)]
enum AfterFrames {
    Shutdown,
    Pending,
}

/// A capture source that replays a fixed list of frames, then `None` (shutdown).
struct FakeCapture {
    frames: VecDeque<CapturedFrame>,
    after_frames: AfterFrames,
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
        if let Some(frame) = self.frames.pop_front() {
            return Ok(Some(frame));
        }
        match self.after_frames {
            AfterFrames::Shutdown => Ok(None),
            AfterFrames::Pending => std::future::pending().await,
        }
    }
}

/// A deterministic OCR stand-in: text derived from the frame's timestamp.
struct FakeOcr;

#[async_trait]
impl OcrProvider for FakeOcr {
    async fn recognize(&self, frame: &CapturedFrame) -> Result<OcrResult> {
        // Emit one line of word spans (as the real WinRT engine does) so the attention
        // filter has geometry to classify and rebuild `content_text` from.
        let text = format!("ocr text for frame at {}", frame.captured_at);
        let mut spans = Vec::new();
        let mut x = 0.1_f32;
        for word in text.split_whitespace() {
            let w = 0.02 * word.chars().count() as f32;
            spans.push(traits::TextSpan {
                normalized_text: traits::normalize_text(word),
                text: word.to_string(),
                source: traits::TextSource::Ocr,
                role: traits::TextRole::Unknown,
                x,
                y: 0.45,
                w,
                h: 0.03,
                line_index: 0,
                is_searchable: true,
                suppress_reason: None,
            });
            x += w + 0.01;
        }
        Ok(OcrResult {
            text,
            mean_confidence: 0.9,
            engine: "fake".to_string(),
            spans,
        })
    }
}

/// OCR provider used to model a missing WinRT OCR engine/language pack.
struct ErrorOcr;

#[async_trait]
impl OcrProvider for ErrorOcr {
    async fn recognize(&self, _frame: &CapturedFrame) -> Result<OcrResult> {
        anyhow::bail!("OCR unavailable: no recognizer language installed")
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
        target_rect: None,
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
        enrich_image_embeddings: false,
        jpeg_quality: 80,
        max_width: 1280,
        chrome_suppress_min_seen: 12,
        chrome_protect_min_chars: 48,
        chrome_region_buckets: 8,
    };

    run_capture_loop(
        Box::new(FakeCapture {
            frames: caps,
            after_frames: AfterFrames::Shutdown,
        }),
        ctx,
        stop_rx,
    )
    .await;

    // every frame: row stored with OCR text + foreground context + a JPEG on disk.
    // content_text is a passthrough copy of raw_text in PR2 (03 §3b).
    for i in 0..3 {
        let id = i64::from(i) + 1;
        let detail = db.get_frame(id).await.unwrap().expect("frame stored");
        assert_eq!(detail.captured_at, 1_000 + i64::from(i));
        assert_eq!(detail.app_hint.as_deref(), Some("Firefox"));
        assert_eq!(detail.window_title.as_deref(), Some("Inbox"));
        assert!(detail.content_text.as_deref().unwrap().contains("ocr text"));
        assert_eq!(detail.raw_text, detail.content_text);
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
        enrich_image_embeddings: false,
        jpeg_quality: 80,
        max_width: 1280,
        chrome_suppress_min_seen: 12,
        chrome_protect_min_chars: 48,
        chrome_region_buckets: 8,
    };

    run_capture_loop(
        Box::new(FakeCapture {
            frames: caps,
            after_frames: AfterFrames::Shutdown,
        }),
        ctx,
        stop_rx,
    )
    .await;

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
        Ok(Box::new(FakeCapture {
            frames: caps,
            after_frames: AfterFrames::Pending,
        }) as Box<dyn CaptureSource>)
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

#[tokio::test]
async fn kernel_clears_capture_and_marks_error_when_source_shuts_down() {
    let tmp = tempfile::tempdir().unwrap();
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().unwrap());
    let ocr: Arc<dyn OcrProvider> = Arc::new(FakeOcr);

    let factory: CaptureFactory = Arc::new(|_cfg| {
        Ok(Box::new(FakeCapture {
            frames: VecDeque::new(),
            after_frames: AfterFrames::Shutdown,
        }) as Box<dyn CaptureSource>)
    });

    let kernel = Kernel::new(
        store,
        ocr,
        factory,
        tmp.path().join("frames"),
        Readiness::default(),
    );

    kernel.start_capture().await.unwrap();

    for _ in 0..50 {
        if !kernel.is_capturing().await {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    assert!(!kernel.is_capturing().await);
    let readiness = kernel.readiness().capture;
    assert_eq!(readiness.status, ComponentStatus::Error);
    assert!(readiness
        .detail
        .as_deref()
        .unwrap_or_default()
        .contains("source shut down"));
}

#[tokio::test]
async fn reload_capture_restarts_loop_with_fresh_config() {
    let tmp = tempfile::tempdir().unwrap();
    let db = Arc::new(SqliteStore::open_in_memory().unwrap());
    let store: Arc<dyn Store> = db.clone();
    let ocr: Arc<dyn OcrProvider> = Arc::new(FakeOcr);

    // Record the CaptureConfig the factory is built with on each (re)start.
    let configs: Arc<std::sync::Mutex<Vec<traits::CaptureConfig>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = configs.clone();
    let factory: CaptureFactory = Arc::new(move |cfg| {
        seen.lock().unwrap().push(cfg);
        Ok(Box::new(FakeCapture {
            frames: VecDeque::new(),
            after_frames: AfterFrames::Pending,
        }) as Box<dyn CaptureSource>)
    });

    let kernel = Kernel::new(
        store.clone(),
        ocr,
        factory,
        tmp.path().join("frames"),
        Readiness::default(),
    );

    // Reload before any start is a no-op (it never starts capture the user hasn't).
    kernel.reload_capture().await.unwrap();
    assert!(!kernel.is_capturing().await);
    assert!(configs.lock().unwrap().is_empty());

    kernel.start_capture().await.unwrap();
    assert_eq!(configs.lock().unwrap().len(), 1);

    // A newly excluded app is persisted, then reloaded: the loop restarts and the rebuilt
    // CaptureConfig carries the new value — the fix for "Excluded Apps never applies".
    store
        .set_setting("privacy.excluded_apps", "[\"SecretApp\"]")
        .await
        .unwrap();
    kernel.reload_capture().await.unwrap();
    assert!(kernel.is_capturing().await);

    let cfgs = configs.lock().unwrap();
    assert_eq!(cfgs.len(), 2, "reload restarted the capture loop");
    assert!(
        cfgs[1].excluded_apps.iter().any(|a| a == "SecretApp"),
        "rebuilt config picks up the newly excluded app"
    );
}

#[tokio::test]
async fn kernel_refuses_to_start_capture_when_ocr_is_unavailable() {
    let tmp = tempfile::tempdir().unwrap();
    let db = Arc::new(SqliteStore::open_in_memory().unwrap());
    let store: Arc<dyn Store> = db.clone();
    let ocr: Arc<dyn OcrProvider> = Arc::new(ErrorOcr);
    let factory: CaptureFactory =
        Arc::new(|_cfg| panic!("capture factory should not run without OCR"));

    let kernel = Kernel::new_with_ocr_unavailable(
        store.clone(),
        ocr,
        factory,
        tmp.path().join("frames"),
        Readiness::default(),
        "no OCR recognizer languages are installed".to_string(),
    );

    let err = kernel
        .start_capture()
        .await
        .expect_err("capture start fails");

    assert!(err.to_string().contains("OCR unavailable"));
    assert_eq!(
        kernel.readiness().capture.status,
        ComponentStatus::Unavailable
    );
    assert!(!kernel.is_capturing().await);
    assert!(db.get_frame(1).await.unwrap().is_none());
    assert_eq!(store.job_stats().await.unwrap().pending, 0);
}

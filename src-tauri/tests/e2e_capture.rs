//! End-to-end P2 verification (`03 §13.1`): the **real** capture happy path —
//! WGC capture → WinRT OCR → Store → `embed_text` job — with no fakes. Windows- and
//! desktop-gated, so `#[ignore]`d in CI; run locally with
//! `cargo test -p screensearch --test e2e_capture -- --ignored`.
//!
//! This is the "observed running" proof for P2: it drives the same `Kernel` the app
//! wires, against a temp on-disk store, and asserts frames + OCR rows + a JPEG land
//! and an `embed_text` job is enqueued — while **no `vision_tag` job ever is**
//! (`13.3`).

use std::sync::Arc;
use std::time::Duration;

use capture::WgcCapture;
use kernel::{CaptureFactory, Kernel};
use ocr::WinRtOcr;
use store::SqliteStore;
use traits::{CaptureSource, JobKind, OcrProvider, Readiness, Store};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "real WGC + WinRT capture; requires a desktop session, run locally"]
async fn capture_pipeline_stores_frames_ocr_and_enqueues_embed_jobs() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("screensearch.db");
    let frames_dir = tmp.path().join("frames");

    let store = Arc::new(SqliteStore::open_path(&db_path).expect("open store"));
    let dyn_store: Arc<dyn Store> = store.clone();

    // Capture quickly and keep every frame so the test doesn't depend on the screen
    // actually changing; disable the lock pause for headless CI runners.
    store
        .set_setting("capture.interval_ms", "300")
        .await
        .unwrap();
    store
        .set_setting("capture.diff_threshold", "0.0")
        .await
        .unwrap();
    store
        .set_setting("privacy.pause_on_lock", "false")
        .await
        .unwrap();

    let ocr: Arc<dyn OcrProvider> = Arc::new(WinRtOcr::spawn().expect("spawn OCR"));
    let factory: CaptureFactory =
        Arc::new(|cfg| Ok(Box::new(WgcCapture::new(cfg)?) as Box<dyn CaptureSource>));

    let kernel = Kernel::new(
        dyn_store,
        ocr,
        factory,
        frames_dir.clone(),
        Readiness::default(),
    );

    kernel.start_capture().await.expect("start capture");
    tokio::time::sleep(Duration::from_secs(3)).await;
    kernel.stop_capture().await;

    // A frame was captured, OCR'd, and stored, with a JPEG on disk.
    let frame = store
        .get_frame(1)
        .await
        .unwrap()
        .expect("at least one frame stored");
    assert!(frame.width > 0 && frame.height > 0);
    assert!(
        tmp.path().join(&frame.image_path).exists(),
        "jpeg written at {}",
        frame.image_path
    );

    // The frame_text row exists (raw/content text may be empty on a blank screen —
    // that's fine); content_text is a passthrough copy of raw_text in PR2 (03 §3b).
    assert!(
        frame.raw_text.is_some(),
        "frame_text row present for the frame"
    );
    assert_eq!(frame.raw_text, frame.content_text);

    // An embed_text job was enqueued; NO vision_tag job ever is (13.3).
    let stats = store.job_stats().await.unwrap();
    assert!(stats.pending >= 1, "embed_text enqueued, got {stats:?}");
    assert!(store
        .claim_jobs(&[JobKind::VisionTag], 16, i64::MAX)
        .await
        .unwrap()
        .is_empty());
}

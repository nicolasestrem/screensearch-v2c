//! P3 enrichment-worker tests (`03 §5/§6/§10/§13.2/§13.4`).
//!
//! The per-job state machine is driven deterministically via [`kernel::process_job`]
//! (claim a job, process it, assert the store's state). The end-to-end test drives
//! the *real* worker pool through [`Kernel::attach_embedder`]: a pending `embed_text`
//! job is drained into a vector and a vector-arm `hybrid_search` then finds the frame
//! — all with a fake embedder, so it runs on any platform with no real model.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use image::RgbaImage;

use kernel::{process_job, CaptureFactory, Kernel};
use store::{SqliteStore, EMBEDDING_DIM};
use traits::{
    CaptureConfig, CaptureSource, Embedding, EmbeddingProvider, JobKind, NewFrame, NewJob,
    OcrProvider, OcrResult, Readiness, Result, SearchQuery, Store, VisionAnalysis, VisionProvider,
};

/// A deterministic embedder: maps each text to a pre-registered vector (so the
/// vector arm is exercised without the real model), or fails every call when
/// `fail` is set (to drive the retry/dead-letter path).
struct MapEmbedder {
    by_text: HashMap<String, Embedding>,
    fail: bool,
}

impl MapEmbedder {
    fn new() -> Self {
        Self {
            by_text: HashMap::new(),
            fail: false,
        }
    }
    fn with(mut self, text: &str, hot: usize) -> Self {
        self.by_text.insert(text.to_string(), one_hot(hot));
        self
    }
    fn failing() -> Self {
        Self {
            by_text: HashMap::new(),
            fail: true,
        }
    }
}

#[async_trait]
impl EmbeddingProvider for MapEmbedder {
    fn dim(&self) -> usize {
        EMBEDDING_DIM
    }
    async fn embed_texts(&self, inputs: &[String]) -> Result<Vec<Embedding>> {
        if self.fail {
            anyhow::bail!("forced embed failure");
        }
        inputs
            .iter()
            .map(|s| {
                self.by_text
                    .get(s)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("no fake vector for {s:?}"))
            })
            .collect()
    }
    async fn embed_image(&self, _image: &RgbaImage) -> Result<Embedding> {
        if self.fail {
            anyhow::bail!("forced embed failure");
        }
        Ok(one_hot(0))
    }
    fn text_model_name(&self) -> &str {
        "fake-text"
    }
}

/// One-hot unit vector — distinct hots are orthogonal (cosine distance 1), identical
/// hots coincide (distance ~0), making KNN ordering deterministic.
fn one_hot(hot: usize) -> Embedding {
    let mut v = vec![0.0_f32; EMBEDDING_DIM];
    v[hot] = 1.0;
    Embedding(v)
}

fn new_frame(captured_at: i64) -> NewFrame {
    NewFrame {
        captured_at,
        monitor_index: 0,
        width: 1920,
        height: 1080,
        image_path: format!("frames/{captured_at}.jpg"),
        content_hash: format!("hash-{captured_at}"),
        app_hint: None,
        window_title: None,
        browser_url: None,
    }
}

fn ocr(text: &str) -> OcrResult {
    OcrResult {
        text: text.to_string(),
        mean_confidence: -1.0,
        engine: "fake".to_string(),
    }
}

fn embed_text_job(frame_id: Option<i64>, max_attempts: i64) -> NewJob {
    NewJob {
        kind: JobKind::EmbedText,
        frame_id,
        priority: 0,
        max_attempts,
        not_before: 0,
    }
}

fn embed_image_job(frame_id: Option<i64>, max_attempts: i64) -> NewJob {
    NewJob {
        kind: JobKind::EmbedImage,
        frame_id,
        priority: 0,
        max_attempts,
        not_before: 0,
    }
}

// --- per-job state machine (deterministic, via process_job) --------------------

#[tokio::test]
async fn process_job_embeds_text_and_completes() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().unwrap());
    let fid = store.insert_frame(new_frame(1_000)).await.unwrap();
    store
        .insert_ocr(fid, ocr("invoice total due"))
        .await
        .unwrap();
    store
        .enqueue_job(embed_text_job(Some(fid), 3))
        .await
        .unwrap();

    let embedder: Arc<dyn EmbeddingProvider> =
        Arc::new(MapEmbedder::new().with("invoice total due", 5));
    let data_dir = PathBuf::from(".");

    let job = store
        .claim_jobs(&[JobKind::EmbedText], 1, 1)
        .await
        .unwrap()
        .pop()
        .expect("a job to claim");
    process_job(&store, &embedder, None, &data_dir, job)
        .await
        .unwrap();

    let stats = store.job_stats().await.unwrap();
    assert_eq!(stats.done, 1);
    assert_eq!(stats.pending, 0);
    assert_eq!(stats.dead, 0);
}

#[tokio::test]
async fn process_job_completes_on_empty_ocr_without_embedding() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().unwrap());
    let fid = store.insert_frame(new_frame(2_000)).await.unwrap();
    store.insert_ocr(fid, ocr("   ")).await.unwrap(); // whitespace-only OCR
    store
        .enqueue_job(embed_text_job(Some(fid), 3))
        .await
        .unwrap();

    // an embedder that would panic if asked — it must not be called
    let embedder: Arc<dyn EmbeddingProvider> = Arc::new(MapEmbedder::new());
    let job = store
        .claim_jobs(&[JobKind::EmbedText], 1, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    process_job(&store, &embedder, None, &PathBuf::from("."), job)
        .await
        .unwrap();

    assert_eq!(store.job_stats().await.unwrap().done, 1);
    // no embedding row was written for an empty-text frame
    let hits = store
        .hybrid_search(&SearchQuery {
            text: "anything".to_string(),
            limit: 10,
            time_range: None,
        })
        .await
        .unwrap();
    assert!(hits.is_empty());
}

#[tokio::test]
async fn process_job_dead_letters_missing_frame_id() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().unwrap());
    store.enqueue_job(embed_text_job(None, 3)).await.unwrap(); // malformed: no frame_id

    let embedder: Arc<dyn EmbeddingProvider> = Arc::new(MapEmbedder::new());
    let job = store
        .claim_jobs(&[JobKind::EmbedText], 1, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    process_job(&store, &embedder, None, &PathBuf::from("."), job)
        .await
        .unwrap();

    let stats = store.job_stats().await.unwrap();
    assert_eq!(stats.dead, 1, "a job with no frame_id is unrecoverable");
    assert_eq!(stats.done, 0);
}

#[tokio::test]
async fn process_job_retries_then_dead_letters_on_persistent_embed_failure() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().unwrap());
    let fid = store.insert_frame(new_frame(3_000)).await.unwrap();
    store.insert_ocr(fid, ocr("some real text")).await.unwrap();
    store
        .enqueue_job(embed_text_job(Some(fid), 2))
        .await
        .unwrap(); // max_attempts = 2

    let embedder: Arc<dyn EmbeddingProvider> = Arc::new(MapEmbedder::failing());
    let data_dir = PathBuf::from(".");

    // attempt 1 → retry (backoff into the future), still pending
    let job = store
        .claim_jobs(&[JobKind::EmbedText], 1, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    process_job(&store, &embedder, None, &data_dir, job)
        .await
        .unwrap();
    assert_eq!(store.job_stats().await.unwrap().pending, 1);

    // attempt 2 → dead-letter (attempts exhausted). Claim with a far-future `now` to
    // bypass the backoff's not_before.
    let job = store
        .claim_jobs(&[JobKind::EmbedText], 1, i64::MAX)
        .await
        .unwrap()
        .pop()
        .expect("the retried job is reclaimable past its backoff");
    process_job(&store, &embedder, None, &data_dir, job)
        .await
        .unwrap();

    let stats = store.job_stats().await.unwrap();
    assert_eq!(stats.dead, 1);
    assert_eq!(stats.done, 0);
    assert_eq!(stats.pending, 0);
}

/// embed_image loads the stored JPEG from disk, embeds it, and upserts the image
/// vector — the optional visual-recall path (`03 §4`).
#[tokio::test]
async fn process_job_embeds_image_from_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().to_path_buf();

    let concrete = Arc::new(SqliteStore::open_in_memory().unwrap());
    let store: Arc<dyn Store> = concrete.clone();
    let fid = store.insert_frame(new_frame(4_000)).await.unwrap(); // image_path = frames/4000.jpg
    store
        .enqueue_job(embed_image_job(Some(fid), 3))
        .await
        .unwrap();

    // write a real JPEG where the worker resolves it: <data_dir>/frames/4000.jpg
    let abs = data_dir.join("frames").join("4000.jpg");
    std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
    let pixels = image::RgbaImage::from_pixel(8, 8, image::Rgba([120, 130, 140, 255]));
    image::DynamicImage::ImageRgba8(pixels)
        .to_rgb8()
        .save(&abs)
        .unwrap();

    let embedder: Arc<dyn EmbeddingProvider> = Arc::new(MapEmbedder::new());
    let job = store
        .claim_jobs(&[JobKind::EmbedImage], 1, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    process_job(&store, &embedder, None, &data_dir, job)
        .await
        .unwrap();

    assert_eq!(store.job_stats().await.unwrap().done, 1);
    assert_eq!(concrete.image_embedding_count().await.unwrap(), 1);
}

/// A missing JPEG (file genuinely gone, not just locked) dead-letters — it won't
/// reappear. (A transient lock would `exists()` and so retry instead.)
#[tokio::test]
async fn process_job_dead_letters_embed_image_when_file_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().to_path_buf();
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().unwrap());
    let fid = store.insert_frame(new_frame(5_000)).await.unwrap(); // frames/5000.jpg never written
    store
        .enqueue_job(embed_image_job(Some(fid), 3))
        .await
        .unwrap();

    let embedder: Arc<dyn EmbeddingProvider> = Arc::new(MapEmbedder::new());
    let job = store
        .claim_jobs(&[JobKind::EmbedImage], 1, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    process_job(&store, &embedder, None, &data_dir, job)
        .await
        .unwrap();

    let stats = store.job_stats().await.unwrap();
    assert_eq!(stats.dead, 1, "a missing JPEG is unrecoverable");
    assert_eq!(stats.done, 0);
}

// --- end-to-end: real worker pool drains the queue, vector arm finds the frame ---

struct NoCapture;
#[async_trait]
impl CaptureSource for NoCapture {
    fn monitors(&self) -> Vec<traits::MonitorInfo> {
        Vec::new()
    }
    async fn next_frame(&mut self) -> Result<Option<traits::CapturedFrame>> {
        Ok(None)
    }
}

struct NoOcr;
#[async_trait]
impl OcrProvider for NoOcr {
    async fn recognize(&self, _f: &traits::CapturedFrame) -> Result<OcrResult> {
        Ok(ocr(""))
    }
}

#[tokio::test]
async fn attach_embedder_drains_backlog_and_vector_arm_finds_frame() {
    let tmp = tempfile::tempdir().unwrap();
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().unwrap());

    // A stored frame with OCR text the FTS query will NOT match, plus its pending
    // embed_text job (as the capture loop would have left it).
    let fid = store.insert_frame(new_frame(7_000)).await.unwrap();
    store
        .insert_ocr(fid, ocr("kubernetes pod logs"))
        .await
        .unwrap();
    store
        .enqueue_job(embed_text_job(Some(fid), 3))
        .await
        .unwrap();

    // The fake maps both the document text and the (FTS-unmatchable) query to the
    // same vector, so only the vector arm can surface the frame.
    let embedder: Arc<dyn EmbeddingProvider> = Arc::new(
        MapEmbedder::new()
            .with("kubernetes pod logs", 7)
            .with("xyzzy", 7),
    );

    let factory: CaptureFactory =
        Arc::new(|_cfg: CaptureConfig| Ok(Box::new(NoCapture) as Box<dyn CaptureSource>));
    let kernel = Kernel::new(
        store.clone(),
        Arc::new(NoOcr) as Arc<dyn OcrProvider>,
        factory,
        tmp.path().join("frames"),
        Readiness::default(),
    );

    kernel.attach_embedder(embedder).await; // lights up the vector arm + starts workers

    // workers drain the pending job
    let mut drained = false;
    for _ in 0..100 {
        if store.job_stats().await.unwrap().done >= 1 {
            drained = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(drained, "worker pool did not drain the embed_text job");

    // the FTS-unmatchable query finds the frame purely via the vector arm
    let hits = store
        .hybrid_search(&SearchQuery {
            text: "xyzzy".to_string(),
            limit: 10,
            time_range: None,
        })
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].frame_id, fid);

    kernel.stop_workers().await;
}

// --- vision_tag routing (P4): process_job drives the fake vision provider ----------

/// A vision provider that returns a fixed analysis (no sidecar) so the worker's
/// `vision_tag` path is exercised on any platform.
struct FakeVision {
    description: String,
}

#[async_trait]
impl VisionProvider for FakeVision {
    async fn analyze(&self, _image: &RgbaImage) -> Result<VisionAnalysis> {
        Ok(VisionAnalysis {
            description: self.description.clone(),
            activity_type: Some("coding".to_string()),
            app_hint: Some("VS Code".to_string()),
            confidence: 0.9,
            model: "fake-vision".to_string(),
        })
    }
}

fn vision_tag_job(frame_id: Option<i64>) -> NewJob {
    NewJob {
        kind: JobKind::VisionTag,
        frame_id,
        priority: 10,
        max_attempts: 3,
        not_before: 0,
    }
}

/// A claimed `vision_tag` job runs the provider and writes the analysis to the store.
#[tokio::test]
async fn process_job_vision_tag_writes_analysis() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().to_path_buf();

    let concrete = Arc::new(SqliteStore::open_in_memory().unwrap());
    let store: Arc<dyn Store> = concrete.clone();
    let fid = store.insert_frame(new_frame(8_000)).await.unwrap(); // image_path = frames/8000.jpg
    store.enqueue_job(vision_tag_job(Some(fid))).await.unwrap();

    // Write a real JPEG where the worker resolves it.
    let abs = data_dir.join("frames").join("8000.jpg");
    std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
    image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 8, image::Rgba([10, 20, 30, 255])))
        .to_rgb8()
        .save(&abs)
        .unwrap();

    let embedder: Arc<dyn EmbeddingProvider> = Arc::new(MapEmbedder::new());
    let vision: Arc<dyn VisionProvider> = Arc::new(FakeVision {
        description: "a Rust editor with tests".to_string(),
    });

    let job = store
        .claim_jobs(&[JobKind::VisionTag], 1, 1)
        .await
        .unwrap()
        .pop()
        .expect("a vision_tag job to claim");
    process_job(&store, &embedder, Some(&vision), &data_dir, job)
        .await
        .unwrap();

    assert_eq!(store.job_stats().await.unwrap().done, 1);
    let frame = concrete
        .get_frame(fid)
        .await
        .unwrap()
        .expect("frame exists");
    let analysis = frame.vision.expect("vision analysis was written");
    assert_eq!(analysis.description, "a Rust editor with tests");
    assert_eq!(frame.activity_type.as_deref(), Some("coding")); // mirrored onto the frame
}

/// With no vision provider attached, a `vision_tag` job retries (stays pending) rather
/// than failing — the backlog drains once the sidecar comes up (`03 §6`).
#[tokio::test]
async fn process_job_vision_tag_retries_without_provider() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().unwrap());
    let fid = store.insert_frame(new_frame(9_000)).await.unwrap();
    store.enqueue_job(vision_tag_job(Some(fid))).await.unwrap();

    let embedder: Arc<dyn EmbeddingProvider> = Arc::new(MapEmbedder::new());
    let job = store
        .claim_jobs(&[JobKind::VisionTag], 1, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    process_job(&store, &embedder, None, &PathBuf::from("."), job)
        .await
        .unwrap();

    let stats = store.job_stats().await.unwrap();
    assert_eq!(stats.pending, 1, "no provider → retry, not fail");
    assert_eq!(stats.dead, 0);
    assert_eq!(stats.done, 0);
}

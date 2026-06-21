//! The normative module contracts (`03 §3`).
//!
//! Names/shapes may be refined in impl, but these boundaries are fixed: `kernel`
//! and module crates depend on these traits, never on each other's concrete impls
//! (`03 §2`). All methods are fallible async; `Result<T>` is [`crate::Result`].

use async_trait::async_trait;
use image::RgbaImage;
use tokio::sync::mpsc::Sender;

use crate::domain::{
    AnswerOpts, CapturedFrame, ChunkSource, Embedding, NewFrame, OcrResult, RetrievedChunk,
    VisionAnalysis,
};
use crate::ipc::{AnswerDelta, SearchHit, SearchQuery};
use crate::jobs::{Job, JobKind, JobStats, NewJob};
use crate::{MonitorInfo, Result};

/// Screen capture source (WGC impl in `capture`, `03 §3`).
#[async_trait]
pub trait CaptureSource: Send + Sync {
    fn monitors(&self) -> Vec<MonitorInfo>;
    /// Yields the next *changed* frame (diff-gated) or `None` on shutdown.
    async fn next_frame(&mut self) -> Result<Option<CapturedFrame>>;
}

/// Text recognition over a captured frame (WinRT `Media.Ocr` impl in `ocr`).
#[async_trait]
pub trait OcrProvider: Send + Sync {
    async fn recognize(&self, frame: &CapturedFrame) -> Result<OcrResult>;
}

/// Dense embeddings (fastembed impl in `embeddings`, `03 §3`).
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Vector dimensionality (768).
    fn dim(&self) -> usize;
    /// NOTE: the quantized text model cannot batch — the impl embeds one input at a time.
    async fn embed_texts(&self, inputs: &[String]) -> Result<Vec<Embedding>>;
    async fn embed_image(&self, image: &RgbaImage) -> Result<Embedding>;
}

/// Deferred vision tagging via the inference sidecar (`inference`).
#[async_trait]
pub trait VisionProvider: Send + Sync {
    async fn analyze(&self, image: &RgbaImage) -> Result<VisionAnalysis>;
}

/// Grounded, streaming RAG answers via the inference sidecar (`inference`).
#[async_trait]
pub trait AnswerProvider: Send + Sync {
    /// Streams answer deltas over `tx`; returns when complete.
    async fn answer(
        &self,
        query: &str,
        context: &[RetrievedChunk],
        opts: AnswerOpts,
        tx: Sender<AnswerDelta>,
    ) -> Result<()>;
}

/// The durable data spine: frames, OCR, embeddings, retrieval, job queue, settings
/// (SQLite + sqlite-vec + FTS5 impl in `store`, `03 §3/§4/§5`).
#[async_trait]
pub trait Store: Send + Sync {
    // frames + ocr + vision
    async fn insert_frame(&self, f: NewFrame) -> Result<i64>;
    async fn insert_ocr(&self, frame_id: i64, ocr: OcrResult) -> Result<()>;
    async fn insert_vision(&self, frame_id: i64, v: VisionAnalysis) -> Result<()>;

    // embeddings
    async fn upsert_text_embedding(
        &self,
        frame_id: i64,
        chunk_index: i32,
        chunk_text: &str,
        source: ChunkSource,
        emb: &Embedding,
        model: &str,
    ) -> Result<()>;
    async fn upsert_image_embedding(
        &self,
        frame_id: i64,
        emb: &Embedding,
        model: &str,
    ) -> Result<()>;

    // retrieval
    async fn hybrid_search(&self, q: &SearchQuery) -> Result<Vec<SearchHit>>;

    // job queue (see `03 §5`)
    async fn enqueue_job(&self, job: NewJob) -> Result<i64>;
    async fn claim_jobs(&self, kinds: &[JobKind], limit: u32, now: i64) -> Result<Vec<Job>>;
    async fn complete_job(&self, id: i64) -> Result<()>;
    async fn fail_job(&self, id: i64, err: &str, retry_at: Option<i64>) -> Result<()>;
    async fn job_stats(&self) -> Result<JobStats>;

    // settings
    async fn get_setting(&self, key: &str) -> Result<Option<String>>;
    async fn set_setting(&self, key: &str, value: &str) -> Result<()>;
}

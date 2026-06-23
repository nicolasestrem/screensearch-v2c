//! The normative module contracts (`03 §3`).
//!
//! Names/shapes may be refined in impl, but these boundaries are fixed: `kernel`
//! and module crates depend on these traits, never on each other's concrete impls
//! (`03 §2`). All methods are fallible async; `Result<T>` is [`crate::Result`].

use std::sync::Arc;

use async_trait::async_trait;
use image::RgbaImage;
use tokio::sync::mpsc::Sender;

use crate::domain::{
    AnswerOpts, CapturedFrame, ChunkSource, Embedding, FrameEnrichmentInput, NewFrame, OcrResult,
    RetrievedChunk, VisionAnalysis,
};
use crate::ipc::{AnswerDelta, InsightsSummary, SearchHit, SearchQuery, TimelineBucket};
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
    /// Identifier of the active text model, recorded in `embeddings.model` for
    /// provenance (`03 §4`). Defaults to `"unknown"` for providers that don't track it.
    fn text_model_name(&self) -> &str {
        "unknown"
    }
    /// Identifier of the active image model, recorded in `image_embeddings.model`.
    fn image_model_name(&self) -> &str {
        "unknown"
    }
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
    /// The minimal inputs the embedding worker needs for a frame (image path + OCR
    /// text), or `None` if the frame no longer exists (`03 §5`).
    async fn get_enrichment_input(&self, frame_id: i64) -> Result<Option<FrameEnrichmentInput>>;

    /// Frame ids that have **no** vision analysis yet (oldest first, capped at
    /// `limit`), optionally restricted to a `[start, end)` capture window. The source
    /// for the timer/idle vision batch and the `enqueue_vision` range target (`03 §5`).
    /// Default returns empty for stores that don't track vision.
    async fn untagged_frame_ids(
        &self,
        _limit: u32,
        _range: Option<(i64, i64)>,
    ) -> Result<Vec<i64>> {
        Ok(Vec::new())
    }

    /// Bulk-fetch the OCR text for many frames in **one** round-trip (avoids an N+1
    /// when hydrating grounding context for `ask`). Returns `frame_id → text` only for
    /// frames that have non-empty OCR text. Default returns empty.
    async fn ocr_texts(
        &self,
        _frame_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, String>> {
        Ok(std::collections::HashMap::new())
    }

    /// Frame-count density buckets across the half-open window `[start, end)`, split
    /// into at most `bucket_count` fixed-width buckets and returned **sparse** (only
    /// occupied buckets, ascending). Backs the `get_timeline` command (`03 §7`).
    /// Default returns empty for stores without a timeline.
    async fn timeline_buckets(
        &self,
        _start: i64,
        _end: i64,
        _bucket_count: u32,
    ) -> Result<Vec<TimelineBucket>> {
        Ok(Vec::new())
    }

    /// Truthful activity aggregates over `[start, end)` for the Insights screen
    /// (`get_insights`, P5). Default returns the honest-empty summary.
    async fn insights_summary(
        &self,
        _start: i64,
        _end: i64,
        _bucket_count: u32,
    ) -> Result<InsightsSummary> {
        Ok(InsightsSummary::default())
    }

    // job queue (see `03 §5`)
    async fn enqueue_job(&self, job: NewJob) -> Result<i64>;
    async fn claim_jobs(&self, kinds: &[JobKind], limit: u32, now: i64) -> Result<Vec<Job>>;
    async fn complete_job(&self, id: i64) -> Result<()>;
    async fn fail_job(&self, id: i64, err: &str, retry_at: Option<i64>) -> Result<()>;
    async fn job_stats(&self) -> Result<JobStats>;
    /// Requeues jobs stuck in `running` whose `updated_at` is older than
    /// `older_than_ms` before now (a worker died mid-job; there is no lease). Resets
    /// them to `pending` so they can be reclaimed; returns how many were requeued
    /// (`03 §6` "restart + requeue"). Passing `0` requeues *all* `running` jobs — the
    /// startup sweep, when by definition no worker is live.
    async fn reset_stale_running_jobs(&self, older_than_ms: i64) -> Result<u64>;

    // settings
    async fn get_setting(&self, key: &str) -> Result<Option<String>>;
    async fn set_setting(&self, key: &str, value: &str) -> Result<()>;

    /// Atomically upserts many `(key, value)` settings in a single transaction —
    /// **all land or none do**. Used by `kernel::settings::save_settings` so a crash
    /// or error mid-save cannot leave the `settings` table in a mixed state (some keys
    /// new, the rest stale), which `load_settings`' per-key default fallback would
    /// silently hide. The default applies them one-by-one via [`Self::set_setting`]
    /// (non-atomic) for stores without transaction support; `SqliteStore` overrides it
    /// with a real `BEGIN … COMMIT`.
    async fn set_settings_batch(&self, kvs: &[(String, String)]) -> Result<()> {
        for (key, value) in kvs {
            self.set_setting(key, value).await?;
        }
        Ok(())
    }

    /// Injects (or replaces) the query-embedding provider that lights up the vector
    /// arm of [`Self::hybrid_search`]. Set once the model has finished loading off
    /// the launch thread (`03 §5`). Default is a no-op for stores that never embed.
    fn set_embedder(&self, _embedder: Arc<dyn EmbeddingProvider>) {}
}

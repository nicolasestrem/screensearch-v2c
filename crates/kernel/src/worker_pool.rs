//! The bounded enrichment worker pool — the consumer half of *enrich-deferred*
//! (`03 §5`). Each worker independently `claim_jobs`→runs the provider→
//! `complete_job`/`fail_job`, so the durable queue P2 fills is finally drained into
//! vectors. P3 handles `embed_text` and (optionally) `embed_image`; **never**
//! `vision_tag` (that needs the P4 sidecar).
//!
//! Shutdown mirrors the capture loop: one `watch` channel stops every worker after
//! its in-flight job. A periodic sweep requeues jobs a dead worker left `running`
//! (`03 §6`, `07` gap #6) — there is no per-job lease, so a visibility timeout is
//! the minimal recovery.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use traits::{ChunkSource, EmbeddingProvider, Job, JobKind, Store};

use crate::events::KernelEvent;

/// Idle poll floor — how long a worker waits after an empty claim before retrying.
const IDLE_MIN: Duration = Duration::from_millis(250);
/// Idle poll ceiling — the backoff caps here so an empty queue costs ~one claim/2 s.
const IDLE_MAX: Duration = Duration::from_secs(2);
/// How often the sweep checks for stale `running` jobs.
const SWEEP_INTERVAL: Duration = Duration::from_secs(60);
/// A `running` job older than this is presumed abandoned (worker died) and requeued.
/// Must exceed the longest plausible single-job runtime (embed ≪ 1 s).
const VISIBILITY_TIMEOUT_MS: i64 = 5 * 60 * 1000;
/// Retry backoff base; the nth failure waits `BASE · 2^attempts`, capped.
const BACKOFF_BASE_MS: i64 = 1_000;
const BACKOFF_CAP_MS: i64 = 60_000;

/// Current unix time in milliseconds (the queue's clock unit, `03 §4`).
pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Exponential backoff (ms) for the next retry given the job's prior attempt count.
fn backoff_ms(attempts: i64) -> i64 {
    let shift = attempts.clamp(0, 16) as u32;
    BACKOFF_BASE_MS
        .saturating_mul(1_i64 << shift)
        .min(BACKOFF_CAP_MS)
}

/// What to do with a job after running its provider.
enum Outcome {
    /// Done (or nothing to do) → `complete_job`.
    Complete,
    /// Unrecoverable (malformed job, missing file) → dead-letter now, no retry.
    DeadLetter(String),
    /// Recoverable (embed/upsert/read error) → `fail_job` with backoff (the store
    /// dead-letters once attempts are exhausted).
    Retry(String),
}

/// Everything a worker needs, shared (via `Arc`) across the pool.
pub(crate) struct Shared {
    pub store: Arc<dyn Store>,
    pub embedder: Arc<dyn EmbeddingProvider>,
    /// App-data root; `frames.image_path` is stored relative to it (`frames/…`).
    pub data_dir: PathBuf,
    pub events: broadcast::Sender<KernelEvent>,
    pub kinds: Vec<JobKind>,
    pub concurrency: usize,
}

/// Handle to the running pool: a stop signal and the spawned task joins.
pub(crate) struct WorkerPool {
    stop: watch::Sender<bool>,
    joins: Vec<JoinHandle<()>>,
}

impl WorkerPool {
    /// Spawns `concurrency` worker tasks plus one stale-job sweep task, all stopped
    /// by a single `watch` channel.
    pub(crate) fn spawn(shared: Shared) -> Self {
        let (stop_tx, stop_rx) = watch::channel(false);
        let shared = Arc::new(shared);
        let mut joins = Vec::with_capacity(shared.concurrency + 1);
        for _ in 0..shared.concurrency.max(1) {
            joins.push(tokio::spawn(worker_loop(shared.clone(), stop_rx.clone())));
        }
        joins.push(tokio::spawn(sweep_loop(shared.clone(), stop_rx.clone())));
        Self {
            stop: stop_tx,
            joins,
        }
    }

    /// Signals every worker to stop and waits for in-flight jobs to finish. Takes the
    /// joins out via `mem::take` (rather than moving out of `self`) because
    /// [`WorkerPool`] implements [`Drop`].
    pub(crate) async fn shutdown(mut self) {
        let _ = self.stop.send(true);
        for join in std::mem::take(&mut self.joins) {
            let _ = join.await;
        }
    }
}

impl Drop for WorkerPool {
    /// Best-effort safety net: if the handle is dropped without a graceful
    /// `shutdown` (an early return or a panic in the owner), still signal stop so the
    /// detached `tokio::spawn` tasks exit their loops promptly instead of draining the
    /// whole queue first. (`drop` can't `.await` the joins, but the signal alone ends
    /// the loops.)
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}

/// One worker: claim a job, process it, emit progress; back off when the queue is
/// empty; stop promptly when signalled.
async fn worker_loop(shared: Arc<Shared>, mut stop: watch::Receiver<bool>) {
    let mut backoff = IDLE_MIN;
    loop {
        if *stop.borrow() {
            break;
        }
        let claimed = match shared.store.claim_jobs(&shared.kinds, 1, now_ms()).await {
            Ok(mut jobs) => jobs.pop(),
            Err(e) => {
                tracing::warn!(error = %e, "worker: claim failed");
                None
            }
        };
        match claimed {
            Some(job) => {
                if let Err(e) =
                    process_job(&shared.store, &shared.embedder, &shared.data_dir, job).await
                {
                    tracing::warn!(error = %e, "worker: finalizing job failed");
                }
                emit_progress(&shared).await;
                backoff = IDLE_MIN;
            }
            None => {
                tokio::select! {
                    biased;
                    _ = stop.changed() => break,
                    _ = tokio::time::sleep(backoff) => {}
                }
                backoff = (backoff * 2).min(IDLE_MAX);
            }
        }
    }
}

/// Periodic stale-`running` recovery (`03 §6`, `07` gap #6).
async fn sweep_loop(shared: Arc<Shared>, mut stop: watch::Receiver<bool>) {
    loop {
        tokio::select! {
            biased;
            _ = stop.changed() => break,
            _ = tokio::time::sleep(SWEEP_INTERVAL) => {
                match shared.store.reset_stale_running_jobs(VISIBILITY_TIMEOUT_MS).await {
                    Ok(n) if n > 0 => tracing::warn!(requeued = n, "sweep: requeued stale running jobs"),
                    Ok(_) => {}
                    Err(e) => tracing::warn!(error = %e, "sweep: failed"),
                }
            }
        }
    }
}

async fn emit_progress(shared: &Shared) {
    if let Ok(stats) = shared.store.job_stats().await {
        let _ = shared.events.send(KernelEvent::JobProgress(stats));
    }
}

/// Runs one already-claimed job to a terminal state: executes the provider, then
/// `complete_job` / `fail_job` (with backoff) / dead-letter. Exposed so tests can
/// drive a single job deterministically without the polling loop.
pub async fn process_job(
    store: &Arc<dyn Store>,
    embedder: &Arc<dyn EmbeddingProvider>,
    data_dir: &Path,
    job: Job,
) -> Result<()> {
    let outcome = match job.kind {
        JobKind::EmbedText => embed_text_outcome(store, embedder, job.frame_id).await,
        JobKind::EmbedImage => embed_image_outcome(store, embedder, data_dir, job.frame_id).await,
        // vision_tag is never claimed by P3 workers; defensive only.
        JobKind::VisionTag => Outcome::DeadLetter("vision_tag is not handled until P4".to_string()),
    };
    match outcome {
        Outcome::Complete => store.complete_job(job.id).await,
        Outcome::DeadLetter(err) => {
            tracing::warn!(job = job.id, error = %err, "job dead-lettered");
            store.fail_job(job.id, &err, None).await
        }
        Outcome::Retry(err) => {
            let retry_at = now_ms() + backoff_ms(job.attempts);
            tracing::debug!(job = job.id, error = %err, retry_at, "job retry scheduled");
            store.fail_job(job.id, &err, Some(retry_at)).await
        }
    }
}

/// Embed a frame's OCR text into one chunk (`chunk_index = 0`, `03 §5`). A purged
/// frame or empty OCR text is a no-op success — embedding nothing is meaningless,
/// not a failure.
async fn embed_text_outcome(
    store: &Arc<dyn Store>,
    embedder: &Arc<dyn EmbeddingProvider>,
    frame_id: Option<i64>,
) -> Outcome {
    let Some(frame_id) = frame_id else {
        return Outcome::DeadLetter("embed_text job has no frame_id".to_string());
    };
    let input = match store.get_enrichment_input(frame_id).await {
        Ok(Some(i)) => i,
        Ok(None) => return Outcome::Complete, // frame purged between enqueue and run
        Err(e) => return Outcome::Retry(format!("read frame {frame_id}: {e}")),
    };
    let text = input.ocr_text.unwrap_or_default();
    if text.trim().is_empty() {
        return Outcome::Complete; // no OCR text to embed
    }
    let emb = match embedder.embed_texts(std::slice::from_ref(&text)).await {
        Ok(mut v) => match v.pop() {
            Some(e) => e,
            None => return Outcome::Retry("text embedder returned no vector".to_string()),
        },
        Err(e) => return Outcome::Retry(format!("embed text: {e}")),
    };
    match store
        .upsert_text_embedding(
            frame_id,
            0,
            &text,
            ChunkSource::Ocr,
            &emb,
            embedder.text_model_name(),
        )
        .await
    {
        Ok(()) => Outcome::Complete,
        Err(e) => Outcome::Retry(format!("upsert text embedding: {e}")),
    }
}

/// Embed a frame's stored JPEG (optional visual recall, `03 §4`). The JPEG lives at
/// `data_dir / image_path`; a missing/corrupt file is dead-lettered (it won't fix
/// itself), an embed/upsert error is retryable.
async fn embed_image_outcome(
    store: &Arc<dyn Store>,
    embedder: &Arc<dyn EmbeddingProvider>,
    data_dir: &Path,
    frame_id: Option<i64>,
) -> Outcome {
    let Some(frame_id) = frame_id else {
        return Outcome::DeadLetter("embed_image job has no frame_id".to_string());
    };
    let input = match store.get_enrichment_input(frame_id).await {
        Ok(Some(i)) => i,
        Ok(None) => return Outcome::Complete,
        Err(e) => return Outcome::Retry(format!("read frame {frame_id}: {e}")),
    };
    let abs = data_dir.join(&input.image_path);
    let image = match load_rgba(abs.clone()).await {
        Ok(img) => img,
        // A transient IO / sharing violation (AV, search indexer, or backup briefly
        // holding the file on Windows) clears on retry; a file that is genuinely gone
        // won't reappear. (The FK cascade deletes a frame's jobs with the frame, so an
        // existing job whose JPEG is missing means an out-of-band deletion.)
        Err(e) => {
            return if abs.exists() {
                Outcome::Retry(format!("load image {}: {e}", abs.display()))
            } else {
                Outcome::DeadLetter(format!("image missing {}: {e}", abs.display()))
            };
        }
    };
    let emb = match embedder.embed_image(&image).await {
        Ok(e) => e,
        Err(e) => return Outcome::Retry(format!("embed image: {e}")),
    };
    match store
        .upsert_image_embedding(frame_id, &emb, embedder.image_model_name())
        .await
    {
        Ok(()) => Outcome::Complete,
        Err(e) => Outcome::Retry(format!("upsert image embedding: {e}")),
    }
}

/// Decode a JPEG/PNG from disk into RGBA on the blocking pool (CPU + file IO).
async fn load_rgba(path: PathBuf) -> Result<image::RgbaImage> {
    tokio::task::spawn_blocking(move || -> Result<image::RgbaImage> {
        Ok(image::open(&path)?.to_rgba8())
    })
    .await
    .map_err(|e| anyhow!("image decode task failed: {e}"))?
}

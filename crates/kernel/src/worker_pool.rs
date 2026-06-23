//! The bounded enrichment worker pool — the consumer half of *enrich-deferred*
//! (`03 §5`). Each worker independently `claim_jobs`→runs the provider→
//! `complete_job`/`fail_job`, so the durable queue P2 fills is finally drained into
//! vectors and deferred vision analyses. Provider attachment is slot-driven:
//! embedding jobs are claimed only when an embedder is attached and the matching
//! setting is enabled, while `vision_tag` jobs are claimed once the sidecar-backed
//! vision provider is attached.
//!
//! Shutdown mirrors the capture loop: one `watch` channel stops every worker after
//! its in-flight job. A periodic sweep requeues jobs a dead worker left `running`
//! (`03 §6`, `07` gap #6). There is no durable per-job lease, so the sweep avoids
//! requeueing while this process still has live in-flight jobs; startup recovery
//! still requeues any `running` jobs before workers are spawned.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use anyhow::{anyhow, Result};
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use traits::{ChunkSource, EmbeddingProvider, Job, JobKind, Store, VisionProvider};

use crate::events::KernelEvent;

/// Shared, runtime-settable embedding provider slot (`None` until the embedder loads
/// off the launch thread). Behind an `Arc<RwLock>` so inference can start the worker
/// pool before embeddings are available, and the pool can pick up embeddings later
/// without a restart.
pub(crate) type EmbedderSlot = Arc<RwLock<Option<Arc<dyn EmbeddingProvider>>>>;

/// Shared, runtime-settable vision provider slot (`None` until the inference sidecar
/// is attached). Behind an `Arc<RwLock>` so the kernel can attach the provider after
/// the pool is already running without restarting it (`03 §6`).
pub(crate) type VisionSlot = Arc<RwLock<Option<Arc<dyn VisionProvider>>>>;

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
    pub embedder: EmbedderSlot,
    /// The vision provider, attached after the sidecar comes up (`None` until then).
    pub vision: VisionSlot,
    pub enable_embed_text: bool,
    pub enable_embed_image: bool,
    /// Job ids currently being processed by this worker pool. The periodic sweep
    /// uses this as a process-local guard so long-running provider calls can still
    /// record their final retry/dead-letter outcome instead of being requeued out
    /// from under themselves.
    pub active_jobs: Arc<Mutex<HashSet<i64>>>,
    /// App-data root; `frames.image_path` is stored relative to it (`frames/…`).
    pub data_dir: PathBuf,
    pub events: broadcast::Sender<KernelEvent>,
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

struct ActiveJobGuard {
    active_jobs: Arc<Mutex<HashSet<i64>>>,
    id: i64,
}

impl ActiveJobGuard {
    fn new(active_jobs: Arc<Mutex<HashSet<i64>>>, id: i64) -> Self {
        active_jobs
            .lock()
            .expect("active job set lock poisoned")
            .insert(id);
        Self { active_jobs, id }
    }
}

impl Drop for ActiveJobGuard {
    fn drop(&mut self) {
        self.active_jobs
            .lock()
            .expect("active job set lock poisoned")
            .remove(&self.id);
    }
}

fn active_job_count(active_jobs: &Mutex<HashSet<i64>>) -> usize {
    active_jobs
        .lock()
        .expect("active job set lock poisoned")
        .len()
}

fn claim_kinds(shared: &Shared) -> ([JobKind; 3], usize) {
    let embedder_ready = shared
        .embedder
        .read()
        .expect("embedder slot lock")
        .is_some();
    let vision_ready = shared.vision.read().expect("vision slot lock").is_some();
    let mut kinds = [JobKind::EmbedText; 3];
    let mut len = 0;
    if embedder_ready {
        if shared.enable_embed_text {
            kinds[len] = JobKind::EmbedText;
            len += 1;
        }
        if shared.enable_embed_image {
            kinds[len] = JobKind::EmbedImage;
            len += 1;
        }
    }
    if vision_ready {
        kinds[len] = JobKind::VisionTag;
        len += 1;
    }
    (kinds, len)
}

/// One worker: claim a job, process it, emit progress; back off when the queue is
/// empty; stop promptly when signalled.
async fn worker_loop(shared: Arc<Shared>, mut stop: watch::Receiver<bool>) {
    let mut backoff = IDLE_MIN;
    loop {
        if *stop.borrow() {
            break;
        }
        let (kinds_arr, len) = claim_kinds(&shared);
        let kinds = &kinds_arr[..len];
        let claimed = if kinds.is_empty() {
            None
        } else {
            match shared.store.claim_jobs(kinds, 1, now_ms()).await {
                Ok(mut jobs) => jobs.pop(),
                Err(e) => {
                    tracing::warn!(error = %e, "worker: claim failed");
                    None
                }
            }
        };
        match claimed {
            Some(job) => {
                let _active = ActiveJobGuard::new(shared.active_jobs.clone(), job.id);
                // Snapshot providers out of the shared slots before the await (the std
                // RwLock guards must not cross it). Claim kinds are also slot-gated,
                // so missing providers here are defensive against races.
                let embedder = shared.embedder.read().expect("embedder slot lock").clone();
                let vision = shared.vision.read().expect("vision slot lock").clone();
                if let Err(e) = process_job_with_providers(
                    &shared.store,
                    embedder.as_ref(),
                    vision.as_ref(),
                    &shared.data_dir,
                    job,
                )
                .await
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

/// Runs one already-claimed job to a terminal state: executes the provider, then
/// `complete_job` / `fail_job` (with backoff) / dead-letter. Exposed so tests can
/// drive a single job deterministically without the polling loop.
pub async fn process_job(
    store: &Arc<dyn Store>,
    embedder: &Arc<dyn EmbeddingProvider>,
    vision: Option<&Arc<dyn VisionProvider>>,
    data_dir: &Path,
    job: Job,
) -> Result<()> {
    process_job_with_providers(store, Some(embedder), vision, data_dir, job).await
}

async fn process_job_with_providers(
    store: &Arc<dyn Store>,
    embedder: Option<&Arc<dyn EmbeddingProvider>>,
    vision: Option<&Arc<dyn VisionProvider>>,
    data_dir: &Path,
    job: Job,
) -> Result<()> {
    let outcome = match job.kind {
        JobKind::EmbedText => match embedder {
            Some(embedder) => embed_text_outcome(store, embedder, job.frame_id).await,
            None => Outcome::Retry("embedding provider not attached".to_string()),
        },
        JobKind::EmbedImage => match embedder {
            Some(embedder) => embed_image_outcome(store, embedder, data_dir, job.frame_id).await,
            None => Outcome::Retry("embedding provider not attached".to_string()),
        },
        JobKind::VisionTag => vision_tag_outcome(store, vision, data_dir, job.frame_id).await,
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

/// Periodic stale-`running` recovery (`03 §6`, `07` gap #6).
async fn sweep_loop(shared: Arc<Shared>, mut stop: watch::Receiver<bool>) {
    loop {
        tokio::select! {
            biased;
            _ = stop.changed() => break,
            _ = tokio::time::sleep(SWEEP_INTERVAL) => {
                let active = active_job_count(&shared.active_jobs);
                if active > 0 {
                    tracing::debug!(active, "sweep: skipped while worker jobs are active");
                    continue;
                }
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

/// Run deferred vision tagging on a frame's stored JPEG via the sidecar provider
/// (`03 §5`). Image-load handling mirrors [`embed_image_outcome`]: a transient IO
/// error retries, a genuinely missing file is dead-lettered. A purged frame is a
/// no-op success. A sidecar/analyze error is retryable (the sidecar may be restarting).
async fn vision_tag_outcome(
    store: &Arc<dyn Store>,
    vision: Option<&Arc<dyn VisionProvider>>,
    data_dir: &Path,
    frame_id: Option<i64>,
) -> Outcome {
    let Some(frame_id) = frame_id else {
        return Outcome::DeadLetter("vision_tag job has no frame_id".to_string());
    };
    let Some(vision) = vision else {
        // A vision_tag job exists but no provider is attached yet — retry; the
        // backlog drains once the sidecar comes up (`03 §6`).
        return Outcome::Retry("vision provider not attached".to_string());
    };
    let input = match store.get_enrichment_input(frame_id).await {
        Ok(Some(i)) => i,
        Ok(None) => return Outcome::Complete, // frame purged between enqueue and run
        Err(e) => return Outcome::Retry(format!("read frame {frame_id}: {e}")),
    };
    let abs = data_dir.join(&input.image_path);
    let image = match load_rgba(abs.clone()).await {
        Ok(img) => img,
        Err(e) => {
            return if abs.exists() {
                Outcome::Retry(format!("load image {}: {e}", abs.display()))
            } else {
                Outcome::DeadLetter(format!("image missing {}: {e}", abs.display()))
            };
        }
    };
    let analysis = match vision.analyze(&image).await {
        Ok(a) => a,
        Err(e) => return Outcome::Retry(format!("vision analyze frame {frame_id}: {e}")),
    };
    match store.insert_vision(frame_id, analysis).await {
        Ok(()) => Outcome::Complete,
        Err(e) => Outcome::Retry(format!("insert vision {frame_id}: {e}")),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_job_guard_tracks_in_flight_job_until_drop() {
        let active_jobs = Arc::new(Mutex::new(HashSet::new()));
        assert_eq!(active_job_count(&active_jobs), 0);

        {
            let _guard = ActiveJobGuard::new(active_jobs.clone(), 42);
            assert_eq!(active_job_count(&active_jobs), 1);
        }

        assert_eq!(active_job_count(&active_jobs), 0);
    }
}

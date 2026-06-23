//! `kernel` — the orchestrator that owns the typed event bus and the capture
//! pipeline (`03 §1/§5`). It depends only on [`traits`] (the contracts), never on a
//! module's concrete impl; `src-tauri` wires impls in at startup via the capture
//! factory (the composition root, `03 §2`).
//!
//! P2 landed the always-on half: [`Kernel::start_capture`] spawns the capture loop
//! (CaptureSource → OcrProvider → Store → `embed_text` job → `capture_tick`). P3 adds
//! the bounded enrichment worker pool ([`Kernel::attach_embedder`]) that drains those
//! jobs into vectors; the `ModelSupervisor` lands in P4.

#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use tokio::sync::{broadcast, watch, Mutex};
use tokio::task::JoinHandle;
use traits::{
    AnswerProvider, CaptureConfig, CaptureSource, ComponentReadiness, ComponentStatus,
    EmbeddingProvider, JobKind, NewJob, OcrProvider, Readiness, SidecarState, SidecarStatus, Store,
    VisionProvider, VisionTarget,
};

mod capture_loop;
mod events;
pub mod settings;
mod vision_scheduler;
mod worker_pool;

pub use capture_loop::{run_capture_loop, CaptureLoopExit, LoopCtx};
pub use events::KernelEvent;
pub use worker_pool::process_job;

/// A platform idle-time source, injected by the composition root (the kernel forbids
/// `unsafe`, so it can't query Win32 itself). Returns milliseconds since the last user
/// input, or `None` if it can't be determined. Drives idle-triggered vision tagging
/// (`03 §5`).
pub type IdleSource = Arc<dyn Fn() -> Option<u64> + Send + Sync>;

/// On-demand vision jobs outrank the background embedding backlog so a user's explicit
/// "tag this" is serviced first (`03 §5`).
const VISION_ONDEMAND_PRIORITY: i64 = 10;
/// Upper bound on frames enqueued for one `enqueue_vision` range request.
const VISION_RANGE_CAP: u32 = 1000;

/// Builds a fresh [`CaptureSource`] from the current [`CaptureConfig`]. Supplied by
/// the composition root so the kernel stays impl-agnostic; called on every
/// `start_capture` so settings changes take effect and `stop` can fully release the
/// OS capture resources by dropping the source.
pub type CaptureFactory =
    Arc<dyn Fn(CaptureConfig) -> anyhow::Result<Box<dyn CaptureSource>> + Send + Sync>;

struct CaptureHandle {
    id: u64,
    stop: watch::Sender<bool>,
    join: JoinHandle<()>,
}

/// The orchestrator. Holds the long-lived providers (store, OCR), the capture
/// factory, the event bus, and the live readiness snapshot; starts/stops the
/// capture loop on demand (`capture_control`, `03 §7`).
pub struct Kernel {
    store: Arc<dyn Store>,
    ocr: Arc<dyn OcrProvider>,
    /// Set when the composition root could not create a real OCR provider. Capture
    /// refuses to start rather than silently storing empty text rows.
    ocr_unavailable: Option<String>,
    capture_factory: CaptureFactory,
    frames_dir: PathBuf,
    events: broadcast::Sender<KernelEvent>,
    readiness: Arc<RwLock<Readiness>>,
    capture: Arc<Mutex<Option<CaptureHandle>>>,
    capture_generation: AtomicU64,
    /// The loaded embedding provider, attached after it finishes loading off the
    /// launch thread (`03 §5`); `None` until [`Kernel::attach_embedder`].
    embedder: Mutex<Option<Arc<dyn EmbeddingProvider>>>,
    /// The running enrichment worker pool; `None` until workers are started.
    workers: Mutex<Option<worker_pool::WorkerPool>>,
    /// The vision provider, shared live with the worker pool so it can be attached
    /// after the pool is running (`03 §6`). `None` until [`Kernel::attach_inference`].
    vision: worker_pool::VisionSlot,
    /// The answer provider (backs the `ask` command); `None` until attached.
    answer: Mutex<Option<Arc<dyn AnswerProvider>>>,
    /// The running timer/idle vision scheduler; `None` until inference is attached.
    scheduler: Mutex<Option<vision_scheduler::SchedulerHandle>>,
}

impl Kernel {
    /// Wires the orchestrator. `initial_readiness` carries the `db` status the
    /// composition root already resolved; `capture` starts `Disabled` (off until
    /// the user starts it — privacy-first, recorded in `07`).
    pub fn new(
        store: Arc<dyn Store>,
        ocr: Arc<dyn OcrProvider>,
        capture_factory: CaptureFactory,
        frames_dir: PathBuf,
        initial_readiness: Readiness,
    ) -> Self {
        Self::new_inner(
            store,
            ocr,
            None,
            capture_factory,
            frames_dir,
            initial_readiness,
        )
    }

    /// Builds a kernel whose OCR dependency is known unavailable. The app can keep
    /// running, but `start_capture` fails before opening WGC so it never creates
    /// misleading empty OCR rows.
    pub fn new_with_ocr_unavailable(
        store: Arc<dyn Store>,
        ocr: Arc<dyn OcrProvider>,
        capture_factory: CaptureFactory,
        frames_dir: PathBuf,
        initial_readiness: Readiness,
        reason: String,
    ) -> Self {
        Self::new_inner(
            store,
            ocr,
            Some(reason),
            capture_factory,
            frames_dir,
            initial_readiness,
        )
    }

    fn new_inner(
        store: Arc<dyn Store>,
        ocr: Arc<dyn OcrProvider>,
        ocr_unavailable: Option<String>,
        capture_factory: CaptureFactory,
        frames_dir: PathBuf,
        mut initial_readiness: Readiness,
    ) -> Self {
        initial_readiness.capture = ComponentReadiness {
            status: ComponentStatus::Disabled,
            detail: Some("not started".to_string()),
        };
        let (events, _rx) = broadcast::channel(256);
        Self {
            store,
            ocr,
            ocr_unavailable,
            capture_factory,
            frames_dir,
            events,
            readiness: Arc::new(RwLock::new(initial_readiness)),
            capture: Arc::new(Mutex::new(None)),
            capture_generation: AtomicU64::new(1),
            embedder: Mutex::new(None),
            workers: Mutex::new(None),
            vision: Arc::new(RwLock::new(None)),
            answer: Mutex::new(None),
            scheduler: Mutex::new(None),
        }
    }

    /// Subscribe to the kernel event bus (the composition root forwards these to
    /// Tauri events).
    pub fn subscribe(&self) -> broadcast::Receiver<KernelEvent> {
        self.events.subscribe()
    }

    /// The current readiness snapshot (backs the `get_readiness` command).
    pub fn readiness(&self) -> Readiness {
        self.readiness
            .read()
            .expect("readiness lock poisoned")
            .clone()
    }

    /// Whether the capture loop is currently running.
    pub async fn is_capturing(&self) -> bool {
        self.capture.lock().await.is_some()
    }

    /// Starts the capture loop (idempotent — a no-op if already running). Loads
    /// settings, builds the capture source via the factory, and spawns the loop;
    /// flips `capture` readiness Initializing → Ready (or Unavailable on failure).
    pub async fn start_capture(&self) -> anyhow::Result<()> {
        let mut guard = self.capture.lock().await;
        if guard.is_some() {
            return Ok(());
        }
        if let Some(reason) = &self.ocr_unavailable {
            let detail = format!("OCR unavailable: {reason}");
            self.set_capture_readiness(ComponentStatus::Unavailable, Some(detail.clone()));
            return Err(anyhow::anyhow!(detail));
        }
        self.set_capture_readiness(ComponentStatus::Initializing, None);

        let settings = settings::load_settings(self.store.as_ref()).await;
        let cfg = settings::capture_config(&settings);
        let capture = match (self.capture_factory)(cfg) {
            Ok(c) => c,
            Err(e) => {
                self.set_capture_readiness(ComponentStatus::Unavailable, Some(e.to_string()));
                return Err(e);
            }
        };

        let (stop_tx, stop_rx) = watch::channel(false);
        let ctx = LoopCtx {
            store: self.store.clone(),
            ocr: self.ocr.clone(),
            frames_dir: self.frames_dir.clone(),
            events: self.events.clone(),
            enrich_embed_text: settings.enrich_embed_text,
            enrich_image_embeddings: settings.enrich_image_embeddings,
            jpeg_quality: settings.storage_jpeg_quality,
            max_width: settings.storage_max_width,
        };
        let id = self.capture_generation.fetch_add(1, Ordering::Relaxed);
        let capture_slot = self.capture.clone();
        let readiness = self.readiness.clone();
        let events = self.events.clone();
        let join = tokio::spawn(async move {
            let exit = run_capture_loop(capture, ctx, stop_rx).await;
            if exit == CaptureLoopExit::SourceShutdown {
                let mut guard = capture_slot.lock().await;
                if guard.as_ref().is_some_and(|h| h.id == id) {
                    *guard = None;
                    drop(guard);
                    set_capture_readiness(
                        &readiness,
                        &events,
                        ComponentStatus::Error,
                        Some("capture source shut down unexpectedly".to_string()),
                    );
                }
            }
        });
        *guard = Some(CaptureHandle {
            id,
            stop: stop_tx,
            join,
        });
        self.set_capture_readiness(ComponentStatus::Ready, None);
        drop(guard);

        tracing::info!("capture started");
        Ok(())
    }

    /// Stops the capture loop (idempotent). Signals the loop, waits for it to drop
    /// the capture source (releasing OS resources), and flips `capture` → Disabled.
    pub async fn stop_capture(&self) {
        let handle = self.capture.lock().await.take();
        if let Some(h) = handle {
            let _ = h.stop.send(true);
            if let Err(e) = h.join.await {
                tracing::warn!(error = %e, "capture task join failed");
            }
        }
        self.set_capture_readiness(ComponentStatus::Disabled, None);
        tracing::info!("capture stopped");
    }

    fn set_capture_readiness(&self, status: ComponentStatus, detail: Option<String>) {
        set_capture_readiness(&self.readiness, &self.events, status, detail);
    }

    /// Attaches the loaded embedding provider: lights up the store's vector arm and
    /// starts the worker pool (`03 §5`). The composition root calls this once the
    /// fastembed model has loaded off the launch thread — independent of capture, so
    /// the queue's backlog drains in the background (`02 §5`).
    pub async fn attach_embedder(&self, embedder: Arc<dyn EmbeddingProvider>) {
        self.store.set_embedder(embedder.clone());
        *self.embedder.lock().await = Some(embedder);
        self.start_workers().await;
    }

    /// Starts the bounded enrichment worker pool (idempotent — a no-op if already
    /// running or if no embedder is attached). Runs the startup stale-job sweep
    /// first so a prior run's interrupted jobs are requeued (`03 §6`).
    pub async fn start_workers(&self) {
        let mut guard = self.workers.lock().await;
        if guard.is_some() {
            return;
        }
        let Some(embedder) = self.embedder.lock().await.clone() else {
            tracing::warn!("start_workers: no embedder attached");
            return;
        };
        // With no worker live yet, every `running` job is stale (`03 §6`).
        match self.store.reset_stale_running_jobs(0).await {
            Ok(n) if n > 0 => tracing::info!(requeued = n, "startup sweep requeued stale jobs"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "startup sweep failed"),
        }
        let settings = settings::load_settings(self.store.as_ref()).await;
        let mut kinds = vec![JobKind::EmbedText];
        if settings.enrich_image_embeddings {
            kinds.push(JobKind::EmbedImage);
        }
        // Always claim vision_tag: such jobs only exist once a producer enqueues them
        // (on-demand/timer/idle), and by then the provider is attached via the shared
        // slot — so this doesn't depend on inference being up yet (`03 §6`).
        kinds.push(JobKind::VisionTag);
        // `frames.image_path` is stored relative to the app-data root (`frames/…`).
        let data_dir = self
            .frames_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.frames_dir.clone());
        let pool = worker_pool::WorkerPool::spawn(worker_pool::Shared {
            store: self.store.clone(),
            embedder,
            vision: self.vision.clone(),
            active_jobs: Arc::new(std::sync::Mutex::new(HashSet::new())),
            data_dir,
            events: self.events.clone(),
            kinds,
            concurrency: settings.enrich_worker_concurrency.max(1) as usize,
        });
        *guard = Some(pool);
        self.set_embed_readiness(ComponentStatus::Ready, None);
        tracing::info!("embedding workers started");
    }

    /// Stops the worker pool (idempotent), waiting for in-flight jobs to finish.
    pub async fn stop_workers(&self) {
        let pool = self.workers.lock().await.take();
        if let Some(pool) = pool {
            pool.shutdown().await;
            tracing::info!("embedding workers stopped");
        }
    }

    /// Sets the `embed_model` readiness and broadcasts the change (`03 §7`).
    pub fn set_embed_readiness(&self, status: ComponentStatus, detail: Option<String>) {
        let snapshot = {
            let mut r = self.readiness.write().expect("readiness lock poisoned");
            r.embed_model = ComponentReadiness { status, detail };
            r.clone()
        };
        let _ = self.events.send(KernelEvent::ReadinessChanged(snapshot));
    }

    /// Attaches the inference providers (`03 §6`): the vision provider lights up the
    /// `vision_tag` worker path via the shared slot (no pool restart needed), the
    /// answer provider backs the `ask` command, and the timer/idle vision scheduler
    /// starts. The composition root calls this once the sidecar binary + default model
    /// are resolved off the launch thread.
    pub async fn attach_inference(
        &self,
        vision: Arc<dyn VisionProvider>,
        answer: Arc<dyn AnswerProvider>,
        idle: Option<IdleSource>,
    ) {
        *self.vision.write().expect("vision slot lock") = Some(vision);
        *self.answer.lock().await = Some(answer);
        self.start_vision_scheduler(idle).await;
        tracing::info!("inference providers attached");
    }

    /// The attached answer provider, if any (backs the `ask` command).
    pub async fn answer_provider(&self) -> Option<Arc<dyn AnswerProvider>> {
        self.answer.lock().await.clone()
    }

    /// Enqueues deferred vision tagging for a frame or a time range (`enqueue_vision`,
    /// `03 §5/§7`). On-demand, so it is the one path that may tag without a timer/idle
    /// trigger. Returns how many `vision_tag` jobs were enqueued. A single frame is
    /// always (re)tagged; a range tags only its still-untagged frames.
    pub async fn enqueue_vision(&self, target: VisionTarget) -> anyhow::Result<u64> {
        let ids = match target {
            VisionTarget::Frame { frame_id } => vec![frame_id],
            VisionTarget::Range { start, end } => {
                self.store
                    .untagged_frame_ids(VISION_RANGE_CAP, Some((start, end)))
                    .await?
            }
        };
        let mut count = 0u64;
        for frame_id in ids {
            let job = NewJob {
                kind: JobKind::VisionTag,
                frame_id: Some(frame_id),
                priority: VISION_ONDEMAND_PRIORITY,
                max_attempts: 3,
                not_before: 0,
            };
            match self.store.enqueue_job(job).await {
                Ok(_) => count += 1,
                Err(e) => tracing::warn!(frame_id, error = %e, "enqueue_vision: enqueue failed"),
            }
        }
        tracing::info!(count, "enqueue_vision enqueued vision_tag jobs");
        Ok(count)
    }

    /// Records a sidecar lifecycle transition: maps it onto `sidecar` readiness and
    /// re-broadcasts both the readiness change and the raw status (`sidecar_status`,
    /// `03 §6/§7`). The composition root bridges the supervisor's status channel here.
    pub fn emit_sidecar_status(&self, status: SidecarStatus) {
        let (component, detail) = sidecar_component(&status);
        let snapshot = {
            let mut r = self.readiness.write().expect("readiness lock poisoned");
            r.sidecar = ComponentReadiness {
                status: component,
                detail,
            };
            r.clone()
        };
        let _ = self.events.send(KernelEvent::ReadinessChanged(snapshot));
        let _ = self.events.send(KernelEvent::SidecarStatus(status));
    }

    /// Sets the `sidecar` readiness directly and broadcasts the change. Used by the
    /// composition root while resolving the binary/model before the supervisor exists
    /// (Initializing → Ready / Unavailable, `03 §7`).
    pub fn set_sidecar_readiness(&self, status: ComponentStatus, detail: Option<String>) {
        let snapshot = {
            let mut r = self.readiness.write().expect("readiness lock poisoned");
            r.sidecar = ComponentReadiness { status, detail };
            r.clone()
        };
        let _ = self.events.send(KernelEvent::ReadinessChanged(snapshot));
    }

    /// Starts the timer/idle vision scheduler (idempotent). `idle` is `None` when no
    /// platform idle source is available (timer-only).
    async fn start_vision_scheduler(&self, idle: Option<IdleSource>) {
        let mut guard = self.scheduler.lock().await;
        if guard.is_some() {
            return;
        }
        *guard = Some(vision_scheduler::spawn(self.store.clone(), idle));
        tracing::info!("vision scheduler started");
    }

    /// Stops the vision scheduler (idempotent). Called on shutdown.
    pub async fn stop_vision_scheduler(&self) {
        if let Some(handle) = self.scheduler.lock().await.take() {
            handle.stop().await;
            tracing::info!("vision scheduler stopped");
        }
    }
}

/// Maps a sidecar lifecycle state to a `sidecar` readiness component (`03 §6/§7`). An
/// evicted sidecar is still `Ready` — it can lazily respawn on the next request.
fn sidecar_component(status: &SidecarStatus) -> (ComponentStatus, Option<String>) {
    match status.state {
        SidecarState::Starting => (ComponentStatus::Initializing, status.model.clone()),
        SidecarState::Ready => (ComponentStatus::Ready, status.model.clone()),
        SidecarState::Evicted => (
            ComponentStatus::Ready,
            Some("evicted (idle); respawns on demand".to_string()),
        ),
        SidecarState::Crashed => (ComponentStatus::Error, Some("sidecar crashed".to_string())),
        SidecarState::Stopped => (ComponentStatus::Disabled, Some("stopped".to_string())),
    }
}

fn set_capture_readiness(
    readiness: &RwLock<Readiness>,
    events: &broadcast::Sender<KernelEvent>,
    status: ComponentStatus,
    detail: Option<String>,
) {
    let snapshot = {
        let mut r = readiness.write().expect("readiness lock poisoned");
        r.capture = ComponentReadiness { status, detail };
        r.clone()
    };
    let _ = events.send(KernelEvent::ReadinessChanged(snapshot));
}

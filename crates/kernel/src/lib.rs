//! `kernel` — the orchestrator that owns the typed event bus and the capture
//! pipeline (`03 §1/§5`). It depends only on [`traits`] (the contracts), never on a
//! module's concrete impl; `src-tauri` wires impls in at startup via the capture
//! factory (the composition root, `03 §2`).
//!
//! P2 lands the always-on half: [`Kernel::start_capture`] spawns the capture loop
//! (CaptureSource → OcrProvider → Store → `embed_text` job → `capture_tick`). The
//! bounded worker pool (embeddings) lands in P3 and the `ModelSupervisor` in P4.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use tokio::sync::{broadcast, watch, Mutex};
use tokio::task::JoinHandle;
use traits::{
    CaptureConfig, CaptureSource, ComponentReadiness, ComponentStatus, OcrProvider, Readiness,
    Store,
};

mod capture_loop;
mod events;
pub mod settings;

pub use capture_loop::{run_capture_loop, LoopCtx};
pub use events::KernelEvent;

/// Builds a fresh [`CaptureSource`] from the current [`CaptureConfig`]. Supplied by
/// the composition root so the kernel stays impl-agnostic; called on every
/// `start_capture` so settings changes take effect and `stop` can fully release the
/// OS capture resources by dropping the source.
pub type CaptureFactory =
    Arc<dyn Fn(CaptureConfig) -> anyhow::Result<Box<dyn CaptureSource>> + Send + Sync>;

struct CaptureHandle {
    stop: watch::Sender<bool>,
    join: JoinHandle<()>,
}

/// The orchestrator. Holds the long-lived providers (store, OCR), the capture
/// factory, the event bus, and the live readiness snapshot; starts/stops the
/// capture loop on demand (`capture_control`, `03 §7`).
pub struct Kernel {
    store: Arc<dyn Store>,
    ocr: Arc<dyn OcrProvider>,
    capture_factory: CaptureFactory,
    frames_dir: PathBuf,
    events: broadcast::Sender<KernelEvent>,
    readiness: RwLock<Readiness>,
    capture: Mutex<Option<CaptureHandle>>,
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
            capture_factory,
            frames_dir,
            events,
            readiness: RwLock::new(initial_readiness),
            capture: Mutex::new(None),
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
            jpeg_quality: settings.storage_jpeg_quality,
            max_width: settings.storage_max_width,
        };
        let join = tokio::spawn(run_capture_loop(capture, ctx, stop_rx));
        *guard = Some(CaptureHandle {
            stop: stop_tx,
            join,
        });
        drop(guard);

        self.set_capture_readiness(ComponentStatus::Ready, None);
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
        let snapshot = {
            let mut r = self.readiness.write().expect("readiness lock poisoned");
            r.capture = ComponentReadiness { status, detail };
            r.clone()
        };
        let _ = self.events.send(KernelEvent::ReadinessChanged(snapshot));
    }
}

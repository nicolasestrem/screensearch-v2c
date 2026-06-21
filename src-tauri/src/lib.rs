//! ScreenSearch V2c — Tauri 2 desktop shell and **composition root** (`03 §2`).
//!
//! This crate is the only place that wires concrete module impls into the kernel.
//! P2 wired the **capture happy path**: it opens the data spine (P1), spawns the
//! WinRT OCR worker and the WGC capture factory, builds the [`Kernel`], forwards
//! kernel events to the WebView2 UI, and exposes `capture_control` / `get_frame`.
//! P3 loads the fastembed model off the launch thread, attaches it to the kernel
//! (starting the enrichment workers + lighting the vector arm), and adds `search`.
//! The inference sidecar lands in P4.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use tauri::{Emitter, Manager, State};
use traits::{
    CaptureControl, CaptureSource, CapturedFrame, ComponentReadiness, ComponentStatus, FrameDetail,
    JobStats, OcrProvider, OcrResult, Readiness, SearchHit, SearchQuery, Store,
};

use embeddings::FastEmbedProvider;
use kernel::{CaptureFactory, Kernel, KernelEvent};
use store::SqliteStore;

/// App-wide state owned by the composition root and shared with command handlers.
struct AppState {
    /// The data spine (concrete, so commands can use its inherent reads like
    /// `get_frame`). `None` only if the DB failed to open.
    store: Option<Arc<SqliteStore>>,
    /// The orchestrator; `None` when the store failed to open (no spine to drive).
    kernel: Option<Arc<Kernel>>,
    /// Readiness used only when there is no kernel (DB-open failure); otherwise the
    /// live snapshot comes from the kernel.
    fallback_readiness: Readiness,
}

/// Liveness probe for the typed IPC bridge (P0 smoke test, retained).
#[tauri::command]
fn ping() -> String {
    "pong".to_string()
}

/// Current subsystem readiness (`03 §7`). Live `db` + `capture` come from the
/// kernel once it exists; otherwise the DB-error fallback is returned.
#[tauri::command]
fn get_readiness(state: State<'_, AppState>) -> Readiness {
    match &state.kernel {
        Some(kernel) => kernel.readiness(),
        None => state.fallback_readiness.clone(),
    }
}

/// Aggregate job-queue counts (`03 §7`). After P2 capture runs, this shows pending
/// `embed_text` jobs (consumed in P3).
#[tauri::command]
async fn get_job_stats(state: State<'_, AppState>) -> Result<JobStats, String> {
    let store = state
        .store
        .clone()
        .ok_or_else(|| "database unavailable".to_string())?;
    store.job_stats().await.map_err(|e| e.to_string())
}

/// Full per-frame detail for the timeline (`03 §7`): base row + OCR text + vision +
/// tags. `None` if the frame id is unknown.
#[tauri::command]
async fn get_frame(
    frame_id: i64,
    state: State<'_, AppState>,
) -> Result<Option<FrameDetail>, String> {
    let store = state
        .store
        .clone()
        .ok_or_else(|| "database unavailable".to_string())?;
    store.get_frame(frame_id).await.map_err(|e| e.to_string())
}

/// Hybrid search over OCR text + (once embeddings exist) vectors, fused via RRF
/// (`search`, `03 §7/§13.4`). The vector arm is live once the embedder has loaded;
/// before that it degrades to FTS-only.
#[tauri::command]
async fn search(query: SearchQuery, state: State<'_, AppState>) -> Result<Vec<SearchHit>, String> {
    let store = state
        .store
        .clone()
        .ok_or_else(|| "database unavailable".to_string())?;
    store.hybrid_search(&query).await.map_err(|e| e.to_string())
}

/// Start/stop the always-on capture loop (`capture_control`, `03 §7`). Capture is
/// off until the user starts it (privacy-first, `07`).
#[tauri::command]
async fn capture_control(
    control: CaptureControl,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let kernel = state
        .kernel
        .clone()
        .ok_or_else(|| "capture unavailable (database not open)".to_string())?;
    match control {
        CaptureControl::Start => kernel.start_capture().await.map_err(|e| e.to_string()),
        CaptureControl::Stop => {
            kernel.stop_capture().await;
            Ok(())
        }
    }
}

/// Application entry point (called from `main.rs`).
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Resolve the per-user app data dir from the bundle identifier and make
            // sure it (and the log dir) exist before we open anything in them.
            let data_dir = app.path().app_data_dir()?;
            let log_dir = data_dir.join("logs");
            std::fs::create_dir_all(&log_dir)?;
            init_tracing(&log_dir);

            let db_path = data_dir.join("screensearch.db");
            let frames_dir = data_dir.join("frames");

            let (store, db_readiness) = open_store(&db_path);
            let readiness = Readiness {
                db: db_readiness,
                ..Default::default()
            };

            // Build the kernel only if the spine opened. It owns the live readiness
            // (capture starts Disabled) and the event bus.
            let kernel = store.as_ref().map(|store| {
                let dyn_store: Arc<dyn Store> = store.clone();
                let ocr = spawn_ocr();
                Arc::new(Kernel::new(
                    dyn_store,
                    ocr,
                    capture_factory(),
                    frames_dir,
                    readiness.clone(),
                ))
            });

            // Forward kernel events to the UI, and kick off the off-thread embedding
            // model load (P3) — app launch is never blocked on the first-run download.
            if let (Some(kernel), Some(store)) = (&kernel, &store) {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(forward_events(kernel.clone(), handle));

                kernel.set_embed_readiness(
                    ComponentStatus::Initializing,
                    Some("loading embedding model".to_string()),
                );
                let models_dir = data_dir.join("models").join("fastembed");
                let dyn_store: Arc<dyn Store> = store.clone();
                tauri::async_runtime::spawn(init_embeddings(kernel.clone(), dyn_store, models_dir));
            }

            app.manage(AppState {
                store,
                kernel,
                fallback_readiness: readiness,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            get_readiness,
            get_job_stats,
            get_frame,
            search,
            capture_control
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // On exit, stop the worker pool so an in-flight embed finishes cleanly.
            // Best-effort only: a job left `running` is requeued by the startup
            // stale-job sweep on the next launch (`03 §6`).
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let state = app_handle.state::<AppState>();
                if let Some(kernel) = state.kernel.clone() {
                    tauri::async_runtime::block_on(kernel.stop_workers());
                }
            }
        });
}

/// Loads the fastembed model off the launch thread and attaches it to the kernel —
/// which lights up the store's vector arm and starts the worker pool that drains the
/// `embed_text` backlog (`03 §5`). App start never blocks on the multi-hundred-MB
/// first-run download; `embed_model` readiness reflects progress
/// (Initializing → Ready / Unavailable / Disabled). Skips loading entirely when both
/// text and image embeddings are off.
async fn init_embeddings(kernel: Arc<Kernel>, store: Arc<dyn Store>, models_dir: PathBuf) {
    let settings = kernel::settings::load_settings(store.as_ref()).await;
    if !settings.enrich_embed_text && !settings.enrich_image_embeddings {
        kernel.set_embed_readiness(
            ComponentStatus::Disabled,
            Some("embeddings disabled in settings".to_string()),
        );
        return;
    }
    let with_image = settings.enrich_image_embeddings;
    match tokio::task::spawn_blocking(move || FastEmbedProvider::new(models_dir, with_image)).await
    {
        Ok(Ok(provider)) => {
            tracing::info!("embedding model loaded; attaching to kernel");
            kernel.attach_embedder(Arc::new(provider)).await;
        }
        Ok(Err(e)) => {
            tracing::error!(error = %e, "embedding model load failed");
            kernel.set_embed_readiness(ComponentStatus::Unavailable, Some(e.to_string()));
        }
        Err(e) => {
            tracing::error!(error = %e, "embedding model load task panicked");
            kernel.set_embed_readiness(
                ComponentStatus::Unavailable,
                Some("model load task panicked".to_string()),
            );
        }
    }
}

/// Forwards [`KernelEvent`]s onto the Tauri event bus for the WebView2 UI (`03 §7`).
async fn forward_events(kernel: Arc<Kernel>, app: tauri::AppHandle) {
    use tokio::sync::broadcast::error::RecvError;
    let mut rx = kernel.subscribe();
    loop {
        match rx.recv().await {
            Ok(KernelEvent::CaptureTick(tick)) => {
                let _ = app.emit("capture_tick", tick);
            }
            Ok(KernelEvent::ReadinessChanged(readiness)) => {
                let _ = app.emit("readiness_changed", readiness);
            }
            Ok(KernelEvent::JobProgress(stats)) => {
                let _ = app.emit("job_progress", stats);
            }
            Err(RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "event bus lagged; some ticks dropped")
            }
            Err(RecvError::Closed) => break,
        }
    }
}

/// Spawns the WinRT OCR worker, falling back to an empty-text provider if no OCR
/// language is installed (logged; capture still runs, frames just lack text).
fn spawn_ocr() -> Arc<dyn OcrProvider> {
    match ocr::WinRtOcr::spawn() {
        Ok(engine) => {
            tracing::info!("WinRT OCR ready");
            Arc::new(engine)
        }
        Err(e) => {
            tracing::error!(error = %e, "OCR unavailable; captured frames will have no text");
            Arc::new(UnavailableOcr)
        }
    }
}

/// Builds a WGC capture source from the current config — the seam that keeps the
/// kernel impl-agnostic (`03 §2`).
fn capture_factory() -> CaptureFactory {
    Arc::new(|config| Ok(Box::new(capture::WgcCapture::new(config)?) as Box<dyn CaptureSource>))
}

/// Fallback OCR used only when the WinRT engine can't be created. Returns **empty
/// text** rather than an error, so capture still runs and frames are stored without
/// OCR (the capture loop drops a frame on a *real* recognize error — missing OCR is
/// not one). Surfaced via the warning logged in [`spawn_ocr`].
struct UnavailableOcr;

#[async_trait]
impl OcrProvider for UnavailableOcr {
    async fn recognize(&self, _frame: &CapturedFrame) -> traits::Result<OcrResult> {
        Ok(OcrResult {
            text: String::new(),
            mean_confidence: ocr::CONFIDENCE_UNKNOWN,
            engine: "unavailable".to_string(),
        })
    }
}

/// Opens the store and returns it with the corresponding `db` readiness. A DB error
/// at either step (open, or the schema-version probe that confirms the connection is
/// usable) surfaces as `Error` with no store — never a Ready store the UI can't query.
fn open_store(db_path: &Path) -> (Option<Arc<SqliteStore>>, ComponentReadiness) {
    let result = SqliteStore::open_path(db_path).and_then(|s| {
        let version = s.schema_version()?;
        Ok((s, version))
    });
    match result {
        Ok((s, version)) => {
            let detail = format!("schema v{version} ({})", db_path.display());
            tracing::info!(db = %db_path.display(), schema_version = version, "store opened");
            (
                Some(Arc::new(s)),
                ComponentReadiness::with_detail(ComponentStatus::Ready, detail),
            )
        }
        Err(e) => {
            tracing::error!(error = %e, db = %db_path.display(), "store unavailable");
            (
                None,
                ComponentReadiness::with_detail(ComponentStatus::Error, e.to_string()),
            )
        }
    }
}

/// Keeps the non-blocking file appender's worker alive for the process lifetime;
/// dropping the guard would stop file logging.
static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

/// Console + daily-rotating file logging (`03 §9`). Privacy: callers must not log
/// screen content or OCR text at info level.
fn init_tracing(log_dir: &Path) {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let file_appender = tracing_appender::rolling::daily(log_dir, "screensearch.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .with(fmt::layer().with_ansi(false).with_writer(non_blocking))
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wiring proof: opening the store at a real path creates the DB file on disk
    /// and reports `db = Ready` (headless P1 guarantee, still valid in P2).
    #[test]
    fn open_store_creates_db_file_and_reports_ready() {
        let dir = std::env::temp_dir().join(format!("ssv2c-ok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("screensearch.db");
        let _ = std::fs::remove_file(&db);

        let (store, readiness) = open_store(&db);

        assert!(store.is_some(), "store handle present");
        assert_eq!(readiness.status, ComponentStatus::Ready);
        assert!(db.exists(), "db file created at {}", db.display());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A DB that cannot be opened surfaces as `db = Error` instead of crashing.
    #[test]
    fn open_store_reports_error_when_db_cannot_open() {
        let db = std::env::temp_dir()
            .join(format!("ssv2c-missing-{}", std::process::id()))
            .join("nope")
            .join("screensearch.db");

        let (store, readiness) = open_store(&db);

        assert!(store.is_none());
        assert_eq!(readiness.status, ComponentStatus::Error);
    }
}

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
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use tauri::{Emitter, Manager, State};
use traits::{
    AnswerDelta, AnswerOpts, AskRequest, CaptureControl, CaptureSource, CapturedFrame,
    ComponentReadiness, ComponentStatus, FrameDetail, InsightsSummary, JobStats, ModelLane,
    OcrProvider, OcrResult, Readiness, RetrievedChunk, SearchHit, SearchQuery, SetModelTier,
    Settings, Store, TimeRange, TimelineBucket, VisionTarget,
};

use embeddings::FastEmbedProvider;
use inference::{AnswerSidecar, ModelSupervisor, SupervisorConfig, VisionSidecar};
use kernel::{CaptureFactory, IdleSource, Kernel, KernelEvent};
use store::SqliteStore;

/// How long to wait for `llama-server` `/health` after a spawn (model load can be slow
/// on first run / large quants).
const SIDECAR_HEALTH_TIMEOUT: Duration = Duration::from_secs(180);
/// Top-K retrieved chunks used as grounding context for an `ask` (`07` decision).
const ASK_TOP_K: u32 = 8;

/// A slot the composition root fills off the launch thread, shared with command
/// handlers (the sidecar supervisor + the concrete tiered providers).
type SharedSlot<T> = Arc<StdMutex<Option<T>>>;

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
    /// The sidecar supervisor, filled once the binary resolves off-thread (P4). Held
    /// here so app exit can shut the sidecar down cleanly.
    supervisor: SharedSlot<Arc<ModelSupervisor>>,
    /// The concrete tiered providers, for `set_model_tier` (the kernel only holds them
    /// as `dyn` traits, which can't switch tier).
    vision: SharedSlot<Arc<VisionSidecar>>,
    answer: SharedSlot<Arc<AnswerSidecar>>,
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

/// Enqueue deferred vision tagging for a frame or a time range (`enqueue_vision`,
/// `03 §7`). Returns the number of `vision_tag` jobs enqueued. This is the on-demand
/// trigger; results land asynchronously and surface via `job_progress` + `get_frame`.
#[tauri::command]
async fn enqueue_vision(target: VisionTarget, state: State<'_, AppState>) -> Result<u64, String> {
    let kernel = state
        .kernel
        .clone()
        .ok_or_else(|| "kernel unavailable (database not open)".to_string())?;
    kernel
        .enqueue_vision(target)
        .await
        .map_err(|e| e.to_string())
}

/// Ask a grounded question about the screen history (`ask`, `03 §7/§13.5`). Returns
/// immediately; the answer streams back as `answer_delta` events. Retrieves the top-K
/// chunks via hybrid search, grounds the answer in their full OCR text, and runs the
/// answer provider on a background task that forwards each delta to the UI.
#[tauri::command]
async fn ask(
    request: AskRequest,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let store = state
        .store
        .clone()
        .ok_or_else(|| "database unavailable".to_string())?;
    let kernel = state
        .kernel
        .clone()
        .ok_or_else(|| "kernel unavailable".to_string())?;
    let answer = kernel
        .answer_provider()
        .await
        .ok_or_else(|| "inference sidecar not ready yet".to_string())?;

    // Retrieve grounding context: top-K hybrid hits, each with its full OCR text
    // (falling back to the search snippet if the text isn't available).
    let hits = store
        .hybrid_search(&SearchQuery {
            text: request.query.clone(),
            limit: ASK_TOP_K,
            time_range: None,
        })
        .await
        .map_err(|e| e.to_string())?;
    // Hydrate grounding text in a single bulk query (avoid an N+1 over the hits),
    // falling back to each hit's snippet when a frame has no OCR text.
    let frame_ids: Vec<i64> = hits.iter().map(|h| h.frame_id).collect();
    let ocr = store.ocr_texts(&frame_ids).await.unwrap_or_default();
    let context: Vec<RetrievedChunk> = hits
        .into_iter()
        .map(|hit| {
            let text = ocr.get(&hit.frame_id).cloned().unwrap_or(hit.snippet);
            RetrievedChunk {
                frame_id: hit.frame_id,
                text,
                score: hit.score,
                captured_at: hit.captured_at,
            }
        })
        .collect();

    // Stream: the provider sends typed deltas on `tx`; a forwarder emits each as an
    // `answer_delta` event. Both end when the provider finishes (tx drops → rx closes).
    let (tx, mut rx) = tokio::sync::mpsc::channel::<AnswerDelta>(64);
    let emitter = app.clone();
    tokio::spawn(async move {
        while let Some(delta) = rx.recv().await {
            let _ = emitter.emit("answer_delta", delta);
        }
    });
    let opts = AnswerOpts {
        thinking: request.thinking,
        max_tokens: request.max_tokens,
    };
    let query = request.query;
    tokio::spawn(async move {
        let _ = answer.answer(&query, &context, opts, tx).await;
    });
    Ok(())
}

/// Change the active model tier for a lane (`set_model_tier`, `03 §7`). Persists the
/// choice to settings and applies it to the live provider; the sidecar switches to the
/// new GGUF on the lane's next request (`03 §6`).
#[tauri::command]
async fn set_model_tier(request: SetModelTier, state: State<'_, AppState>) -> Result<(), String> {
    let store = state
        .store
        .clone()
        .ok_or_else(|| "database unavailable".to_string())?;
    let value = serde_json::to_string(&request.tier).map_err(|e| e.to_string())?;
    let key = match request.lane {
        ModelLane::Vision => "models.vision_tier",
        ModelLane::Answer => "models.answer_tier",
    };
    store
        .set_setting(key, &value)
        .await
        .map_err(|e| e.to_string())?;

    match request.lane {
        ModelLane::Vision => {
            if let Some(v) = state.vision.lock().expect("vision slot").clone() {
                v.set_tier(request.tier);
            }
        }
        ModelLane::Answer => {
            if let Some(a) = state.answer.lock().expect("answer slot").clone() {
                a.set_tier(request.tier);
            }
        }
    }
    Ok(())
}

/// Frame-count density buckets over `[start, end)` for the Scanline Timeline
/// (`get_timeline`, `03 §7`). `bucket_count` is presentation-driven (the UI passes a
/// count derived from the ribbon width); the store returns sparse, occupied buckets.
#[tauri::command]
async fn get_timeline(
    range: TimeRange,
    bucket_count: u32,
    state: State<'_, AppState>,
) -> Result<Vec<TimelineBucket>, String> {
    let store = state
        .store
        .clone()
        .ok_or_else(|| "database unavailable".to_string())?;
    store
        .timeline_buckets(range.start, range.end, bucket_count.max(1))
        .await
        .map_err(|e| e.to_string())
}

/// Truthful activity aggregates over `[start, end)` for the Insights screen
/// (`get_insights`, P5). Real DB counts only — honest-empty when the window is bare.
#[tauri::command]
async fn get_insights(
    range: TimeRange,
    state: State<'_, AppState>,
) -> Result<InsightsSummary, String> {
    let store = state
        .store
        .clone()
        .ok_or_else(|| "database unavailable".to_string())?;
    store
        .insights_summary(range.start, range.end)
        .await
        .map_err(|e| e.to_string())
}

/// Read the persisted user settings (`get_settings`, `03 §7/§8`). Missing keys fall
/// back to defaults (a fresh DB returns [`Settings::default`]).
#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let store = state
        .store
        .clone()
        .ok_or_else(|| "database unavailable".to_string())?;
    Ok(kernel::settings::load_settings(store.as_ref()).await)
}

/// Persist user settings (`set_settings`, `03 §7/§8`). All keys are written durably.
/// Model tiers **hot-apply** to the live providers here (same path as
/// `set_model_tier`); `answer.thinking` is read per-`ask`; capture/storage/privacy
/// take effect on the next capture start and the rest on app restart (the Settings
/// UI labels each accordingly — no fictional live reconfiguration).
#[tauri::command]
async fn set_settings(settings: Settings, state: State<'_, AppState>) -> Result<(), String> {
    let store = state
        .store
        .clone()
        .ok_or_else(|| "database unavailable".to_string())?;
    kernel::settings::save_settings(store.as_ref(), &settings)
        .await
        .map_err(|e| e.to_string())?;

    // Hot-apply the lane tiers to the live providers (the sidecar switches GGUF on
    // the lane's next request, `03 §6`); everything else is persisted only.
    if let Some(v) = state.vision.lock().expect("vision slot").clone() {
        v.set_tier(settings.models_vision_tier);
    }
    if let Some(a) = state.answer.lock().expect("answer slot").clone() {
        a.set_tier(settings.models_answer_tier);
    }
    Ok(())
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

            // Slots filled off the launch thread (P4 inference), shared with command
            // handlers and the exit path.
            let supervisor_slot: SharedSlot<Arc<ModelSupervisor>> = Arc::new(StdMutex::new(None));
            let vision_slot: SharedSlot<Arc<VisionSidecar>> = Arc::new(StdMutex::new(None));
            let answer_slot: SharedSlot<Arc<AnswerSidecar>> = Arc::new(StdMutex::new(None));

            // Forward kernel events to the UI, and kick off the off-thread model loads
            // (P3 embeddings + P4 inference) — app launch is never blocked on the
            // first-run downloads.
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

                // Resolve the sidecar binary + wire the inference providers off-thread.
                kernel.set_sidecar_readiness(
                    ComponentStatus::Initializing,
                    Some("resolving llama-server".to_string()),
                );
                let dyn_store: Arc<dyn Store> = store.clone();
                tauri::async_runtime::spawn(init_inference(
                    kernel.clone(),
                    dyn_store,
                    data_dir.clone(),
                    supervisor_slot.clone(),
                    vision_slot.clone(),
                    answer_slot.clone(),
                ));
            }

            app.manage(AppState {
                store,
                kernel,
                fallback_readiness: readiness,
                supervisor: supervisor_slot,
                vision: vision_slot,
                answer: answer_slot,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            get_readiness,
            get_job_stats,
            get_frame,
            search,
            capture_control,
            enqueue_vision,
            ask,
            set_model_tier,
            get_timeline,
            get_insights,
            get_settings,
            set_settings
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // On exit, stop the vision scheduler + worker pool so in-flight work
            // finishes cleanly, then shut the sidecar down. Best-effort: a job left
            // `running` is requeued by the startup stale-job sweep, and the Job Object
            // would terminate the sidecar anyway (`03 §6`).
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let state = app_handle.state::<AppState>();
                if let Some(kernel) = state.kernel.clone() {
                    tauri::async_runtime::block_on(async {
                        kernel.stop_vision_scheduler().await;
                        kernel.stop_workers().await;
                    });
                }
                let supervisor = state.supervisor.lock().expect("supervisor slot").clone();
                if let Some(supervisor) = supervisor {
                    tauri::async_runtime::block_on(supervisor.shutdown());
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

/// Resolves the `llama-server` binary off the launch thread, builds the
/// [`ModelSupervisor`] + the tiered vision/answer providers, and attaches them to the
/// kernel (P4, `03 §6`). Lazy by design: the sidecar binary is fetched now, but models
/// download — and the process spawns — only on the first real request. `sidecar`
/// readiness reflects progress (Initializing → Ready / Unavailable), and the
/// supervisor's lifecycle transitions are bridged into the kernel's event bus.
async fn init_inference(
    kernel: Arc<Kernel>,
    store: Arc<dyn Store>,
    data_dir: PathBuf,
    supervisor_slot: SharedSlot<Arc<ModelSupervisor>>,
    vision_slot: SharedSlot<Arc<VisionSidecar>>,
    answer_slot: SharedSlot<Arc<AnswerSidecar>>,
) {
    let sidecar_dir = data_dir.join("sidecar");
    let models_root = data_dir.join("models");
    if let Err(e) = std::fs::create_dir_all(&sidecar_dir) {
        kernel.set_sidecar_readiness(ComponentStatus::Unavailable, Some(e.to_string()));
        return;
    }

    // Fetch the prebuilt Vulkan llama-server (idempotent; skipped if already present).
    let binary = match inference::download::ensure_binary(&sidecar_dir).await {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "llama-server unavailable");
            kernel.set_sidecar_readiness(
                ComponentStatus::Unavailable,
                Some(format!("llama-server unavailable: {e}")),
            );
            return;
        }
    };

    let settings = kernel::settings::load_settings(store.as_ref()).await;
    let config = SupervisorConfig {
        binary,
        pidfile: sidecar_dir.join("llama-server.pid"),
        idle_ttl: Duration::from_secs(settings.sidecar_idle_ttl_secs as u64),
        health_timeout: SIDECAR_HEALTH_TIMEOUT,
    };
    let supervisor = match ModelSupervisor::new(config) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "model supervisor init failed");
            kernel.set_sidecar_readiness(ComponentStatus::Unavailable, Some(e.to_string()));
            return;
        }
    };

    let vision = Arc::new(VisionSidecar::new(
        supervisor.clone(),
        models_root.clone(),
        settings.models_vision_tier,
        settings.sidecar_ngl,
    ));
    let answer = Arc::new(AnswerSidecar::new(
        supervisor.clone(),
        models_root,
        settings.models_answer_tier,
        settings.sidecar_ngl,
    ));
    *vision_slot.lock().expect("vision slot") = Some(vision.clone());
    *answer_slot.lock().expect("answer slot") = Some(answer.clone());
    *supervisor_slot.lock().expect("supervisor slot") = Some(supervisor.clone());

    // Bridge the supervisor's lifecycle transitions into the kernel (updates `sidecar`
    // readiness + re-broadcasts as `sidecar_status`).
    {
        let kernel = kernel.clone();
        let mut rx = supervisor.subscribe();
        tauri::async_runtime::spawn(async move {
            while let Ok(status) = rx.recv().await {
                kernel.emit_sidecar_status(status);
            }
        });
    }

    // Idle source for idle-triggered tagging (the kernel forbids `unsafe`).
    let idle: IdleSource = Arc::new(capture::user_idle_ms);
    kernel
        .attach_inference(
            vision as Arc<dyn traits::VisionProvider>,
            answer as Arc<dyn traits::AnswerProvider>,
            Some(idle),
        )
        .await;
    kernel.set_sidecar_readiness(
        ComponentStatus::Ready,
        Some("ready (model downloads + spawns on first use)".to_string()),
    );
    tracing::info!("inference attached; sidecar ready (lazy spawn)");
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
            Ok(KernelEvent::SidecarStatus(status)) => {
                let _ = app.emit("sidecar_status", status);
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

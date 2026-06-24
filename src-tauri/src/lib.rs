//! ScreenSearch V2c — Tauri 2 desktop shell and **composition root** (`03 §2`).
//!
//! This crate is the only place that wires concrete module impls into the kernel.
//! P2 wired the **capture happy path**: it opens the data spine (P1), spawns the
//! WinRT OCR worker and the WGC capture factory, builds the [`Kernel`], forwards
//! kernel events to the WebView2 UI, and exposes `capture_control` / `get_frame`.
//! P3/P4 load fastembed and the inference sidecar off the launch thread, attaching
//! provider slots that start the enrichment workers and light up search, vision
//! tagging, and grounded answers. P5 adds the full Command Deck command surface.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use tauri::{Emitter, Manager, State};
use traits::{
    AnswerEvent, AnswerOpts, AskRequest, CaptureControl, CaptureSource, CapturedFrame,
    ComponentReadiness, ComponentStatus, FrameDetail, FrameMeta, InsightsSummary, JobStats,
    ModelLane, MonitorInfo, OcrProvider, OcrResult, Readiness, RetrievedChunk, SearchHit,
    SearchQuery, SetModelTier, Settings, StorageStats, Store, TimeRange, TimelineBucket,
    ToastLevel, VisionTarget,
};

use embeddings::FastEmbedProvider;
use inference::{AnswerSidecar, ModelSupervisor, SupervisorConfig, VisionSidecar};
use kernel::{CaptureFactory, IdleSource, Kernel, KernelEvent};
use store::SqliteStore;
use tokio::task::JoinHandle;

/// How long to wait for `llama-server` `/health` after a spawn (model load can be slow
/// on first run / large quants).
const SIDECAR_HEALTH_TIMEOUT: Duration = Duration::from_secs(180);
/// Top-K retrieved chunks used as grounding context for an `ask` (`07` decision).
const ASK_TOP_K: u32 = 8;
const MAX_FRAME_LIST_LIMIT: u32 = 500;
const MAX_FRAME_CONTEXT_LIMIT_EACH: u32 = 50;
const MIN_TIMELINE_BUCKETS: u32 = 120;
const MAX_TIMELINE_BUCKETS: u32 = 2000;
const MIN_INSIGHTS_BUCKETS: u32 = 24;
const MAX_INSIGHTS_BUCKETS: u32 = 720;
const RETENTION_BATCH: u32 = 1000;
const RETENTION_INTERVAL: Duration = Duration::from_secs(60 * 60);
const DAY_MS: i64 = 86_400_000;
static NEXT_ASK_ID: AtomicU64 = AtomicU64::new(1);

fn next_ask_id() -> u64 {
    NEXT_ASK_ID.fetch_add(1, Ordering::Relaxed)
}

fn clamp_timeline_buckets(bucket_count: u32) -> u32 {
    bucket_count.clamp(MIN_TIMELINE_BUCKETS, MAX_TIMELINE_BUCKETS)
}

fn clamp_insights_buckets(bucket_count: u32) -> u32 {
    bucket_count.clamp(MIN_INSIGHTS_BUCKETS, MAX_INSIGHTS_BUCKETS)
}

fn clamp_frame_list_limit(limit: u32) -> u32 {
    limit.min(MAX_FRAME_LIST_LIMIT)
}

fn clamp_frame_context_limit(limit_each: u32) -> u32 {
    limit_each.min(MAX_FRAME_CONTEXT_LIMIT_EACH)
}

/// A slot the composition root fills off the launch thread, shared with command
/// handlers (the sidecar supervisor + the concrete tiered providers).
type SharedSlot<T> = Arc<StdMutex<Option<T>>>;
type AskTasks = Arc<tokio::sync::Mutex<HashMap<String, JoinHandle<()>>>>;

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
    sidecar_binary: SharedSlot<PathBuf>,
    /// In-flight answer providers keyed by client request id; `cancel_ask` aborts them.
    ask_tasks: AskTasks,
    /// Whether the currently attached FastEmbed provider loaded the optional image lane.
    embedder_with_image: Arc<StdMutex<bool>>,
    embed_models_dir: PathBuf,
    db_path: PathBuf,
    frames_dir: PathBuf,
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

/// Storage footprint for the StatusRail (`get_storage_stats`, P5 follow-up).
#[tauri::command]
async fn get_storage_stats(state: State<'_, AppState>) -> Result<StorageStats, String> {
    storage_stats(state.db_path.clone(), state.frames_dir.clone()).await
}

/// Connected monitor metadata for Settings.
#[tauri::command]
fn get_monitors() -> Result<Vec<MonitorInfo>, String> {
    Ok(capture::enumerate_monitors())
}

/// Lists llama.cpp device ids reported by the resolved sidecar binary (`--list-devices`).
#[tauri::command]
async fn list_sidecar_devices(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let binary = state
        .sidecar_binary
        .lock()
        .expect("sidecar binary slot")
        .clone()
        .ok_or_else(|| "llama-server binary is not resolved yet".to_string())?;
    tokio::task::spawn_blocking(move || list_devices_from_binary(&binary))
        .await
        .map_err(|e| format!("device listing task failed: {e}"))?
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
    let request_id = request
        .request_id
        .clone()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| format!("legacy-{}", next_ask_id()));
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

    // Stream: the provider sends typed deltas on `tx`; a forwarder tags each with
    // `request_id` before emitting it as an `answer_delta` event. The provider task
    // is kept in `ask_tasks` so the UI can cancel superseded streams.
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let emitter = app.clone();
    let event_request_id = request_id.clone();
    tokio::spawn(async move {
        while let Some(delta) = rx.recv().await {
            let _ = emitter.emit(
                "answer_delta",
                AnswerEvent {
                    request_id: event_request_id.clone(),
                    delta,
                },
            );
        }
    });
    let opts = AnswerOpts {
        thinking: request.thinking,
        max_tokens: request.max_tokens,
    };
    let query = request.query;
    let mut tasks = state.ask_tasks.lock().await;
    let tasks_clone = state.ask_tasks.clone();
    let task_request_id = request_id.clone();
    let handle = tokio::spawn(async move {
        let _ = answer.answer(&query, &context, opts, tx).await;
        tasks_clone.lock().await.remove(&task_request_id);
    });
    if let Some(old) = tasks.insert(request_id, handle) {
        old.abort();
    }
    Ok(())
}

/// Cancel a streaming answer request if it is still running.
#[tauri::command]
async fn cancel_ask(request_id: String, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(task) = state.ask_tasks.lock().await.remove(&request_id) {
        task.abort();
    }
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

/// Eagerly load a lane's model into the sidecar (the "Load model" control). Spawns or
/// switches the sidecar so the next request on that lane is instant; surfaces any
/// download/spawn error so the panel can show it.
#[tauri::command]
async fn load_model(lane: ModelLane, state: State<'_, AppState>) -> Result<(), String> {
    match lane {
        ModelLane::Vision => {
            let v = state
                .vision
                .lock()
                .expect("vision slot")
                .clone()
                .ok_or_else(|| "inference sidecar not ready yet".to_string())?;
            v.preload().await.map_err(|e| e.to_string())
        }
        ModelLane::Answer => {
            let a = state
                .answer
                .lock()
                .expect("answer slot")
                .clone()
                .ok_or_else(|| "inference sidecar not ready yet".to_string())?;
            a.preload().await.map_err(|e| e.to_string())
        }
    }
}

/// Unload the resident sidecar model now, freeing its VRAM/RAM (the "Unload" control).
/// No-op if nothing is loaded; the next request lazily respawns the model.
#[tauri::command]
async fn unload_model(state: State<'_, AppState>) -> Result<(), String> {
    let supervisor = state
        .supervisor
        .lock()
        .expect("supervisor slot")
        .clone()
        .ok_or_else(|| "inference sidecar not ready yet".to_string())?;
    supervisor.unload().await;
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
        .timeline_buckets(range.start, range.end, clamp_timeline_buckets(bucket_count))
        .await
        .map_err(|e| e.to_string())
}

/// Lightweight frame list over `[start, end)` for browsing (`get_frames`, P5): the
/// Timeline's hover thumbnails, the Deck's "jump back in" recents, and a Moment's
/// neighbour context. Most-recent-first, capped at `limit`. Each row is a
/// [`FrameMeta`]; open one with `get_frame` for the full detail.
#[tauri::command]
async fn get_frames(
    range: TimeRange,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<FrameMeta>, String> {
    let store = state
        .store
        .clone()
        .ok_or_else(|| "database unavailable".to_string())?;
    store
        .frames_in_range(range.start, range.end, clamp_frame_list_limit(limit))
        .await
        .map_err(|e| e.to_string())
}

/// The frame whose capture time is nearest `at` (unix ms), or `null` if the DB has
/// no frames (`get_nearest_frame`, P5). Resolves the Timeline scan-head position to a
/// concrete frame id so "Enter opens the moment under the head" lands on a real frame.
#[tauri::command]
async fn get_nearest_frame(
    at: i64,
    range: Option<TimeRange>,
    state: State<'_, AppState>,
) -> Result<Option<FrameMeta>, String> {
    let store = state
        .store
        .clone()
        .ok_or_else(|| "database unavailable".to_string())?;
    match range {
        Some(range) => store
            .nearest_frame_in_range(at, range.start, range.end)
            .await
            .map_err(|e| e.to_string()),
        None => store.nearest_frame(at).await.map_err(|e| e.to_string()),
    }
}

/// The captures bracketing `at` (unix ms) for a Moment's prev/next + context strip
/// (`get_frame_context`, P5): the `limit_each` closest frames on each side within
/// `±half_window_ms`, ascending by capture time, excluding the anchor. Unlike
/// `get_frames` (newest-first, capped), this guarantees the *adjacent* captures, so
/// prev/next always point at the true neighbours even in a busy session.
#[tauri::command]
async fn get_frame_context(
    at: i64,
    half_window_ms: i64,
    limit_each: u32,
    state: State<'_, AppState>,
) -> Result<Vec<FrameMeta>, String> {
    let store = state
        .store
        .clone()
        .ok_or_else(|| "database unavailable".to_string())?;
    store
        .neighbour_frames(at, half_window_ms, clamp_frame_context_limit(limit_each))
        .await
        .map_err(|e| e.to_string())
}

/// Truthful activity aggregates over `[start, end)` for the Insights screen
/// (`get_insights`, P5). Real DB counts only — honest-empty when the window is bare.
#[tauri::command]
async fn get_insights(
    range: TimeRange,
    bucket_count: u32,
    state: State<'_, AppState>,
) -> Result<InsightsSummary, String> {
    let store = state
        .store
        .clone()
        .ok_or_else(|| "database unavailable".to_string())?;
    store
        .insights_summary(range.start, range.end, clamp_insights_buckets(bucket_count))
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
/// take effect on the next capture start; enrichment workers reconfigure after save
/// and the image embedder reloads when the image lane is newly enabled.
#[tauri::command]
async fn set_settings(settings: Settings, state: State<'_, AppState>) -> Result<(), String> {
    let store = state
        .store
        .clone()
        .ok_or_else(|| "database unavailable".to_string())?;
    // Clamp once up front so the values handed to the live providers below are exactly what
    // gets persisted. `save_settings` sanitizes internally too, but a direct IPC call could
    // pass out-of-range values (e.g. a huge `sidecar_ctx_size`); without this, the DB would
    // store the clamped value while the next sidecar spawn ran the raw one until restart.
    let settings = kernel::settings::sanitize_settings(settings);
    kernel::settings::save_settings(store.as_ref(), &settings)
        .await
        .map_err(|e| e.to_string())?;

    // Hot-apply sidecar lane settings; embedding workers are handled below. The launch
    // params (ngl/device/ctx/KV/flash) are all launch args, so they take effect when the
    // supervisor relaunches on the next request (a changed param makes `needs_restart`
    // true) — no immediate process kill here.
    let sidecar_params = traits::SidecarParams::from(&settings);
    if let Some(v) = state.vision.lock().expect("vision slot").clone() {
        v.set_tier(settings.models_vision_tier);
        v.set_launch_options(sidecar_params.clone());
    }
    if let Some(a) = state.answer.lock().expect("answer slot").clone() {
        a.set_tier(settings.models_answer_tier);
        a.set_launch_options(sidecar_params);
    }
    if let Some(kernel) = state.kernel.clone() {
        let embeddings_enabled = settings.enrich_embed_text || settings.enrich_image_embeddings;
        let embed_status = kernel.readiness().embed_model.status;
        let needs_image_embedder = settings.enrich_image_embeddings
            && !*state
                .embedder_with_image
                .lock()
                .expect("embedder image flag");
        if embeddings_enabled
            && matches!(
                embed_status,
                ComponentStatus::Disabled | ComponentStatus::Unavailable | ComponentStatus::Unknown
            )
        {
            kernel.set_embed_readiness(
                ComponentStatus::Initializing,
                Some("loading embedding model".to_string()),
            );
            let dyn_store: Arc<dyn Store> = store.clone();
            tauri::async_runtime::spawn(init_embeddings(
                kernel,
                dyn_store,
                state.embed_models_dir.clone(),
                state.embedder_with_image.clone(),
            ));
        } else if needs_image_embedder {
            kernel.set_embed_readiness(
                ComponentStatus::Initializing,
                Some("loading image embedding model".to_string()),
            );
            let dyn_store: Arc<dyn Store> = store.clone();
            tauri::async_runtime::spawn(init_embeddings(
                kernel,
                dyn_store,
                state.embed_models_dir.clone(),
                state.embedder_with_image.clone(),
            ));
        } else {
            kernel.reconfigure_enrichment().await;
        }
    }
    Ok(())
}

/// Application entry point (called from `main.rs`).
pub fn run() {
    tauri::Builder::default()
        // MUST be the first plugin. Users double-click or relaunch when nothing seems to be
        // happening; without this a second instance runs against the *same* app-data dir and
        // races the first — corrupting the SQLite DB and colliding on hf-hub's per-model cache
        // lock (the ~5 s `LockAcquisition` download failure + retry storm we diagnosed). Instead,
        // focus the existing window and let the second process exit.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            // Resolve the per-user app data dir from the bundle identifier and make
            // sure it (and the log dir) exist before we open anything in them.
            let data_dir = app.path().app_data_dir()?;
            let log_dir = data_dir.join("logs");
            std::fs::create_dir_all(&log_dir)?;
            init_tracing(&log_dir);

            let db_path = data_dir.join("screensearch.db");
            let frames_dir = data_dir.join("frames");
            let embed_models_dir = data_dir.join("models").join("fastembed");

            let (store, db_readiness) = open_store(&db_path);
            let readiness = Readiness {
                db: db_readiness,
                ..Default::default()
            };

            // Build the kernel only if the spine opened. It owns the live readiness
            // (capture starts Disabled) and the event bus.
            let kernel = store.as_ref().map(|store| {
                let dyn_store: Arc<dyn Store> = store.clone();
                let (ocr, ocr_unavailable) = spawn_ocr();
                match ocr_unavailable {
                    Some(reason) => Arc::new(Kernel::new_with_ocr_unavailable(
                        dyn_store,
                        ocr,
                        capture_factory(),
                        frames_dir.clone(),
                        readiness.clone(),
                        reason,
                    )),
                    None => Arc::new(Kernel::new(
                        dyn_store,
                        ocr,
                        capture_factory(),
                        frames_dir.clone(),
                        readiness.clone(),
                    )),
                }
            });

            // Slots filled off the launch thread (P4 inference), shared with command
            // handlers and the exit path.
            let supervisor_slot: SharedSlot<Arc<ModelSupervisor>> = Arc::new(StdMutex::new(None));
            let vision_slot: SharedSlot<Arc<VisionSidecar>> = Arc::new(StdMutex::new(None));
            let answer_slot: SharedSlot<Arc<AnswerSidecar>> = Arc::new(StdMutex::new(None));
            let sidecar_binary_slot: SharedSlot<PathBuf> = Arc::new(StdMutex::new(None));
            let embedder_with_image = Arc::new(StdMutex::new(false));

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
                let dyn_store: Arc<dyn Store> = store.clone();
                tauri::async_runtime::spawn(init_embeddings(
                    kernel.clone(),
                    dyn_store,
                    embed_models_dir.clone(),
                    embedder_with_image.clone(),
                ));

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
                    sidecar_binary_slot.clone(),
                ));
                tauri::async_runtime::spawn(retention_sweeper(
                    store.clone(),
                    kernel.clone(),
                    data_dir.clone(),
                ));
            }

            app.manage(AppState {
                store,
                kernel,
                fallback_readiness: readiness,
                supervisor: supervisor_slot,
                vision: vision_slot,
                answer: answer_slot,
                sidecar_binary: sidecar_binary_slot,
                ask_tasks: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                embedder_with_image,
                embed_models_dir,
                db_path,
                frames_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            get_readiness,
            get_job_stats,
            get_storage_stats,
            get_monitors,
            list_sidecar_devices,
            get_frame,
            search,
            capture_control,
            enqueue_vision,
            ask,
            cancel_ask,
            set_model_tier,
            load_model,
            unload_model,
            get_timeline,
            get_frames,
            get_nearest_frame,
            get_frame_context,
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
async fn init_embeddings(
    kernel: Arc<Kernel>,
    store: Arc<dyn Store>,
    models_dir: PathBuf,
    embedder_with_image: Arc<StdMutex<bool>>,
) {
    let settings = kernel::settings::load_settings(store.as_ref()).await;
    if !settings.enrich_embed_text && !settings.enrich_image_embeddings {
        *embedder_with_image.lock().expect("embedder image flag") = false;
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
            *embedder_with_image.lock().expect("embedder image flag") = with_image;
        }
        Ok(Err(e)) => {
            *embedder_with_image.lock().expect("embedder image flag") = false;
            tracing::error!(error = %e, "embedding model load failed");
            kernel.set_embed_readiness(ComponentStatus::Unavailable, Some(e.to_string()));
        }
        Err(e) => {
            *embedder_with_image.lock().expect("embedder image flag") = false;
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
    sidecar_binary_slot: SharedSlot<PathBuf>,
) {
    let sidecar_dir = data_dir.join("sidecar");
    let models_root = data_dir.join("models");
    if let Err(e) = std::fs::create_dir_all(&sidecar_dir) {
        kernel.set_sidecar_readiness(ComponentStatus::Unavailable, Some(e.to_string()));
        return;
    }
    let pidfile = sidecar_dir.join("llama-server.pid");
    let mut reap_binaries = inference::download::installed_binary_candidates(&sidecar_dir);
    if inference::supervisor::reap_stray_any(&pidfile, &reap_binaries) {
        tracing::warn!("startup reap terminated a stray sidecar before binary resolution");
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
    *sidecar_binary_slot.lock().expect("sidecar binary slot") = Some(binary.clone());
    reap_binaries.push(binary.clone());
    // Probe the bundled binary once so `build_args` only emits memory-tuning flags it
    // actually accepts. The probe spawns `llama-server --help` (a blocking syscall), so it
    // runs on a blocking pool rather than parking this async worker; on failure we fall
    // back to the conservative caps (context-only) the probe itself would return.
    // Probed once per process: the binary download path is idempotent (skip-if-present),
    // so a running app keeps the same binary — and thus the same caps — until restart.
    let binary_for_probe = binary.clone();
    let caps = tokio::task::spawn_blocking(move || inference::probe_caps(&binary_for_probe))
        .await
        .unwrap_or_else(|_| inference::SidecarCaps::conservative());
    let config = SupervisorConfig {
        binary,
        reap_binaries,
        pidfile,
        idle_ttl: Duration::from_secs(settings.sidecar_idle_ttl_secs as u64),
        health_timeout: SIDECAR_HEALTH_TIMEOUT,
        caps,
    };
    let supervisor = match ModelSupervisor::new(config) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "model supervisor init failed");
            kernel.set_sidecar_readiness(ComponentStatus::Unavailable, Some(e.to_string()));
            return;
        }
    };

    let sidecar_params = traits::SidecarParams::from(&settings);
    // One downloader, shared by both lanes: it serializes per-lane fetches (no concurrent
    // races) and broadcasts progress for the UI.
    let downloader = inference::ModelDownloader::new(models_root.clone());
    let vision = Arc::new(VisionSidecar::new(
        supervisor.clone(),
        downloader.clone(),
        models_root.clone(),
        settings.models_vision_tier,
        sidecar_params.clone(),
    ));
    let answer = Arc::new(AnswerSidecar::new(
        supervisor.clone(),
        downloader.clone(),
        models_root,
        settings.models_answer_tier,
        sidecar_params,
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
            // A `Lagged` (receiver fell behind a burst) must NOT end the bridge — that
            // would silently freeze the StatusRail for the rest of the session. Skip the
            // missed events and keep going; only stop when the sender is gone (`Closed`).
            loop {
                match rx.recv().await {
                    Ok(status) => kernel.emit_sidecar_status(status),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    // Bridge model-download progress into the kernel (re-broadcast as `model_download`).
    {
        let kernel = kernel.clone();
        let mut rx = downloader.subscribe();
        tauri::async_runtime::spawn(async move {
            // Same as above: a lagging receiver must keep bridging progress, not die and
            // leave the download chip frozen for the rest of the run.
            loop {
                match rx.recv().await {
                    Ok(status) => kernel.emit_model_download(status),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    // Idle source for idle-triggered tagging (the kernel forbids `unsafe`).
    let idle: IdleSource = Arc::new(capture::user_idle_ms);
    // The supervisor is the keep-warm authority: while idle backfill drains the backlog it
    // suppresses idle-TTL eviction so the model stays loaded for the whole drain (`03 §5`).
    let backfill = supervisor.clone() as Arc<dyn traits::BackfillControl>;
    kernel
        .attach_inference(
            vision as Arc<dyn traits::VisionProvider>,
            answer as Arc<dyn traits::AnswerProvider>,
            Some(idle),
            Some(backfill),
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
            Ok(KernelEvent::JobCompleted(completed)) => {
                let _ = app.emit("job_completed", completed);
            }
            Ok(KernelEvent::SidecarStatus(status)) => {
                let _ = app.emit("sidecar_status", status);
            }
            Ok(KernelEvent::ModelDownload(status)) => {
                let _ = app.emit("model_download", status);
            }
            Ok(KernelEvent::Toast(toast)) => {
                let _ = app.emit("toast", toast);
            }
            Err(RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "event bus lagged; some ticks dropped")
            }
            Err(RecvError::Closed) => break,
        }
    }
}

async fn storage_stats(db_path: PathBuf, frames_dir: PathBuf) -> Result<StorageStats, String> {
    tokio::task::spawn_blocking(move || {
        let db_bytes = db_file_family_size(&db_path);
        let frame_bytes = dir_size(&frames_dir).map_err(|e| e.to_string())?;
        Ok(StorageStats {
            db_bytes,
            frame_bytes,
            total_bytes: db_bytes.saturating_add(frame_bytes),
        })
    })
    .await
    .map_err(|e| format!("storage stats task failed: {e}"))?
}

fn db_file_family_size(db_path: &Path) -> u64 {
    let mut total = file_size(db_path);
    for suffix in ["-wal", "-shm"] {
        let mut os = db_path.as_os_str().to_os_string();
        os.push(suffix);
        total = total.saturating_add(file_size(Path::new(&os)));
    }
    total
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn dir_size(dir: &Path) -> std::io::Result<u64> {
    let mut total = 0_u64;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(0);
    };
    for entry in entries {
        let entry = entry?;
        let meta = entry.metadata()?;
        if meta.is_dir() {
            total = total.saturating_add(dir_size(&entry.path())?);
        } else if meta.is_file() {
            total = total.saturating_add(meta.len());
        }
    }
    Ok(total)
}

fn list_devices_from_binary(binary: &Path) -> Result<Vec<String>, String> {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW: enumerating GPU devices for the Settings panel runs
    // `llama-server --list-devices`, a console exe; without this flag it flashes a terminal
    // window every time Settings opens. Mirrors `inference::flags`/`inference::process`.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new(binary)
        .arg("--list-devices")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("run {} --list-devices: {e}", binary.display()))?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() {
        return Err(combined.trim().to_string());
    }
    Ok(parse_sidecar_device_ids(&combined))
}

fn parse_sidecar_device_ids(output: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim().trim_start_matches(['-', '*', ' ']).trim();
        let token = trimmed
            .split([':', ' ', '\t'])
            .next()
            .unwrap_or_default()
            .trim();
        // llama.cpp device ids are at least four characters (`CUDA0`,
        // `Vulkan0`, `Metal`, etc.); shorter tokens are usually log noise.
        if token.len() >= 4
            && token.chars().any(|c| c.is_ascii_digit())
            && token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            && !out.iter().any(|d| d == token)
        {
            out.push(token.to_string());
        }
    }
    out
}

async fn retention_sweeper(store: Arc<SqliteStore>, kernel: Arc<Kernel>, data_dir: PathBuf) {
    loop {
        match run_retention_once(store.clone(), kernel.clone(), &data_dir).await {
            Ok(n) if n > 0 => tracing::info!(purged = n, "retention sweep purged captures"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "retention sweep failed"),
        }
        tokio::time::sleep(RETENTION_INTERVAL).await;
    }
}

async fn run_retention_once(
    store: Arc<SqliteStore>,
    kernel: Arc<Kernel>,
    data_dir: &Path,
) -> Result<u64, String> {
    let settings = kernel::settings::load_settings(store.as_ref()).await;
    if settings.storage_retention_days == 0 {
        return Ok(0);
    }
    let retention_ms = i64::from(settings.storage_retention_days).saturating_mul(DAY_MS);
    let cutoff = now_ms().saturating_sub(retention_ms);
    let candidates = store
        .frames_older_than(cutoff, RETENTION_BATCH)
        .await
        .map_err(|e| e.to_string())?;
    let mut purged = 0_u64;
    for frame in candidates {
        let image_path = safe_frame_path(data_dir, &frame.image_path);
        if let Some(path) = image_path {
            if let Err(e) = std::fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(path = %path.display(), error = %e, "retention could not delete frame file");
                    continue;
                }
            }
        }
        if let Err(e) = store.delete_frame(frame.frame_id).await {
            tracing::error!(frame_id = frame.frame_id, error = %e, "retention could not delete frame from database");
            continue;
        }
        purged += 1;
    }
    if purged > 0 {
        kernel.emit_toast(
            ToastLevel::Info,
            format!("Retention purged {purged} old captures"),
        );
    }
    Ok(purged)
}

fn safe_frame_path(data_dir: &Path, image_path: &str) -> Option<PathBuf> {
    use std::path::Component;
    let rel = Path::new(image_path);
    if rel.is_absolute() {
        return None;
    }
    let mut components = rel.components();
    match components.next()? {
        Component::Normal(first) if first == "frames" => {}
        _ => return None,
    }
    let mut safe = PathBuf::from("frames");
    for component in components {
        match component {
            Component::Normal(part) => safe.push(part),
            _ => return None,
        }
    }
    Some(data_dir.join(safe))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Spawns the WinRT OCR worker and records an unavailable reason if OCR cannot
/// be initialized. Capture start is blocked in that state.
fn spawn_ocr() -> (Arc<dyn OcrProvider>, Option<String>) {
    match ocr::WinRtOcr::spawn() {
        Ok(engine) => {
            tracing::info!("WinRT OCR ready");
            (Arc::new(engine), None)
        }
        Err(e) => {
            let reason = e.to_string();
            tracing::error!(error = %reason, "OCR unavailable; capture cannot start");
            (
                Arc::new(UnavailableOcr {
                    reason: reason.clone(),
                }),
                Some(reason),
            )
        }
    }
}

/// Builds a WGC capture source from the current config — the seam that keeps the
/// kernel impl-agnostic (`03 §2`).
fn capture_factory() -> CaptureFactory {
    Arc::new(|config| Ok(Box::new(capture::WgcCapture::new(config)?) as Box<dyn CaptureSource>))
}

/// Defensive fallback used only when the WinRT engine can't be created. The kernel
/// refuses to start capture before this provider can be called; if that invariant is
/// broken, returning an error still prevents silently writing empty OCR rows.
struct UnavailableOcr {
    reason: String,
}

#[async_trait]
impl OcrProvider for UnavailableOcr {
    async fn recognize(&self, _frame: &CapturedFrame) -> traits::Result<OcrResult> {
        Err(std::io::Error::other(format!("OCR unavailable: {}", self.reason)).into())
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

    #[test]
    fn ipc_presentation_limits_are_clamped() {
        assert_eq!(clamp_frame_list_limit(u32::MAX), MAX_FRAME_LIST_LIMIT);
        assert_eq!(
            clamp_frame_context_limit(u32::MAX),
            MAX_FRAME_CONTEXT_LIMIT_EACH
        );
        assert_eq!(clamp_timeline_buckets(1), MIN_TIMELINE_BUCKETS);
        assert_eq!(clamp_timeline_buckets(u32::MAX), MAX_TIMELINE_BUCKETS);
        assert_eq!(clamp_insights_buckets(1), MIN_INSIGHTS_BUCKETS);
        assert_eq!(clamp_insights_buckets(u32::MAX), MAX_INSIGHTS_BUCKETS);
    }

    #[test]
    fn safe_frame_path_accepts_only_relative_frames_children() {
        let data = PathBuf::from(r"C:\appdata");

        assert_eq!(
            safe_frame_path(&data, "frames/day-1/100.jpg"),
            Some(data.join("frames").join("day-1").join("100.jpg"))
        );
        assert!(safe_frame_path(&data, r"C:\outside\100.jpg").is_none());
        assert!(safe_frame_path(&data, "../frames/100.jpg").is_none());
        assert!(safe_frame_path(&data, "screens/100.jpg").is_none());
        assert!(safe_frame_path(&data, "frames/../100.jpg").is_none());
    }

    #[test]
    fn parses_llama_cpp_device_ids() {
        let output = "
Available devices:
  Vulkan0: AMD Radeon
  Vulkan1: NVIDIA RTX
  CUDA0: NVIDIA RTX
";
        assert_eq!(
            parse_sidecar_device_ids(output),
            vec!["Vulkan0", "Vulkan1", "CUDA0"]
        );
    }

    #[test]
    fn db_file_family_size_includes_wal_and_shm() {
        let dir = std::env::temp_dir().join(format!("ssv2c-size-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("screensearch.db");
        std::fs::write(&db, [0_u8; 3]).unwrap();
        std::fs::write(dir.join("screensearch.db-wal"), [0_u8; 5]).unwrap();
        std::fs::write(dir.join("screensearch.db-shm"), [0_u8; 7]).unwrap();

        assert_eq!(db_file_family_size(&db), 15);

        let _ = std::fs::remove_dir_all(&dir);
    }
}

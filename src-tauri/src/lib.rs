//! ScreenSearch V2c — Tauri 2 desktop shell and **composition root** (`03 §2`).
//!
//! This crate is the only place that wires concrete module impls into the kernel.
//! P1 wires the **data spine**: on launch it opens the SQLite store at the app
//! data dir, exposes it through typed IPC, and reports DB readiness. Capture (P2),
//! embeddings (P3), and the sidecar (P4) are registered in later phases.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use tauri::{Manager, State};
use traits::{ComponentReadiness, ComponentStatus, JobStats, Readiness, Store};

/// App-wide state owned by the composition root and shared with command handlers.
struct AppState {
    /// The data spine. `None` only if the DB failed to open (readiness reflects it).
    store: Option<Arc<dyn Store>>,
    /// Subsystem readiness snapshot (`03 §7`). Immutable in P1; later phases that
    /// flip capture/embed/sidecar will move this behind interior mutability.
    readiness: Readiness,
}

/// Liveness probe for the typed IPC bridge (P0 smoke test, retained).
#[tauri::command]
fn ping() -> String {
    "pong".to_string()
}

/// Current subsystem readiness (`03 §7`). In P1 `db` is real (Ready/Error from the
/// store open); capture/embed_model/sidecar stay `Unknown` until their phases.
#[tauri::command]
fn get_readiness(state: State<'_, AppState>) -> Readiness {
    state.readiness.clone()
}

/// Aggregate job-queue counts (`03 §7`). Proves the store is live over IPC; the
/// queue is empty until producers (P2+) enqueue work.
#[tauri::command]
async fn get_job_stats(state: State<'_, AppState>) -> Result<JobStats, String> {
    let store = state
        .store
        .clone()
        .ok_or_else(|| "database unavailable".to_string())?;
    store.job_stats().await.map_err(|e| e.to_string())
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
            let state = open_state(&db_path);
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![ping, get_readiness, get_job_stats])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Opens the store and builds the initial [`AppState`] + readiness. A DB open
/// failure is surfaced as `db = Error` rather than crashing the shell.
fn open_state(db_path: &Path) -> AppState {
    // A DB error at either step (open, or the schema-version probe that confirms
    // the connection is usable) surfaces as `db = Error` with no store — never a
    // Ready store the UI can't actually query.
    let result = store::SqliteStore::open_path(db_path).and_then(|s| {
        let version = s.schema_version()?;
        Ok((s, version))
    });
    match result {
        Ok((s, version)) => {
            let detail = format!("schema v{version} ({})", db_path.display());
            tracing::info!(db = %db_path.display(), schema_version = version, "store opened");
            AppState {
                store: Some(Arc::new(s)),
                readiness: Readiness {
                    db: ComponentReadiness::with_detail(ComponentStatus::Ready, detail),
                    ..Default::default()
                },
            }
        }
        Err(e) => {
            tracing::error!(error = %e, db = %db_path.display(), "store unavailable");
            AppState {
                store: None,
                readiness: Readiness {
                    db: ComponentReadiness::with_detail(ComponentStatus::Error, e.to_string()),
                    ..Default::default()
                },
            }
        }
    }
}

/// Keeps the non-blocking file appender's worker alive for the process lifetime;
/// dropping the guard would stop file logging.
static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

/// Console + daily-rotating file logging (`03 §9`). Now that P1 resolves the app
/// data dir, the file sink deferred in P0 is wired here (`07`). Privacy: callers
/// must not log screen content or OCR text at info level.
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
    /// and reports `db = Ready` (the P1 "observably running" guarantee, headless).
    #[test]
    fn open_state_creates_db_file_and_reports_ready() {
        let dir = std::env::temp_dir().join(format!("ssv2c-ok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("screensearch.db");
        let _ = std::fs::remove_file(&db);

        let state = open_state(&db);

        assert!(state.store.is_some(), "store handle present");
        assert_eq!(state.readiness.db.status, ComponentStatus::Ready);
        assert!(db.exists(), "db file created at {}", db.display());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A DB that cannot be opened surfaces as `db = Error` instead of crashing.
    #[test]
    fn open_state_reports_error_when_db_cannot_open() {
        // parent directories intentionally absent → sqlite cannot create the file
        let db = std::env::temp_dir()
            .join(format!("ssv2c-missing-{}", std::process::id()))
            .join("nope")
            .join("screensearch.db");

        let state = open_state(&db);

        assert!(state.store.is_none());
        assert_eq!(state.readiness.db.status, ComponentStatus::Error);
    }
}

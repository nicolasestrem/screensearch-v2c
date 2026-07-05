//! Auto-update manager (0.3.2 PR2, #69; `03 §11b`, `docs/0.3.2.md` D1/D2).
//!
//! Wraps `tauri-plugin-updater` behind three typed commands and one event so the whole
//! feature is Rust-driven and the UI consumes only `ts-rs` types (never the plugin's JS
//! surface — no `updater:*` capability, D1). The public key + GitHub-Releases endpoint
//! live in `tauri.conf.json` (`plugins.updater`); the plugin verifies the minisign
//! signature of both the manifest and the downloaded artifact against that key, so a
//! tampered / unsigned / wrong-key update is rejected before anything installs (`03 §11b`).
//!
//! **UX (pull-based, quiet — D1):** a launch check (release builds only) and a manual
//! "Check for updates" both funnel through [`run_check`]. When a newer signed release is
//! found it is **downloaded in the background** and held in memory; it installs **only**
//! when the user invokes [`restart_to_apply_update`]. No modal, no nag, no auto-restart.
//! `Idle` renders zero UI presence.

use std::sync::atomic::Ordering;
use std::sync::Mutex as StdMutex;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::Mutex as TokioMutex;
use traits::UpdateStatus;

/// Event name (core → UI). Payload is [`UpdateStatus`]; the UI mirrors it into its
/// `updateStatus` query cache. Broadcast on every state transition.
const UPDATE_STATUS_CHANGED: &str = "update_status_changed";

/// A verified, downloaded update held for install-on-restart. The `Update` handle carries
/// the release metadata + signature; `bytes` is the signature-verified installer payload.
/// Held in RAM until the user restarts (the NSIS installer is ~13 MB — acceptable; a
/// temp-file spill is the escape hatch if artifact size ever grows materially).
struct PendingUpdate {
    update: Update,
    bytes: Vec<u8>,
}

/// App-wide updater state, managed by the composition root (its own `app.manage`, like
/// `OverlayState`; not an `AppState` field, so PR3's tray can reach it the same way via
/// `app.state::<UpdaterState>()`).
#[derive(Default)]
pub struct UpdaterState {
    /// Snapshot returned by `get_update_status`; written only through [`set_status`],
    /// which also emits [`UPDATE_STATUS_CHANGED`].
    status: StdMutex<UpdateStatus>,
    /// The downloaded, verified update awaiting a user-initiated restart.
    pending: TokioMutex<Option<PendingUpdate>>,
    /// Single-flight guard: a manual check racing the launch check (or a double-click)
    /// never starts a second check/download — the loser returns the live snapshot.
    in_flight: std::sync::atomic::AtomicBool,
}

/// Read the current snapshot (cheap; no I/O).
fn current_status(app: &AppHandle) -> UpdateStatus {
    app.state::<UpdaterState>()
        .status
        .lock()
        .expect("update status lock")
        .clone()
}

/// Store `status` and broadcast it to the UI. The mutex guard never crosses an `.await`.
fn set_status(app: &AppHandle, status: UpdateStatus) {
    {
        let state = app.state::<UpdaterState>();
        *state.status.lock().expect("update status lock") = status.clone();
    }
    let _ = app.emit(UPDATE_STATUS_CHANGED, status);
}

/// Launch-time check, spawned from `setup` on release builds only (dev builds skip it so
/// `npm run tauri dev` never hits the live endpoint on every start).
pub async fn launch_check(app: AppHandle) {
    run_check(&app).await;
}

/// The single entry point for both the launch check and the manual command. Runs the
/// check and, if an update is found, the background download — behind a single-flight
/// guard so the two paths can never double-download.
async fn run_check(app: &AppHandle) {
    // Claim the single-flight slot; if a check/download is already running, do nothing
    // (the UI already reflects Checking / Downloading).
    let claimed = app
        .state::<UpdaterState>()
        .in_flight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok();
    if !claimed {
        return;
    }

    let outcome = check_and_download(app).await;
    if let Err(message) = outcome {
        tracing::warn!(error = %message, "update check/download failed");
        set_status(app, UpdateStatus::Error { message });
    }

    // Always release the slot (the body returns Result, so this line is reached on every
    // non-panic path; a panicked spawned task is the only leak, and would strand the
    // guard until restart — acceptable, and never observed with the Result-only body).
    app.state::<UpdaterState>()
        .in_flight
        .store(false, Ordering::Release);
}

/// Check the endpoint, and on a newer signed release download + verify it in the
/// background, leaving a [`PendingUpdate`] ready for [`restart_to_apply_update`].
async fn check_and_download(app: &AppHandle) -> Result<(), String> {
    set_status(app, UpdateStatus::Checking);

    let updater = app.updater().map_err(|e| e.to_string())?;
    let found = updater.check().await.map_err(|e| e.to_string())?;

    let Some(update) = found else {
        // No newer release → zero UI presence, and clear any stale pending download.
        *app.state::<UpdaterState>().pending.lock().await = None;
        set_status(app, UpdateStatus::Idle);
        return Ok(());
    };

    let version = update.version.clone();
    tracing::info!(version = %version, "update available; starting background download");
    set_status(
        app,
        UpdateStatus::Available {
            version: version.clone(),
        },
    );
    set_status(
        app,
        UpdateStatus::Downloading {
            version: version.clone(),
        },
    );

    // The plugin verifies the minisign signature of the downloaded artifact against the
    // baked pubkey here; a tampered / unsigned / wrong-key payload fails this call, which
    // surfaces as `Error` (the negative-test path, `03 §11b`).
    let bytes = update
        .download(|_chunk, _total| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!(version = %version, bytes = bytes.len(), "update downloaded + signature-verified");

    *app.state::<UpdaterState>().pending.lock().await = Some(PendingUpdate { update, bytes });
    set_status(app, UpdateStatus::Ready { version });
    Ok(())
}

/// Current updater snapshot (`get_update_status`). Cheap synchronous read.
#[tauri::command]
pub fn get_update_status(app: AppHandle) -> UpdateStatus {
    current_status(&app)
}

/// Manual "Check for updates" (Settings App section + the NavRail footer control). Runs
/// the check (and background download if newer), then returns the post-check snapshot so
/// the caller sees the immediate result; the download, if any, continues in the
/// background and finishes via the `update_status_changed` event.
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<UpdateStatus, String> {
    run_check(&app).await;
    Ok(current_status(&app))
}

/// Install the downloaded update and restart — the **only** install trigger (D1). Errors
/// if nothing is `Ready`. Runs the same graceful shutdown as a normal quit before handing
/// off to the installer, so capture is stopped and the sidecar is torn down cleanly
/// (`03 §6` — no orphaned `llama-server`).
#[tauri::command]
pub async fn restart_to_apply_update(app: AppHandle) -> Result<(), String> {
    let pending = app.state::<UpdaterState>().pending.lock().await.take();
    let Some(PendingUpdate { update, bytes }) = pending else {
        return Err("no update is ready to install".to_string());
    };

    tracing::info!(version = %update.version, "installing update on user-initiated restart");
    crate::graceful_shutdown(&app).await;

    if let Err(e) = update.install(bytes) {
        // Rare: the app is left with subsystems stopped (the pre-quit state); the panel
        // shows the error and the user can close/reopen. Abrupt exit stays safe anyway
        // (Job Object KILL_ON_JOB_CLOSE, SQLite WAL).
        let message = e.to_string();
        tracing::error!(error = %message, "update install failed");
        set_status(
            &app,
            UpdateStatus::Error {
                message: message.clone(),
            },
        );
        return Err(message);
    }

    // On Windows the passive NSIS installer takes over and terminates this process; the
    // restart is a fallback for the rare case `install` returns without exiting.
    app.restart();
}

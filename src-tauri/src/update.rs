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
//! "Check for updates" both funnel through [`run_check`], which does the check and, when a
//! newer signed release is found, **downloads it in a background task** so neither path
//! blocks on the transfer. The verified bytes are held in memory and install **only** when
//! the user invokes [`restart_to_apply_update`]. No modal, no nag, no auto-restart. `Idle`
//! renders zero UI presence.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex as StdMutex;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
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
    /// which also emits [`UPDATE_STATUS_CHANGED`]. Never locked across an `.await`.
    status: StdMutex<UpdateStatus>,
    /// The downloaded, verified update awaiting a user-initiated restart. Never locked
    /// across an `.await` (lock → assign/take → unlock, all synchronous).
    pending: StdMutex<Option<PendingUpdate>>,
    /// Single-flight guard: a manual check racing the launch check (or a double-click)
    /// never starts a second check/download — the loser returns the live snapshot. Held
    /// (via [`InFlightGuard`]) until the background download finishes, so a check that
    /// lands mid-download is a no-op too.
    in_flight: AtomicBool,
}

/// RAII release of the single-flight slot. Dropping it clears `in_flight`, whatever the
/// exit path (early return, `?`, a panic, or the background download task finishing). It
/// holds an `AppHandle` clone so it can reach the managed `UpdaterState` after `run_check`
/// has returned — the download runs in a spawned task that owns the guard.
struct InFlightGuard(AppHandle);

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0
            .state::<UpdaterState>()
            .in_flight
            .store(false, Ordering::Release);
    }
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
/// `npm run dev` never hits the live endpoint on every start).
pub async fn launch_check(app: AppHandle) {
    run_check(&app).await;
}

/// The single entry point for both the launch check and the manual command. Runs the check
/// synchronously; if a newer release is found, the **download runs in a background task**
/// (holding the single-flight guard until it completes) so neither path blocks on the
/// transfer. A second check while one is in progress — including during a background
/// download — is a no-op (the guard is still held). `pub(crate)` so the tray's "Check for
/// updates" menu item can drive the same single-flight check (0.3.2 PR3).
pub(crate) async fn run_check(app: &AppHandle) {
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
    let guard = InFlightGuard(app.clone());

    match check(app).await {
        Ok(Some(update)) => {
            // A newer signed release. Announce `Available`, then download + verify in the
            // background: the guard moves into the task and releases the slot only when the
            // download finishes, so a concurrent check can't start a second download. The
            // spawn is a real yield point, so `Available` is observable before `Downloading`.
            set_status(
                app,
                UpdateStatus::Available {
                    version: update.version.clone(),
                },
            );
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _guard = guard;
                download_and_stage(&app, update).await;
            });
        }
        Ok(None) => {
            // No newer release: `check` already set Idle; the guard drops here → slot freed.
        }
        Err(message) => {
            tracing::warn!(error = %message, "update check failed");
            set_status(app, UpdateStatus::Error { message });
            // guard drops here → slot freed
        }
    }
}

/// Query the endpoint. Sets `Checking`, and on "no newer release" sets `Idle` and clears any
/// stale pending download. Returns the `Update` handle when a newer signed release exists.
async fn check(app: &AppHandle) -> Result<Option<Update>, String> {
    set_status(app, UpdateStatus::Checking);
    let updater = app.updater().map_err(|e| e.to_string())?;
    let found = updater.check().await.map_err(|e| e.to_string())?;
    if found.is_none() {
        // Zero UI presence, and drop any stale pending download from a prior check.
        *app.state::<UpdaterState>()
            .pending
            .lock()
            .expect("pending lock") = None;
        set_status(app, UpdateStatus::Idle);
    }
    Ok(found)
}

/// Download + signature-verify the update in the background, stage the verified bytes for
/// install-on-restart, and set `Ready`. The plugin verifies the minisign signature of the
/// downloaded artifact against the baked pubkey inside `download`, so a tampered / unsigned /
/// wrong-key payload fails here and surfaces as `Error` (the negative-test path, `03 §11b`).
async fn download_and_stage(app: &AppHandle, update: Update) {
    let version = update.version.clone();
    tracing::info!(version = %version, "update available; starting background download");
    set_status(
        app,
        UpdateStatus::Downloading {
            version: version.clone(),
        },
    );
    match update.download(|_chunk, _total| {}, || {}).await {
        Ok(bytes) => {
            tracing::info!(version = %version, bytes = bytes.len(), "update downloaded + signature-verified");
            *app.state::<UpdaterState>()
                .pending
                .lock()
                .expect("pending lock") = Some(PendingUpdate { update, bytes });
            set_status(app, UpdateStatus::Ready { version });
        }
        Err(e) => {
            let message = e.to_string();
            tracing::warn!(error = %message, "update download failed");
            // Drop any previously-staged update: a failed (re)download must not leave the
            // machine in `Error` while `pending` still holds an installer, which would be an
            // inconsistent state with no UI path to apply it (the restart button renders only
            // for `Ready`). Consistency wins over reusing the earlier bytes.
            *app.state::<UpdaterState>()
                .pending
                .lock()
                .expect("pending lock") = None;
            set_status(app, UpdateStatus::Error { message });
        }
    }
}

/// Current updater snapshot (`get_update_status`). Cheap synchronous read.
#[tauri::command]
pub fn get_update_status(app: AppHandle) -> UpdateStatus {
    current_status(&app)
}

/// Manual "Check for updates" (Settings App section + the NavRail footer control). Runs the
/// check and returns the **post-check** snapshot (`available` / `idle` / `error`) so the
/// caller sees the immediate result; a found update then downloads in the **background** and
/// finishes via the `update_status_changed` event — it does not block this command.
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
    let pending = app
        .state::<UpdaterState>()
        .pending
        .lock()
        .expect("pending lock")
        .take();
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

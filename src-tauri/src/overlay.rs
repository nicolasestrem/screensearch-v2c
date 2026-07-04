//! Flow overlay shell integration (0.3.0 PR5).
//!
//! This module owns the Tauri-only surface: global hotkey registration, the hidden
//! always-on-top overlay window, and cross-window events. Search and Ask still go
//! through the existing typed IPC commands; no retrieval path lives here.

use std::sync::Mutex;
use std::time::Instant;

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, State, WebviewWindow,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use traits::{HotkeyStatus, MarkToast, OpenMoment, Toast, ToastLevel};

const OVERLAY_LABEL: &str = "overlay";
const MAIN_LABEL: &str = "main";
const OVERLAY_HOTKEY_ID: &str = "overlay.hotkey";
// "Ctrl+Alt+Z": the original "Ctrl+Alt+Space" collided with Claude Desktop's quick-entry
// shortcut. Must match `Settings::default().overlay_hotkey` and the UI `DEFAULT_OVERLAY_HOTKEY`.
const OVERLAY_DEFAULT_CHORD: &str = "Ctrl+Alt+Z";
const MARKS_HOTKEY_ID: &str = "marks.hotkey";
const MARKS_DEFAULT_CHORD: &str = "Ctrl+Alt+M";
const FALLBACK_WIDTH: u32 = 640;
const FALLBACK_HEIGHT: u32 = 460;

#[derive(Default)]
pub struct OverlayState {
    hotkeys: Mutex<Vec<HotkeyStatus>>,
    registered_overlay_chord: Mutex<Option<String>>,
    /// The mark-this-moment chord currently registered (0.3.0 PR6; `03 §7b`, D6).
    registered_marks_chord: Mutex<Option<String>>,
    summon_started: Mutex<Option<Instant>>,
}

/// Trims a chord, substituting `default` when empty — the shell never registers a blank
/// hotkey (`03 §8`; the D6 warning covers a bad/colliding value).
fn normalize_chord<'a>(chord: &'a str, default: &'a str) -> &'a str {
    let trimmed = chord.trim();
    if trimmed.is_empty() {
        default
    } else {
        trimmed
    }
}

#[tauri::command]
pub fn get_hotkey_status(state: State<'_, OverlayState>) -> Vec<HotkeyStatus> {
    state.hotkeys.lock().expect("hotkey status lock").clone()
}

#[tauri::command]
pub fn hide_overlay(app: AppHandle) -> Result<(), String> {
    hide_overlay_window(&app)
}

#[tauri::command]
pub fn overlay_shown_ack(state: State<'_, OverlayState>) -> Result<(), String> {
    if let Some(started) = state
        .summon_started
        .lock()
        .expect("overlay summon timer lock")
        .take()
    {
        tracing::info!(
            target: "overlay_perf",
            input_ready_ms = started.elapsed().as_millis() as u64,
            "overlay first paint acknowledged"
        );
    }
    Ok(())
}

#[tauri::command]
pub fn open_moment(app: AppHandle, frame_id: i64) -> Result<(), String> {
    let main = app
        .get_webview_window(MAIN_LABEL)
        .ok_or_else(|| "main window unavailable".to_string())?;
    main.show().map_err(|e| e.to_string())?;
    main.unminimize().map_err(|e| e.to_string())?;
    main.set_focus().map_err(|e| e.to_string())?;
    app.emit_to(MAIN_LABEL, "open_moment", OpenMoment { frame_id })
        .map_err(|e| e.to_string())?;
    hide_overlay_window(&app)
}

pub fn init_overlay_hotkey<R: Runtime>(app: &AppHandle<R>, chord: &str) {
    let chord = normalize_chord(chord, OVERLAY_DEFAULT_CHORD);
    match register_overlay_hotkey(app, chord) {
        Ok(()) => {
            *app.state::<OverlayState>()
                .registered_overlay_chord
                .lock()
                .expect("registered hotkey lock") = Some(chord.to_string());
            set_status(app, ok_status(OVERLAY_HOTKEY_ID, chord));
        }
        Err(error) => {
            *app.state::<OverlayState>()
                .registered_overlay_chord
                .lock()
                .expect("registered hotkey lock") = None;
            set_status(app, failed_status(OVERLAY_HOTKEY_ID, chord, error.clone()));
            emit_hotkey_warning(app, format!("Flow overlay hotkey unavailable: {error}"));
        }
    }
}

pub fn reregister_overlay_hotkey<R: Runtime>(app: &AppHandle<R>, chord: &str) {
    let chord = normalize_chord(chord, OVERLAY_DEFAULT_CHORD);
    let state = app.state::<OverlayState>();
    let old = state
        .registered_overlay_chord
        .lock()
        .expect("registered hotkey lock")
        .clone();

    // `registered_overlay_chord` only advances on a successful (re)register, so when it
    // already names `chord` the OS still holds that shortcut. Re-registering it would
    // fail as a duplicate — so refresh the status to OK (this also clears a stale
    // warning from an earlier failed save that reverted to this chord) and return.
    if old.as_deref() == Some(chord) {
        set_status(app, ok_status(OVERLAY_HOTKEY_ID, chord));
        return;
    }

    match register_overlay_hotkey(app, chord) {
        Ok(()) => {
            if let Some(old_chord) = old.as_deref().filter(|old| *old != chord) {
                if let Err(error) = app.global_shortcut().unregister(old_chord) {
                    tracing::warn!(%old_chord, error = %error, "failed to unregister previous overlay hotkey");
                }
            }
            *state
                .registered_overlay_chord
                .lock()
                .expect("registered hotkey lock") = Some(chord.to_string());
            set_status(app, ok_status(OVERLAY_HOTKEY_ID, chord));
        }
        Err(error) => {
            set_status(app, failed_status(OVERLAY_HOTKEY_ID, chord, error.clone()));
            let suffix = if old.is_some() {
                "; still using the previous combination"
            } else {
                ""
            };
            emit_hotkey_warning(
                app,
                format!("Could not register Flow overlay hotkey{suffix}: {error}"),
            );
        }
    }
}

fn register_overlay_hotkey<R: Runtime>(app: &AppHandle<R>, chord: &str) -> Result<(), String> {
    let chord_for_handler = chord.to_string();
    app.global_shortcut()
        .on_shortcut(chord, move |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                if let Err(error) = toggle_overlay(app) {
                    tracing::warn!(
                        chord = %chord_for_handler,
                        error = %error,
                        "failed to toggle Flow overlay"
                    );
                }
            }
        })
        .map_err(|e| e.to_string())
}

// ── Mark-this-moment hotkey + quiet toast (0.3.0 PR6; `03 §7b`, D6/D8) ─────────────

pub fn init_marks_hotkey<R: Runtime>(app: &AppHandle<R>, chord: &str) {
    let chord = normalize_chord(chord, MARKS_DEFAULT_CHORD);
    match register_marks_hotkey(app, chord) {
        Ok(()) => {
            *app.state::<OverlayState>()
                .registered_marks_chord
                .lock()
                .expect("registered hotkey lock") = Some(chord.to_string());
            set_status(app, ok_status(MARKS_HOTKEY_ID, chord));
        }
        Err(error) => {
            *app.state::<OverlayState>()
                .registered_marks_chord
                .lock()
                .expect("registered hotkey lock") = None;
            set_status(app, failed_status(MARKS_HOTKEY_ID, chord, error.clone()));
            emit_hotkey_warning(app, format!("Mark hotkey unavailable: {error}"));
        }
    }
}

pub fn reregister_marks_hotkey<R: Runtime>(app: &AppHandle<R>, chord: &str) {
    let chord = normalize_chord(chord, MARKS_DEFAULT_CHORD);
    let state = app.state::<OverlayState>();
    let old = state
        .registered_marks_chord
        .lock()
        .expect("registered hotkey lock")
        .clone();

    // `registered_marks_chord` only advances on a successful (re)register, so when it
    // already names `chord` the OS still holds that shortcut. Re-registering it would
    // fail as a duplicate — so refresh the status to OK (this also clears a stale
    // warning from an earlier failed save that reverted to this chord) and return.
    if old.as_deref() == Some(chord) {
        set_status(app, ok_status(MARKS_HOTKEY_ID, chord));
        return;
    }

    match register_marks_hotkey(app, chord) {
        Ok(()) => {
            if let Some(old_chord) = old.as_deref().filter(|old| *old != chord) {
                if let Err(error) = app.global_shortcut().unregister(old_chord) {
                    tracing::warn!(%old_chord, error = %error, "failed to unregister previous mark hotkey");
                }
            }
            *state
                .registered_marks_chord
                .lock()
                .expect("registered hotkey lock") = Some(chord.to_string());
            set_status(app, ok_status(MARKS_HOTKEY_ID, chord));
        }
        Err(error) => {
            set_status(app, failed_status(MARKS_HOTKEY_ID, chord, error.clone()));
            let suffix = if old.is_some() {
                "; still using the previous combination"
            } else {
                ""
            };
            emit_hotkey_warning(
                app,
                format!("Could not register mark hotkey{suffix}: {error}"),
            );
        }
    }
}

fn register_marks_hotkey<R: Runtime>(app: &AppHandle<R>, chord: &str) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(chord, move |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                handle_mark_hotkey(app);
            }
        })
        .map_err(|e| e.to_string())
}

/// Fires a `capture_now` mark on hotkey press and shows the quiet confirmation toast.
/// Runs the async `add_mark` on the tauri runtime; the result drives a non-focus-
/// stealing toast in the overlay window (`03 §7b`, D8) and a `marks_changed` refresh.
fn handle_mark_hotkey<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let kernel = {
            let state = app.state::<crate::AppState>();
            state.kernel.clone()
        };
        let Some(kernel) = kernel else {
            let _ = show_mark_toast(
                &app,
                MarkToast {
                    mark_id: None,
                    level: ToastLevel::Warning,
                    message: "Capture is off — mark not saved".to_string(),
                },
            );
            return;
        };
        match kernel.add_mark(None, true, None).await {
            Ok(mark_id) => {
                let _ = show_mark_toast(
                    &app,
                    MarkToast {
                        mark_id: Some(mark_id),
                        level: ToastLevel::Success,
                        message: "Marked".to_string(),
                    },
                );
                // The Deck's marks query lives in the main window's QueryClient; a
                // cross-window event refreshes the Intentions strip.
                let _ = app.emit("marks_changed", ());
            }
            Err(error) => {
                let _ = show_mark_toast(
                    &app,
                    MarkToast {
                        mark_id: None,
                        level: ToastLevel::Warning,
                        message: map_mark_error(&error.to_string()),
                    },
                );
            }
        }
    });
}

/// Maps a `capture_now` failure to a short, honest user message. "Capture is off" is the
/// common case (the demanded capture had no running worker — user decision D-capture-off).
fn map_mark_error(reason: &str) -> String {
    if reason.contains("capture is off") || reason.contains("capture stopped") {
        "Capture is off — mark not saved".to_string()
    } else {
        format!("Couldn't mark: {reason}")
    }
}

/// Shows the mark confirmation toast in the overlay window **without stealing focus**
/// (user decision D1): the window is made non-focusable before `show()`, so the user
/// keeps typing in their app; clicking the note field focuses it
/// ([`focus_overlay_for_note`]). The overlay is positioned on the foreground monitor,
/// reusing the search-overlay placement.
fn show_mark_toast<R: Runtime>(app: &AppHandle<R>, payload: MarkToast) -> Result<(), String> {
    let overlay = overlay_window(app)?;
    let _ = overlay.set_focusable(false);
    position_overlay(app, &overlay)?;
    overlay.show().map_err(|e| e.to_string())?;
    app.emit_to(OVERLAY_LABEL, "mark_toast", payload)
        .map_err(|e| e.to_string())
}

/// Makes the overlay focusable and focuses it — invoked when the user clicks the mark
/// toast's note field so keystrokes land in the note (the toast is otherwise
/// non-focusable, D1). From then on the normal blur-hide applies.
#[tauri::command]
pub fn focus_overlay_for_note(app: AppHandle) -> Result<(), String> {
    let overlay = overlay_window(&app)?;
    overlay.set_focusable(true).map_err(|e| e.to_string())?;
    overlay.set_focus().map_err(|e| e.to_string())
}

/// Dismisses the mark toast, restoring the overlay to focusable so the next search
/// summon behaves normally.
#[tauri::command]
pub fn dismiss_mark_toast(app: AppHandle) -> Result<(), String> {
    let overlay = overlay_window(&app)?;
    let _ = overlay.set_focusable(true);
    hide_overlay_window(&app)
}

pub fn toggle_overlay<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let overlay = overlay_window(app)?;
    if overlay.is_visible().unwrap_or(false) {
        return hide_overlay_window(app);
    }
    summon_overlay(app, &overlay)
}

fn summon_overlay<R: Runtime>(
    app: &AppHandle<R>,
    overlay: &WebviewWindow<R>,
) -> Result<(), String> {
    *app.state::<OverlayState>()
        .summon_started
        .lock()
        .expect("overlay summon timer lock") = Some(Instant::now());
    position_overlay(app, overlay)?;
    let started = Instant::now();
    // A prior mark toast may have left the window non-focusable (D1); the search overlay
    // always takes focus, so restore it before showing.
    let _ = overlay.set_focusable(true);
    overlay.show().map_err(|e| e.to_string())?;
    overlay.set_focus().map_err(|e| e.to_string())?;
    tracing::info!(
        target: "overlay_perf",
        visible_ms = started.elapsed().as_millis() as u64,
        "overlay window shown"
    );
    app.emit_to(OVERLAY_LABEL, "overlay_shown", ())
        .map_err(|e| e.to_string())
}

pub fn hide_overlay_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let overlay = overlay_window(app)?;
    overlay.hide().map_err(|e| e.to_string())?;
    app.emit_to(OVERLAY_LABEL, "overlay_hidden", ())
        .map_err(|e| e.to_string())
}

fn position_overlay<R: Runtime>(
    app: &AppHandle<R>,
    overlay: &WebviewWindow<R>,
) -> Result<(), String> {
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    let target = foreground_point()
        .and_then(|point| {
            monitors
                .iter()
                .find(|monitor| point_in_work_area(point, monitor.work_area()))
                .cloned()
        })
        .or_else(|| app.primary_monitor().ok().flatten())
        .or_else(|| monitors.first().cloned());

    let Some(monitor) = target else {
        return Ok(());
    };

    let area = monitor.work_area();
    let scale = monitor.scale_factor().max(1.0);
    let size = overlay.outer_size().unwrap_or_else(|_| {
        PhysicalSize::new(
            (f64::from(FALLBACK_WIDTH) * scale).round() as u32,
            (f64::from(FALLBACK_HEIGHT) * scale).round() as u32,
        )
    });
    let area_w = area.size.width as i32;
    let area_h = area.size.height as i32;
    let overlay_w = size.width as i32;
    let overlay_h = size.height as i32;
    let x_offset = ((area_w - overlay_w) / 2).max(0);
    let y_offset = (area_h / 6).min((area_h - overlay_h).max(0)).max(0);

    overlay
        .set_position(PhysicalPosition::new(
            area.position.x + x_offset,
            area.position.y + y_offset,
        ))
        .map_err(|e| e.to_string())
}

fn foreground_point() -> Option<(i32, i32)> {
    #[cfg(windows)]
    {
        capture::foreground_window_rect()
            .map(|(left, top, right, bottom)| ((left + right) / 2, (top + bottom) / 2))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn point_in_work_area((x, y): (i32, i32), area: &tauri::PhysicalRect<i32, u32>) -> bool {
    let left = area.position.x;
    let top = area.position.y;
    x >= left && x < left + area.size.width as i32 && y >= top && y < top + area.size.height as i32
}

fn overlay_window<R: Runtime>(app: &AppHandle<R>) -> Result<WebviewWindow<R>, String> {
    app.get_webview_window(OVERLAY_LABEL)
        .ok_or_else(|| "overlay window unavailable".to_string())
}

fn set_status<R: Runtime>(app: &AppHandle<R>, status: HotkeyStatus) {
    let state = app.state::<OverlayState>();
    let mut hotkeys = state.hotkeys.lock().expect("hotkey status lock");
    if let Some(existing) = hotkeys.iter_mut().find(|h| h.id == status.id) {
        *existing = status;
    } else {
        hotkeys.push(status);
    }
}

fn ok_status(id: &str, chord: &str) -> HotkeyStatus {
    HotkeyStatus {
        id: id.to_string(),
        chord: chord.to_string(),
        registered: true,
        error: None,
    }
}

fn failed_status(id: &str, chord: &str, error: String) -> HotkeyStatus {
    HotkeyStatus {
        id: id.to_string(),
        chord: chord.to_string(),
        registered: false,
        error: Some(error),
    }
}

fn emit_hotkey_warning<R: Runtime>(app: &AppHandle<R>, message: String) {
    let _ = app.emit(
        "toast",
        Toast {
            level: ToastLevel::Warning,
            message,
        },
    );
}

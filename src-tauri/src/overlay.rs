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
use traits::{HotkeyStatus, OpenMoment, Toast, ToastLevel};

const OVERLAY_LABEL: &str = "overlay";
const MAIN_LABEL: &str = "main";
const OVERLAY_HOTKEY_ID: &str = "overlay.hotkey";
const FALLBACK_WIDTH: u32 = 640;
const FALLBACK_HEIGHT: u32 = 460;

#[derive(Default)]
pub struct OverlayState {
    hotkeys: Mutex<Vec<HotkeyStatus>>,
    registered_overlay_chord: Mutex<Option<String>>,
    summon_started: Mutex<Option<Instant>>,
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
    let sanitized = chord.trim();
    let chord = if sanitized.is_empty() {
        "Ctrl+Alt+Space"
    } else {
        sanitized
    };
    match register_overlay_hotkey(app, chord) {
        Ok(()) => {
            *app.state::<OverlayState>()
                .registered_overlay_chord
                .lock()
                .expect("registered hotkey lock") = Some(chord.to_string());
            set_status(app, ok_status(chord));
        }
        Err(error) => {
            *app.state::<OverlayState>()
                .registered_overlay_chord
                .lock()
                .expect("registered hotkey lock") = None;
            set_status(app, failed_status(chord, error.clone()));
            emit_hotkey_warning(app, format!("Flow overlay hotkey unavailable: {error}"));
        }
    }
}

pub fn reregister_overlay_hotkey<R: Runtime>(app: &AppHandle<R>, chord: &str) {
    let sanitized = chord.trim();
    let chord = if sanitized.is_empty() {
        "Ctrl+Alt+Space"
    } else {
        sanitized
    };
    let state = app.state::<OverlayState>();
    let old = state
        .registered_overlay_chord
        .lock()
        .expect("registered hotkey lock")
        .clone();

    if old.as_deref() == Some(chord)
        && state
            .hotkeys
            .lock()
            .expect("hotkey status lock")
            .iter()
            .any(|s| s.id == OVERLAY_HOTKEY_ID && s.registered)
    {
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
            set_status(app, ok_status(chord));
        }
        Err(error) => {
            set_status(app, failed_status(chord, error.clone()));
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

fn point_in_work_area(
    (x, y): (i32, i32),
    area: &tauri::PhysicalRect<i32, u32>,
) -> bool {
    let left = area.position.x;
    let top = area.position.y;
    x >= left
        && x < left + area.size.width as i32
        && y >= top
        && y < top + area.size.height as i32
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

fn ok_status(chord: &str) -> HotkeyStatus {
    HotkeyStatus {
        id: OVERLAY_HOTKEY_ID.to_string(),
        chord: chord.to_string(),
        registered: true,
        error: None,
    }
}

fn failed_status(chord: &str, error: String) -> HotkeyStatus {
    HotkeyStatus {
        id: OVERLAY_HOTKEY_ID.to_string(),
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

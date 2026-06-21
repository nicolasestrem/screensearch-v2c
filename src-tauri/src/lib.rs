//! ScreenSearch V2c — Tauri 2 desktop shell and **composition root** (`03 §2`).
//!
//! This crate is the only place that wires concrete module impls into the kernel.
//! In P0 there are no impls yet, so it exposes a minimal pair of typed-IPC smoke
//! commands (`ping`, `get_readiness`) that prove the Tauri bridge + `ts-rs`
//! bindings work end-to-end. Phases P1–P4 register the real modules here.

use traits::ipc::{ComponentStatus, Readiness};

/// Liveness probe for the typed IPC bridge (P0 smoke test).
#[tauri::command]
fn ping() -> String {
    "pong".to_string()
}

/// Current subsystem readiness (`03 §7`).
///
/// In P0 nothing is wired yet, so every component honestly reports `Unknown`;
/// later phases flip these as impls land — db (P1), capture (P2), embed model
/// (P3), sidecar (P4).
#[tauri::command]
fn get_readiness() -> Readiness {
    Readiness {
        capture: ComponentStatus::Unknown,
        db: ComponentStatus::Unknown,
        embed_model: ComponentStatus::Unknown,
        sidecar: ComponentStatus::Unknown,
    }
}

/// Application entry point (called from `main.rs`).
pub fn run() {
    init_tracing();
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![ping, get_readiness])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Console tracing for P0. The daily-rotating file sink (`03 §9`) is wired in P1,
/// once the app data dir is resolved.
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).try_init();
}

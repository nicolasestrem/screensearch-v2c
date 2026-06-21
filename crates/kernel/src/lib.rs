//! `kernel` — the orchestrator that owns the typed event bus, the bounded worker
//! pool, and the `ModelSupervisor` (`03 §1/§5/§6`).
//!
//! The kernel depends only on [`traits`] (the contracts), never on a module's
//! concrete impl; `src-tauri` wires impls in at startup (the composition root,
//! `03 §2`).
//!
//! **Status:** P0 scaffold — no orchestration logic yet. The event bus and worker
//! pool land in **P1–P3**, the `ModelSupervisor` in **P4** (`02 §5`).

#![forbid(unsafe_code)]

/// The module contracts this kernel orchestrates over, re-exported for the
/// composition root's convenience.
pub use traits;

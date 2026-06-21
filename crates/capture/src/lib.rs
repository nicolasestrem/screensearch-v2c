//! `capture` — `CaptureSource` via Windows.Graphics.Capture (WGC) plus the
//! diff gate that yields only *changed* frames (`03 §3`).
//!
//! Windows-native by design — no cross-platform abstraction (`04` guardrails).
//! WGC capture is COM STA-bound (`01 §6`).
//!
//! **Status:** P0 scaffold — no implementation yet. WGC capture lands in **P2**
//! (`02 §5`), behind an early spike to de-risk the new code path.
//!
//! (No `forbid(unsafe_code)` here: the P2 WGC/COM path will require `unsafe` FFI.)

/// The contract this crate implements (impl arrives in P2).
pub use traits::CaptureSource;

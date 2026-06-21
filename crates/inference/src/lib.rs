//! `inference` — `VisionProvider` + `AnswerProvider`, both backed by the single
//! supervised, model-agnostic **llama.cpp sidecar** (OpenAI-compatible HTTP), plus
//! the `ModelSupervisor` that owns its lifecycle (`03 §6`).
//!
//! The sidecar is bound to the app via a Windows **Job Object**
//! (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) so it can never orphan after a crash —
//! a hard requirement; P4 does not ship until the no-orphan test passes (`03 §6`).
//!
//! **Status:** P0 scaffold — no implementation yet. The Job-Object lifecycle lands
//! first in **P4**, before any real inference wiring (`02 §5`, `04 §3`).
//!
//! (No `forbid(unsafe_code)` here: the P4 Job-Object path uses `unsafe` Win32 FFI.)

/// The contracts this crate implements (impls arrive in P4).
pub use traits::{AnswerProvider, VisionProvider};

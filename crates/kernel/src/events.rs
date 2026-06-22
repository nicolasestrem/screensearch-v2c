//! The kernel's typed event bus payloads (`03 §1/§7`).
//!
//! The kernel is shell-agnostic: it broadcasts [`KernelEvent`]s over a
//! [`tokio::sync::broadcast`] channel, and the composition root (`src-tauri`)
//! forwards them to the WebView2 UI as Tauri events. The kernel never knows about
//! Tauri.

use traits::{CaptureTick, JobStats, Readiness, SidecarStatus};

/// An event the kernel emits for the UI (forwarded to Tauri events by `src-tauri`).
///
/// Cloneable because `broadcast` delivers a clone to every subscriber.
#[derive(Debug, Clone)]
pub enum KernelEvent {
    /// A frame was captured, OCR'd, and stored — drives the live timeline
    /// (`capture_tick`, `03 §7`).
    CaptureTick(CaptureTick),
    /// A subsystem's readiness changed — drives the readiness strip
    /// (`readiness_changed`, `03 §7`).
    ReadinessChanged(Readiness),
    /// Queue depth changed after a worker finished a job — drives the job-queue
    /// indicator (`job_progress`, `03 §7`).
    JobProgress(JobStats),
    /// The inference sidecar changed lifecycle state — drives the sidecar status
    /// indicator (`sidecar_status`, `03 §6/§7`).
    SidecarStatus(SidecarStatus),
}

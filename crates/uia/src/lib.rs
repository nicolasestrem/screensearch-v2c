//! `uia` — [`traits::OcrProvider`] backed by Windows **UI Automation** (`docs/0.2.0.md`
//! #48). It reads the *foreground/target window's* accessibility text — more structured
//! and higher-fidelity than full-screen OCR — and returns it through the same
//! [`OcrResult`] contract OCR uses, so the capture loop, the PR3 text filter, embeddings,
//! and retrieval all flow unchanged. Spans are tagged `source = TextSource::Uia` and
//! `role = TextRole::Unknown` (PR3 classifies roles); `engine = "uia"` lets the store set
//! `frame_text.primary_source = 'uia'`.
//!
//! ## Threading
//! UI Automation is COM and free-threaded; Microsoft recommends an **MTA**. A dedicated
//! long-lived worker thread ([`worker`]) owns the `IUIAutomation` instance under
//! `COINIT_MULTITHREADED`; [`UiaTextProvider::recognize`] dispatches plain `Send` data to
//! it over a channel (no COM pointer ever crosses a thread boundary) and enforces a hard
//! timeout against a wedged worker.
//!
//! ## Fallback (in the composition root, not here)
//! This crate is the **primary** UIA provider only. The OCR fallback is composed in
//! `src-tauri` (the only place that may wire two concrete impls, `03 §2`): when UIA errors,
//! exceeds its latency budget, or yields too little text, `recognize` returns `Err` and the
//! composite falls back to OCR for that frame.
//!
//! ## Privacy
//! UIA reads the on-screen accessibility text of the target window — the same privacy class
//! as OCR. Two UIA-specific guards in [`worker`]: password fields (`CurrentIsPassword`) are
//! never read, and offscreen/occluded elements (`CurrentIsOffscreen`) plus spans outside
//! the target-window rect are dropped, preserving OCR's "only what was visible" parity.
//!
//! Windows-only by design — no cross-platform fallback (`04` guardrails).

mod classify;
mod geometry;
mod monitors;
mod worker;

use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::oneshot;
use traits::{CapturedFrame, OcrProvider, OcrResult, Result};

/// Sentinel `mean_confidence`: UI Automation, like WinRT OCR, exposes no confidence, so we
/// record "unknown" rather than inventing a number (mirrors [`ocr::CONFIDENCE_UNKNOWN`]).
pub const CONFIDENCE_UNKNOWN: f32 = -1.0;

/// Per-frame gating thresholds, sourced from clamped settings (never hardcoded, `03 §3b`).
#[derive(Debug, Clone, Copy)]
pub struct UiaBudget {
    /// Soft wall-clock budget for one tree walk (`capture.uia_latency_budget_ms`); the walk
    /// abandons past it and `recognize` enforces a 2× hard timeout on top.
    pub latency_ms: u64,
    /// Minimum gathered text length below which the read is treated as a thin yield
    /// (`capture.uia_min_text_chars`) → `Err` → OCR fallback for that frame.
    pub min_text_chars: usize,
}

/// UI Automation text provider backed by a dedicated COM MTA worker thread.
///
/// `Mutex<Sender>` makes the provider `Sync` (the trait requires it); the lock is only held
/// to clone the sender, never across the UIA call.
pub struct UiaTextProvider {
    tx: Mutex<mpsc::Sender<worker::Request>>,
    budget: UiaBudget,
}

impl UiaTextProvider {
    /// Spawns the MTA worker and creates the `IUIAutomation` object once. This **is** the
    /// capability probe: it returns `Err` when UI Automation cannot be initialized on this
    /// session, so the caller can compose without a UIA arm (OCR carries every frame).
    pub fn spawn(budget: UiaBudget) -> Result<Self> {
        let (tx, rx) = mpsc::channel::<worker::Request>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();

        std::thread::Builder::new()
            .name("uia-mta".to_string())
            .spawn(move || worker::worker_main(rx, ready_tx, budget))
            .map_err(|e| anyhow::anyhow!("failed to spawn UIA MTA thread: {e}"))?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                tx: Mutex::new(tx),
                budget,
            }),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(anyhow::anyhow!("UIA worker exited during init: {e}")),
        }
    }
}

#[async_trait]
impl OcrProvider for UiaTextProvider {
    async fn recognize(&self, frame: &CapturedFrame) -> Result<OcrResult> {
        let width = frame.width;
        let height = frame.height;
        let target_rect = frame.target_rect;
        let monitor_index = frame.monitor_index;
        let foreground_hwnd = frame.foreground_hwnd;
        let tx = self.tx.lock().expect("uia sender poisoned").clone();
        // Hard ceiling against a wedged worker, on top of the worker's own soft budget.
        let timeout = Duration::from_millis(self.budget.latency_ms.saturating_mul(2).max(1));

        // The request send is non-blocking (unbounded channel) and the reply is a `tokio`
        // oneshot, so we await it directly — no `spawn_blocking` task is parked per frame.
        // On a hard timeout the receiver is dropped here and the worker's later `send`
        // simply no-ops, so a wedged worker can never grow the blocking pool.
        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(worker::Request {
            width,
            height,
            target_rect,
            monitor_index,
            foreground_hwnd,
            resp: resp_tx,
        })
        .map_err(|_| anyhow::anyhow!("UIA worker thread is gone"))?;

        match tokio::time::timeout(timeout, resp_rx).await {
            Ok(joined) => joined.map_err(|_| anyhow::anyhow!("UIA worker dropped the response"))?,
            Err(_) => Err(anyhow::anyhow!(
                "UIA recognize exceeded {} ms hard timeout",
                timeout.as_millis()
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::sync::Arc;
    use traits::{CaptureTrigger, TextSource};

    fn frame(w: u32, h: u32) -> CapturedFrame {
        CapturedFrame {
            monitor_index: 0,
            width: w,
            height: h,
            captured_at: 1,
            pixels: Arc::new(RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]))),
            content_hash: "test".to_string(),
            app_hint: None,
            window_title: None,
            // Full-screen target rect: UIA only runs when this frame's monitor holds the
            // foreground window (a `None` rect now falls back to OCR), so the live test must
            // supply one to exercise the real UIA path.
            target_rect: Some([0.0, 0.0, 1.0, 1.0]),
            // `None` ⇒ the worker skips the focus-change check (it can't know the live HWND).
            foreground_hwnd: None,
            trigger: CaptureTrigger::Timer,
        }
    }

    /// Real UI Automation against the live desktop's foreground window: must spawn, return
    /// `engine = "uia"` with the unknown-confidence sentinel, and every emitted span's bbox
    /// normalized to `[0,1]` and tagged `TextSource::Uia`. `#[ignore]`d in CI (needs a real
    /// desktop session); run locally with `cargo test -p uia -- --ignored`.
    #[tokio::test]
    #[ignore = "requires a real desktop (UI Automation); run locally"]
    async fn uia_provider_spawns_and_recognizes_foreground() {
        let provider = UiaTextProvider::spawn(UiaBudget {
            latency_ms: 150,
            min_text_chars: 0,
        })
        .expect("spawn uia worker");
        let result = provider
            .recognize(&frame(1920, 1080))
            .await
            .expect("uia recognize ok");
        assert_eq!(result.engine, "uia");
        assert_eq!(result.mean_confidence, CONFIDENCE_UNKNOWN);
        for span in &result.spans {
            assert!(
                (0.0..=1.0).contains(&span.x)
                    && (0.0..=1.0).contains(&span.y)
                    && span.x + span.w <= 1.0 + 1e-6
                    && span.y + span.h <= 1.0 + 1e-6,
                "span bbox not normalized: {span:?}"
            );
            assert!(matches!(span.source, TextSource::Uia));
        }
    }
}

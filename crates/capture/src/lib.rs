//! `capture` — [`CaptureSource`] via Windows.Graphics.Capture (WGC) plus the diff
//! gate that yields only *changed* frames (`03 §3`).
//!
//! Windows-native by design — no cross-platform abstraction (`04` guardrails). The
//! WGC + D3D11 work runs on a dedicated COM thread ([`wgc::worker_main`], `01 §6`);
//! [`WgcCapture::next_frame`] paces by `capture.interval_ms`, applies the privacy
//! gate, and drains the worker's changed frames one at a time. The diff metric and
//! the excluded-app matcher are pure and unit-tested ([`diff`], [`privacy`]).
//!
//! (No `forbid(unsafe_code)`: the WGC/COM/D3D11 path requires `unsafe` FFI.)

use std::collections::VecDeque;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use image::RgbaImage;
use std::sync::Arc;
use tokio::sync::oneshot;
use traits::{CaptureConfig, CaptureSource, CapturedFrame, MonitorInfo, Result};

mod diff;
mod idle;
mod monitors;
mod privacy;
mod wgc;

pub use idle::user_idle_ms;
use wgc::CaptureRequest;

/// WGC-backed capture source. Owns the channel to the capture worker thread, the
/// monitor list, and a small queue of changed frames produced by the last cycle.
pub struct WgcCapture {
    monitors: Vec<MonitorInfo>,
    config: CaptureConfig,
    req_tx: Mutex<mpsc::Sender<CaptureRequest>>,
    queue: VecDeque<CapturedFrame>,
}

impl WgcCapture {
    /// Spawns the capture worker, sets up per-monitor WGC sessions, and returns once
    /// the monitor list is known. Errors if there are no capturable monitors.
    pub fn new(config: CaptureConfig) -> Result<Self> {
        let (req_tx, req_rx) = mpsc::channel::<CaptureRequest>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<Vec<MonitorInfo>>>();

        let cfg = config.clone();
        std::thread::Builder::new()
            .name("wgc-capture".to_string())
            .spawn(move || wgc::worker_main(cfg, req_rx, ready_tx))
            .map_err(|e| anyhow::anyhow!("failed to spawn capture thread: {e}"))?;

        let monitors = match ready_rx.recv() {
            Ok(Ok(m)) => m,
            Ok(Err(e)) => return Err(e),
            Err(e) => return Err(anyhow::anyhow!("capture worker exited during init: {e}")),
        };

        Ok(Self {
            monitors,
            config,
            req_tx: Mutex::new(req_tx),
            queue: VecDeque::new(),
        })
    }

    /// Asks the worker for one capture cycle and returns the changed frames.
    /// `None` means the worker is gone (shutdown).
    async fn capture_cycle(&self) -> Option<Vec<wgc::FrameData>> {
        let (resp_tx, resp_rx) = oneshot::channel();
        {
            let tx = self.req_tx.lock().expect("capture req sender poisoned");
            if tx.send(CaptureRequest { resp: resp_tx }).is_err() {
                return None;
            }
        }
        match resp_rx.await {
            Ok(Ok(frames)) => Some(frames),
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "capture cycle failed");
                Some(Vec::new())
            }
            Err(_) => None,
        }
    }
}

#[async_trait]
impl CaptureSource for WgcCapture {
    fn monitors(&self) -> Vec<MonitorInfo> {
        self.monitors.clone()
    }

    async fn next_frame(&mut self) -> Result<Option<CapturedFrame>> {
        loop {
            if let Some(frame) = self.queue.pop_front() {
                return Ok(Some(frame));
            }

            // Pace to the capture interval (the timer lives inside the source).
            tokio::time::sleep(Duration::from_millis(u64::from(
                self.config.interval_ms.max(1),
            )))
            .await;

            // Privacy gate (03 §8): pause while locked, skip while an excluded app
            // is focused. The foreground read also yields the frame context.
            if self.config.pause_on_lock && privacy::is_workstation_locked() {
                continue;
            }
            let (app_hint, window_title) = privacy::foreground_context();
            if privacy::is_excluded(
                app_hint.as_deref(),
                window_title.as_deref(),
                &self.config.excluded_apps,
            ) {
                continue;
            }

            let Some(frames) = self.capture_cycle().await else {
                return Ok(None); // worker gone
            };

            let captured_at = now_ms();
            for fd in frames {
                let Some(pixels) = RgbaImage::from_raw(fd.width, fd.height, fd.rgba) else {
                    continue;
                };
                self.queue.push_back(CapturedFrame {
                    monitor_index: fd.monitor_index,
                    width: fd.width,
                    height: fd.height,
                    captured_at,
                    pixels: Arc::new(pixels),
                    content_hash: fd.content_hash,
                    app_hint: app_hint.clone(),
                    window_title: window_title.clone(),
                });
            }
            // Loop: drain the queue, or sleep and try again if nothing changed.
        }
    }
}

/// Current time in unix-epoch milliseconds.
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

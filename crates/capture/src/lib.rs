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

/// Enumerates connected monitors as user-facing metadata. The raw `HMONITOR`s stay
/// inside the capture crate; Settings only needs the stable index/name/size fields.
pub fn enumerate_monitors() -> Vec<MonitorInfo> {
    monitors::enumerate().into_iter().map(|m| m.info).collect()
}

/// WGC-backed capture source. Owns the channel to the capture worker thread, the
/// monitor list, and a small queue of changed frames produced by the last cycle.
pub struct WgcCapture {
    monitors: Vec<MonitorInfo>,
    /// Per-monitor screen bounds (same index as the captured frame's `monitor_index`),
    /// used to map the foreground-window rect into each frame for PR3's `target_rect`.
    monitor_bounds: Vec<monitors::MonitorBounds>,
    config: CaptureConfig,
    req_tx: Mutex<mpsc::Sender<CaptureRequest>>,
    queue: VecDeque<CapturedFrame>,
}

/// Maps a foreground-window screen rect `(left, top, right, bottom)` into a monitor's
/// frame, normalized to `[0,1]` (origin top-left). Returns `None` unless the window's
/// centre lies on this monitor (so a window on another display yields no `target_rect`)
/// — pure, so it is unit-tested without any Win32 calls (`03 §3b`).
fn normalize_window_rect(
    win: (i32, i32, i32, i32),
    mon: monitors::MonitorBounds,
) -> Option<[f32; 4]> {
    let (wl, wt, wr, wb) = win;
    if mon.width <= 0 || mon.height <= 0 || wr <= wl || wb <= wt {
        return None;
    }
    let cx = (wl + wr) / 2;
    let cy = (wt + wb) / 2;
    if cx < mon.left || cx >= mon.left + mon.width || cy < mon.top || cy >= mon.top + mon.height {
        return None;
    }
    let (mw, mh) = (mon.width as f32, mon.height as f32);
    let nx = ((wl - mon.left) as f32 / mw).clamp(0.0, 1.0);
    let ny = ((wt - mon.top) as f32 / mh).clamp(0.0, 1.0);
    let nr = ((wr - mon.left) as f32 / mw).clamp(0.0, 1.0);
    let nb = ((wb - mon.top) as f32 / mh).clamp(0.0, 1.0);
    Some([nx, ny, (nr - nx).max(0.0), (nb - ny).max(0.0)])
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
            // Same enumeration order as the worker → indices align with monitor_index.
            monitor_bounds: monitors::monitor_bounds(),
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
            // Never capture our own window: it only contains app chrome (sidebar nav,
            // command palette) and a results pane that echoes other captures' chrome —
            // the dominant source of the PR3 'Deck'/'Recall' self-capture leak
            // (docs/AUDIT_0.2.0_PR3_2026-06-26.md). PID-based, so it can't mismatch a
            // third-party window merely titled "screensearch".
            if privacy::is_own_foreground_window() {
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
            // Foreground-window rect, read at the same instant as the app/title so the
            // target window is consistent with the frame's context (PR3 `target_rect`).
            let fg_rect = privacy::foreground_window_rect();

            let Some(frames) = self.capture_cycle().await else {
                return Ok(None); // worker gone
            };

            let captured_at = now_ms();
            for fd in frames {
                let Some(pixels) = RgbaImage::from_raw(fd.width, fd.height, fd.rgba) else {
                    continue;
                };
                // Map the foreground rect into this monitor's frame; `None` unless the
                // window is on this monitor → no positional suppression there.
                let target_rect = fg_rect.and_then(|w| {
                    self.monitor_bounds
                        .iter()
                        .find(|b| b.index == fd.monitor_index)
                        .and_then(|b| normalize_window_rect(w, *b))
                });
                self.queue.push_back(CapturedFrame {
                    monitor_index: fd.monitor_index,
                    width: fd.width,
                    height: fd.height,
                    captured_at,
                    pixels: Arc::new(pixels),
                    content_hash: fd.content_hash,
                    app_hint: app_hint.clone(),
                    window_title: window_title.clone(),
                    target_rect,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitors::MonitorBounds;

    fn mon(index: u32, left: i32, top: i32, width: i32, height: i32) -> MonitorBounds {
        MonitorBounds {
            index,
            left,
            top,
            width,
            height,
        }
    }

    #[test]
    fn window_rect_normalizes_within_its_monitor() {
        let m = mon(0, 0, 0, 1920, 1080);
        let r = normalize_window_rect((100, 50, 900, 650), m).expect("window is on this monitor");
        assert!((r[0] - 100.0 / 1920.0).abs() < 1e-5);
        assert!((r[1] - 50.0 / 1080.0).abs() < 1e-5);
        assert!((r[2] - 800.0 / 1920.0).abs() < 1e-5);
        assert!((r[3] - 600.0 / 1080.0).abs() < 1e-5);
    }

    #[test]
    fn window_on_another_monitor_is_none() {
        // Secondary monitor to the right; a window centred on it is not on monitor 0.
        let m0 = mon(0, 0, 0, 1920, 1080);
        // Window at x∈[2000,2800] → centre 2400, outside monitor 0.
        assert!(normalize_window_rect((2000, 100, 2800, 700), m0).is_none());
    }

    #[test]
    fn window_offset_maps_relative_to_monitor_origin() {
        // Secondary monitor with a non-zero origin: coords are relative to it.
        let m1 = mon(1, 1920, 0, 1920, 1080);
        let r = normalize_window_rect((1920, 0, 1920 + 960, 540), m1).expect("on monitor 1");
        assert!((r[0]).abs() < 1e-5, "left edge maps to 0");
        assert!((r[2] - 0.5).abs() < 1e-5, "half width");
        assert!((r[3] - 0.5).abs() < 1e-5, "half height");
    }

    #[test]
    fn degenerate_inputs_are_none() {
        assert!(normalize_window_rect((100, 100, 50, 50), mon(0, 0, 0, 1920, 1080)).is_none());
        assert!(normalize_window_rect((0, 0, 10, 10), mon(0, 0, 0, 0, 0)).is_none());
    }
}

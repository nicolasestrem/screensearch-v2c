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
use traits::{
    CaptureConfig, CaptureNowReceiver, CaptureNowRequest, CaptureSource, CaptureTrigger,
    CapturedFrame, MonitorInfo, Result,
};

mod diff;
mod events;
mod idle;
mod monitors;
mod privacy;
mod trigger;
mod wgc;

pub use idle::user_idle_ms;
#[cfg(windows)]
pub use privacy::foreground_window_rect;
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
    /// Event-driven capture state machine (debounce / rate-ceiling / idle edges).
    /// Driven only when `config.event_driven_enabled`; inert otherwise.
    machine: trigger::TriggerMachine,
    /// The Win32 input-hook thread, present only in event mode **and** when foreground
    /// capture is enabled (the sole hook-backed trigger after the 0.3.0 PR2 trim — the
    /// out-of-context `EVENT_SYSTEM_FOREGROUND` WinEvent hook). Idle needs no hook (it
    /// polls [`user_idle_ms`]). Dropped on stop/reload, tearing the thread down.
    events: Option<events::InputEventSource>,
    /// Mark-this-moment `capture_now` demands (`03 §7b`, D8). The kernel holds the
    /// sender on the live capture handle; the wait loop selects on this alongside the
    /// timer tick / event trigger. A demand captures Manual past the diff gate.
    capture_now_rx: CaptureNowReceiver,
    /// Set once the demand channel closes (the kernel dropped its sender — capture is
    /// stopping), so the demand `select!` arm goes inert instead of hot-looping.
    demand_closed: bool,
}

/// The reason [`WgcCapture::next_frame`] woke: a normal timer/event trigger, or a
/// mark-this-moment `capture_now` demand carrying its ack channel (`03 §7b`).
enum Wake {
    Trigger(CaptureTrigger),
    Demand(CaptureNowRequest),
}

/// How often (ms) the event-mode loop polls the trigger machine for idle edges and to
/// flush a settled debounce window. An internal responsiveness constant (like the WGC
/// cold-start sleep), not a user-facing threshold — the *durations* it observes
/// (debounce / idle / min-interval) are all configurable settings.
const EVENT_POLL_INTERVAL_MS: u64 = 250;

/// Builds the pure trigger config from the capture config.
fn trigger_config(c: &CaptureConfig) -> trigger::TriggerConfig {
    trigger::TriggerConfig {
        on_foreground: c.event_on_foreground,
        on_idle: c.event_on_idle,
        debounce_ms: i64::from(c.event_debounce_ms),
        min_interval_ms: i64::from(c.event_min_interval_ms),
        idle_threshold_ms: i64::from(c.event_idle_threshold_ms),
    }
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

/// The monitor a `capture_now` frame should be marked on (`03 §7b`, D8): the monitor
/// holding the foreground window (its rect resolves onto that monitor), falling back to
/// the **primary** monitor, then the first enumerated one. Deterministic — one
/// `capture_now` marks exactly one monitor's frame, never the ambiguous "first queued".
/// Pure, so it is unit-tested without any Win32 calls.
fn select_target_monitor(
    fg_rect: Option<(i32, i32, i32, i32)>,
    bounds: &[monitors::MonitorBounds],
    monitors: &[MonitorInfo],
) -> u32 {
    if let Some(rect) = fg_rect {
        if let Some(b) = bounds
            .iter()
            .find(|b| normalize_window_rect(rect, **b).is_some())
        {
            return b.index;
        }
    }
    monitors
        .iter()
        .find(|m| m.is_primary)
        .or_else(|| monitors.first())
        .map_or(0, |m| m.index)
}

impl WgcCapture {
    /// Spawns the capture worker, sets up per-monitor WGC sessions, and returns once
    /// the monitor list is known. Errors if there are no capturable monitors.
    ///
    /// `capture_now_rx` is the receiving half of the mark-this-moment demand channel
    /// (`03 §7b`, D8); the kernel keeps the sender on the live capture handle. Timer-only
    /// callers with no marks surface can pass a receiver whose sender they drop.
    pub fn new(config: CaptureConfig, capture_now_rx: CaptureNowReceiver) -> Result<Self> {
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

        let machine = trigger::TriggerMachine::new(trigger_config(&config));
        // Start the Win32 hook thread only when event mode is on and foreground capture
        // is enabled (the sole hook-backed trigger after the 0.3.0 PR2 trim). A failure
        // to install the hook is non-fatal: capture falls back to the event-mode
        // fallback timer + idle polling (the machine still runs).
        let events = if config.event_driven_enabled && config.event_on_foreground {
            match events::InputEventSource::start() {
                Ok(source) => Some(source),
                Err(e) => {
                    tracing::warn!(error = %e, "event-driven capture: input hooks unavailable; using fallback timer + idle only");
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            monitors,
            // Same enumeration order as the worker → indices align with monitor_index.
            monitor_bounds: monitors::monitor_bounds(),
            config,
            req_tx: Mutex::new(req_tx),
            queue: VecDeque::new(),
            machine,
            events,
            capture_now_rx,
            demand_closed: false,
        })
    }

    /// Waits for the next reason to capture: a normal timer tick / event trigger, or a
    /// mark-this-moment `capture_now` demand (`03 §7b`). In timer mode the interval sleep
    /// races the demand channel; in event mode the demand arm joins the trigger machine's
    /// `select!` (see [`Self::next_event_trigger`]).
    async fn next_wake(&mut self) -> Wake {
        if self.config.event_driven_enabled {
            return self.next_event_trigger().await;
        }
        let interval = Duration::from_millis(u64::from(self.config.interval_ms.max(1)));
        let sleep = tokio::time::sleep(interval);
        tokio::pin!(sleep);
        let demand_open = !self.demand_closed;
        let rx = &mut self.capture_now_rx;
        tokio::select! {
            _ = &mut sleep => Wake::Trigger(CaptureTrigger::Timer),
            req = recv_demand(rx, demand_open) => match req {
                Some(req) => Wake::Demand(req),
                None => {
                    // The demand channel closed (capture tearing down); go inert and pace
                    // as a normal timer cycle. `run_capture_loop` will exit on `stop`.
                    self.demand_closed = true;
                    Wake::Trigger(CaptureTrigger::Timer)
                }
            }
        }
    }

    /// Event mode's pacing: wait until the trigger machine decides to capture (a settled
    /// debounce, an idle/typing edge), the fallback interval elapses, or a `capture_now`
    /// demand arrives, and return the reason. Coexists with — does not replace — the timer
    /// path used in timer mode.
    async fn next_event_trigger(&mut self) -> Wake {
        let fallback_ms = u64::from(self.config.event_fallback_interval_ms.max(1));
        // Disjoint field borrows: the machine + demand receiver mutably, the hook source
        // shared. `events` is a `Copy` `Option<&_>` we clear locally the moment the hook
        // channel closes.
        let machine = &mut self.machine;
        let mut events = self.events.as_ref();
        let demand_open = !self.demand_closed;
        let rx = &mut self.capture_now_rx;

        let mut poll = tokio::time::interval(Duration::from_millis(EVENT_POLL_INTERVAL_MS));
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let fallback = tokio::time::sleep(Duration::from_millis(fallback_ms));
        tokio::pin!(fallback);

        let mut hook_died = false;
        let mut demand_closed_now = false;
        let wake = loop {
            // Not `biased`: a continuous event stream must not starve the fallback/poll
            // arms (tokio picks a ready arm pseudo-randomly).
            tokio::select! {
                kind = recv_event(events) => {
                    match kind {
                        Some(kind) => machine.on_input_event(kind, now_ms()),
                        // `recv()` also returns `None` when the channel closes — i.e. the
                        // hook thread exited after startup (e.g. `GetMessageW` error). Stop
                        // using the dead source so this arm goes `pending` instead of
                        // hot-looping; fallback + idle polling carry on. `hook_died` then
                        // retires it on `self` after the loop so later cycles don't re-arm.
                        None => {
                            if events.is_some() {
                                tracing::warn!("event-driven capture: hook thread exited; degrading to fallback timer + idle polling");
                                events = None;
                                hook_died = true;
                            }
                        }
                    }
                }
                _ = poll.tick() => {
                    if let Some(trigger) = machine.poll(now_ms(), idle_ms_now()) {
                        break Wake::Trigger(trigger);
                    }
                }
                _ = &mut fallback => break Wake::Trigger(CaptureTrigger::Timer),
                req = recv_demand(rx, demand_open && !demand_closed_now) => {
                    match req {
                        Some(req) => break Wake::Demand(req),
                        // Demand channel closed (capture tearing down): retire the arm so
                        // it goes pending instead of hot-looping (mirrors the hook arm).
                        None => demand_closed_now = true,
                    }
                }
            }
        };

        // Permanently retire a source whose hook thread has died: dropping it joins the
        // already-exited thread (instant) and makes future cycles see no source, so the
        // closed-channel wake + warning don't repeat every fallback interval.
        if hook_died {
            self.events = None;
        }
        if demand_closed_now {
            self.demand_closed = true;
        }
        wake
    }

    /// Asks the worker for one capture cycle and returns the changed frames. `bypass_for`
    /// is the monitor (if any) whose diff gate to bypass for a `capture_now` demand
    /// (`03 §7b`). `None` means the worker is gone (shutdown).
    async fn capture_cycle(&self, bypass_for: Option<u32>) -> Option<Vec<wgc::FrameData>> {
        let (resp_tx, resp_rx) = oneshot::channel();
        {
            let tx = self.req_tx.lock().expect("capture req sender poisoned");
            if tx
                .send(CaptureRequest {
                    resp: resp_tx,
                    bypass_for,
                })
                .is_err()
            {
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

            // Decide when to capture and why. Timer mode keeps the 0.2.0 interval cadence;
            // event mode waits for a user-activity trigger (with a fallback interval).
            // Either wait also races a mark-this-moment `capture_now` demand, which
            // captures `Manual` past the diff gate (`03 §7b`, D8).
            let (trigger, demand) = match self.next_wake().await {
                Wake::Trigger(t) => (t, None),
                Wake::Demand(req) => (CaptureTrigger::Manual, Some(req)),
            };

            // Privacy gate (03 §8) — still applies to a demand. A demanded frame from a
            // locked screen / our own window / an excluded app is denied **honestly** (the
            // mark hotkey surfaces the reason), never a silent no-op.
            if self.config.pause_on_lock && privacy::is_workstation_locked() {
                deny_demand(demand, "the workstation is locked");
                continue;
            }
            // Never capture our own windows: the main UI and Flow overlay only contain app
            // chrome and result rows that echo other captures. PID-based matching covers
            // every app-owned window, while the overlay's contentProtected window flag
            // closes the unfocused-visible capture race (D7).
            if privacy::is_own_foreground_window() {
                deny_demand(demand, "a ScreenSearch window is focused");
                continue;
            }
            let (app_hint, window_title) = privacy::foreground_context();
            if privacy::is_excluded(
                app_hint.as_deref(),
                window_title.as_deref(),
                &self.config.excluded_apps,
            ) {
                deny_demand(demand, "the focused app is excluded from capture");
                continue;
            }
            // Foreground-window rect, read at the same instant as the app/title so the
            // target window is consistent with the frame's context (PR3 `target_rect`).
            let fg_rect = privacy::foreground_window_rect();
            // Foreground window handle, so a live-window text provider (UIA) can detect a
            // focus change between capture and recognition and fall back to OCR (`07` #48).
            let fg_hwnd = privacy::foreground_hwnd();

            // For a demand, bypass the diff gate on the foreground-window monitor (primary
            // as fallback) so exactly one guaranteed frame is marked (`03 §7b`).
            let bypass_for = demand
                .as_ref()
                .map(|_| select_target_monitor(fg_rect, &self.monitor_bounds, &self.monitors));

            let Some(frames) = self.capture_cycle(bypass_for).await else {
                deny_demand(demand, "the capture worker is unavailable");
                return Ok(None); // worker gone
            };

            let captured_at = now_ms();
            let mut demanded_found = false;
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
                // Flag only the demanded monitor's frame so the kernel can hand its
                // `frame_id` back to the waiting `capture_now` caller (`03 §7b`).
                let is_demanded = bypass_for == Some(fd.monitor_index);
                demanded_found |= is_demanded;
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
                    foreground_hwnd: fg_hwnd,
                    trigger,
                    demanded: is_demanded,
                });
            }
            // Ack the demand once its frame is queued — the kernel then reads it back and
            // returns the mark's `frame_id`. An `Ok` guarantees the loop will observe the
            // demanded frame; report a miss honestly if the target produced nothing.
            if let Some(req) = demand {
                let _ = req.ack.send(if demanded_found {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "no frame captured for the foreground monitor"
                    ))
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

/// Awaits the next hook event, or never resolves when there is no hook source — so the
/// event `select!` arm is simply inert (idle/typing-pause + fallback still drive).
async fn recv_event(events: Option<&events::InputEventSource>) -> Option<trigger::InputEventKind> {
    match events {
        Some(source) => source.recv().await,
        None => std::future::pending().await,
    }
}

/// Awaits the next `capture_now` demand, or never resolves when the demand channel is
/// inactive/closed (`open == false`) — so the demand `select!` arm is inert without
/// hot-looping (mirrors [`recv_event`]).
async fn recv_demand(rx: &mut CaptureNowReceiver, open: bool) -> Option<CaptureNowRequest> {
    if open {
        rx.recv().await
    } else {
        std::future::pending().await
    }
}

/// Sends an honest failure back to a waiting `capture_now` caller (`03 §7b`, D8) — never
/// a silent no-op. A dropped receiver (the caller already timed out) is ignored.
fn deny_demand(demand: Option<CaptureNowRequest>, reason: &str) {
    if let Some(req) = demand {
        let _ = req.ack.send(Err(anyhow::anyhow!(reason.to_string())));
    }
}

/// Current user idle time in ms (since last keyboard/mouse input), `0` if unreadable
/// (treated as active). Privacy-safe: `GetLastInputInfo` exposes only a timestamp,
/// never key content.
fn idle_ms_now() -> i64 {
    user_idle_ms().unwrap_or(0) as i64
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

    fn info(index: u32, is_primary: bool) -> MonitorInfo {
        MonitorInfo {
            index,
            name: format!("Monitor {index}"),
            width: 1920,
            height: 1080,
            is_primary,
        }
    }

    #[test]
    fn target_monitor_is_the_one_holding_the_foreground_window() {
        // Monitor 0 primary at origin; monitor 1 to its right.
        let bounds = [mon(0, 0, 0, 1920, 1080), mon(1, 1920, 0, 1920, 1080)];
        let monitors = [info(0, true), info(1, false)];
        // A window centred on the secondary monitor → target = monitor 1.
        let rect = Some((2000, 100, 2800, 700));
        assert_eq!(select_target_monitor(rect, &bounds, &monitors), 1);
        // A window on the primary → target = monitor 0.
        let rect = Some((100, 100, 900, 700));
        assert_eq!(select_target_monitor(rect, &bounds, &monitors), 0);
    }

    #[test]
    fn target_monitor_falls_back_to_primary_then_first() {
        let bounds = [mon(0, 0, 0, 1920, 1080), mon(1, 1920, 0, 1920, 1080)];
        // No foreground rect (minimized / unresolved) → the primary monitor (index 1 here).
        let monitors = [info(0, false), info(1, true)];
        assert_eq!(select_target_monitor(None, &bounds, &monitors), 1);
        // No primary flagged at all → the first enumerated monitor.
        let monitors = [info(3, false), info(4, false)];
        assert_eq!(select_target_monitor(None, &bounds, &monitors), 3);
        // A rect on no monitor also falls back to primary.
        let monitors = [info(0, false), info(1, true)];
        let offscreen = Some((10_000, 10_000, 10_800, 10_700));
        assert_eq!(select_target_monitor(offscreen, &bounds, &monitors), 1);
    }
}

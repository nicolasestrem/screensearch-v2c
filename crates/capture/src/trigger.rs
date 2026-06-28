//! The pure event-driven-capture trigger state machine (`docs/0.2.0.md` event-driven
//! capture; `07` #47).
//!
//! This is the testable core of event-driven capture: **no Win32, no `unsafe`, no
//! real clock**. The Win32 hook thread ([`crate::events`]) feeds it discrete input
//! events ([`InputEventKind`]) and the capture loop polls it with the current idle
//! time; it decides *when* to capture and with *which* [`CaptureTrigger`], applying:
//!
//! - **trailing-edge debounce** — a burst of discrete events (e.g. tabbing through
//!   several windows) collapses to one capture fired once the burst settles
//!   (`event_debounce_ms`);
//! - a **global min-interval rate ceiling** — no two event-driven captures closer
//!   than `event_min_interval_ms`, so a mix of triggers can't storm;
//! - **per-trigger enable gating** — a disabled trigger never fires;
//! - **edge-detected idle / typing-pause** — derived purely from the polled idle time
//!   (`GetLastInputInfo`, never key content), each firing once per quiet period.
//!
//! Time is the caller-supplied `now_ms` so the whole thing is deterministic in tests.

use traits::CaptureTrigger;

/// A discrete user-activity event from the Win32 hook thread ([`crate::events`]).
/// Idle / typing-pause are *not* here — they are derived from the polled idle time,
/// not pushed as events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputEventKind {
    /// The foreground window / app changed (`EVENT_SYSTEM_FOREGROUND`).
    Foreground,
    /// The clipboard changed (`WM_CLIPBOARDUPDATE`) — change event only, no contents.
    Clipboard,
}

/// Which triggers are enabled and the timing thresholds, in milliseconds. Built from
/// [`traits::CaptureConfig`]'s `event_*` fields by [`crate::WgcCapture`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct TriggerConfig {
    pub on_foreground: bool,
    pub on_clipboard: bool,
    pub on_idle: bool,
    pub on_typing_pause: bool,
    /// Trailing-edge quiet window that collapses a discrete-event burst into one
    /// capture (`event_debounce_ms`).
    pub debounce_ms: i64,
    /// Minimum gap between any two event-driven captures (`event_min_interval_ms`).
    pub min_interval_ms: i64,
    /// Idle time that counts as a typing pause (`event_typing_pause_ms`).
    pub typing_pause_ms: i64,
    /// Idle time that counts as "gone idle" (`event_idle_threshold_ms`).
    pub idle_threshold_ms: i64,
}

/// The trigger state machine. See the module docs for the policy it enforces.
pub(crate) struct TriggerMachine {
    cfg: TriggerConfig,
    /// A discrete trigger waiting out its debounce window: `(trigger, latest-event-ms)`.
    pending: Option<(CaptureTrigger, i64)>,
    /// When the last capture was emitted (the min-interval rate ceiling).
    last_emit_ms: Option<i64>,
    /// Whether the current idle period already emitted an `Idle` capture (re-armed
    /// once the user is active again).
    fired_idle: bool,
    /// Whether the current quiet period already emitted a `TypingPause` capture.
    fired_typing_pause: bool,
}

impl TriggerMachine {
    pub(crate) fn new(cfg: TriggerConfig) -> Self {
        Self {
            cfg,
            pending: None,
            last_emit_ms: None,
            fired_idle: false,
            fired_typing_pause: false,
        }
    }

    /// Record a discrete hook event. An enabled kind (re)starts the debounce window;
    /// a disabled kind is ignored. A new event during an existing debounce window
    /// replaces the pending trigger (the latest activity wins) — capture coalesces to
    /// one regardless.
    pub(crate) fn on_input_event(&mut self, kind: InputEventKind, now_ms: i64) {
        let trigger = match kind {
            InputEventKind::Foreground if self.cfg.on_foreground => {
                CaptureTrigger::ForegroundChange
            }
            InputEventKind::Clipboard if self.cfg.on_clipboard => CaptureTrigger::ClipboardChange,
            // Disabled kind → ignored entirely (never even arms the debounce).
            _ => return,
        };
        self.pending = Some((trigger, now_ms));
    }

    /// Tick the machine with the current time and idle duration (ms since last input).
    /// Returns `Some(trigger)` if a capture should fire now. Call on every loop wake.
    pub(crate) fn poll(&mut self, now_ms: i64, idle_ms: i64) -> Option<CaptureTrigger> {
        // Re-arm idle / typing-pause once the user is active again (idle dropped back
        // below each threshold), so the next quiet period fires afresh.
        if idle_ms < self.cfg.typing_pause_ms {
            self.fired_typing_pause = false;
        }
        if idle_ms < self.cfg.idle_threshold_ms {
            self.fired_idle = false;
        }

        // Idle (higher threshold) takes priority over typing-pause; emitting Idle also
        // marks typing-pause fired so a single growing-idle poll can't double-fire. The
        // `fired_*` flags are set only on a SUCCESSFUL emit — if the min-interval rate
        // ceiling blocks it, the edge stays armed and retries on the next poll, so the
        // ceiling delays rather than consuming the once-per-quiet-period firing.
        if self.cfg.on_idle && idle_ms >= self.cfg.idle_threshold_ms && !self.fired_idle {
            if let Some(t) = self.try_emit(CaptureTrigger::Idle, now_ms) {
                self.fired_idle = true;
                self.fired_typing_pause = true;
                return Some(t);
            }
        } else if self.cfg.on_typing_pause
            && idle_ms >= self.cfg.typing_pause_ms
            && !self.fired_typing_pause
        {
            if let Some(t) = self.try_emit(CaptureTrigger::TypingPause, now_ms) {
                self.fired_typing_pause = true;
                return Some(t);
            }
        }

        // Flush a discrete event once its debounce window has settled. Clear `pending`
        // only on a successful emit — a min-interval block keeps the (single) pending
        // slot so the event is retried next poll, not dropped (a newer event of either
        // kind still overwrites it, so no unbounded backlog builds up).
        if let Some((trigger, latest_event_ms)) = self.pending {
            if now_ms - latest_event_ms >= self.cfg.debounce_ms {
                if let Some(t) = self.try_emit(trigger, now_ms) {
                    self.pending = None;
                    return Some(t);
                }
            }
        }
        None
    }

    /// Emit `trigger` unless the global min-interval rate ceiling blocks it. The caller
    /// keeps the trigger armed on a block and retries on the next poll, so the rate
    /// ceiling **delays** a capture rather than dropping it; only a single discrete
    /// event is ever pending (a newer event overwrites it), so no backlog can build up.
    fn try_emit(&mut self, trigger: CaptureTrigger, now_ms: i64) -> Option<CaptureTrigger> {
        if let Some(last) = self.last_emit_ms {
            if now_ms - last < self.cfg.min_interval_ms {
                return None;
            }
        }
        self.last_emit_ms = Some(now_ms);
        Some(trigger)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> TriggerConfig {
        TriggerConfig {
            on_foreground: true,
            on_clipboard: true,
            on_idle: true,
            on_typing_pause: true,
            debounce_ms: 500,
            min_interval_ms: 1000,
            typing_pause_ms: 1500,
            idle_threshold_ms: 5000,
        }
    }

    #[test]
    fn foreground_event_emits_after_debounce() {
        let mut m = TriggerMachine::new(cfg());
        m.on_input_event(InputEventKind::Foreground, 0);
        assert_eq!(m.poll(100, 0), None, "still inside the debounce window");
        assert_eq!(
            m.poll(500, 0),
            Some(CaptureTrigger::ForegroundChange),
            "debounce elapsed → one capture"
        );
        assert_eq!(m.poll(600, 0), None, "nothing pending after the flush");
    }

    #[test]
    fn burst_of_events_collapses_to_one_capture() {
        let mut m = TriggerMachine::new(cfg());
        m.on_input_event(InputEventKind::Foreground, 0);
        m.on_input_event(InputEventKind::Foreground, 200);
        m.on_input_event(InputEventKind::Foreground, 400); // each refreshes debounce
        assert_eq!(m.poll(450, 0), None, "450-400=50 < debounce");
        assert_eq!(
            m.poll(900, 0),
            Some(CaptureTrigger::ForegroundChange),
            "900-400=500 → exactly one capture for the burst"
        );
        assert_eq!(m.poll(1000, 0), None, "burst already collapsed");
    }

    #[test]
    fn disabled_foreground_never_emits() {
        let mut m = TriggerMachine::new(TriggerConfig {
            on_foreground: false,
            ..cfg()
        });
        m.on_input_event(InputEventKind::Foreground, 0);
        assert_eq!(m.poll(1000, 0), None);
        assert_eq!(m.poll(5000, 0), None);
    }

    #[test]
    fn clipboard_event_emits_when_enabled() {
        let mut m = TriggerMachine::new(cfg());
        m.on_input_event(InputEventKind::Clipboard, 0);
        assert_eq!(m.poll(500, 0), Some(CaptureTrigger::ClipboardChange));
    }

    #[test]
    fn min_interval_suppresses_a_second_capture() {
        let mut m = TriggerMachine::new(TriggerConfig {
            debounce_ms: 100,
            min_interval_ms: 1000,
            ..cfg()
        });
        m.on_input_event(InputEventKind::Foreground, 0);
        assert_eq!(
            m.poll(100, 0),
            Some(CaptureTrigger::ForegroundChange),
            "first capture at 100"
        );
        m.on_input_event(InputEventKind::Clipboard, 200);
        assert_eq!(
            m.poll(300, 0),
            None,
            "300-100=200 < min_interval → rate-ceiling suppressed"
        );
        m.on_input_event(InputEventKind::Clipboard, 1100);
        assert_eq!(
            m.poll(1200, 0),
            Some(CaptureTrigger::ClipboardChange),
            "1200-100=1100 >= min_interval → allowed"
        );
    }

    #[test]
    fn typing_pause_fires_once_per_quiet_period() {
        let mut m = TriggerMachine::new(TriggerConfig {
            on_idle: false,
            ..cfg()
        });
        assert_eq!(m.poll(0, 0), None, "user active");
        assert_eq!(
            m.poll(1500, 1500),
            Some(CaptureTrigger::TypingPause),
            "idle crossed the typing-pause threshold"
        );
        assert_eq!(m.poll(1600, 1600), None, "still quiet → no re-fire");
        assert_eq!(m.poll(3000, 3000), None, "still quiet → no re-fire");
        assert_eq!(m.poll(3100, 0), None, "user active again → re-arms");
        assert_eq!(
            m.poll(5000, 1500),
            Some(CaptureTrigger::TypingPause),
            "new quiet period fires again"
        );
    }

    #[test]
    fn idle_takes_priority_and_suppresses_same_period_typing_pause() {
        let mut m = TriggerMachine::new(cfg());
        // Jump straight past the idle threshold (long absence; first poll after).
        assert_eq!(
            m.poll(5000, 5000),
            Some(CaptureTrigger::Idle),
            "idle threshold crossed → Idle, not TypingPause"
        );
        assert_eq!(
            m.poll(5100, 6000),
            None,
            "already fired idle for this period"
        );
    }

    #[test]
    fn idle_and_typing_pause_disabled_never_emit_from_polling() {
        let mut m = TriggerMachine::new(TriggerConfig {
            on_idle: false,
            on_typing_pause: false,
            ..cfg()
        });
        assert_eq!(m.poll(2000, 2000), None);
        assert_eq!(m.poll(6000, 6000), None);
    }

    #[test]
    fn idle_poll_while_active_is_quiet() {
        let mut m = TriggerMachine::new(cfg());
        assert_eq!(m.poll(0, 0), None);
        assert_eq!(m.poll(1000, 50), None, "tiny idle, nothing pending");
    }

    #[test]
    fn pending_event_retries_after_min_interval_block() {
        // The rate ceiling must DELAY a settled discrete event, not drop it: a window
        // switch within the min-interval window is still captured once the window opens,
        // rather than lost until the fallback timer.
        let mut m = TriggerMachine::new(TriggerConfig {
            debounce_ms: 100,
            min_interval_ms: 1000,
            ..cfg()
        });
        m.on_input_event(InputEventKind::Foreground, 0);
        assert_eq!(
            m.poll(100, 0),
            Some(CaptureTrigger::ForegroundChange),
            "first capture sets the rate-ceiling clock"
        );
        m.on_input_event(InputEventKind::Clipboard, 200);
        assert_eq!(
            m.poll(400, 0),
            None,
            "debounce settled (400-200>=100) but min_interval blocks (400-100<1000)"
        );
        assert_eq!(
            m.poll(1200, 0),
            Some(CaptureTrigger::ClipboardChange),
            "the blocked event is retried once min_interval allows, not silently dropped"
        );
    }

    #[test]
    fn idle_retries_after_min_interval_block() {
        // Same contract for the idle edge: a min-interval block must not consume the
        // once-per-quiet-period firing — it retries until it actually emits.
        let mut m = TriggerMachine::new(TriggerConfig {
            on_typing_pause: false,
            debounce_ms: 100,
            min_interval_ms: 1000,
            idle_threshold_ms: 5000,
            ..cfg()
        });
        m.on_input_event(InputEventKind::Foreground, 0);
        assert_eq!(m.poll(100, 0), Some(CaptureTrigger::ForegroundChange));
        assert_eq!(
            m.poll(500, 5000),
            None,
            "idle crossed the threshold but min_interval blocks"
        );
        assert_eq!(
            m.poll(1200, 6000),
            Some(CaptureTrigger::Idle),
            "the blocked idle edge retries once min_interval allows"
        );
    }
}

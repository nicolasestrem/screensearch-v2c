//! Per-app UIA circuit breaker — pure, no Win32, unit-tested in CI.
//!
//! ## Why
//! A UIA tree walk is synchronous cross-process COM marshalled onto the *target* app's UI
//! thread. Chromium/Electron apps (Chrome, Edge, Codex, Claude Desktop, VS Code, Discord…)
//! keep large accessibility trees, so a walk can consume the whole latency budget or time out
//! — and, worse, the first walk flips the whole browser process into accessibility mode
//! (sticky for that process's lifetime). We can't undo that stickiness from here, but we can
//! stop *re-hammering* an app that has repeatedly struggled: after
//! [`BREAKER_CONSECUTIVE_BAD`] consecutive bad walks against one app, the breaker opens for
//! that app and every subsequent frame goes straight to OCR for [`BREAKER_COOLDOWN_MS`]. A
//! single good walk resets the streak. State is keyed by the frame's foreground `app_hint`.
//!
//! Per the user decision for the 0.3.0 surface-reduction arc these are **named constants, not
//! settings** (recorded in `specs/07_KNOWN_GAPS.md`).
//!
//! Time is injected (`now_ms`) so every decision is deterministic and testable; the composite
//! ([`crate::UiaTextProvider`]'s caller in `src-tauri`) supplies the wall clock.

use std::collections::HashMap;

/// Consecutive bad walks against one app before its breaker opens.
pub const BREAKER_CONSECUTIVE_BAD: u32 = 3;
/// How long the breaker stays open (UIA skipped → OCR carries that app's frames). 30 min.
pub const BREAKER_COOLDOWN_MS: u64 = 30 * 60 * 1000;

/// How one UIA walk ended, as the breaker's [`signal`] classifier sees it. Built by the
/// composite from the worker's reply (`crate::worker::WalkReply`) and the caller-side timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkEnd {
    /// The walk returned. `ok` = it produced a usable [`traits::OcrResult`]; `over_budget` =
    /// it consumed at least the full latency budget (the target app was heavy) — true even
    /// for an `ok` walk, because that walk still hammered the target's UI thread.
    Completed { ok: bool, over_budget: bool },
    /// The caller's hard timeout (2× budget) fired before the worker replied — the worst case.
    HardTimeout,
    /// The request never reached the target: a walk was already in flight (busy skip) or the
    /// bounded queue was full. Neutral — it says nothing about the target's health.
    Busy,
}

/// The breaker's view of a walk: does it count against the app, reset its streak, or neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiaSignal {
    /// A within-budget successful walk — the target answered promptly. Resets the streak.
    Good,
    /// The target struggled: a hard timeout, or a walk that consumed the full budget (even a
    /// successful one). Counts toward opening the breaker.
    Bad,
    /// Neither counts nor resets: the walk never touched the target (busy), or it returned a
    /// benign within-budget `Err` (thin yield / wrong-monitor / hwnd-changed bail) — the
    /// target answered promptly, so it isn't "bad", but it produced no text to call "good".
    Neutral,
}

/// Classifies one walk's outcome for the breaker (pure).
pub fn signal(end: WalkEnd) -> UiaSignal {
    match end {
        WalkEnd::Completed {
            over_budget: true, ..
        } => UiaSignal::Bad,
        WalkEnd::HardTimeout => UiaSignal::Bad,
        WalkEnd::Completed {
            ok: true,
            over_budget: false,
        } => UiaSignal::Good,
        // Within-budget error: the target answered quickly but we got no usable text. We
        // can't distinguish a benign thin-yield from a real element failure without typed
        // sub-errors, and both mean the target wasn't slow — so neither count nor reset.
        WalkEnd::Completed {
            ok: false,
            over_budget: false,
        } => UiaSignal::Neutral,
        WalkEnd::Busy => UiaSignal::Neutral,
    }
}

/// A breaker state change worth logging exactly once (the composite logs it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerTransition {
    /// The breaker just opened for this app — UIA suppressed to OCR for the cooldown.
    Opened,
    /// The cooldown expired and the first walk since ran — UIA is live again for this app.
    Closed,
}

#[derive(Debug, Default, Clone, Copy)]
struct PerApp {
    /// Consecutive bad walks since the last good/open. Reset to 0 on Good, on open, on expiry.
    consecutive_bad: u32,
    /// `Some(t)` while open; UIA is skipped for this app until `now_ms >= t`.
    open_until_ms: Option<u64>,
}

/// Per-app UIA circuit breaker. Not `Sync`; the composite wraps it in a `Mutex`.
#[derive(Debug)]
pub struct AppBreaker {
    apps: HashMap<String, PerApp>,
    threshold: u32,
    cooldown_ms: u64,
}

impl AppBreaker {
    pub fn new(threshold: u32, cooldown_ms: u64) -> Self {
        Self {
            apps: HashMap::new(),
            threshold: threshold.max(1),
            cooldown_ms,
        }
    }

    /// Whether UIA is currently suppressed for `app` (breaker open). Pure read: an expired
    /// cooldown reads as closed (`false`); the entry is actually cleared and the `Closed`
    /// transition emitted by the next [`record`](Self::record) call (which runs only when the
    /// breaker is closed, i.e. after a real walk).
    pub fn is_open(&self, app: &str, now_ms: u64) -> bool {
        matches!(
            self.apps.get(app),
            Some(PerApp { open_until_ms: Some(t), .. }) if now_ms < *t
        )
    }

    /// Feed one walk's [`signal`] for `app`. Returns `Some(transition)` exactly when the
    /// breaker state changed, so the caller logs each open/close once. Called only when the
    /// breaker was closed for `app` (an open breaker skips the walk).
    pub fn record(
        &mut self,
        app: &str,
        signal: UiaSignal,
        now_ms: u64,
    ) -> Option<BreakerTransition> {
        let mut transition = None;

        // A cooldown that lapsed since it was set: clear it and report the resumption once.
        // `is_open` returned false above (now >= t), so we're genuinely running walks again.
        if let Some(p) = self.apps.get_mut(app) {
            if p.open_until_ms.is_some_and(|t| now_ms >= t) {
                p.open_until_ms = None;
                p.consecutive_bad = 0;
                transition = Some(BreakerTransition::Closed);
            }
        }

        match signal {
            UiaSignal::Neutral => {}
            UiaSignal::Good => {
                if let Some(p) = self.apps.get_mut(app) {
                    p.consecutive_bad = 0;
                }
            }
            UiaSignal::Bad => {
                let p = self.apps.entry(app.to_string()).or_default();
                p.consecutive_bad += 1;
                if p.consecutive_bad >= self.threshold {
                    p.open_until_ms = Some(now_ms.saturating_add(self.cooldown_ms));
                    p.consecutive_bad = 0;
                    // Opening supersedes a same-call Closed (a walk can't both resume service
                    // and immediately re-trip on a single result in practice, but be explicit).
                    transition = Some(BreakerTransition::Opened);
                }
            }
        }

        transition
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: u32 = BREAKER_CONSECUTIVE_BAD; // 3
    const COOL: u64 = BREAKER_COOLDOWN_MS;

    fn breaker() -> AppBreaker {
        AppBreaker::new(T, COOL)
    }

    #[test]
    fn signal_bad_on_hard_timeout_and_over_budget() {
        assert_eq!(signal(WalkEnd::HardTimeout), UiaSignal::Bad);
        assert_eq!(
            signal(WalkEnd::Completed {
                ok: true,
                over_budget: true
            }),
            UiaSignal::Bad
        );
        assert_eq!(
            signal(WalkEnd::Completed {
                ok: false,
                over_budget: true
            }),
            UiaSignal::Bad
        );
    }

    #[test]
    fn signal_good_within_budget_ok() {
        assert_eq!(
            signal(WalkEnd::Completed {
                ok: true,
                over_budget: false
            }),
            UiaSignal::Good
        );
    }

    #[test]
    fn signal_neutral_on_busy_and_within_budget_err() {
        assert_eq!(signal(WalkEnd::Busy), UiaSignal::Neutral);
        assert_eq!(
            signal(WalkEnd::Completed {
                ok: false,
                over_budget: false
            }),
            UiaSignal::Neutral
        );
    }

    #[test]
    fn breaker_opens_after_three_consecutive_bad() {
        let mut b = breaker();
        assert_eq!(b.record("chrome", UiaSignal::Bad, 0), None);
        assert!(!b.is_open("chrome", 0));
        assert_eq!(b.record("chrome", UiaSignal::Bad, 1), None);
        assert!(!b.is_open("chrome", 1));
        // The 3rd consecutive bad opens the breaker.
        assert_eq!(
            b.record("chrome", UiaSignal::Bad, 2),
            Some(BreakerTransition::Opened)
        );
        assert!(b.is_open("chrome", 2));
        // Still open partway through the cooldown.
        assert!(b.is_open("chrome", COOL / 2));
    }

    #[test]
    fn breaker_good_resets_the_streak() {
        let mut b = breaker();
        b.record("chrome", UiaSignal::Bad, 0);
        b.record("chrome", UiaSignal::Bad, 1);
        // A good walk clears the two bad walks…
        assert_eq!(b.record("chrome", UiaSignal::Good, 2), None);
        assert!(!b.is_open("chrome", 2));
        // …so it now takes a fresh run of 3 bad walks to open (the 3rd trips it).
        assert_eq!(b.record("chrome", UiaSignal::Bad, 3), None);
        assert_eq!(b.record("chrome", UiaSignal::Bad, 4), None);
        assert_eq!(
            b.record("chrome", UiaSignal::Bad, 5),
            Some(BreakerTransition::Opened),
            "3rd bad after reset opens it"
        );
        assert!(b.is_open("chrome", 5));
    }

    #[test]
    fn breaker_neutral_neither_counts_nor_resets() {
        let mut b = breaker();
        b.record("chrome", UiaSignal::Bad, 0);
        b.record("chrome", UiaSignal::Neutral, 1); // ignored
        b.record("chrome", UiaSignal::Bad, 2);
        // Two bad + a neutral in between = 2 bad; not yet open.
        assert!(!b.is_open("chrome", 2));
        assert_eq!(
            b.record("chrome", UiaSignal::Bad, 3),
            Some(BreakerTransition::Opened)
        );
    }

    #[test]
    fn breaker_cooldown_expiry_closes_and_resets() {
        let mut b = breaker();
        b.record("chrome", UiaSignal::Bad, 0);
        b.record("chrome", UiaSignal::Bad, 1);
        b.record("chrome", UiaSignal::Bad, 2); // opens at t=2, open_until = 2 + COOL
        assert!(b.is_open("chrome", 2));
        // Just before expiry: still open.
        assert!(b.is_open("chrome", 2 + COOL - 1));
        // At/after expiry: reads closed…
        let expiry = 2 + COOL;
        assert!(!b.is_open("chrome", expiry));
        // …and the first walk after expiry reports the resumption exactly once.
        assert_eq!(
            b.record("chrome", UiaSignal::Good, expiry),
            Some(BreakerTransition::Closed)
        );
        // Subsequent walks don't re-report Closed.
        assert_eq!(b.record("chrome", UiaSignal::Good, expiry + 1), None);
        // And after expiry the streak is fresh: 3 more bad to re-open.
        assert_eq!(b.record("chrome", UiaSignal::Bad, expiry + 2), None);
        assert_eq!(b.record("chrome", UiaSignal::Bad, expiry + 3), None);
        assert_eq!(
            b.record("chrome", UiaSignal::Bad, expiry + 4),
            Some(BreakerTransition::Opened)
        );
    }

    #[test]
    fn breaker_isolates_apps() {
        let mut b = breaker();
        b.record("chrome", UiaSignal::Bad, 0);
        b.record("chrome", UiaSignal::Bad, 1);
        b.record("chrome", UiaSignal::Bad, 2); // chrome opens
                                               // A different app is unaffected.
        assert!(b.is_open("chrome", 2));
        assert!(!b.is_open("codex", 2));
        assert_eq!(b.record("codex", UiaSignal::Bad, 2), None);
        assert!(!b.is_open("codex", 2));
    }

    #[test]
    fn breaker_reports_transitions_once() {
        let mut b = breaker();
        // Opened fires only on the Nth bad, not before or after.
        assert_eq!(b.record("a", UiaSignal::Bad, 0), None);
        assert_eq!(b.record("a", UiaSignal::Bad, 1), None);
        assert_eq!(
            b.record("a", UiaSignal::Bad, 2),
            Some(BreakerTransition::Opened)
        );
        // While open we don't call record (is_open would gate the walk); simulate the resume.
        let expiry = 2 + COOL;
        assert_eq!(
            b.record("a", UiaSignal::Neutral, expiry),
            Some(BreakerTransition::Closed)
        );
        assert_eq!(b.record("a", UiaSignal::Neutral, expiry + 1), None);
    }

    #[test]
    fn threshold_is_clamped_to_at_least_one() {
        let mut b = AppBreaker::new(0, COOL);
        // A zero threshold would open on every bad walk; clamp keeps it sane (opens on 1st bad).
        assert_eq!(
            b.record("x", UiaSignal::Bad, 0),
            Some(BreakerTransition::Opened)
        );
    }
}

//! Pure decisions for the UIA tree walk — no Win32, so unit-tested in CI. The live walk
//! ([`crate::worker`]) reads each element's control type / password / offscreen flags and
//! delegates the "emit this element's text?" and "split it into word spans" decisions here.

// UIA control-type IDs (windows-rs `UIA_*ControlTypeId.0`), kept as raw `i32` so this
// module stays portable and testable without the `windows` crate. Source: the verified
// 0.62 reference (UIA_PaneControlTypeId = 50033, etc.).
const PANE: i32 = 50033;
const WINDOW: i32 = 50032;
const GROUP: i32 = 50026;
const TOOLBAR: i32 = 50021;
const MENUBAR: i32 = 50010;
const TREE: i32 = 50023;
const LIST: i32 = 50008;
const TABLE: i32 = 50036;
const SCROLLBAR: i32 = 50014;
const TAB: i32 = 50018; // the Tab container; individual tabs are TabItem (50019)
const SEPARATOR: i32 = 50038;
const TITLEBAR: i32 = 50037;
// Content controls whose body text lives in a TextPattern (documents / editable text).
const DOCUMENT: i32 = 50030;
const EDIT: i32 = 50004;
const TEXT: i32 = 50020;

/// Pure containers carry no body text of their own — their children do. The walk still
/// descends into them; it just emits nothing for the container element itself.
pub(crate) fn is_container(control_type: i32) -> bool {
    matches!(
        control_type,
        PANE | WINDOW
            | GROUP
            | TOOLBAR
            | MENUBAR
            | TREE
            | LIST
            | TABLE
            | SCROLLBAR
            | SEPARATOR
            | TAB
            | TITLEBAR
    )
}

/// Whether an element should contribute its own text to `content_text`. Never read a
/// password field (UIA can expose what OCR can't see) or an offscreen / occluded element
/// (preserve OCR's "only what was visible" parity), and skip pure containers.
pub(crate) fn should_emit(control_type: i32, is_password: bool, is_offscreen: bool) -> bool {
    !is_password && !is_offscreen && !is_container(control_type)
}

/// Whether an element's control type is one whose text is worth reading via the **live,
/// uncacheable, cross-process** `TextPattern` visible ranges: documents and editable/text
/// controls. For every other control type, `Name`/`Value` already cover the text and a
/// `TextPattern` probe would just be an extra provider round-trip — the cost the hang fix
/// bounds (`07` #71). Gating by control type means non-text nodes skip the probe entirely.
pub(crate) fn control_type_wants_textpattern(control_type: i32) -> bool {
    matches!(control_type, DOCUMENT | EDIT | TEXT)
}

/// Policy controlling which capture triggers run UIA, seeded from settings (`03 §3b`).
#[derive(Debug, Clone, Copy)]
pub struct UiaTriggerPolicy {
    /// When `true`, even high-frequency interactive triggers (scroll/click) run UIA; when
    /// `false` (the default), they fall back to OCR (`capture.uia_run_on_interactive`, `07`
    /// #71). Default-off because a UIA walk during scroll is what froze Chromium/Electron.
    pub run_on_interactive: bool,
}

/// Whether a capture trigger should run the (expensive) UIA tree walk. High-frequency
/// interactive triggers — `ScrollStop` and `Click` — are gated off by default: each fires a
/// fresh full-subtree walk against the foreground app, and on a large Chromium/Electron a11y
/// tree that storm of synchronous cross-process calls freezes the target's UI thread ("Not
/// responding"; scrolling is the exact reproducer). Gated frames fall back to OCR (the
/// captured bitmap), which never touches the target app. `policy.run_on_interactive` opts
/// back into the legacy behavior. Every other trigger is low-frequency enough that one
/// bounded walk is safe. The match is exhaustive so a new `CaptureTrigger` forces a decision.
///
/// Residual at the `Timer` arm: the default timer-only capture path (event-driven capture
/// off) tags *every* frame `Timer`, so this gate alone would still walk the tree on a timer
/// tick that lands mid-scroll — bounded now (control view + node/`TextPattern` caps + the
/// latency budget) but still synchronous cross-process COM against the foreground app. That
/// residual is handled separately by [`input_gate_skips_uia`], which routes `Timer` frames
/// captured during active input to OCR; the deeper single-round-trip `FindAllBuildCache`
/// rewrite that would remove the per-node cost entirely is tracked in `07` #71.
pub fn trigger_runs_uia(trigger: traits::CaptureTrigger, policy: UiaTriggerPolicy) -> bool {
    use traits::CaptureTrigger::{
        Click, ClipboardChange, ForegroundChange, Idle, Manual, ScrollStop, Timer, TypingPause,
    };
    match trigger {
        ScrollStop | Click => policy.run_on_interactive,
        // `Timer` is additionally input-gated by `input_gate_skips_uia` (`07` #71) — see above.
        Timer | Idle | TypingPause | ForegroundChange | ClipboardChange | Manual => true,
    }
}

/// Whether to skip the UIA walk for this frame because the user is *actively* producing
/// input. `ms_since_last_input` is the system-wide idle time (Win32 `GetLastInputInfo`);
/// `suppress_window_ms` is `capture.uia_suppress_during_input_ms` (`0` disables the gate).
///
/// This closes the gap [`trigger_runs_uia`] alone leaves: in the default timer-only capture
/// path (event-driven capture off) *every* frame is tagged `Timer`, so the scroll/click gate
/// is inert and a `Timer` tick can still land mid-scroll on a heavy Chromium/Electron a11y
/// tree — the exact freeze (`07` #71). So **only `Timer`** is input-gated: it is the periodic
/// trigger that fires blind to user activity. Every other trigger is exempt — `ScrollStop`/
/// `Click` are already handled by [`trigger_runs_uia`]; `ForegroundChange`/`ClipboardChange`/
/// `Manual` must run UIA *because* the user just acted (capture the new state); `Idle`/
/// `TypingPause` fire only after input has quiesced. When the user has opted into UIA during
/// interaction (`policy.run_on_interactive`), the gate is bypassed, so that one knob coherently
/// governs all "run UIA while interacting" behavior.
pub fn input_gate_skips_uia(
    trigger: traits::CaptureTrigger,
    ms_since_last_input: u32,
    suppress_window_ms: u32,
    policy: UiaTriggerPolicy,
) -> bool {
    if policy.run_on_interactive || suppress_window_ms == 0 {
        return false;
    }
    matches!(trigger, traits::CaptureTrigger::Timer) && ms_since_last_input < suppress_window_ms
}

/// Splits a (possibly multi-line) text block into `(line_index, word)` pairs starting at
/// `first_line`, returning the pairs and the next free line index. Mirrors OCR's
/// `Lines()`→`Words()` grouping: each `\n`-separated line that has any words is one
/// `line_index`; blank lines and empty words are skipped so indices stay contiguous.
pub(crate) fn split_words(text: &str, first_line: u32) -> (Vec<(u32, String)>, u32) {
    let mut out = Vec::new();
    let mut line = first_line;
    for raw_line in text.split('\n') {
        let mut had_word = false;
        for word in raw_line.split_whitespace() {
            out.push((line, word.to_string()));
            had_word = true;
        }
        if had_word {
            line += 1;
        }
    }
    (out, line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use traits::CaptureTrigger;

    const GATED: UiaTriggerPolicy = UiaTriggerPolicy {
        run_on_interactive: false,
    };
    const PERMISSIVE: UiaTriggerPolicy = UiaTriggerPolicy {
        run_on_interactive: true,
    };

    #[test]
    fn high_frequency_interactive_triggers_skip_uia_by_default() {
        // Scroll and click are the exact triggers that reproduce the Chromium hang: each
        // fires a fresh full-tree walk against the target app. Under the default policy they
        // must NOT run UIA; OCR (the captured bitmap) carries those frames instead.
        assert!(!trigger_runs_uia(CaptureTrigger::ScrollStop, GATED));
        assert!(!trigger_runs_uia(CaptureTrigger::Click, GATED));
    }

    #[test]
    fn interactive_triggers_run_uia_when_policy_opts_in() {
        // The opt-in restores the legacy behavior (the guard + bounded queue + control view
        // still bound the load).
        assert!(trigger_runs_uia(CaptureTrigger::ScrollStop, PERMISSIVE));
        assert!(trigger_runs_uia(CaptureTrigger::Click, PERMISSIVE));
    }

    #[test]
    fn low_frequency_triggers_run_uia_under_either_policy() {
        // Everything else is low-frequency enough that one bounded walk is safe regardless of
        // the interactive policy, and these frames benefit most from UIA's higher fidelity.
        for policy in [GATED, PERMISSIVE] {
            assert!(trigger_runs_uia(CaptureTrigger::Timer, policy));
            assert!(trigger_runs_uia(CaptureTrigger::Idle, policy));
            assert!(trigger_runs_uia(CaptureTrigger::TypingPause, policy));
            assert!(trigger_runs_uia(CaptureTrigger::ForegroundChange, policy));
            assert!(trigger_runs_uia(CaptureTrigger::ClipboardChange, policy));
            assert!(trigger_runs_uia(CaptureTrigger::Manual, policy));
        }
    }

    #[test]
    fn input_gate_skips_timer_walks_during_active_input() {
        // The default (timer-only) capture path tags every frame `Timer`, so the scroll/click
        // trigger gate is inert there. A `Timer` tick can therefore land mid-scroll on a heavy
        // Chromium/Electron tree — the exact freeze the hang fix avoids. With a 500 ms window,
        // a `Timer` frame whose last input was 100 ms ago (the user is actively interacting)
        // must skip UIA and fall back to OCR.
        assert!(input_gate_skips_uia(CaptureTrigger::Timer, 100, 500, GATED));
        // Once input has settled past the window, the periodic walk runs again.
        assert!(!input_gate_skips_uia(
            CaptureTrigger::Timer,
            800,
            500,
            GATED
        ));
        // Exactly at the window boundary counts as settled (strictly-less-than).
        assert!(!input_gate_skips_uia(
            CaptureTrigger::Timer,
            500,
            500,
            GATED
        ));
    }

    #[test]
    fn input_gate_is_disabled_by_zero_window_or_opt_in() {
        // A zero window turns the gate off entirely (the documented disable value).
        assert!(!input_gate_skips_uia(CaptureTrigger::Timer, 0, 0, GATED));
        // Opting into UIA-during-interaction (`run_on_interactive`) bypasses the gate too, so
        // the one knob coherently governs all "UIA while interacting" behavior.
        assert!(!input_gate_skips_uia(
            CaptureTrigger::Timer,
            50,
            500,
            PERMISSIVE
        ));
    }

    #[test]
    fn input_gate_only_touches_timer_triggers() {
        // Every non-`Timer` trigger is exempt even with input 0 ms ago: scroll/click are
        // already handled by `trigger_runs_uia`; foreground/clipboard/manual must run UIA
        // *because* the user just acted (capture the new state); idle/typing-pause fire only
        // after input has quiesced. Gating them on recent input would wrongly suppress them.
        for trigger in [
            CaptureTrigger::ScrollStop,
            CaptureTrigger::Click,
            CaptureTrigger::ForegroundChange,
            CaptureTrigger::ClipboardChange,
            CaptureTrigger::Manual,
            CaptureTrigger::Idle,
            CaptureTrigger::TypingPause,
        ] {
            assert!(
                !input_gate_skips_uia(trigger, 0, 500, GATED),
                "{trigger:?} must not be input-gated"
            );
        }
    }

    #[test]
    fn only_document_and_text_controls_want_textpattern() {
        // TextPattern visible-range reads are the one live, uncacheable, cross-process cost;
        // worth it only for documents/editable text. Everything else uses Name/Value.
        assert!(control_type_wants_textpattern(50030), "Document");
        assert!(control_type_wants_textpattern(50004), "Edit");
        assert!(control_type_wants_textpattern(50020), "Text");
        assert!(!control_type_wants_textpattern(50000), "Button");
        assert!(!control_type_wants_textpattern(50033), "Pane");
        assert!(!control_type_wants_textpattern(50008), "List");
    }

    #[test]
    fn containers_are_skipped_but_content_controls_emit() {
        assert!(is_container(PANE));
        assert!(is_container(WINDOW));
        assert!(is_container(GROUP));
        assert!(!is_container(50020), "Text control is not a container");
        assert!(!is_container(50004), "Edit control is not a container");
        assert!(!is_container(50000), "Button is not a container");
    }

    #[test]
    fn never_emits_password_or_offscreen_or_container() {
        // A normal Text control on screen emits.
        assert!(should_emit(50020, false, false));
        // Password masked → never read, even if visible.
        assert!(!should_emit(50004, true, false));
        // Offscreen / occluded → skipped (OCR-parity: only what was visible).
        assert!(!should_emit(50020, false, true));
        // A container never emits its own text.
        assert!(!should_emit(PANE, false, false));
    }

    #[test]
    fn split_words_groups_lines_and_skips_blanks() {
        let (spans, next) = split_words("alpha beta\ngamma", 0);
        assert_eq!(
            spans,
            vec![
                (0, "alpha".to_string()),
                (0, "beta".to_string()),
                (1, "gamma".to_string()),
            ]
        );
        assert_eq!(next, 2);

        // A leading blank line and extra whitespace don't create index gaps.
        let (spans, next) = split_words("  \n  x   y \n", 5);
        assert_eq!(spans, vec![(5, "x".to_string()), (5, "y".to_string())]);
        assert_eq!(next, 6);

        // Empty input yields nothing and doesn't advance the line index.
        let (spans, next) = split_words("   ", 3);
        assert!(spans.is_empty());
        assert_eq!(next, 3);
    }
}

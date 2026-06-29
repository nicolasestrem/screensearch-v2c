//! System-wide input-activity probe — how long since the user last touched the
//! keyboard or mouse. Used to gate the UIA tree walk off frames captured while the user
//! is actively interacting (e.g. scrolling), the residual freeze risk that
//! [`classify::input_gate_skips_uia`](crate::classify::input_gate_skips_uia) closes for the
//! default timer-only capture path (`07` #71). One cheap syscall pair, no input hooks.

use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

/// Milliseconds since the last system-wide keyboard/mouse input, or `None` if the OS could
/// not report it (the caller then treats activity as unknown and does not suppress UIA).
///
/// `GetLastInputInfo` reports the tick at which the last input event occurred, on the same
/// tick base as `GetTickCount`; the difference is the idle time. `wrapping_sub` is correct
/// across the ~49.7-day `GetTickCount` rollover (both values wrap on the same base).
pub fn ms_since_last_input() -> Option<u32> {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: `info` is a fully-initialized `LASTINPUTINFO` with `cbSize` set to its own
    // size, exactly as `GetLastInputInfo` requires; it writes only `dwTime`.
    let ok = unsafe { GetLastInputInfo(&mut info) };
    if ok.as_bool() {
        // SAFETY: `GetTickCount` takes no arguments and only reads the system tick counter.
        let now = unsafe { GetTickCount() };
        Some(now.wrapping_sub(info.dwTime))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On a real desktop the probe returns a plausible idle time. `#[ignore]`d in CI (the
    /// build agent has no interactive session); run locally with `cargo test -p uia -- --ignored`.
    #[test]
    #[ignore = "requires a real desktop session"]
    fn reports_an_idle_time() {
        let ms = ms_since_last_input().expect("GetLastInputInfo should succeed on a desktop");
        // A loose upper bound: any real session has had input within the last day.
        assert!(ms < 86_400_000);
    }
}

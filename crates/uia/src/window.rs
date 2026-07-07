//! Win32 window-class probe used to route Chromium/Electron windows away from UIA.
//!
//! The class read is a single cheap `GetClassNameW` on a window handle — no COM, no tree
//! touch — so it can gate every frame *before* any (potentially wedging) UIA call. The pure
//! match lives in [`crate::classify::is_chromium_window_class`]; this module only supplies the
//! live class string. Windows-only by design (`04` guardrails).

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;

use crate::classify::is_chromium_window_class;

/// The Win32 class name of `hwnd`, or `None` if it can't be read. Cheap, local, no COM.
pub(crate) fn class_name_of(hwnd: HWND) -> Option<String> {
    // `Chrome_WidgetWin_1` is 18 code units; 256 is ample headroom for any real class name.
    let mut buf = [0u16; 256];
    // SAFETY: plain Win32 query on the calling thread; `GetClassNameW` copies at most
    // `buf.len() - 1` code units plus a NUL and returns the count copied (0 on failure).
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    if len <= 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..len as usize]))
}

/// Whether the window identified by a raw `i64` handle (as recorded on
/// [`traits::CapturedFrame`]'s `foreground_hwnd`) is a Chromium/Electron/CEF window that must
/// be routed to OCR instead of walked by UIA (`07` #93). Returns `false` on any failure to read
/// the class — the safe default (don't over-skip; the worker's own guards + the breaker still
/// apply). The composition root calls this to skip such frames before dispatching a walk.
pub fn hwnd_is_chromium(hwnd_raw: i64) -> bool {
    let hwnd = HWND(hwnd_raw as isize as *mut core::ffi::c_void);
    class_name_of(hwnd).is_some_and(|c| is_chromium_window_class(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live runtime check against a **real** window handle (no CI — needs a real desktop). Pass
    /// a top-level window's handle in `UIA_PROBE_HWND` (e.g. PowerShell
    /// `(Get-Process chrome | ? MainWindowTitle | Select -First 1).MainWindowHandle`), then run:
    ///   `UIA_PROBE_HWND=<i64> cargo test -p uia -- --ignored --nocapture live_hwnd_classification`
    /// It prints the class `GetClassNameW` actually returns and our verdict, and asserts the two
    /// agree — so running it once against a browser and once against a native window proves the
    /// Chromium skip fires on real Chrome/Edge/Electron and never on native apps (`07` #93).
    #[test]
    #[ignore = "requires a real desktop; pass UIA_PROBE_HWND=<i64>"]
    fn live_hwnd_classification() {
        let raw: i64 = std::env::var("UIA_PROBE_HWND")
            .expect("set UIA_PROBE_HWND to a top-level window handle")
            .parse()
            .expect("UIA_PROBE_HWND must be an integer handle");
        let hwnd = HWND(raw as isize as *mut core::ffi::c_void);
        let class = class_name_of(hwnd).unwrap_or_default();
        let verdict = hwnd_is_chromium(raw);
        println!("hwnd={raw} class={class:?} is_chromium={verdict}");
        assert_eq!(
            verdict,
            class.starts_with("Chrome_WidgetWin_"),
            "verdict must match the live class name"
        );
    }
}

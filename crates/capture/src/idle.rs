//! User idle-time probe for idle-triggered vision tagging (`03 §5`). The kernel
//! forbids `unsafe`, so the composition root wraps this in the injected `IdleSource`.

use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

/// Milliseconds since the last keyboard/mouse input, or `None` if it can't be read.
pub fn user_idle_ms() -> Option<u64> {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: `cbSize` is set to the struct size; `GetLastInputInfo` fills `dwTime`
    // and returns a BOOL indicating success.
    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return None;
    }
    // `GetTickCount` and `dwTime` are both 32-bit millisecond tick counts; wrapping
    // subtraction handles the ~49-day rollover correctly.
    let now = unsafe { GetTickCount() };
    Some(now.wrapping_sub(info.dwTime) as u64)
}

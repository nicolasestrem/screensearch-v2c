//! Privacy gate (`03 §8`): skip capture when the foreground app is excluded, and
//! pause when the workstation is locked. The foreground/lock probes are thin Win32
//! calls (the capture crate is the Windows-API crate, so these live here rather than
//! leaking into the kernel).
//!
//! The pure excluded-apps matcher moved to [`traits::is_excluded`] so the kernel can
//! reuse the identical semantics for where-was-i candidacy (`03 §7b`, D9) without
//! depending on `capture` (`03 §2`); it is re-exported here so this crate's call
//! sites and tests are unchanged.

pub use traits::is_excluded;

/// Whether a foreground-window process id belongs to this process. PID-based matching
/// covers every ScreenSearch-owned window, including the hidden Flow overlay, without
/// relying on process/window-name heuristics.
pub fn is_own_window_pid(fg_pid: u32, own_pid: u32) -> bool {
    fg_pid != 0 && own_pid != 0 && fg_pid == own_pid
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HWND, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
    use windows::Win32::System::StationsAndDesktops::{
        CloseDesktop, OpenInputDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS,
    };
    use windows::Win32::System::Threading::{
        GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
    };

    /// `(app/process name, window title)` for the current foreground window, each
    /// `None` if it can't be resolved. Reused both for the excluded-apps gate and
    /// to populate `frames.app_hint` / `window_title`.
    pub fn foreground_context() -> (Option<String>, Option<String>) {
        // SAFETY: plain Win32 queries on the calling thread; no aliasing.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return (None, None);
            }
            (process_name(hwnd), window_title(hwnd))
        }
    }

    /// Screen-space rect `(left, top, right, bottom)` of the current foreground window
    /// (physical pixels / virtual-desktop coords — the same space as `rcMonitor` and
    /// the WGC texture, given the process is per-monitor-DPI-aware, `07` #54). Prefers
    /// the visual frame bounds (`DWMWA_EXTENDED_FRAME_BOUNDS`, excludes the invisible
    /// resize border), falling back to `GetWindowRect`. `None` when there is no
    /// foreground window or it is minimized — PR3 then leaves `target_rect` unset and
    /// suppresses nothing positionally (the safe default, `03 §3b`).
    pub fn foreground_window_rect() -> Option<(i32, i32, i32, i32)> {
        // SAFETY: plain Win32 queries on the calling thread; `rect` is fully written by
        // the API before we read it, and the DWM out-buffer size is `size_of::<RECT>()`.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() || IsIconic(hwnd).as_bool() {
                return None;
            }
            let mut rect = RECT::default();
            let dwm_ok = DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                std::ptr::addr_of_mut!(rect) as *mut c_void,
                std::mem::size_of::<RECT>() as u32,
            )
            .is_ok();
            if !dwm_ok && GetWindowRect(hwnd, &mut rect).is_err() {
                return None;
            }
            Some((rect.left, rect.top, rect.right, rect.bottom))
        }
    }

    /// Raw handle of the current foreground window as an `i64` (`None` when there is no
    /// foreground window or it is minimized). Recorded on each [`traits::CapturedFrame`]
    /// so a live-window text provider (UIA) can confirm focus hasn't moved between capture
    /// and recognition (`07` #48). A plain integer keeps the frame `Send`.
    pub fn foreground_hwnd() -> Option<i64> {
        // SAFETY: plain Win32 query on the calling thread.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() || IsIconic(hwnd).as_bool() {
                return None;
            }
            Some(hwnd.0 as isize as i64)
        }
    }

    unsafe fn window_title(hwnd: HWND) -> Option<String> {
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut buf);
        if len <= 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }

    unsafe fn process_name(hwnd: HWND) -> Option<String> {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 512];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        ok.ok()?;
        let path = String::from_utf16_lossy(&buf[..size as usize]);
        std::path::Path::new(&path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
    }

    /// Whether the current foreground window belongs to **our own** process (any
    /// ScreenSearch window, including the Flow overlay). Capturing our own UI only
    /// indexes app chrome — sidebar nav, command palette, overlay rows, and a results
    /// pane that echoes other captures' chrome —
    /// which was the dominant source of the PR3 `Deck`/`Recall` self-capture leak
    /// (`docs/AUDIT_0.2.0_PR3_2026-06-26.md`). PID-based so it is exact: it never
    /// mismatches a third-party window that merely has "screensearch" in its title
    /// (e.g. a browser tab on the project's GitHub page), unlike a name match.
    pub fn is_own_foreground_window() -> bool {
        // SAFETY: plain Win32 queries on the calling thread; no aliasing.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return false;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            super::is_own_window_pid(pid, GetCurrentProcessId())
        }
    }

    /// Whether the workstation is locked. Heuristic (`07`): a non-elevated process
    /// cannot open the input desktop while the secure (lock) desktop is active, so
    /// a failed `OpenInputDesktop` is treated as "locked".
    pub fn is_workstation_locked() -> bool {
        // SAFETY: opens and immediately closes the input desktop handle.
        unsafe {
            match OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS) {
                Ok(desktop) => {
                    let _ = CloseDesktop(desktop);
                    false
                }
                Err(_) => true,
            }
        }
    }
}

#[cfg(windows)]
pub use win::{
    foreground_context, foreground_hwnd, foreground_window_rect, is_own_foreground_window,
    is_workstation_locked,
};

#[cfg(test)]
mod tests {
    // `is_excluded` moved to `traits::privacy` (its tests moved with it); this module
    // keeps the Win32-adjacent `is_own_window_pid` matcher tests.
    use super::is_own_window_pid;

    #[test]
    fn own_window_pid_matches_any_nonzero_own_process_window() {
        assert!(is_own_window_pid(42, 42));
    }

    #[test]
    fn own_window_pid_rejects_foreign_process() {
        assert!(!is_own_window_pid(42, 7));
    }

    #[test]
    fn own_window_pid_rejects_unknown_foreground_pid() {
        assert!(!is_own_window_pid(0, 42));
    }
}

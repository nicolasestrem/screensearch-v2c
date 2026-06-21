//! Privacy gate (`03 §8`): skip capture when the foreground app is excluded, and
//! pause when the workstation is locked. The matcher is pure (unit-tested); the
//! foreground/lock probes are thin Win32 calls (the capture crate is the
//! Windows-API crate, so these live here rather than leaking into the kernel).

/// Case-insensitive substring match of any `excluded` entry against the foreground
/// app/process name or window title (`privacy.excluded_apps`). Empty entries are
/// ignored so a stray `""` can't match everything.
pub fn is_excluded(app: Option<&str>, title: Option<&str>, excluded: &[String]) -> bool {
    let app = app.unwrap_or_default().to_ascii_lowercase();
    let title = title.unwrap_or_default().to_ascii_lowercase();
    excluded.iter().any(|e| {
        let needle = e.trim().to_ascii_lowercase();
        !needle.is_empty() && (app.contains(&needle) || title.contains(&needle))
    })
}

#[cfg(windows)]
mod win {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HWND};
    use windows::Win32::System::StationsAndDesktops::{
        CloseDesktop, OpenInputDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
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
pub use win::{foreground_context, is_workstation_locked};

#[cfg(test)]
mod tests {
    use super::is_excluded;

    fn excluded() -> Vec<String> {
        vec![
            "1Password".to_string(),
            "KeePass".to_string(),
            "Bitwarden".to_string(),
        ]
    }

    #[test]
    fn matches_process_name_case_insensitively() {
        assert!(is_excluded(Some("1password"), None, &excluded()));
        assert!(is_excluded(Some("KeePassXC"), None, &excluded()));
    }

    #[test]
    fn matches_window_title() {
        assert!(is_excluded(
            Some("explorer"),
            Some("Bitwarden — Vault"),
            &excluded()
        ));
    }

    #[test]
    fn allows_unrelated_apps() {
        assert!(!is_excluded(Some("firefox"), Some("Inbox"), &excluded()));
        assert!(!is_excluded(None, None, &excluded()));
    }

    #[test]
    fn empty_excluded_entry_never_matches() {
        assert!(!is_excluded(
            Some("anything"),
            Some("at all"),
            &["".to_string(), "  ".to_string()]
        ));
    }
}

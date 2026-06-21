//! Monitor enumeration for WGC (`03 §3`). Walks the connected displays with
//! `EnumDisplayMonitors`, producing a stable index, name, size, and the `HMONITOR`
//! each WGC capture item is created from.

use traits::MonitorInfo;
use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;

/// A connected monitor and the `HMONITOR` to capture it from.
pub struct EnumeratedMonitor {
    pub info: MonitorInfo,
    pub hmonitor: HMONITOR,
}

/// Enumerates all connected monitors in OS order, assigning `info.index` by that
/// order (matching `frames.monitor_index`).
pub fn enumerate() -> Vec<EnumeratedMonitor> {
    let mut out: Vec<EnumeratedMonitor> = Vec::new();
    // SAFETY: `proc` receives our `&mut Vec` via lparam for the duration of the call.
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(proc),
            LPARAM(&mut out as *mut Vec<EnumeratedMonitor> as isize),
        );
    }
    out
}

unsafe extern "system" fn proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let out = &mut *(lparam.0 as *mut Vec<EnumeratedMonitor>);

    let mut mi = MONITORINFOEXW::default();
    mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(hmonitor, std::ptr::addr_of_mut!(mi) as *mut MONITORINFO).as_bool() {
        let rc = mi.monitorInfo.rcMonitor;
        let name = String::from_utf16_lossy(&mi.szDevice)
            .trim_end_matches('\0')
            .to_string();
        out.push(EnumeratedMonitor {
            info: MonitorInfo {
                index: out.len() as u32,
                name,
                width: (rc.right - rc.left).max(0) as u32,
                height: (rc.bottom - rc.top).max(0) as u32,
                is_primary: mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            },
            hmonitor,
        });
    }
    BOOL(1) // keep enumerating
}

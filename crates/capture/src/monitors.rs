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

/// A connected monitor and the `HMONITOR` to capture it from. `left`/`top` are the
/// monitor's screen-space origin (`rcMonitor`, physical pixels / virtual-desktop
/// coords) — the size lives in `info.width`/`info.height`.
pub struct EnumeratedMonitor {
    pub info: MonitorInfo,
    pub hmonitor: HMONITOR,
    pub left: i32,
    pub top: i32,
}

/// Screen-space bounds of a monitor (physical pixels, virtual-desktop coords), keyed by
/// the same index as `frames.monitor_index`. Used to map the foreground-window rect into
/// a captured monitor's frame for PR3's `target_rect` (`03 §3b`). Plain `i32`s (Send) so
/// it can be cached on the capture source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorBounds {
    pub index: u32,
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

/// Per-monitor screen bounds in the same order/index as [`enumerate`] (and therefore as
/// `frames.monitor_index`). Cheap; called once when the capture source starts.
pub fn monitor_bounds() -> Vec<MonitorBounds> {
    enumerate()
        .into_iter()
        .map(|m| MonitorBounds {
            index: m.info.index,
            left: m.left,
            top: m.top,
            width: m.info.width as i32,
            height: m.info.height as i32,
        })
        .collect()
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
        // Convert up to the first NUL only — bytes past the terminator may be
        // uninitialized garbage that a trailing-NUL trim wouldn't remove.
        let end = mi
            .szDevice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(mi.szDevice.len());
        let name = String::from_utf16_lossy(&mi.szDevice[..end]);
        out.push(EnumeratedMonitor {
            info: MonitorInfo {
                index: out.len() as u32,
                name,
                width: (rc.right - rc.left).max(0) as u32,
                height: (rc.bottom - rc.top).max(0) as u32,
                is_primary: mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            },
            hmonitor,
            left: rc.left,
            top: rc.top,
        });
    }
    BOOL(1) // keep enumerating
}

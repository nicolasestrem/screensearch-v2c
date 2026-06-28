//! Minimal monitor enumeration: maps a `monitor_index` to its screen-pixel origin.
//! Replicates `capture::monitors`' `EnumDisplayMonitors` order so the index aligns with
//! `frames.monitor_index` — the `uia` crate may not depend on `capture` (`03 §2`), so the
//! tiny walk is duplicated rather than shared. UIA bounding rects are virtual-desktop
//! coordinates, so [`crate::geometry::normalize_screen_rect`] subtracts this origin.

use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};

/// Screen-space origin `(left, top)` of the monitor at `index` (same OS enumeration order
/// as the capture crate, hence as `frames.monitor_index`), or `None` if out of range.
pub(crate) fn monitor_origin(index: u32) -> Option<(i32, i32)> {
    enumerate_origins().get(index as usize).copied()
}

fn enumerate_origins() -> Vec<(i32, i32)> {
    let mut out: Vec<(i32, i32)> = Vec::new();
    // SAFETY: `proc` receives our `&mut Vec` via lparam for the duration of the call.
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(proc),
            LPARAM(&mut out as *mut Vec<(i32, i32)> as isize),
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
    let out = &mut *(lparam.0 as *mut Vec<(i32, i32)>);
    let mut mi = MONITORINFOEXW::default();
    mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(hmonitor, std::ptr::addr_of_mut!(mi) as *mut MONITORINFO).as_bool() {
        let rc = mi.monitorInfo.rcMonitor;
        out.push((rc.left, rc.top));
    }
    BOOL(1) // keep enumerating
}

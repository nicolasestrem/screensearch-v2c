//! Pure geometry for mapping UI Automation bounding rectangles into a captured
//! frame's normalized `[0,1]` space. No Win32 here, so it is unit-tested in CI.

/// Maps a UIA `BoundingRectangle` (virtual-desktop **screen pixels**, given as
/// `left/top/right/bottom`) into the captured monitor's frame, normalized to `[0,1]`
/// with origin top-left.
///
/// Unlike OCR's normalizer (which runs on a bitmap whose origin is `(0,0)`), UIA rects
/// are desktop-relative, so the captured monitor's origin (`monitor_origin`) must be
/// subtracted first. `frame_size` is the monitor's pixel size. Clamps so the box
/// stays inside the frame and `x + w <= 1`, `y + h <= 1`; a zero-area frame or a
/// degenerate rect yields a zero box.
///
/// The rect is passed as four scalars (the caller has them straight from a `RECT`); the
/// monitor origin and frame size are tuples (the `monitors`/`Request` sources already pair
/// them) — this also keeps the argument count within clippy's bound.
pub(crate) fn normalize_screen_rect(
    l: i32,
    t: i32,
    r: i32,
    b: i32,
    monitor_origin: (i32, i32),
    frame_size: (u32, u32),
) -> (f32, f32, f32, f32) {
    let (mon_left, mon_top) = monitor_origin;
    let (frame_w, frame_h) = frame_size;
    if frame_w == 0 || frame_h == 0 || r <= l || b <= t {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let (fw, fh) = (frame_w as f32, frame_h as f32);
    // Subtract the captured monitor's origin: UIA rects are virtual-desktop coordinates,
    // not frame-relative. Clip the left/top edge to the monitor *before* measuring, so an
    // element straddling the left/top edge reports only its on-frame extent (mirrors
    // capture's `normalize_window_rect`, which feeds the very `target_rect` this is compared
    // against) — otherwise the off-frame portion inflates the width/height and shifts the
    // span's center used by the containment filter.
    let vis_left = l.max(mon_left);
    let vis_top = t.max(mon_top);
    let x = ((vis_left - mon_left) as f32 / fw).clamp(0.0, 1.0);
    let y = ((vis_top - mon_top) as f32 / fh).clamp(0.0, 1.0);
    let w = ((r - vis_left) as f32 / fw).clamp(0.0, 1.0 - x);
    let h = ((b - vis_top) as f32 / fh).clamp(0.0, 1.0 - y);
    (x, y, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_monitor_maps_proportionally() {
        // 800×600 box at (100,50) on the primary monitor (origin 0,0), 1920×1080 frame.
        let (x, y, w, h) = normalize_screen_rect(100, 50, 900, 650, (0, 0), (1920, 1080));
        assert!((x - 100.0 / 1920.0).abs() < 1e-5, "x = {x}");
        assert!((y - 50.0 / 1080.0).abs() < 1e-5, "y = {y}");
        assert!((w - 800.0 / 1920.0).abs() < 1e-5, "w = {w}");
        assert!((h - 600.0 / 1080.0).abs() < 1e-5, "h = {h}");
    }

    #[test]
    fn secondary_monitor_subtracts_its_origin() {
        // A monitor to the right (origin x=1920). A window flush to its left edge must map
        // to x≈0, NOT 1.0 — this is the whole reason UIA rects need origin subtraction.
        let (x, _y, w, h) =
            normalize_screen_rect(1920, 0, 1920 + 960, 540, (1920, 0), (1920, 1080));
        assert!(
            x.abs() < 1e-5,
            "left edge of the secondary monitor maps to 0, got {x}"
        );
        assert!((w - 0.5).abs() < 1e-5, "half width, got {w}");
        assert!((h - 0.5).abs() < 1e-5, "half height, got {h}");
    }

    #[test]
    fn left_top_straddling_box_reports_only_on_frame_extent() {
        // An element from x=-100..300, y=-40..60 on the primary monitor (origin 0,0): only
        // 0..300 / 0..60 is on-frame, so the normalized box must start at (0,0) and measure
        // the *visible* extent (0.3 × 0.06), not the full off-frame extent (0.4 × 0.1).
        let (x, y, w, h) = normalize_screen_rect(-100, -40, 300, 60, (0, 0), (1000, 1000));
        assert!(x.abs() < 1e-5, "x clamps to 0, got {x}");
        assert!(y.abs() < 1e-5, "y clamps to 0, got {y}");
        assert!((w - 0.3).abs() < 1e-5, "on-frame width 0.3, got {w}");
        assert!((h - 0.06).abs() < 1e-5, "on-frame height 0.06, got {h}");
    }

    #[test]
    fn overrunning_box_is_clamped_to_unit_square() {
        let (x, y, w, h) = normalize_screen_rect(1800, 1000, 2200, 1300, (0, 0), (1920, 1080));
        assert!(x + w <= 1.0 + 1e-6, "x+w = {}", x + w);
        assert!(y + h <= 1.0 + 1e-6, "y+h = {}", y + h);
    }

    #[test]
    fn degenerate_inputs_are_zero() {
        assert_eq!(
            normalize_screen_rect(10, 10, 5, 5, (0, 0), (1920, 1080)),
            (0.0, 0.0, 0.0, 0.0)
        );
        assert_eq!(
            normalize_screen_rect(0, 0, 10, 10, (0, 0), (0, 0)),
            (0.0, 0.0, 0.0, 0.0)
        );
    }
}

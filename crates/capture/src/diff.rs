//! The diff gate (`03 §3/§8`): a cheap, normalized frame-change metric so only
//! *changed* frames are stored. Pure logic — no Windows APIs — so it is unit-tested
//! directly.
//!
//! A frame is reduced to a small grayscale grid ("fingerprint"); two fingerprints
//! are compared by normalized mean-absolute luma difference in `[0, 1]`. A frame
//! passes the gate when that value exceeds `capture.diff_threshold` (default
//! `0.006`). The first frame on a monitor, and any resolution change, always pass.

use image::RgbaImage;

/// Side of the square downscaled luma grid used for change detection.
const GRID: u32 = 32;

/// A tiny grayscale fingerprint of a frame plus its source dimensions.
#[derive(Debug, Clone, PartialEq)]
pub struct Fingerprint {
    width: u32,
    height: u32,
    /// `GRID * GRID` BT.601 luma samples (nearest-neighbour downscaled).
    luma: Vec<u8>,
}

/// Reduces a frame to a [`Fingerprint`] by sampling a `GRID × GRID` luma grid.
pub fn fingerprint(img: &RgbaImage) -> Fingerprint {
    let (iw, ih) = (img.width().max(1), img.height().max(1));
    let mut luma = Vec::with_capacity((GRID * GRID) as usize);
    for gy in 0..GRID {
        let sy = (gy * ih / GRID).min(ih - 1);
        for gx in 0..GRID {
            let sx = (gx * iw / GRID).min(iw - 1);
            let p = img.get_pixel(sx, sy).0;
            // BT.601 luma; integer math keeps it cheap.
            let l = (77 * p[0] as u32 + 150 * p[1] as u32 + 29 * p[2] as u32) >> 8;
            luma.push(l as u8);
        }
    }
    Fingerprint {
        width: img.width(),
        height: img.height(),
        luma,
    }
}

/// Normalized `[0, 1]` difference between two fingerprints. Different source
/// dimensions count as a full change (`1.0`).
pub fn difference(a: &Fingerprint, b: &Fingerprint) -> f32 {
    if a.width != b.width || a.height != b.height {
        return 1.0;
    }
    let sum: u32 = a
        .luma
        .iter()
        .zip(&b.luma)
        .map(|(x, y)| u32::from(x.abs_diff(*y)))
        .sum();
    sum as f32 / (a.luma.len() as f32 * 255.0)
}

/// Stable content hash of the raw RGBA pixels (`frames.content_hash`, `03 §4`).
pub fn content_hash(img: &RgbaImage) -> String {
    blake3::hash(img.as_raw()).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn solid(w: u32, h: u32, rgb: [u8; 3]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba([rgb[0], rgb[1], rgb[2], 255]))
    }

    #[test]
    fn identical_frames_have_zero_difference() {
        let a = fingerprint(&solid(200, 100, [40, 80, 120]));
        let b = fingerprint(&solid(200, 100, [40, 80, 120]));
        assert_eq!(difference(&a, &b), 0.0);
    }

    #[test]
    fn black_vs_white_is_near_full_difference() {
        let a = fingerprint(&solid(200, 100, [0, 0, 0]));
        let b = fingerprint(&solid(200, 100, [255, 255, 255]));
        assert!(difference(&a, &b) > 0.9, "got {}", difference(&a, &b));
    }

    #[test]
    fn resolution_change_is_full_difference() {
        let a = fingerprint(&solid(200, 100, [10, 10, 10]));
        let b = fingerprint(&solid(201, 100, [10, 10, 10]));
        assert_eq!(difference(&a, &b), 1.0);
    }

    #[test]
    fn tiny_change_stays_below_default_threshold() {
        // one channel nudged by 1 across the whole frame → well under 0.006
        let a = fingerprint(&solid(200, 100, [100, 100, 100]));
        let b = fingerprint(&solid(200, 100, [100, 100, 101]));
        assert!(difference(&a, &b) < 0.006, "got {}", difference(&a, &b));
    }

    #[test]
    fn content_hash_is_stable_and_distinct() {
        let a = solid(8, 8, [1, 2, 3]);
        let b = solid(8, 8, [1, 2, 3]);
        let c = solid(8, 8, [9, 9, 9]);
        assert_eq!(content_hash(&a), content_hash(&b));
        assert_ne!(content_hash(&a), content_hash(&c));
    }
}

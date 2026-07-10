//! Deterministic session-confidence tiers (`06` #27[e], D11).
//!
//! Anchored AI/meeting sessions occupy 0.70..=0.95 and lose up to 0.25 as explicitly
//! absorbed (unrecognized) time rises. Focus-with-stem occupies 0.45..=0.65 based on
//! its margin above the density gate. Bare focus is 0.30. The disjoint bands make the
//! normative invariant explicit: every anchorless result is lower-confidence than every
//! anchored result.

pub(crate) fn anchored_confidence(recognized_ms: i64, absorbed_ms: i64) -> f64 {
    let recognized = recognized_ms.max(0) as f64;
    let absorbed = absorbed_ms.max(0) as f64;
    let total = recognized + absorbed;
    let absorbed_share = if total > 0.0 { absorbed / total } else { 1.0 };
    (0.95 - 0.25 * absorbed_share).clamp(0.70, 0.95)
}

pub(crate) fn focus_confidence(
    dominant_stem: Option<&str>,
    density_fph: f64,
    density_gate_fph: i64,
) -> f64 {
    if dominant_stem.is_none_or(str::is_empty) {
        return 0.30;
    }
    if density_gate_fph <= 0 {
        return 0.60;
    }
    let ratio = (density_fph / density_gate_fph as f64).clamp(1.0, 2.0);
    0.45 + (ratio - 1.0) * 0.20
}

//! Scoring the candidate segmenter against the hand labels (`docs/0.4.0.md` §3 PR2).
//!
//! - **Boundary precision/recall/F1** with a ± tolerance window. Matching is **typed** (a
//!   session start only matches a labeled start, an end only an end — otherwise a predicted
//!   end could match the next session's labeled start at a back-to-back transition and inflate
//!   the score) and **optimal**, not greedy: an O(n·m) monotonic DP finds the maximum 1:1
//!   non-crossing match within tolerance (greedy nearest-first undercounts). Boundaries within
//!   tolerance of the export's own start/end are excluded from both sides (a midnight-clipped
//!   or still-open span is an export artifact, not a segmentation error).
//! - **Tool-recognition accuracy** on labeled AI sessions: each labeled `kind = ai` session is
//!   matched to the predicted span of maximum temporal overlap; correct iff the predicted
//!   `tool` equals the label's.
//!
//! All functions are pure over in-memory spans + labels — no file IO — so PR4 can drive the
//! same scoring with its shipped segmenter's spans (the D9 referee contract).

use crate::group::{group_with, segment_grouped};
use crate::labels::ResolvedLabel;
use crate::model::{GroupParams, Kind, SegParams, SessionSpan};
use crate::segmenter::{segment_micro, FrameRow};
use crate::taxonomy::Taxonomy;

/// Snap each labeled boundary to the nearest captured frame time. Minute-granular labels that fall
/// inside a no-frame idle gap otherwise miss the tolerance by the gap size (the observed ~2.3 min
/// start artifact); snapping to the activity edge is a disclosed scoring policy applied to LABELS
/// ONLY, never predictions. A no-op when `frame_times` is empty.
pub fn snap_label_boundaries(labels: &[ResolvedLabel], frame_times: &[i64]) -> Vec<ResolvedLabel> {
    labels
        .iter()
        .map(|l| {
            let start = snap_one(l.start_ms, frame_times);
            let end = snap_one(l.end_ms, frame_times).max(start);
            ResolvedLabel {
                start_ms: start,
                end_ms: end,
                kind: l.kind,
                tool: l.tool.clone(),
                host: l.host,
            }
        })
        .collect()
}

fn snap_one(t: i64, frame_times: &[i64]) -> i64 {
    frame_times
        .iter()
        .copied()
        .min_by_key(|&ft| (ft - t).abs())
        .unwrap_or(t)
}

/// The captured-frame times of a loaded day (for label snapping).
fn day_frame_times(d: &LoadedDay) -> Vec<i64> {
    d.frames.iter().map(|f| f.captured_at).collect()
}

/// Boundary match tallies for one day (start + end boundaries pooled).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BoundaryScore {
    pub predicted: usize,
    pub labeled: usize,
    pub matched: usize,
}

impl BoundaryScore {
    pub fn precision(&self) -> f64 {
        if self.predicted == 0 {
            1.0
        } else {
            self.matched as f64 / self.predicted as f64
        }
    }
    pub fn recall(&self) -> f64 {
        if self.labeled == 0 {
            1.0
        } else {
            self.matched as f64 / self.labeled as f64
        }
    }
    pub fn f1(&self) -> f64 {
        let (p, r) = (self.precision(), self.recall());
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }
    pub fn add(&mut self, o: &BoundaryScore) {
        self.predicted += o.predicted;
        self.labeled += o.labeled;
        self.matched += o.matched;
    }
}

/// One day's full score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayScore {
    pub date: String,
    pub boundaries: BoundaryScore,
    pub tool_correct: usize,
    pub tool_total: usize,
}

/// Maximum 1:1 non-crossing match count between two boundary-time sequences within `tol_ms`.
/// Both are sorted internally; the monotonic DP is optimal (unlike greedy nearest-first).
fn optimal_match(a: &[i64], b: &[i64], tol_ms: i64) -> usize {
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    a.sort_unstable();
    b.sort_unstable();
    let m = b.len();
    // dp[j] over the rolling row.
    let mut prev = vec![0usize; m + 1];
    for &ai in &a {
        let mut cur = vec![0usize; m + 1];
        for j in 1..=m {
            let hit = if (ai - b[j - 1]).abs() <= tol_ms {
                1
            } else {
                0
            };
            cur[j] = cur[j - 1].max(prev[j]).max(prev[j - 1] + hit);
        }
        prev = cur;
    }
    prev[m]
}

/// Drop boundaries within `tol_ms` of the export's own start/end (clip artifacts).
fn keep_interior(events: &[i64], day_start_ms: i64, day_end_ms: i64, tol_ms: i64) -> Vec<i64> {
    events
        .iter()
        .copied()
        .filter(|&t| (t - day_start_ms).abs() > tol_ms && (day_end_ms - t).abs() > tol_ms)
        .collect()
}

/// Score one day's predicted spans against its labels within `tol_ms`. `day_start_ms`/
/// `day_end_ms` bound edge exclusion (the export's first/last frame times).
pub fn score_day(
    date: &str,
    spans: &[SessionSpan],
    labels: &[ResolvedLabel],
    day_start_ms: i64,
    day_end_ms: i64,
    tol_ms: i64,
) -> DayScore {
    let pred_starts = keep_interior(
        &spans.iter().map(|s| s.start_ms).collect::<Vec<_>>(),
        day_start_ms,
        day_end_ms,
        tol_ms,
    );
    let pred_ends = keep_interior(
        &spans.iter().map(|s| s.end_ms).collect::<Vec<_>>(),
        day_start_ms,
        day_end_ms,
        tol_ms,
    );
    let lab_starts = keep_interior(
        &labels.iter().map(|l| l.start_ms).collect::<Vec<_>>(),
        day_start_ms,
        day_end_ms,
        tol_ms,
    );
    let lab_ends = keep_interior(
        &labels.iter().map(|l| l.end_ms).collect::<Vec<_>>(),
        day_start_ms,
        day_end_ms,
        tol_ms,
    );

    let matched = optimal_match(&pred_starts, &lab_starts, tol_ms)
        + optimal_match(&pred_ends, &lab_ends, tol_ms);
    let boundaries = BoundaryScore {
        predicted: pred_starts.len() + pred_ends.len(),
        labeled: lab_starts.len() + lab_ends.len(),
        matched,
    };
    let (tool_correct, tool_total) = tool_accuracy(spans, labels);
    DayScore {
        date: date.to_string(),
        boundaries,
        tool_correct,
        tool_total,
    }
}

/// Tool-recognition accuracy on labeled AI sessions: match each labeled `ai` session to the
/// predicted span of maximum temporal overlap; correct iff `tool` matches.
pub fn tool_accuracy(spans: &[SessionSpan], labels: &[ResolvedLabel]) -> (usize, usize) {
    let mut correct = 0;
    let mut total = 0;
    for l in labels.iter().filter(|l| l.kind == Kind::Ai) {
        total += 1;
        let best = spans
            .iter()
            .filter_map(|s| {
                let ov = (s.end_ms.min(l.end_ms) - s.start_ms.max(l.start_ms)).max(0);
                (ov > 0).then_some((ov, s))
            })
            .max_by_key(|(ov, _)| *ov)
            .map(|(_, s)| s);
        if let Some(s) = best {
            if s.tool == l.tool {
                correct += 1;
            }
        }
    }
    (correct, total)
}

/// Pool day scores into one aggregate (boundaries summed, then P/R/F1 recomputed).
pub fn pool(days: &[DayScore]) -> (BoundaryScore, usize, usize) {
    let mut b = BoundaryScore::default();
    let mut tc = 0;
    let mut tt = 0;
    for d in days {
        b.add(&d.boundaries);
        tc += d.tool_correct;
        tt += d.tool_total;
    }
    (b, tc, tt)
}

/// A labeled day loaded from `harness-data/<day>/` (pure data; loaded by `export`/`main`).
/// `day_start_ms`/`day_end_ms` are the export's first/last frame times (edge-exclusion bounds).
#[derive(Debug, Clone)]
pub struct LoadedDay {
    pub date: String,
    pub frames: Vec<FrameRow>,
    pub day_start_ms: i64,
    pub day_end_ms: i64,
    pub labels: Vec<ResolvedLabel>,
}

impl LoadedDay {
    /// The export's frame-time span (edge-exclusion bounds), or `(0,0)` if empty.
    pub fn bounds(&self) -> (i64, i64) {
        match (self.frames.first(), self.frames.last()) {
            (Some(a), Some(b)) => (a.captured_at, b.captured_at),
            _ => (0, 0),
        }
    }
}

/// One Stage-A sweep cell: a `(merge_gap, absorb_max)` combination scored (pooled) through the
/// GROUPED pipeline, plus the predicted session count (the mega-merge honesty column).
#[derive(Debug, Clone, PartialEq)]
pub struct SweepCell {
    pub merge_gap_secs: i64,
    pub absorb_max_secs: i64,
    pub boundary: BoundaryScore,
    pub tool_correct: usize,
    pub tool_total: usize,
    pub pred_sessions: usize,
}

impl SweepCell {
    pub fn tool_accuracy(&self) -> f64 {
        if self.tool_total == 0 {
            1.0
        } else {
            self.tool_correct as f64 / self.tool_total as f64
        }
    }
}

/// Score all days through the grouped pipeline at the given params, returning the day scores and
/// the total predicted session count. Labels are snapped to the nearest frame first (disclosed).
fn score_days_grouped(
    days: &[LoadedDay],
    tax: &Taxonomy,
    sp: &SegParams,
    gp: &GroupParams,
    qualify_ms: i64,
    tol_ms: i64,
) -> (Vec<DayScore>, usize) {
    let mut day_scores = Vec::new();
    let mut pred = 0usize;
    for d in days {
        let micros = segment_micro(&d.frames, tax, sp);
        let spans: Vec<SessionSpan> = group_with(&micros, tax, sp, gp, qualify_ms)
            .into_iter()
            .map(|g| g.span)
            .collect();
        pred += spans.len();
        let labels = snap_label_boundaries(&d.labels, &day_frame_times(d));
        day_scores.push(score_day(
            &d.date,
            &spans,
            &labels,
            d.day_start_ms,
            d.day_end_ms,
            tol_ms,
        ));
    }
    (day_scores, pred)
}

/// Stage A: sweep the `(merge_gap, absorb_max)` grid through the grouped pipeline (all other knobs
/// at `gp_base` / the default `IDENTITY_QUALIFY_MS`), scoring each cell pooled over all days.
pub fn sweep_grid(
    days: &[LoadedDay],
    tax: &Taxonomy,
    sp: &SegParams,
    gp_base: &GroupParams,
    merge_gaps_secs: &[i64],
    absorb_maxes_secs: &[i64],
    tol_ms: i64,
) -> Vec<SweepCell> {
    let qualify = crate::group::IDENTITY_QUALIFY_MS;
    let mut cells = Vec::new();
    for &mg in merge_gaps_secs {
        for &am in absorb_maxes_secs {
            let gp = GroupParams {
                merge_gap_ms: mg * 1000,
                absorb_max_ms: am * 1000,
                ..*gp_base
            };
            let (ds, pred) = score_days_grouped(days, tax, sp, &gp, qualify, tol_ms);
            let (boundary, tc, tt) = pool(&ds);
            cells.push(SweepCell {
                merge_gap_secs: mg,
                absorb_max_secs: am,
                boundary,
                tool_correct: tc,
                tool_total: tt,
                pred_sessions: pred,
            });
        }
    }
    cells
}

/// A tunable knob for the Stage-B 1-D sweeps. `value` is seconds, except `FocusDensity` (fph).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Knob {
    MergeGap,
    AbsorbMax,
    MeetingGap,
    FocusMinLen,
    FocusDensity,
    GapClose,
    MinLen,
    IdentityQualify,
}

impl Knob {
    pub fn label(self) -> &'static str {
        match self {
            Knob::MergeGap => "merge_gap_secs",
            Knob::AbsorbMax => "absorb_max_secs",
            Knob::MeetingGap => "meeting_gap_secs",
            Knob::FocusMinLen => "focus_min_len_secs",
            Knob::FocusDensity => "focus_min_density_fph",
            Knob::GapClose => "gap_close_secs",
            Knob::MinLen => "min_len_secs",
            Knob::IdentityQualify => "identity_qualify_secs",
        }
    }
}

/// One Stage-B 1-D sweep point: one knob at one value, all others at base.
#[derive(Debug, Clone, PartialEq)]
pub struct OneDimPoint {
    pub knob: &'static str,
    pub value: i64,
    pub boundary: BoundaryScore,
    pub tool_correct: usize,
    pub tool_total: usize,
    pub pred_sessions: usize,
}

impl OneDimPoint {
    pub fn tool_accuracy(&self) -> f64 {
        if self.tool_total == 0 {
            1.0
        } else {
            self.tool_correct as f64 / self.tool_total as f64
        }
    }
}

/// Stage B: vary one `knob` over `values`, holding everything else at the base params, scoring
/// each through the grouped pipeline. A knob whose response is flat across its range is proposed
/// to PR4 as a named constant rather than a settings key (the parsimony discipline).
pub fn sweep_1d(
    days: &[LoadedDay],
    tax: &Taxonomy,
    sp_base: &SegParams,
    gp_base: &GroupParams,
    knob: Knob,
    values: &[i64],
    tol_ms: i64,
) -> Vec<OneDimPoint> {
    values
        .iter()
        .map(|&v| {
            let mut sp = *sp_base;
            let mut gp = *gp_base;
            let mut qualify = crate::group::IDENTITY_QUALIFY_MS;
            match knob {
                Knob::MergeGap => gp.merge_gap_ms = v * 1000,
                Knob::AbsorbMax => gp.absorb_max_ms = v * 1000,
                Knob::MeetingGap => gp.meeting_gap_ms = v * 1000,
                Knob::FocusMinLen => gp.focus_min_len_ms = v * 1000,
                Knob::FocusDensity => gp.focus_min_density_fph = v,
                Knob::GapClose => sp.gap_close_ms = v * 1000,
                Knob::MinLen => sp.min_len_ms = v * 1000,
                Knob::IdentityQualify => qualify = v * 1000,
            }
            let (ds, pred) = score_days_grouped(days, tax, &sp, &gp, qualify, tol_ms);
            let (boundary, tc, tt) = pool(&ds);
            OneDimPoint {
                knob: knob.label(),
                value: v,
                boundary,
                tool_correct: tc,
                tool_total: tt,
                pred_sessions: pred,
            }
        })
        .collect()
}

/// One freeze-lookback stability measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StabilityPoint {
    pub lookback_secs: i64,
    /// Max distance (ms) a boundary older than the lookback moved between a truncated replay
    /// and the full replay, across all cutoffs. `0` = every old boundary reproduced exactly.
    pub max_drift_ms: i64,
    /// How many old boundaries had no counterpart in a truncated replay (appeared only later).
    pub disappeared: usize,
    /// `true` iff no old boundary moved or disappeared for this lookback.
    pub stable: bool,
}

fn boundaries(spans: &[SessionSpan]) -> Vec<i64> {
    let mut v: Vec<i64> = spans.iter().flat_map(|s| [s.start_ms, s.end_ms]).collect();
    v.sort_unstable();
    v
}

/// Freeze-lookback stability (`03 §7e`): replay each day truncated at hourly cutoffs and, for
/// each candidate lookback window, check whether boundaries older than that window match the
/// full replay exactly. The smallest lookback with `stable == true` is "the window that makes
/// boundaries stop moving in practice". Confirms or amends the proposed 24 h default.
pub fn stability(
    days: &[LoadedDay],
    tax: &Taxonomy,
    sp: &SegParams,
    gp: &GroupParams,
    lookbacks_secs: &[i64],
    cutoff_step_ms: i64,
) -> Vec<StabilityPoint> {
    lookbacks_secs
        .iter()
        .map(|&w_secs| {
            let w_ms = w_secs * 1000;
            let mut max_drift = 0i64;
            let mut disappeared = 0usize;
            for d in days {
                if d.frames.is_empty() {
                    continue;
                }
                let full = boundaries(&segment_grouped(&d.frames, tax, sp, gp));
                let (first, last) = d.bounds();
                let mut t = first + cutoff_step_ms;
                while t <= last {
                    let trunc: Vec<FrameRow> = d
                        .frames
                        .iter()
                        .filter(|f| f.captured_at <= t)
                        .cloned()
                        .collect();
                    let tb = boundaries(&segment_grouped(&trunc, tax, sp, gp));
                    let old_cutoff = t - w_ms;
                    for &fb in full.iter().filter(|&&b| b <= old_cutoff) {
                        match tb.iter().map(|&x| (x - fb).abs()).min() {
                            Some(dm) => max_drift = max_drift.max(dm),
                            None => disappeared += 1,
                        }
                    }
                    t += cutoff_step_ms;
                }
            }
            StabilityPoint {
                lookback_secs: w_secs,
                max_drift_ms: max_drift,
                disappeared,
                stable: max_drift == 0 && disappeared == 0,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Host;

    fn span(start_s: i64, end_s: i64, kind: Kind, tool: Option<&str>) -> SessionSpan {
        SessionSpan {
            start_ms: start_s * 1000,
            end_ms: end_s * 1000,
            context_key: "k".into(),
            kind,
            tool: tool.map(str::to_string),
            host: Some(Host::Terminal),
            frame_count: 10,
            first_frame_id: start_s,
            last_frame_id: end_s,
        }
    }
    fn label(start_s: i64, end_s: i64, kind: Kind, tool: Option<&str>) -> ResolvedLabel {
        ResolvedLabel {
            start_ms: start_s * 1000,
            end_ms: end_s * 1000,
            kind,
            tool: tool.map(str::to_string),
            host: None,
        }
    }

    #[test]
    fn optimal_match_beats_greedy() {
        // predicted [100,130], labeled [90,105], tol 30 -> optimal 2 (greedy would get 1).
        assert_eq!(optimal_match(&[100, 130], &[90, 105], 30), 2);
    }

    #[test]
    fn optimal_match_respects_tolerance_and_one_to_one() {
        assert_eq!(optimal_match(&[100], &[131], 30), 0); // just over tol
        assert_eq!(optimal_match(&[100], &[130], 30), 1); // exactly tol
                                                          // One labeled boundary cannot match two predicted.
        assert_eq!(optimal_match(&[100, 110], &[105], 30), 1);
    }

    #[test]
    fn typed_matching_does_not_cross_start_and_end() {
        // A predicted END near a labeled START must not match: score_day matches starts to
        // starts and ends to ends separately. Two back-to-back labeled sessions; predicted
        // has one span whose end coincides with the second label's start.
        let spans = vec![span(0, 600, Kind::Focus, None)]; // 0..600 (10 min)
        let labels = vec![
            label(0, 600, Kind::Focus, None),    // start 0, end 600
            label(600, 1200, Kind::Focus, None), // start 600 (== predicted end), end 1200
        ];
        // Wide day so nothing is edge-excluded (day start well before 0, end well after).
        let s = score_day("d", &spans, &labels, -1_000_000, 3_000_000, 120_000);
        // predicted boundaries: {start 0, end 600}. labeled: {starts 0,600; ends 600,1200}.
        // start matches: pred start 0 <-> lab start 0 = 1. end matches: pred end 600 <-> lab
        // end 600 = 1 (NOT lab start 600, which is a start). matched = 2.
        assert_eq!(s.boundaries.matched, 2);
        assert_eq!(s.boundaries.predicted, 2);
        assert_eq!(s.boundaries.labeled, 4);
    }

    #[test]
    fn edge_boundaries_are_excluded_both_sides() {
        // A predicted + labeled boundary right at the day start are both dropped.
        let spans = vec![span(0, 600, Kind::Focus, None)];
        let labels = vec![label(0, 600, Kind::Focus, None)];
        // Day starts at 0: the start boundaries (both at 0) are within tol of day start.
        let s = score_day("d", &spans, &labels, 0, 600_000, 120_000);
        // Only the end boundary at 600 s survives on each side (600 s is > tol from both 0 and
        // 600_000 ms day end? day_end 600_000 ms == 600 s, so end at 600 s IS within tol of
        // day end too -> also excluded). So everything is edge-excluded here.
        assert_eq!(s.boundaries.predicted, 0);
        assert_eq!(s.boundaries.labeled, 0);
        assert_eq!(s.boundaries.f1(), 1.0); // vacuous: nothing to disagree on
    }

    #[test]
    fn perfect_day_scores_one() {
        let spans = vec![
            span(200, 800, Kind::Ai, Some("claude-code")),
            span(1000, 1600, Kind::Focus, None),
        ];
        let labels = vec![
            label(200, 800, Kind::Ai, Some("claude-code")),
            label(1000, 1600, Kind::Focus, None),
        ];
        let s = score_day("d", &spans, &labels, 0, 2_000_000, 120_000);
        assert_eq!(s.boundaries.matched, 4);
        assert_eq!(s.boundaries.precision(), 1.0);
        assert_eq!(s.boundaries.recall(), 1.0);
        assert_eq!(s.boundaries.f1(), 1.0);
        assert_eq!((s.tool_correct, s.tool_total), (1, 1));
    }

    #[test]
    fn missed_and_spurious_boundaries_lower_pr() {
        // Labeled two sessions; predicted merged them into one (missed the middle boundary)
        // plus emitted one spurious extra session late.
        let spans = vec![
            span(200, 1600, Kind::Focus, None),  // merged
            span(1800, 2000, Kind::Focus, None), // spurious-ish
        ];
        let labels = vec![
            label(200, 800, Kind::Focus, None),
            label(1000, 1600, Kind::Focus, None),
        ];
        let s = score_day("d", &spans, &labels, 0, 3_000_000, 120_000);
        // predicted starts {200,1800}, ends {1600,2000}; labeled starts {200,1000} ends {800,1600}.
        // start matches: 200<->200 =1 (1800 vs 1000 too far). end matches: 1600<->1600 =1.
        assert_eq!(s.boundaries.matched, 2);
        assert_eq!(s.boundaries.predicted, 4);
        assert_eq!(s.boundaries.labeled, 4);
        assert!((s.boundaries.precision() - 0.5).abs() < 1e-9);
        assert!((s.boundaries.recall() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn tool_accuracy_max_overlap_and_no_overlap_is_wrong() {
        let spans = vec![span(0, 600, Kind::Ai, Some("codex"))];
        // Labeled claude-code overlapping the codex span -> wrong tool.
        let mislabeled = vec![label(0, 600, Kind::Ai, Some("claude-code"))];
        assert_eq!(tool_accuracy(&spans, &mislabeled), (0, 1));
        // Correct tool.
        let right = vec![label(0, 600, Kind::Ai, Some("codex"))];
        assert_eq!(tool_accuracy(&spans, &right), (1, 1));
        // An AI label with no overlapping predicted span -> counted, wrong.
        let disjoint = vec![label(10_000, 11_000, Kind::Ai, Some("codex"))];
        assert_eq!(tool_accuracy(&spans, &disjoint), (0, 1));
    }

    fn frow(id: i64, sec: i64, app: Option<&str>, title: Option<&str>) -> FrameRow {
        FrameRow {
            frame_id: id,
            captured_at: sec * 1000,
            app_hint: app.map(str::to_string),
            window_title: title.map(str::to_string),
            browser_url: None,
        }
    }

    fn run_frames(
        id0: i64,
        app: &str,
        title: &str,
        start: i64,
        end: i64,
        step: i64,
    ) -> Vec<FrameRow> {
        let mut v = Vec::new();
        let (mut t, mut id) = (start, id0);
        while t <= end {
            v.push(frow(id, t, Some(app), Some(title)));
            t += step;
            id += 1;
        }
        v
    }

    #[test]
    fn sweep_grid_scores_every_cell_through_the_grouped_pipeline() {
        // One sustained claude run -> one anchored session per cell.
        let frames = run_frames(1, "WindowsTerminal", "claude - repo", 0, 600, 30);
        let day = LoadedDay {
            date: "d".into(),
            day_start_ms: 0,
            day_end_ms: 600_000,
            labels: vec![label(0, 600, Kind::Ai, Some("claude-code"))],
            frames,
        };
        let cells = sweep_grid(
            &[day],
            &Taxonomy::seed(),
            &SegParams::default(),
            &GroupParams::default(),
            &[600, 1500],
            &[600, 1200],
            120_000,
        );
        assert_eq!(cells.len(), 4, "2 merge_gaps x 2 absorb_maxes");
        assert!(cells.iter().all(|c| c.pred_sessions >= 1));
        assert!(cells
            .iter()
            .any(|c| c.merge_gap_secs == 1500 && c.absorb_max_secs == 1200));
    }

    #[test]
    fn sweep_1d_varies_one_knob() {
        let frames = run_frames(1, "WindowsTerminal", "claude - repo", 0, 600, 30);
        let day = LoadedDay {
            date: "d".into(),
            day_start_ms: 0,
            day_end_ms: 600_000,
            labels: vec![label(0, 600, Kind::Ai, Some("claude-code"))],
            frames,
        };
        let pts = sweep_1d(
            &[day],
            &Taxonomy::seed(),
            &SegParams::default(),
            &GroupParams::default(),
            Knob::MergeGap,
            &[600, 900, 1500],
            120_000,
        );
        assert_eq!(pts.len(), 3);
        assert!(pts.iter().all(|p| p.knob == "merge_gap_secs"));
        assert_eq!(
            pts.iter().map(|p| p.value).collect::<Vec<_>>(),
            vec![600, 900, 1500]
        );
    }

    #[test]
    fn stability_small_lookback_unstable_large_lookback_stable() {
        // A short Notepad chunk (0..60 s, below the 120 s floor alone) then a later chunk that
        // merges with it in the full replay (gap 140 s < gap_close 300 s) into [0..400].
        // At an early cutoff (T=100 s) the merged session does not exist yet, so its boundary
        // at 0 is absent: a small lookback treats 0 as "old" and sees it disappear (unstable);
        // a large lookback does not yet consider 0 old (stable).
        // Two claude chunks that merge in the full replay (gap 140 s < gap_close 300 s) into one
        // AI session [0, 400]; the early chunk alone (60 s) is below the 120 s floor, so a
        // truncated replay before the merge has no session there.
        let mut frames = run_frames(1, "WindowsTerminal", "claude - repo", 0, 60, 30);
        frames.extend(run_frames(
            100,
            "WindowsTerminal",
            "claude - repo",
            200,
            400,
            30,
        ));
        let day = LoadedDay {
            date: "d".into(),
            day_start_ms: 0,
            day_end_ms: 400_000,
            labels: vec![],
            frames,
        };
        let sp = SegParams {
            gap_close_ms: 300_000,
            min_len_ms: 120_000,
        };
        let gp = GroupParams::default();
        let pts = stability(&[day], &Taxonomy::seed(), &sp, &gp, &[0, 600], 50_000);
        let w0 = pts.iter().find(|p| p.lookback_secs == 0).unwrap();
        let w600 = pts.iter().find(|p| p.lookback_secs == 600).unwrap();
        assert!(
            !w0.stable,
            "lookback 0 should see the boundary appear late: {w0:?}"
        );
        assert!(w600.stable, "a 600 s lookback should be stable: {w600:?}");
    }

    #[test]
    fn pooling_sums_then_recomputes() {
        let d1 = DayScore {
            date: "a".into(),
            boundaries: BoundaryScore {
                predicted: 4,
                labeled: 4,
                matched: 4,
            },
            tool_correct: 1,
            tool_total: 1,
        };
        let d2 = DayScore {
            date: "b".into(),
            boundaries: BoundaryScore {
                predicted: 4,
                labeled: 4,
                matched: 2,
            },
            tool_correct: 0,
            tool_total: 2,
        };
        let (b, tc, tt) = pool(&[d1, d2]);
        assert_eq!((b.predicted, b.labeled, b.matched), (8, 8, 6));
        assert_eq!((tc, tt), (1, 3));
        assert!((b.precision() - 0.75).abs() < 1e-9);
    }
}

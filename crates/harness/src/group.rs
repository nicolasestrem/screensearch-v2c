//! Pass 2 of the two-pass segmenter (`03 §7e` amendment, `06` #27): the macro grouping walk that
//! accretes the unfloored micro-runs of [`crate::segmenter::segment_micro`] into **task-level**
//! sessions. This is the gap-#110 redesign — the app-level micro key over-segments real days
//! ~10-40×; grouping merges the app-runs of one task (Codex, browser, terminal, chat all in
//! flight) into one session anchored by the recognized tool/meeting identity.
//!
//! The pipeline, per the design panel synthesis:
//! - **2a Meeting bands.** Meeting-identity micro-runs of one id are chained (gap ≤
//!   `meeting_gap_ms`); a chain whose extent reaches `focus_min_len_ms` is a **hard band** at its
//!   exact presence endpoints that owns every micro-run inside it and is an impassable barrier
//!   (recovers meeting start/end + the boundary of whatever follows). Sub-floor chains demote to
//!   noise (never anchor, never split).
//! - **2b Grouping walk.** One open group accretes free micro-runs. It closes on an inactivity
//!   gap ≥ `merge_gap_ms`, on a **sustained foreign identity** (a different recognized AI id, or
//!   unrecognized activity while an AI anchor is active, lasting > `absorb_max_ms`), or at a band
//!   edge. Shorter foreign runs are **absorbed** (the single rule that fixes the over-segmentation
//!   — 71 short Codex runs inside one labeled Claude evening stay one session). The anchor is the
//!   AI id with ≥ `IDENTITY_QUALIFY_MS` summed presence, chosen by [`host_rank`] precedence; it may
//!   flip as presence accrues (convert-not-split: a group opened on focus ramp-up keeps its start
//!   when its AI anchor qualifies).
//! - **Floors + gate.** Anchored AI sessions floor at `min_len_ms`; anchorless focus sessions at
//!   `focus_min_len_ms` AND `focus_min_density_fph` frames/hour (AI/meeting exempt). Dropped
//!   groups leave their frames sessionless — the world is additive (D10). No model calls (D3).
//!
//! Output is `SessionSpan`s (the frozen referee unit) — strictly sorted and non-overlapping
//! (a `debug_assert!` guards it; the referee shifts scores silently on overlap rather than
//! failing). The close reason is kept for `cmd_replay` display but is not a `SessionSpan` field.

use std::collections::HashMap;

use crate::model::{GroupParams, Host, Kind, SegParams, SessionSpan};
use crate::segmenter::{segment_micro, FrameRow, MicroSpan};
use crate::taxonomy::Taxonomy;

/// Minimum summed member duration (ms) for an AI identity to anchor / attribute a group. A named
/// constant, not a setting: it guards attribution correctness (scattered singleton AI frames sum
/// to ~0 and cannot flip a focus block into an AI session). It splits the former triple duty of
/// `min_len_secs` (this is the anchor-qualification third; `07` #112). PR2 sweeps it in the
/// harness only, to prove the placement before it ships as a constant.
pub const IDENTITY_QUALIFY_MS: i64 = 120_000;

/// Why a group closed. Kept for `cmd_replay` display + PR4 explainability; deliberately NOT a
/// `SessionSpan` field (the referee shape is frozen).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    /// Inactivity gap since the last frame reached `merge_gap_ms`.
    Gap,
    /// A different recognized identity was sustained beyond `absorb_max_ms`.
    SustainedForeignIdentity,
    /// A meeting band opened or closed at this edge.
    MeetingBandEdge,
    /// End of the frame stream (the export edge; in production the still-open session).
    EndOfInput,
}

/// A grouped session with its close reason (the detailed grouping output). [`segment_grouped`]
/// projects these to bare [`SessionSpan`]s for the referee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupedSession {
    pub span: SessionSpan,
    pub close_reason: CloseReason,
}

/// The identity class of a micro-run, folded from its recognition.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Identity {
    Ai(String),
    Meeting(String),
    None,
}

fn identity_of(m: &MicroSpan) -> Identity {
    match &m.recog {
        Some(r) if r.kind == Kind::Ai => Identity::Ai(r.id.clone()),
        Some(r) if r.kind == Kind::Meeting => Identity::Meeting(r.id.clone()),
        _ => Identity::None,
    }
}

/// A free micro-run's identity: a meeting id that never formed a band is demoted to `None`
/// (absorbable noise), so only band-owned meeting frames ever anchor a meeting session.
fn free_identity(m: &MicroSpan) -> Identity {
    match identity_of(m) {
        Identity::Meeting(_) => Identity::None,
        other => other,
    }
}

/// The app stem (first `|`-separated component of the micro-run key).
fn stem_of(m: &MicroSpan) -> &str {
    m.key.split('|').next().unwrap_or("")
}

fn dur(m: &MicroSpan) -> i64 {
    (m.last_ts - m.first_ts).max(0)
}

fn midpoint(m: &MicroSpan) -> i64 {
    m.first_ts + (m.last_ts - m.first_ts) / 2
}

/// Host precedence rank (lower wins): a spinner terminal is direct evidence of an agent working;
/// a desktop chat app lingers as a companion. Frame-majority provably mis-attributes on the
/// ground truth (a desktop AI app out-frames its own terminal session), so identity is picked by
/// this order, then larger summed presence, then taxonomy file order. One-data-point choice,
/// recorded as revisit-with-more-days (`07` #112).
fn host_rank(h: Option<Host>) -> u8 {
    match h {
        Some(Host::Terminal) => 0,
        Some(Host::Ide) => 1,
        Some(Host::Desktop) => 2,
        Some(Host::Browser) => 3,
        None => 4,
    }
}

// ---- Pass 2a: meeting bands ----------------------------------------------------------------

/// A qualified meeting band: a hard-anchored, impassable session at exact presence endpoints.
struct Band {
    mstart: i64,
    mend: i64,
    id: String,
    host: Option<Host>,
    first_id: i64,
    last_id: i64,
    frames: usize,
    /// Summed member duration; the winner of an overlap merge (larger presence keeps its id).
    presence: i64,
}

fn push_band_if_qualified(
    chain: &[usize],
    micros: &[MicroSpan],
    id: &str,
    gp: &GroupParams,
    out: &mut Vec<Band>,
) {
    let (Some(&first_i), Some(&last_i)) = (chain.first(), chain.last()) else {
        return;
    };
    let first = &micros[first_i];
    let last = &micros[last_i];
    if last.last_ts - first.first_ts < gp.focus_min_len_ms {
        return; // a sub-floor meeting chain never anchors (and never splits real work).
    }
    out.push(Band {
        mstart: first.first_ts,
        mend: last.last_ts,
        id: id.to_string(),
        host: first.recog.as_ref().and_then(|r| r.host),
        first_id: first.first_id,
        last_id: last.last_id,
        frames: 0,
        presence: chain.iter().map(|&i| dur(&micros[i])).sum(),
    });
}

/// Merge overlapping bands (of any id): union the ranges, and the larger-presence band keeps its
/// id/host (a meeting inside a meeting is one session, attributed to the dominant one).
fn merge_overlapping_bands(sorted: Vec<Band>) -> Vec<Band> {
    let mut out: Vec<Band> = Vec::new();
    for b in sorted {
        match out.last_mut() {
            Some(acc) if b.mstart < acc.mend => {
                if b.presence > acc.presence {
                    acc.id = b.id;
                    acc.host = b.host;
                    acc.presence = b.presence;
                }
                if b.mend > acc.mend {
                    acc.mend = b.mend;
                    acc.last_id = b.last_id;
                }
            }
            _ => out.push(b),
        }
    }
    out
}

/// Build the qualified, non-overlapping meeting bands from the micro-runs.
fn build_bands(micros: &[MicroSpan], gp: &GroupParams) -> Vec<Band> {
    // Meeting micro-runs by id, in time order.
    let mut by_id: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, m) in micros.iter().enumerate() {
        if let Identity::Meeting(id) = identity_of(m) {
            by_id.entry(id).or_default().push(i);
        }
    }
    let mut bands: Vec<Band> = Vec::new();
    for (id, idxs) in &by_id {
        let mut chain: Vec<usize> = Vec::new();
        for &i in idxs {
            if let Some(&prev) = chain.last() {
                if micros[i].first_ts - micros[prev].last_ts > gp.meeting_gap_ms {
                    push_band_if_qualified(&chain, micros, id, gp, &mut bands);
                    chain.clear();
                }
            }
            chain.push(i);
        }
        push_band_if_qualified(&chain, micros, id, gp, &mut bands);
    }
    bands.sort_by_key(|b| (b.mstart, b.mend));
    let mut bands = merge_overlapping_bands(bands);
    // Owned frames: any micro-run whose midpoint lands in a band belongs to that band.
    for m in micros {
        let mid = midpoint(m);
        if let Some(b) = bands.iter_mut().find(|b| mid >= b.mstart && mid <= b.mend) {
            b.frames += m.frames;
        }
    }
    bands
}

// ---- Pass 2b: grouping walk ----------------------------------------------------------------

/// The open (still-accreting) group state.
struct OpenGroup {
    start_ms: i64,
    end_ms: i64,
    first_id: i64,
    last_id: i64,
    frames: usize,
    /// Summed member duration + host per AI identity present (for anchor selection).
    ai_dur: HashMap<String, i64>,
    ai_host: HashMap<String, Option<Host>>,
    /// Summed member duration + frames per app stem (for the focus context_key).
    stem_dur: HashMap<String, i64>,
    stem_frames: HashMap<String, usize>,
}

impl OpenGroup {
    fn new(m: &MicroSpan, start_clamp: i64) -> Self {
        let start = m.first_ts.max(start_clamp);
        let mut g = OpenGroup {
            start_ms: start,
            end_ms: m.last_ts.max(start),
            first_id: m.first_id,
            last_id: m.last_id,
            frames: m.frames,
            ai_dur: HashMap::new(),
            ai_host: HashMap::new(),
            stem_dur: HashMap::new(),
            stem_frames: HashMap::new(),
        };
        g.accrue(m);
        g
    }

    fn absorb(&mut self, m: &MicroSpan) {
        self.end_ms = self.end_ms.max(m.last_ts);
        self.last_id = m.last_id;
        self.frames += m.frames;
        self.accrue(m);
    }

    fn accrue(&mut self, m: &MicroSpan) {
        let d = dur(m);
        *self.stem_dur.entry(stem_of(m).to_string()).or_insert(0) += d;
        *self.stem_frames.entry(stem_of(m).to_string()).or_insert(0) += m.frames;
        if let Identity::Ai(id) = identity_of(m) {
            *self.ai_dur.entry(id.clone()).or_insert(0) += d;
            self.ai_host
                .entry(id)
                .or_insert_with(|| m.recog.as_ref().and_then(|r| r.host));
        }
    }

    /// The AI anchor (id + host), or `None` if no identity reached `qualify_ms` summed presence.
    fn anchor(&self, tax: &Taxonomy, qualify_ms: i64) -> Option<(String, Option<Host>)> {
        let mut cands: Vec<(String, i64, Option<Host>)> = self
            .ai_dur
            .iter()
            .filter(|(_, &d)| d >= qualify_ms)
            .map(|(id, &d)| (id.clone(), d, self.ai_host.get(id).copied().flatten()))
            .collect();
        cands.sort_by(|a, b| {
            host_rank(a.2)
                .cmp(&host_rank(b.2)) // precedence: terminal > ide > desktop > browser
                .then(b.1.cmp(&a.1)) // larger summed presence
                .then(tax.entry_index(&a.0).cmp(&tax.entry_index(&b.0))) // taxonomy file order
        });
        cands.first().map(|(id, _, h)| (id.clone(), *h))
    }

    /// The dominant app stem (largest summed duration; ties by frames, then lexicographically
    /// smallest) for an anchorless focus session's context key.
    fn dominant_stem(&self) -> String {
        let mut cands: Vec<(&String, i64, usize)> = self
            .stem_dur
            .iter()
            .map(|(s, &d)| (s, d, self.stem_frames.get(s).copied().unwrap_or(0)))
            .collect();
        cands.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)).then(a.0.cmp(b.0)));
        cands
            .first()
            .map(|(s, _, _)| (*s).clone())
            .unwrap_or_default()
    }
}

/// Decide whether micro-run `m` (identity `idm`, duration `dur_m`) absorbs into a group with
/// anchor `c`, or closes it. The absorb-budget rule (a foreign run up to `absorb_max_ms` is
/// absorbed, longer is a real switch) is the single change that fixes the over-segmentation.
fn should_absorb(
    c: &Option<(String, Option<Host>)>,
    idm: &Identity,
    dur_m: i64,
    gp: &GroupParams,
) -> bool {
    match (c, idm) {
        // Same identity, or unrecognized-into-unrecognized: continuity, absorb unconditionally.
        (None, Identity::None) => true,
        (Some((t, _)), Identity::Ai(u)) if u == t => true,
        // Unrecognized activity while an AI anchor is active: absorb up to the budget.
        (Some(_), Identity::None) => dur_m <= gp.absorb_max_ms,
        // A different (or first) AI identity: absorb up to the budget, else it is a real switch.
        (None, Identity::Ai(_)) => dur_m <= gp.absorb_max_ms,
        (Some(_), Identity::Ai(_)) => dur_m <= gp.absorb_max_ms,
        // `idm` is never Meeting here (free_identity demotes meetings), but stay exhaustive.
        (_, Identity::Meeting(_)) => true,
    }
}

/// The referee params that travel together through the grouping walk.
struct Ctx<'a> {
    tax: &'a Taxonomy,
    sp: &'a SegParams,
    gp: &'a GroupParams,
    qualify_ms: i64,
}

/// Finalize an open group into a `SessionSpan` (or drop it below the floors / density gate).
fn finalize(
    g: OpenGroup,
    end_override: Option<i64>,
    reason: CloseReason,
    cx: &Ctx,
    out: &mut Vec<GroupedSession>,
) {
    let (sp, gp) = (cx.sp, cx.gp);
    let end = end_override.unwrap_or(g.end_ms).max(g.start_ms);
    let duration = end - g.start_ms;

    let (kind, tool, host, key) = match g.anchor(cx.tax, cx.qualify_ms) {
        Some((id, host)) => (Kind::Ai, Some(id.clone()), host, format!("ai:{id}")),
        None => {
            let stem = g.dominant_stem();
            let key = if stem.is_empty() {
                "focus".to_string()
            } else {
                format!("focus:{stem}")
            };
            (Kind::Focus, None, None, key)
        }
    };

    // Floors + density gate. Anchored AI sessions floor at min_len and are gate-exempt; anchorless
    // focus sessions need focus_min_len AND focus_min_density_fph (0 disables the gate).
    let keep = match kind {
        Kind::Ai => duration >= sp.min_len_ms,
        _ => {
            let long_enough = duration >= gp.focus_min_len_ms;
            let dense_enough = gp.focus_min_density_fph <= 0
                || (duration > 0
                    && (g.frames as f64 * 3_600_000.0 / duration as f64)
                        >= gp.focus_min_density_fph as f64);
            long_enough && dense_enough
        }
    };
    if !keep {
        return;
    }

    out.push(GroupedSession {
        span: SessionSpan {
            start_ms: g.start_ms,
            end_ms: end,
            context_key: key,
            kind,
            tool,
            host,
            frame_count: g.frames,
            first_frame_id: g.first_id,
            last_frame_id: g.last_id,
        },
        close_reason: reason,
    });
}

/// Clamp any residual overlap (touching is allowed): a session that would start before the
/// previous one's end is pulled forward. Bands + barriers prevent overlap in practice; this is
/// the belt-and-braces the `debug_assert!` then verifies.
fn enforce_non_overlap(sessions: &mut [GroupedSession]) {
    for i in 1..sessions.len() {
        let prev_end = sessions[i - 1].span.end_ms;
        let s = &mut sessions[i].span;
        if s.start_ms < prev_end {
            s.start_ms = prev_end;
            if s.end_ms < s.start_ms {
                s.end_ms = s.start_ms;
            }
        }
    }
}

/// Pass 2 of the segmenter: micro-runs → task-level grouped sessions (with close reasons), at the
/// default [`IDENTITY_QUALIFY_MS`]. Use [`group_with`] to vary the anchor-qualification threshold
/// (the PR2 sweep proves its placement before it ships as a constant).
pub fn group(
    micros: &[MicroSpan],
    tax: &Taxonomy,
    sp: &SegParams,
    gp: &GroupParams,
) -> Vec<GroupedSession> {
    group_with(micros, tax, sp, gp, IDENTITY_QUALIFY_MS)
}

/// [`group`] with an explicit anchor-qualification threshold `qualify_ms` (harness sweep only).
pub fn group_with(
    micros: &[MicroSpan],
    tax: &Taxonomy,
    sp: &SegParams,
    gp: &GroupParams,
    qualify_ms: i64,
) -> Vec<GroupedSession> {
    let cx = Ctx {
        tax,
        sp,
        gp,
        qualify_ms,
    };
    let bands = build_bands(micros, gp);

    // Emit band sessions; partition the rest into the "free" micro-runs that drive the walk.
    let mut out: Vec<GroupedSession> = Vec::new();
    for b in &bands {
        out.push(GroupedSession {
            span: SessionSpan {
                start_ms: b.mstart,
                end_ms: b.mend,
                context_key: format!("meeting:{}", b.id),
                kind: Kind::Meeting,
                tool: None,
                host: b.host,
                frame_count: b.frames,
                first_frame_id: b.first_id,
                last_frame_id: b.last_id,
            },
            close_reason: CloseReason::MeetingBandEdge,
        });
    }
    let free: Vec<&MicroSpan> = micros
        .iter()
        .filter(|m| {
            let mid = midpoint(m);
            !bands.iter().any(|b| mid >= b.mstart && mid <= b.mend)
        })
        .collect();

    // Walk the free micro-runs. Bands are impassable barriers: a band starting within/after the
    // open group closes it clipped to the band start, and the next group opens no earlier than the
    // band end.
    let mut open: Option<OpenGroup> = None;
    let mut barrier_end = i64::MIN;
    let mut bi = 0usize;
    for m in &free {
        while bi < bands.len() && bands[bi].mstart <= m.first_ts {
            if let Some(g) = open.take() {
                let clip = g.end_ms.min(bands[bi].mstart);
                finalize(g, Some(clip), CloseReason::MeetingBandEdge, &cx, &mut out);
            }
            barrier_end = barrier_end.max(bands[bi].mend);
            bi += 1;
        }
        match open.take() {
            None => open = Some(OpenGroup::new(m, barrier_end)),
            Some(mut g) => {
                let gap = m.first_ts - g.end_ms;
                if gap >= gp.merge_gap_ms {
                    finalize(g, None, CloseReason::Gap, &cx, &mut out);
                    open = Some(OpenGroup::new(m, barrier_end));
                } else if should_absorb(&g.anchor(tax, qualify_ms), &free_identity(m), dur(m), gp) {
                    g.absorb(m);
                    open = Some(g);
                } else {
                    finalize(
                        g,
                        None,
                        CloseReason::SustainedForeignIdentity,
                        &cx,
                        &mut out,
                    );
                    open = Some(OpenGroup::new(m, barrier_end));
                }
            }
        }
    }
    if let Some(g) = open.take() {
        finalize(g, None, CloseReason::EndOfInput, &cx, &mut out);
    }

    out.sort_by_key(|s| (s.span.start_ms, s.span.first_frame_id));
    enforce_non_overlap(&mut out);
    debug_assert!(
        out.windows(2)
            .all(|w| w[0].span.end_ms <= w[1].span.start_ms
                && w[0].span.start_ms <= w[0].span.end_ms),
        "grouped sessions must be sorted + strictly non-overlapping: {out:?}"
    );
    out
}

/// The composed grouped pipeline the harness referee scores: frames → micro-runs → task-level
/// sessions. PR4 adapts its shipped segmenter's output to this shape for the D9 re-run.
pub fn segment_grouped(
    frames: &[FrameRow],
    tax: &Taxonomy,
    sp: &SegParams,
    gp: &GroupParams,
) -> Vec<SessionSpan> {
    segment_grouped_detailed(frames, tax, sp, gp)
        .into_iter()
        .map(|g| g.span)
        .collect()
}

/// As [`segment_grouped`] but keeps each session's close reason (for `cmd_replay`).
pub fn segment_grouped_detailed(
    frames: &[FrameRow],
    tax: &Taxonomy,
    sp: &SegParams,
    gp: &GroupParams,
) -> Vec<GroupedSession> {
    let micros = segment_micro(frames, tax, sp);
    group(&micros, tax, sp, gp)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Compact params so synthetic fixtures stay small. gap_close (micro) 300 s < merge_gap
    // (macro) 600 s so a same-identity macro lull can be tested; absorb_max 200 s sits above the
    // 120 s IDENTITY_QUALIFY constant so an AI run can both absorb AND qualify (the convert path).
    fn sp() -> SegParams {
        SegParams {
            gap_close_ms: 300_000,
            min_len_ms: 60_000,
        }
    }
    fn gp() -> GroupParams {
        GroupParams {
            merge_gap_ms: 600_000,
            absorb_max_ms: 200_000,
            meeting_gap_ms: 150_000,
            focus_min_len_ms: 100_000,
            focus_min_density_fph: 0,
        }
    }

    fn f(id: i64, sec: i64, app: Option<&str>, title: Option<&str>) -> FrameRow {
        FrameRow {
            frame_id: id,
            captured_at: sec * 1000,
            app_hint: app.map(str::to_string),
            window_title: title.map(str::to_string),
            browser_url: None,
        }
    }

    fn run(id0: i64, app: &str, title: &str, start: i64, end: i64, step: i64) -> Vec<FrameRow> {
        let mut v = Vec::new();
        let (mut t, mut id) = (start, id0);
        while t <= end {
            v.push(f(id, t, Some(app), Some(title)));
            t += step;
            id += 1;
        }
        v
    }

    fn spans(frames: &[FrameRow]) -> Vec<SessionSpan> {
        segment_grouped(frames, &Taxonomy::seed(), &sp(), &gp())
    }

    // Recognized-title shorthands.
    const CC: (&str, &str) = ("WindowsTerminal", "claude - repo"); // claude-code (terminal)
    const CX: (&str, &str) = ("codex", "Codex"); // codex (desktop)
    const NP: (&str, &str) = ("Notepad", "n"); // unrecognized focus
    const MEET: (&str, &str) = ("chrome", "Google Meet"); // meeting

    #[test]
    fn empty_and_keyless_produce_no_sessions() {
        assert!(spans(&[]).is_empty());
        let keyless = vec![f(1, 0, None, None), f(2, 100, None, None)];
        assert!(spans(&keyless).is_empty());
    }

    #[test]
    fn single_ai_run_is_one_anchored_session() {
        let s = spans(&run(1, CC.0, CC.1, 0, 200, 20));
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].kind, Kind::Ai);
        assert_eq!(s[0].tool.as_deref(), Some("claude-code"));
        assert_eq!(s[0].host, Some(Host::Terminal));
        assert_eq!(s[0].context_key, "ai:claude-code");
        assert_eq!((s[0].start_ms, s[0].end_ms), (0, 200_000));
    }

    #[test]
    fn short_foreign_ai_run_is_absorbed() {
        // claude, a 90 s codex run (< absorb_max), claude again -> ONE claude session (the 71
        // short Codex runs inside a labeled Claude evening are this shape).
        let mut frames = run(1, CC.0, CC.1, 0, 150, 30);
        frames.extend(run(100, CX.0, CX.1, 180, 270, 30)); // 90 s
        frames.extend(run(200, CC.0, CC.1, 300, 450, 30));
        let s = spans(&frames);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].tool.as_deref(), Some("claude-code"));
        assert_eq!((s[0].start_ms, s[0].end_ms), (0, 450_000));
    }

    #[test]
    fn sustained_foreign_ai_runs_split() {
        // Each foreign run is 250 s (> absorb_max) -> three adjacent AI sessions.
        let mut frames = run(1, CC.0, CC.1, 0, 150, 30);
        frames.extend(run(100, CX.0, CX.1, 180, 430, 30)); // 250 s codex
        frames.extend(run(200, CC.0, CC.1, 460, 710, 30)); // 250 s claude
        let s = spans(&frames);
        assert_eq!(s.len(), 3, "{s:?}");
        assert_eq!(s[0].tool.as_deref(), Some("claude-code"));
        assert_eq!(s[1].tool.as_deref(), Some("codex"));
        assert_eq!(s[2].tool.as_deref(), Some("claude-code"));
    }

    #[test]
    fn unrecognized_excursion_below_budget_is_absorbed() {
        let mut frames = run(1, CC.0, CC.1, 0, 200, 30);
        frames.extend(run(100, NP.0, NP.1, 230, 320, 30)); // 90 s unrecognized
        frames.extend(run(200, CC.0, CC.1, 350, 500, 30));
        let s = spans(&frames);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].tool.as_deref(), Some("claude-code"));
        assert_eq!(s[0].end_ms, 500_000);
    }

    #[test]
    fn unrecognized_excursion_above_budget_splits_off_focus() {
        // A 220 s unrecognized block (> absorb_max, >= focus_min_len) between two long AI runs
        // becomes its own focus session.
        let mut frames = run(1, CC.0, CC.1, 0, 200, 30);
        frames.extend(run(100, NP.0, NP.1, 230, 450, 30)); // 220 s focus
        frames.extend(run(200, CC.0, CC.1, 480, 730, 30)); // 250 s claude (> absorb_max -> splits)
        let s = spans(&frames);
        assert_eq!(s.len(), 3, "{s:?}");
        assert_eq!(s[0].kind, Kind::Ai);
        assert_eq!(s[1].kind, Kind::Focus);
        assert_eq!(s[1].tool, None);
        assert_eq!(s[1].context_key, "focus:notepad");
        // Notepad's last frame is at 440 s (230..=450 step 30).
        assert_eq!((s[1].start_ms, s[1].end_ms), (230_000, 440_000));
        assert_eq!(s[2].kind, Kind::Ai);
    }

    #[test]
    fn intra_session_lull_below_merge_gap_holds() {
        // Same identity, a 350 s gap (> micro gap_close 300, < merge_gap 600) -> one session.
        let mut frames = run(1, CC.0, CC.1, 0, 150, 30);
        frames.extend(run(100, CC.0, CC.1, 500, 650, 30));
        let s = spans(&frames);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!((s[0].start_ms, s[0].end_ms), (0, 650_000));
    }

    #[test]
    fn gap_at_merge_gap_splits() {
        // A 650 s gap (>= merge_gap 600) splits the same identity into two sessions.
        let mut frames = run(1, CC.0, CC.1, 0, 150, 30);
        frames.extend(run(100, CC.0, CC.1, 800, 950, 30));
        let s = spans(&frames);
        assert_eq!(s.len(), 2, "{s:?}");
        assert_eq!(s[0].end_ms, 150_000);
        assert_eq!(s[1].start_ms, 800_000);
    }

    #[test]
    fn focus_ramp_converts_to_ai_keeping_start() {
        // A group opened on a focus ramp acquires its AI anchor and KEEPS its start (the 16:59
        // shape) — no start-snap post-pass. The 150 s codex run absorbs (<= absorb_max) and
        // qualifies (>= IDENTITY_QUALIFY), flipping the anchor.
        let mut frames = run(1, NP.0, NP.1, 0, 80, 20);
        frames.extend(run(100, CX.0, CX.1, 110, 260, 30)); // 150 s codex
        let s = spans(&frames);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].kind, Kind::Ai);
        assert_eq!(s[0].tool.as_deref(), Some("codex"));
        assert_eq!(s[0].start_ms, 0, "start retained from the focus ramp");
        assert_eq!(s[0].end_ms, 260_000);
    }

    #[test]
    fn meeting_band_is_a_hard_session_at_presence_endpoints() {
        let s = spans(&run(1, MEET.0, MEET.1, 0, 300, 30));
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].kind, Kind::Meeting);
        assert_eq!(s[0].tool, None, "meetings carry no tool id");
        assert_eq!(s[0].context_key, "meeting:meet");
        assert_eq!(s[0].host, Some(Host::Browser));
        assert_eq!((s[0].start_ms, s[0].end_ms), (0, 300_000));
    }

    #[test]
    fn meeting_band_splits_the_surrounding_work() {
        let mut frames = run(1, CC.0, CC.1, 0, 200, 30);
        frames.extend(run(100, MEET.0, MEET.1, 250, 550, 30)); // 300 s meeting band
        frames.extend(run(200, CC.0, CC.1, 600, 800, 30));
        let s = spans(&frames);
        assert_eq!(s.len(), 3, "{s:?}");
        assert_eq!(s[0].kind, Kind::Ai);
        // The claude run's last frame is at 180 s (0..=200 step 30); the band starts at 250 s.
        assert_eq!((s[0].start_ms, s[0].end_ms), (0, 180_000));
        assert_eq!(s[1].kind, Kind::Meeting);
        assert_eq!((s[1].start_ms, s[1].end_ms), (250_000, 550_000));
        assert_eq!(s[2].kind, Kind::Ai);
        assert_eq!(s[2].start_ms, 600_000);
        assert!(s[0].end_ms <= s[1].start_ms && s[1].end_ms <= s[2].start_ms);
    }

    #[test]
    fn short_meeting_chain_is_demoted_not_a_band() {
        // A 70 s meet run (< focus_min_len) forms no band and never splits real work: it is
        // absorbed into the following AI session as noise.
        let mut frames = run(1, MEET.0, MEET.1, 0, 70, 30);
        frames.extend(run(100, CC.0, CC.1, 100, 300, 30));
        let s = spans(&frames);
        assert!(
            s.iter().all(|x| x.kind != Kind::Meeting),
            "no fake band: {s:?}"
        );
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].tool.as_deref(), Some("claude-code"));
    }

    #[test]
    fn anchorless_focus_below_floor_is_dropped() {
        // 80 s < focus_min_len 100 -> no session (frames stay sessionless).
        assert!(spans(&run(1, NP.0, NP.1, 0, 80, 20)).is_empty());
    }

    #[test]
    fn anchorless_focus_above_floor_is_kept() {
        let s = spans(&run(1, NP.0, NP.1, 0, 200, 20));
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].kind, Kind::Focus);
        assert_eq!(s[0].tool, None);
        assert_eq!(s[0].context_key, "focus:notepad");
    }

    #[test]
    fn density_gate_suppresses_sparse_focus_but_keeps_dense() {
        let gate = GroupParams {
            focus_min_density_fph: 90,
            ..gp()
        };
        // 7 frames over 600 s = 42 fph < 90 -> suppressed.
        let sparse = run(1, NP.0, NP.1, 0, 600, 100);
        assert!(
            segment_grouped(&sparse, &Taxonomy::seed(), &sp(), &gate).is_empty(),
            "sparse anchorless focus is suppressed"
        );
        // 31 frames over 600 s = 186 fph > 90 -> kept.
        let dense = run(1, NP.0, NP.1, 0, 600, 20);
        let d = segment_grouped(&dense, &Taxonomy::seed(), &sp(), &gate);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].kind, Kind::Focus);
    }

    #[test]
    fn anchored_ai_is_exempt_from_the_density_gate() {
        let gate = GroupParams {
            focus_min_density_fph: 90,
            ..gp()
        };
        // 3 claude frames over 200 s = 54 fph < 90, but AI is exempt -> kept.
        let sparse_ai = run(1, CC.0, CC.1, 0, 200, 100);
        let s = segment_grouped(&sparse_ai, &Taxonomy::seed(), &sp(), &gate);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].kind, Kind::Ai);
    }

    #[test]
    fn host_precedence_picks_terminal_over_desktop() {
        // A group where both claude-code (terminal) and codex (desktop) qualify: terminal wins,
        // even though frame-majority could favor the desktop app.
        let mut frames = run(1, CC.0, CC.1, 0, 150, 30);
        frames.extend(run(100, CX.0, CX.1, 180, 330, 30)); // 150 s codex, absorbs
        let s = spans(&frames);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].tool.as_deref(), Some("claude-code"));
    }

    #[test]
    fn sub_qualify_ai_run_does_not_flip_a_focus_session() {
        // A 100 s claude run (>= micro dwell, < IDENTITY_QUALIFY 120 s) inside a focus block does
        // not anchor it: the session stays focus (the IDENTITY_QUALIFY guard).
        let mut frames = run(1, NP.0, NP.1, 0, 300, 30);
        frames.extend(run(100, CC.0, CC.1, 330, 430, 25)); // 100 s claude
        let s = spans(&frames);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].kind, Kind::Focus);
        assert_eq!(s[0].context_key, "focus:notepad");
    }

    #[test]
    fn back_to_back_sustained_tools_split_at_the_handoff() {
        // Codex then Claude, no gap, both sustained (> absorb_max): two adjacent AI sessions that
        // touch (the visible handoff; the invisible 19:00 case is out of reach by design).
        let mut frames = run(1, CX.0, CX.1, 0, 300, 30);
        frames.extend(run(100, CC.0, CC.1, 300, 600, 30));
        let s = spans(&frames);
        assert_eq!(s.len(), 2, "{s:?}");
        assert_eq!(s[0].tool.as_deref(), Some("codex"));
        assert_eq!(s[1].tool.as_deref(), Some("claude-code"));
        assert_eq!(s[0].end_ms, s[1].start_ms, "touching, non-overlapping");
    }

    #[test]
    fn mixed_day_output_is_sorted_and_non_overlapping() {
        let mut frames = run(1, CC.0, CC.1, 0, 200, 30);
        frames.extend(run(100, MEET.0, MEET.1, 250, 550, 30));
        frames.extend(run(200, CX.0, CX.1, 600, 800, 30));
        frames.extend(run(300, NP.0, NP.1, 1600, 1800, 20)); // after a > merge_gap gap
        let s = spans(&frames);
        for w in s.windows(2) {
            assert!(
                w[0].end_ms <= w[1].start_ms,
                "sorted + non-overlapping: {s:?}"
            );
            assert!(w[0].start_ms <= w[0].end_ms, "non-inverted span");
        }
        assert!(s.len() >= 3, "{s:?}");
    }
}

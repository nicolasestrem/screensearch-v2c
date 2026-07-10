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

// ---- Concurrent per-identity-track walk (`07` #114 / `06` #28) --------------------------------
//
// The concurrent model resolves `07` #114: sessions of **different identities may overlap** in
// wall-clock time (an AI track spans through a meeting; two AI tools run at once), while a frame
// still belongs to **exactly one** session (exclusive ownership — one `app_hint` → one micro-run →
// one track). The serial single-open-group walk above (`group`/`segment_grouped`) is byte-untouched
// and kept as the `--algo grouped` A/B baseline; this path is `--algo concurrent`.
//
// Shape: the single open group becomes a **map of open AI tracks** (keyed by tool id) plus one open
// **focus** track. An AI micro extends/opens its own id's track (a foreign AI id opens ITS OWN track
// rather than closing the incumbent — no `SustainedForeignIdentity` close here). A short
// unrecognized run is buffered and absorbed into the **last-touched** open AI track (the per-track
// transcription of the serial absorb-budget), a leading one **ramps** into an opening track
// (convert-not-split), and a run over `absorb_max_ms` is focus material. Anchor **selection** and
// `HOST_PRECEDENCE` are inert here (each id is its own track), but anchor **qualification** survives:
// an AI track emits only if its summed own-recognized presence ≥ `IDENTITY_QUALIFY_MS` (the span
// floor alone passes hollow chains). Meeting bands are **no longer barriers** (an AI track spans a
// meeting) and overlapping meetings are **not merged** (they emit overlapping sessions).

/// An open per-identity track: an AI id (the track key), or the single residual focus track.
struct OpenTrack {
    start_ms: i64,
    end_ms: i64,
    first_id: i64,
    last_id: i64,
    frames: usize,
    /// `Some(id)` for an AI track (the tool id = the track key); `None` for the focus track.
    ai_id: Option<String>,
    ai_host: Option<Host>,
    /// Summed **recognized** presence (ms). Only AI micros of this id add to it; absorbed
    /// unrecognized runs and the leading ramp extend the span + frame_count but never the presence
    /// (the `IDENTITY_QUALIFY_MS` guard — a hollow chain must not emit an AI session).
    ai_presence: i64,
    /// Focus context-key stats (populated only for the focus track, from its unrecognized runs).
    stem_dur: HashMap<String, i64>,
    stem_frames: HashMap<String, usize>,
}

impl OpenTrack {
    fn new_ai(m: &MicroSpan, id: &str, host: Option<Host>) -> Self {
        OpenTrack {
            start_ms: m.first_ts,
            end_ms: m.last_ts.max(m.first_ts),
            first_id: m.first_id,
            last_id: m.last_id,
            frames: m.frames,
            ai_id: Some(id.to_string()),
            ai_host: host,
            ai_presence: dur(m),
            stem_dur: HashMap::new(),
            stem_frames: HashMap::new(),
        }
    }

    fn new_focus(m: &MicroSpan) -> Self {
        let mut t = OpenTrack {
            start_ms: m.first_ts,
            end_ms: m.last_ts.max(m.first_ts),
            first_id: m.first_id,
            last_id: m.last_id,
            frames: m.frames,
            ai_id: None,
            ai_host: None,
            ai_presence: 0,
            stem_dur: HashMap::new(),
            stem_frames: HashMap::new(),
        };
        t.accrue_stem(m);
        t
    }

    fn accrue_stem(&mut self, m: &MicroSpan) {
        *self.stem_dur.entry(stem_of(m).to_string()).or_insert(0) += dur(m);
        *self.stem_frames.entry(stem_of(m).to_string()).or_insert(0) += m.frames;
    }

    /// Extend the span + frame_count with a later micro-run (`last_id` follows the latest end).
    fn extend_span(&mut self, m: &MicroSpan) {
        if m.last_ts >= self.end_ms {
            self.end_ms = m.last_ts;
            self.last_id = m.last_id;
        }
        self.frames += m.frames;
    }

    /// Absorb a recognized AI micro of this track's id (extends span + presence).
    fn absorb_ai(&mut self, m: &MicroSpan) {
        self.ai_presence += dur(m);
        self.extend_span(m);
    }

    /// Absorb an unrecognized micro (extends span + frame_count, NOT presence). The focus track
    /// also accrues the stem stats that pick its `focus:<stem>` context key.
    fn absorb_none(&mut self, m: &MicroSpan) {
        self.extend_span(m);
        if self.ai_id.is_none() {
            self.accrue_stem(m);
        }
    }

    /// Prepend a leading focus ramp (frames captured before the AI id was recognized): extend the
    /// start backward + add frames, never presence (the convert-not-split start retention).
    fn prepend_ramp(&mut self, start_ms: i64, first_id: i64, frames: usize) {
        if start_ms < self.start_ms {
            self.start_ms = start_ms;
            self.first_id = first_id;
        }
        self.frames += frames;
    }

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

/// Build the qualified meeting bands for the concurrent path: like [`build_bands`] but **without**
/// `merge_overlapping_bands` (concurrent meetings stay separate, emitting overlapping sessions) and
/// with band-frame ownership skipping AI micros (an AI micro inside a band stays free, feeding its
/// own track — the band owns only its meeting micros + interior unrecognized micros).
fn build_bands_concurrent(micros: &[MicroSpan], gp: &GroupParams) -> Vec<Band> {
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
    // Deterministic order; NO merge (overlapping meetings are distinct concurrent sessions).
    bands.sort_by(|a, b| {
        a.mstart
            .cmp(&b.mstart)
            .then(a.mend.cmp(&b.mend))
            .then(a.id.cmp(&b.id))
    });
    for m in micros {
        if matches!(identity_of(m), Identity::Ai(_)) {
            continue; // AI micros inside a band stay free (feed their own track).
        }
        if let Some(bi) = owning_band_index(midpoint(m), &bands) {
            bands[bi].frames += m.frames;
        }
    }
    bands
}

/// The band owning a non-AI micro at `mid`: the one containing it with the largest presence
/// (ties broken by earliest start), or `None` if no band contains it.
fn owning_band_index(mid: i64, bands: &[Band]) -> Option<usize> {
    bands
        .iter()
        .enumerate()
        .filter(|(_, b)| mid >= b.mstart && mid <= b.mend)
        .max_by(|(_, a), (_, b)| {
            a.presence.cmp(&b.presence).then(b.mstart.cmp(&a.mstart)) // tie: earlier start wins
        })
        .map(|(i, _)| i)
}

/// The identity-track partition key for per-track non-overlap: an AI/meeting session partitions on
/// its `context_key` (one track per id), and every focus session shares one `"focus"` partition
/// (the single residual focus track, whatever its `focus:<stem>` key).
fn track_partition(s: &SessionSpan) -> String {
    match s.kind {
        Kind::Focus => "focus".to_string(),
        _ => s.context_key.clone(),
    }
}

/// Finalize an open track into a `SessionSpan` (or drop it below the floors / density gate).
/// AI tracks floor at `min_len_ms` AND require `qualify_ms` recognized presence; focus tracks floor
/// at `focus_min_len_ms` AND pass the density gate (AI exempt).
fn finalize_track(t: OpenTrack, reason: CloseReason, cx: &Ctx, out: &mut Vec<GroupedSession>) {
    let (sp, gp) = (cx.sp, cx.gp);
    let end = t.end_ms.max(t.start_ms);
    let duration = end - t.start_ms;
    let (kind, tool, host, key, keep) = match &t.ai_id {
        Some(id) => {
            let keep = duration >= sp.min_len_ms && t.ai_presence >= cx.qualify_ms;
            (
                Kind::Ai,
                Some(id.clone()),
                t.ai_host,
                format!("ai:{id}"),
                keep,
            )
        }
        None => {
            let stem = t.dominant_stem();
            let key = if stem.is_empty() {
                "focus".to_string()
            } else {
                format!("focus:{stem}")
            };
            let long_enough = duration >= gp.focus_min_len_ms;
            let dense_enough = gp.focus_min_density_fph <= 0
                || (duration > 0
                    && (t.frames as f64 * 3_600_000.0 / duration as f64)
                        >= gp.focus_min_density_fph as f64);
            (Kind::Focus, None, None, key, long_enough && dense_enough)
        }
    };
    if !keep {
        return;
    }
    out.push(GroupedSession {
        span: SessionSpan {
            start_ms: t.start_ms,
            end_ms: end,
            context_key: key,
            kind,
            tool,
            host,
            frame_count: t.frames,
            first_frame_id: t.first_id,
            last_frame_id: t.last_id,
        },
        close_reason: reason,
    });
}

/// Extend or open the single focus track with an unrecognized micro, closing it first on its own
/// inactivity gap ≥ `merge_gap_ms`.
fn focus_extend_or_open(
    focus: &mut Option<OpenTrack>,
    m: &MicroSpan,
    mg: i64,
    cx: &Ctx,
    out: &mut Vec<GroupedSession>,
) {
    if let Some(fo) = focus.as_ref() {
        if m.first_ts - fo.end_ms >= mg {
            finalize_track(focus.take().unwrap(), CloseReason::Gap, cx, out);
        }
    }
    match focus.as_mut() {
        Some(fo) => fo.absorb_none(m),
        None => *focus = Some(OpenTrack::new_focus(m)),
    }
}

/// Attribute every buffered unrecognized ("pending") micro-run. Each goes to the **last-touched**
/// open AI track (max `end_ms`, within `merge_gap`); failing that, to the `ramp` target if one is
/// opening and contiguous (accumulated + returned for `prepend_ramp`); failing that, to focus.
#[allow(clippy::too_many_arguments)]
fn flush_pending(
    pending: &mut Vec<&MicroSpan>,
    ramp: Option<(&str, i64)>,
    tracks: &mut HashMap<String, OpenTrack>,
    focus: &mut Option<OpenTrack>,
    mg: i64,
    cx: &Ctx,
    out: &mut Vec<GroupedSession>,
) -> Option<(i64, i64, usize)> {
    let mut ramp_start: Option<i64> = None;
    let mut ramp_first: i64 = i64::MAX;
    let mut ramp_frames: usize = 0;
    for n in pending.drain(..) {
        // Last-touched open AI track within merge_gap (deterministic tie-break by id).
        let best = tracks
            .iter()
            .filter(|(_, t)| n.first_ts - t.end_ms < mg)
            .max_by(|a, b| a.1.end_ms.cmp(&b.1.end_ms).then(a.0.cmp(b.0)))
            .map(|(k, _)| k.clone());
        if let Some(k) = best {
            tracks.get_mut(&k).unwrap().absorb_none(n);
        } else if let Some((_, r_start)) = ramp.filter(|(_, rs)| rs - n.last_ts < mg) {
            ramp_start = Some(ramp_start.map_or(n.first_ts, |s| s.min(n.first_ts)));
            ramp_first = ramp_first.min(n.first_id);
            ramp_frames += n.frames;
            let _ = r_start;
        } else {
            focus_extend_or_open(focus, n, mg, cx, out);
        }
    }
    ramp_start.map(|s| (s, ramp_first, ramp_frames))
}

/// Pass 2 (concurrent): micro-runs → per-identity concurrent grouped sessions, at an explicit
/// `qualify_ms` (the sweep varies it). Same-identity sessions never overlap; different identities
/// may. Output is globally sorted by start; a `debug_assert!` guards per-track non-overlap.
pub fn group_concurrent(
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
    let mg = gp.merge_gap_ms;
    let bands = build_bands_concurrent(micros, gp);

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

    // Free micro-runs: every AI micro (even inside a band), and every non-AI micro not owned by a
    // band. `segment_micro` already returns them sorted by (first_ts, first_id).
    let free: Vec<&MicroSpan> = micros
        .iter()
        .filter(|m| {
            matches!(identity_of(m), Identity::Ai(_))
                || owning_band_index(midpoint(m), &bands).is_none()
        })
        .collect();

    let mut tracks: HashMap<String, OpenTrack> = HashMap::new();
    let mut focus: Option<OpenTrack> = None;
    let mut pending: Vec<&MicroSpan> = Vec::new();

    for &m in &free {
        // Stale buffered runs (disconnected from m by >= merge_gap) settle before m is handled.
        if let Some(last) = pending.last() {
            if m.first_ts - last.last_ts >= mg {
                flush_pending(
                    &mut pending,
                    None,
                    &mut tracks,
                    &mut focus,
                    mg,
                    &cx,
                    &mut out,
                );
            }
        }
        match free_identity(m) {
            Identity::Ai(aid) => {
                // Close this id's own track if it gapped out; a fresh one opens below.
                let gapped = tracks
                    .get(&aid)
                    .is_some_and(|t| m.first_ts - t.end_ms >= mg);
                if gapped {
                    finalize_track(
                        tracks.remove(&aid).unwrap(),
                        CloseReason::Gap,
                        &cx,
                        &mut out,
                    );
                }
                let is_open = tracks.contains_key(&aid);
                let ramp_target = if is_open {
                    None
                } else {
                    Some((aid.as_str(), m.first_ts))
                };
                let ramp_acc = flush_pending(
                    &mut pending,
                    ramp_target,
                    &mut tracks,
                    &mut focus,
                    mg,
                    &cx,
                    &mut out,
                );
                if is_open {
                    tracks.get_mut(&aid).unwrap().absorb_ai(m);
                } else {
                    let host = m.recog.as_ref().and_then(|r| r.host);
                    let mut t = OpenTrack::new_ai(m, &aid, host);
                    if let Some((rs, rf, rc)) = ramp_acc {
                        t.prepend_ramp(rs, rf, rc);
                    }
                    tracks.insert(aid, t);
                }
            }
            _ => {
                // Unrecognized (including demoted sub-floor meetings).
                if dur(m) > gp.absorb_max_ms {
                    // A sustained focus block: settle pending, then extend/open the focus track.
                    flush_pending(
                        &mut pending,
                        None,
                        &mut tracks,
                        &mut focus,
                        mg,
                        &cx,
                        &mut out,
                    );
                    focus_extend_or_open(&mut focus, m, mg, &cx, &mut out);
                } else {
                    pending.push(m);
                }
            }
        }
    }
    // End of input: settle remaining pending, then finalize every still-open track.
    flush_pending(
        &mut pending,
        None,
        &mut tracks,
        &mut focus,
        mg,
        &cx,
        &mut out,
    );
    let mut remaining: Vec<OpenTrack> = tracks.into_values().collect();
    if let Some(fo) = focus.take() {
        remaining.push(fo);
    }
    for t in remaining {
        finalize_track(t, CloseReason::EndOfInput, &cx, &mut out);
    }

    // Global sort by start (then frame id, then key for determinism); per-track non-overlap.
    out.sort_by(|a, b| {
        a.span
            .start_ms
            .cmp(&b.span.start_ms)
            .then(a.span.first_frame_id.cmp(&b.span.first_frame_id))
            .then(a.span.context_key.cmp(&b.span.context_key))
    });
    enforce_non_overlap_per_track(&mut out);
    debug_assert!(
        out.windows(2)
            .all(|w| w[0].span.start_ms <= w[1].span.start_ms),
        "concurrent sessions must be globally sorted by start: {out:?}"
    );
    debug_assert!(
        {
            let mut last: HashMap<String, i64> = HashMap::new();
            out.iter().all(|gs| {
                let ok = gs.span.start_ms <= gs.span.end_ms
                    && last
                        .get(&track_partition(&gs.span))
                        .is_none_or(|&pe| gs.span.start_ms >= pe);
                last.insert(track_partition(&gs.span), gs.span.end_ms);
                ok
            })
        },
        "same-identity sessions must be non-overlapping (cross-identity overlap is allowed): {out:?}"
    );
    out
}

/// Clamp any residual **same-track** overlap (touching allowed) by pulling a later same-identity
/// start forward to the previous same-track end. Cross-identity overlap is left intact. Sessions
/// must already be globally sorted by start.
fn enforce_non_overlap_per_track(sessions: &mut [GroupedSession]) {
    let mut last_end: HashMap<String, i64> = HashMap::new();
    for gs in sessions.iter_mut() {
        let key = track_partition(&gs.span);
        if let Some(&pe) = last_end.get(&key) {
            if gs.span.start_ms < pe {
                gs.span.start_ms = pe;
                if gs.span.end_ms < gs.span.start_ms {
                    gs.span.end_ms = gs.span.start_ms;
                }
            }
        }
        last_end.insert(key, gs.span.end_ms);
    }
}

/// The composed concurrent pipeline the referee scores: frames → micro-runs → concurrent
/// per-identity sessions (`--algo concurrent`). PR4 adapts its shipped segmenter to this shape.
pub fn segment_concurrent(
    frames: &[FrameRow],
    tax: &Taxonomy,
    sp: &SegParams,
    gp: &GroupParams,
) -> Vec<SessionSpan> {
    segment_concurrent_detailed(frames, tax, sp, gp)
        .into_iter()
        .map(|g| g.span)
        .collect()
}

/// As [`segment_concurrent`] but keeps each session's close reason (for `cmd_replay`).
pub fn segment_concurrent_detailed(
    frames: &[FrameRow],
    tax: &Taxonomy,
    sp: &SegParams,
    gp: &GroupParams,
) -> Vec<GroupedSession> {
    let micros = segment_micro(frames, tax, sp);
    group_concurrent(&micros, tax, sp, gp, IDENTITY_QUALIFY_MS)
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

    // ---- Concurrent path (`07` #114 / `06` #28): --algo concurrent ---------------------------

    const ZOOM: (&str, &str) = ("zoom", "Zoom Meeting"); // a second meeting id, for overlap tests

    fn spans_cc(frames: &[FrameRow]) -> Vec<SessionSpan> {
        segment_concurrent(frames, &Taxonomy::seed(), &sp(), &gp())
    }

    /// The `ai:<tool>` session of one tool from a concurrent result (asserts uniqueness).
    fn ai(s: &[SessionSpan], tool: &str) -> SessionSpan {
        let mut it = s.iter().filter(|x| x.tool.as_deref() == Some(tool));
        let found = it
            .next()
            .unwrap_or_else(|| panic!("no {tool} session in {s:?}"));
        assert!(it.next().is_none(), "expected one {tool} session: {s:?}");
        found.clone()
    }

    #[test]
    fn two_tools_interleaved_form_two_overlapping_sessions() {
        // claude runs [0-150],[360-510]; codex runs [180-330],[540-690] - the tools alternate
        // foreground but both work through the same window -> two OVERLAPPING sessions.
        let mut frames = run(1, CC.0, CC.1, 0, 150, 30);
        frames.extend(run(100, CX.0, CX.1, 180, 330, 30));
        frames.extend(run(200, CC.0, CC.1, 360, 510, 30));
        frames.extend(run(300, CX.0, CX.1, 540, 690, 30));
        let s = spans_cc(&frames);
        assert_eq!(s.len(), 2, "{s:?}");
        let cc = ai(&s, "claude-code");
        let cx = ai(&s, "codex");
        assert_eq!((cc.start_ms, cc.end_ms), (0, 510_000));
        assert_eq!((cx.start_ms, cx.end_ms), (180_000, 690_000));
        assert!(
            cx.start_ms < cc.end_ms,
            "the two AI tracks overlap in wall-clock"
        );
    }

    #[test]
    fn same_track_never_overlaps_itself() {
        // Same tool split by a >= merge_gap gap -> two non-overlapping claude sessions.
        let mut frames = run(1, CC.0, CC.1, 0, 150, 30);
        frames.extend(run(100, CC.0, CC.1, 800, 950, 30)); // gap 650 >= merge_gap 600
        let s = spans_cc(&frames);
        assert_eq!(s.len(), 2, "{s:?}");
        assert!(s.iter().all(|x| x.tool.as_deref() == Some("claude-code")));
        assert!(
            s[0].end_ms <= s[1].start_ms,
            "same track cannot overlap itself"
        );
    }

    #[test]
    fn sustained_foreign_run_no_longer_splits_a_track() {
        // The serial `sustained_foreign_ai_runs_split` fixture, now CONCURRENT: claude is ONE
        // continuous track overlapping a codex track, not three serial sessions.
        let mut frames = run(1, CC.0, CC.1, 0, 150, 30);
        frames.extend(run(100, CX.0, CX.1, 180, 430, 30)); // 250 s codex
        frames.extend(run(200, CC.0, CC.1, 460, 710, 30)); // 250 s claude
        let s = spans_cc(&frames);
        assert_eq!(s.len(), 2, "{s:?}");
        let cc = ai(&s, "claude-code");
        let cx = ai(&s, "codex");
        assert_eq!((cc.start_ms, cc.end_ms), (0, 700_000), "claude continuous");
        assert_eq!(
            (cx.start_ms, cx.end_ms),
            (180_000, 420_000),
            "codex inside it"
        );
    }

    #[test]
    fn short_foreign_run_dropped_by_qualification() {
        // A 90 s codex run (presence 90 < IDENTITY_QUALIFY 120) between claude runs: claude is one
        // session; the codex track fails qualification and emits nothing (frames sessionless).
        let mut frames = run(1, CC.0, CC.1, 0, 300, 30);
        frames.extend(run(100, CX.0, CX.1, 330, 420, 30)); // 90 s codex
        frames.extend(run(200, CC.0, CC.1, 450, 600, 30));
        let s = spans_cc(&frames);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].tool.as_deref(), Some("claude-code"));
        assert_eq!((s[0].start_ms, s[0].end_ms), (0, 600_000));
    }

    #[test]
    fn none_sandwich_within_budget_absorbs_into_the_track() {
        // claude, a 90 s unrecognized run (<= absorb_max), claude -> one claude session that owns
        // the unrecognized frames.
        let mut frames = run(1, CC.0, CC.1, 0, 200, 30);
        frames.extend(run(100, NP.0, NP.1, 230, 320, 30)); // 90 s unrecognized
        frames.extend(run(200, CC.0, CC.1, 350, 500, 30));
        let s = spans_cc(&frames);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].tool.as_deref(), Some("claude-code"));
        assert_eq!((s[0].start_ms, s[0].end_ms), (0, 500_000));
    }

    #[test]
    fn none_run_over_budget_becomes_focus_overlapping_the_track() {
        // The serial `unrecognized_excursion_above_budget` fixture, now CONCURRENT: claude is one
        // continuous track and the 220 s unrecognized block is a focus session OVERLAPPING it (2,
        // not the serial 3).
        let mut frames = run(1, CC.0, CC.1, 0, 200, 30);
        frames.extend(run(100, NP.0, NP.1, 230, 450, 30)); // 220 s > absorb_max 200
        frames.extend(run(200, CC.0, CC.1, 480, 730, 30));
        let s = spans_cc(&frames);
        assert_eq!(s.len(), 2, "{s:?}");
        let cc = ai(&s, "claude-code");
        let focus = s.iter().find(|x| x.kind == Kind::Focus).unwrap();
        assert_eq!((cc.start_ms, cc.end_ms), (0, 720_000), "claude continuous");
        assert_eq!(focus.context_key, "focus:notepad");
        assert!(
            focus.start_ms < cc.end_ms && focus.start_ms > cc.start_ms,
            "focus overlaps claude"
        );
    }

    #[test]
    fn trailing_none_extends_the_last_touched_track() {
        // claude then a 90 s unrecognized tail at end-of-input -> claude's end extends past it.
        let mut frames = run(1, CC.0, CC.1, 0, 200, 30);
        frames.extend(run(100, NP.0, NP.1, 230, 320, 30));
        let s = spans_cc(&frames);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].tool.as_deref(), Some("claude-code"));
        assert_eq!(
            s[0].end_ms, 320_000,
            "end extended over the trailing unrecognized tail"
        );
    }

    #[test]
    fn leading_none_ramp_attaches_to_an_opening_track() {
        // The serial focus-ramp fixture, concurrent: a leading 80 s focus ramp converts into the
        // opening codex session, which KEEPS start 0.
        let mut frames = run(1, NP.0, NP.1, 0, 80, 20);
        frames.extend(run(100, CX.0, CX.1, 110, 260, 30)); // 150 s codex
        let s = spans_cc(&frames);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].tool.as_deref(), Some("codex"));
        assert_eq!(
            (s[0].start_ms, s[0].end_ms),
            (0, 260_000),
            "ramp retains start 0"
        );
    }

    #[test]
    fn ramp_does_not_fire_when_a_track_is_open() {
        // claude open, a 60 s unrecognized run, codex opens: the run absorbs into the LAST-TOUCHED
        // open track (claude), and codex starts at its own first frame (no ramp-back).
        let mut frames = run(1, CC.0, CC.1, 0, 200, 30);
        frames.extend(run(100, NP.0, NP.1, 230, 290, 30)); // 60 s unrecognized
        frames.extend(run(200, CX.0, CX.1, 320, 470, 30));
        let s = spans_cc(&frames);
        assert_eq!(s.len(), 2, "{s:?}");
        let cc = ai(&s, "claude-code");
        let cx = ai(&s, "codex");
        assert_eq!(cc.end_ms, 290_000, "claude absorbed the unrecognized run");
        assert_eq!(
            cx.start_ms, 320_000,
            "codex opens at its own start, no ramp"
        );
    }

    #[test]
    fn scattered_sub_qualify_presence_emits_no_ai_session() {
        // A 100 s claude run (< IDENTITY_QUALIFY 120) beside a focus block: no AI session, the
        // focus session stands (the qualification guard, concurrent form).
        let mut frames = run(1, NP.0, NP.1, 0, 300, 30);
        frames.extend(run(100, CC.0, CC.1, 330, 430, 25)); // 100 s claude
        let s = spans_cc(&frames);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].kind, Kind::Focus);
        assert_eq!(s[0].context_key, "focus:notepad");
    }

    #[test]
    fn low_density_background_ai_track_survives() {
        // Sparse claude runs chained under merge_gap: low frame density, but AI is density-exempt
        // and the summed presence qualifies -> one background AI session spanning the glances.
        let gate = GroupParams {
            focus_min_density_fph: 90,
            ..gp()
        };
        let mut frames = run(1, CC.0, CC.1, 0, 60, 30);
        frames.extend(run(100, CC.0, CC.1, 300, 360, 30));
        frames.extend(run(200, CC.0, CC.1, 600, 660, 30));
        let s = segment_concurrent(&frames, &Taxonomy::seed(), &sp(), &gate);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].tool.as_deref(), Some("claude-code"));
        assert_eq!((s[0].start_ms, s[0].end_ms), (0, 660_000));
    }

    #[test]
    fn ai_track_spans_through_a_meeting_band() {
        // claude before, a claude glance INSIDE a meeting band, claude after -> one claude session
        // spanning the meeting, plus the meeting session (overlapping).
        let mut frames = run(1, CC.0, CC.1, 0, 200, 30);
        frames.extend(run(100, MEET.0, MEET.1, 250, 550, 30)); // 300 s meeting band
        frames.push(f(150, 400, Some(CC.0), Some(CC.1))); // a claude glance inside the band
        frames.extend(run(200, CC.0, CC.1, 600, 800, 30));
        frames.sort_by_key(|x| (x.captured_at, x.frame_id));
        let s = spans_cc(&frames);
        let cc = ai(&s, "claude-code");
        let meet = s.iter().find(|x| x.kind == Kind::Meeting).unwrap();
        assert_eq!(
            (cc.start_ms, cc.end_ms),
            (0, 780_000),
            "claude spans the meeting"
        );
        assert_eq!((meet.start_ms, meet.end_ms), (250_000, 550_000));
        assert!(
            cc.start_ms < meet.start_ms && meet.end_ms < cc.end_ms,
            "AI track spans the band"
        );
    }

    #[test]
    fn overlapping_meetings_emit_overlapping_sessions() {
        // Two meeting ids whose spans overlap are NOT merged (unlike the serial path) -> two
        // overlapping meeting sessions. Foreground alternates in coarse SUSTAINED blocks (real
        // meetings do not interleave frame-by-frame; a sub-min_len block would be consolidated
        // away by segment_micro). meet 0-300 (two blocks) and zoom 110-445 interleave and overlap.
        let mut frames = run(1, MEET.0, MEET.1, 0, 100, 25); // meet block A (100 s)
        frames.extend(run(100, ZOOM.0, ZOOM.1, 110, 190, 20)); // zoom block A (80 s, sustained)
        frames.extend(run(200, MEET.0, MEET.1, 200, 300, 25)); // meet block B -> meet band 0-300
        frames.extend(run(300, ZOOM.0, ZOOM.1, 320, 445, 25)); // zoom block B -> zoom band 110-445
        frames.sort_by_key(|x| (x.captured_at, x.frame_id));
        let s = spans_cc(&frames);
        assert_eq!(s.len(), 2, "{s:?}");
        assert!(s.iter().all(|x| x.kind == Kind::Meeting), "{s:?}");
        let keys: Vec<&str> = s.iter().map(|x| x.context_key.as_str()).collect();
        assert!(
            keys.contains(&"meeting:meet") && keys.contains(&"meeting:zoom"),
            "{keys:?}"
        );
        let overlap = s.iter().any(|a| {
            s.iter()
                .any(|b| !std::ptr::eq(a, b) && a.start_ms < b.end_ms && b.start_ms < a.end_ms)
        });
        assert!(overlap, "the two meetings overlap: {s:?}");
    }

    #[test]
    fn band_interior_unrecognized_is_owned_by_the_meeting() {
        // An unrecognized run inside a meeting band counts toward the band's frames, not a separate
        // focus session. Meet 0-90 + 210-300 chains across a 120 s notepad interior.
        let mut frames = run(1, MEET.0, MEET.1, 0, 90, 30); // 4 frames
        frames.extend(run(100, NP.0, NP.1, 120, 180, 30)); // 3 frames, interior
        frames.extend(run(200, MEET.0, MEET.1, 210, 300, 30)); // 4 frames
        frames.sort_by_key(|x| (x.captured_at, x.frame_id));
        let s = spans_cc(&frames);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].kind, Kind::Meeting);
        assert_eq!(
            s[0].frame_count, 11,
            "band owns its meeting frames + the interior unrecognized"
        );
    }

    #[test]
    fn per_track_gap_close_is_independent() {
        // claude gaps out (>= merge_gap) while a codex track continues underneath -> claude splits
        // into two, codex stays one.
        let mut frames = run(1, CC.0, CC.1, 0, 150, 30);
        frames.extend(run(100, CX.0, CX.1, 200, 750, 30)); // codex spans the claude gap
        frames.extend(run(300, CC.0, CC.1, 800, 950, 30)); // claude gap 650 >= merge_gap
        frames.sort_by_key(|x| (x.captured_at, x.frame_id));
        let s = spans_cc(&frames);
        assert_eq!(s.len(), 3, "{s:?}");
        assert_eq!(
            s.iter()
                .filter(|x| x.tool.as_deref() == Some("codex"))
                .count(),
            1
        );
        assert_eq!(
            s.iter()
                .filter(|x| x.tool.as_deref() == Some("claude-code"))
                .count(),
            2
        );
    }

    #[test]
    fn output_globally_sorted_with_cross_track_overlap_present() {
        // The overlapping shape must still be globally start-sorted (the debug_assert guards it),
        // while genuinely overlapping across tracks.
        let mut frames = run(1, CC.0, CC.1, 0, 150, 30);
        frames.extend(run(100, CX.0, CX.1, 180, 430, 30));
        frames.extend(run(200, CC.0, CC.1, 460, 710, 30));
        let s = spans_cc(&frames);
        assert!(
            s.windows(2).all(|w| w[0].start_ms <= w[1].start_ms),
            "globally sorted: {s:?}"
        );
        let overlap = s.iter().any(|a| {
            s.iter()
                .any(|b| !std::ptr::eq(a, b) && a.start_ms < b.end_ms && b.start_ms < a.end_ms)
        });
        assert!(overlap, "cross-track overlap is present: {s:?}");
    }

    #[test]
    fn concurrent_walk_is_deterministic() {
        let mut frames = run(1, CC.0, CC.1, 0, 200, 30);
        frames.extend(run(100, CX.0, CX.1, 100, 400, 30)); // equal-timestamp collisions with claude
        frames.extend(run(300, NP.0, NP.1, 500, 900, 30));
        frames.extend(run(400, CC.0, CC.1, 950, 1100, 30));
        let a = spans_cc(&frames);
        let b = spans_cc(&frames);
        assert_eq!(a, b, "identical inputs must yield identical sessions");
        // Permuting equal-timestamp frames must not change the result (segment_micro re-sorts).
        let mut permuted = frames.clone();
        permuted.reverse();
        let c = spans_cc(&permuted);
        assert_eq!(a, c, "equal-timestamp permutation is order-independent");
    }

    #[test]
    fn same_tool_two_instances_fold_into_one_track() {
        // #116(a): two codex runs (the taxonomy cannot tell two instances apart) within merge_gap
        // fold into ONE codex session.
        let mut frames = run(1, CX.0, CX.1, 0, 150, 30);
        frames.extend(run(100, CX.0, CX.1, 300, 450, 30)); // gap 150 < merge_gap 600
        let s = spans_cc(&frames);
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].tool.as_deref(), Some("codex"));
        assert_eq!((s[0].start_ms, s[0].end_ms), (0, 450_000));
    }
}

//! The validated concurrent per-identity-track segmenter (`06` #26/#28), ported from the
//! dev-only harness and extended with exclusive owned frame ids plus open-state/confidence.

use std::collections::{HashMap, HashSet};

use traits::{SegmentationParams, SegmenterFrame, SessionDraft, SessionHost, SessionKind};

use crate::confidence::{anchored_confidence, focus_confidence};
use crate::taxonomy::{Recognized, Taxonomy};

#[derive(Debug, Clone)]
struct Segment {
    key: String,
    recognition: Option<Recognized>,
    first_ts: i64,
    last_ts: i64,
    first_id: i64,
    frame_ids: Vec<i64>,
}

#[derive(Debug, Clone)]
struct MicroSpan {
    key: String,
    recognition: Option<Recognized>,
    first_ts: i64,
    last_ts: i64,
    first_id: i64,
    frame_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Identity {
    Ai(String),
    Meeting(String),
    None,
}

fn identity_of(span: &MicroSpan) -> Identity {
    match &span.recognition {
        Some(r) if r.kind == SessionKind::Ai => Identity::Ai(r.id.clone()),
        Some(r) if r.kind == SessionKind::Meeting => Identity::Meeting(r.id.clone()),
        _ => Identity::None,
    }
}

fn free_identity(span: &MicroSpan) -> Identity {
    match identity_of(span) {
        Identity::Meeting(_) => Identity::None,
        other => other,
    }
}

fn duration(span: &MicroSpan) -> i64 {
    (span.last_ts - span.first_ts).max(0)
}

fn midpoint(span: &MicroSpan) -> i64 {
    span.first_ts + (span.last_ts - span.first_ts) / 2
}

fn stem_of(span: &MicroSpan) -> &str {
    span.key.split('|').next().unwrap_or("")
}

fn browser_domain(url: Option<&str>) -> Option<String> {
    let url = url?.trim();
    if url.is_empty() {
        return None;
    }
    let after_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = after_scheme
        .split(['/', '?', ':', '#'])
        .next()
        .unwrap_or("");
    let host = host.strip_prefix("www.").unwrap_or(host).trim();
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

fn context_of(frame: &SegmenterFrame, taxonomy: &Taxonomy) -> Option<(String, Option<Recognized>)> {
    let app = frame.app_hint.as_deref()?.trim();
    if app.is_empty() {
        return None;
    }
    let app = app.to_ascii_lowercase();
    let domain = browser_domain(frame.browser_url.as_deref()).unwrap_or_default();
    let recognition = taxonomy.recognize(frame.app_hint.as_deref(), frame.window_title.as_deref());
    let identity = recognition
        .as_ref()
        .map(|r| r.id.as_str())
        .unwrap_or_default();
    let key = format!("{app}|{domain}|{identity}")
        .trim_end_matches('|')
        .to_string();
    Some((key, recognition))
}

fn gap_has_sustained_interrupter(gap: &[Segment], min_len_ms: i64) -> bool {
    let mut spans: HashMap<&str, (i64, i64)> = HashMap::new();
    for segment in gap {
        let entry = spans
            .entry(segment.key.as_str())
            .or_insert((segment.first_ts, segment.last_ts));
        entry.0 = entry.0.min(segment.first_ts);
        entry.1 = entry.1.max(segment.last_ts);
    }
    spans
        .values()
        .any(|(first, last)| last - first >= min_len_ms)
}

fn segment_micro(
    frames: &[SegmenterFrame],
    taxonomy: &Taxonomy,
    params: &SegmentationParams,
) -> Vec<MicroSpan> {
    let mut ordered: Vec<&SegmenterFrame> = frames.iter().collect();
    ordered.sort_by_key(|frame| (frame.captured_at, frame.id));

    let mut segments: Vec<Segment> = Vec::new();
    for frame in ordered {
        let Some((key, recognition)) = context_of(frame, taxonomy) else {
            continue;
        };
        match segments.last_mut() {
            Some(segment)
                if segment.key == key
                    && frame.captured_at - segment.last_ts < params.gap_close_ms =>
            {
                segment.last_ts = frame.captured_at;
                segment.frame_ids.push(frame.id);
            }
            _ => segments.push(Segment {
                key,
                recognition,
                first_ts: frame.captured_at,
                last_ts: frame.captured_at,
                first_id: frame.id,
                frame_ids: vec![frame.id],
            }),
        }
    }

    let mut consumed = vec![false; segments.len()];
    let mut micros = Vec::new();
    for start in 0..segments.len() {
        if consumed[start] {
            continue;
        }
        consumed[start] = true;
        let key = segments[start].key.clone();
        let recognition = segments[start].recognition.clone();
        let first_ts = segments[start].first_ts;
        let first_id = segments[start].first_id;
        let mut last_ts = segments[start].last_ts;
        let mut frame_ids = segments[start].frame_ids.clone();
        let mut last_segment = start;

        while let Some(next) =
            (last_segment + 1..segments.len()).find(|&index| segments[index].key == key)
        {
            let elapsed = segments[next].first_ts - segments[last_segment].last_ts;
            if elapsed >= params.gap_close_ms
                || gap_has_sustained_interrupter(
                    &segments[last_segment + 1..next],
                    params.min_len_ms,
                )
            {
                break;
            }
            for flag in consumed.iter_mut().take(next + 1).skip(last_segment + 1) {
                *flag = true;
            }
            last_ts = segments[next].last_ts;
            frame_ids.extend_from_slice(&segments[next].frame_ids);
            last_segment = next;
        }

        micros.push(MicroSpan {
            key,
            recognition,
            first_ts,
            last_ts,
            first_id,
            frame_ids,
        });
    }
    micros.sort_by_key(|span| (span.first_ts, span.first_id));
    micros
}

#[derive(Debug)]
struct Band {
    start_ms: i64,
    end_ms: i64,
    id: String,
    host: Option<SessionHost>,
    presence_ms: i64,
    absorbed_ms: i64,
    frame_ids: Vec<i64>,
}

fn push_band_if_qualified(
    chain: &[usize],
    micros: &[MicroSpan],
    id: &str,
    params: &SegmentationParams,
    out: &mut Vec<Band>,
) {
    let (Some(&first_index), Some(&last_index)) = (chain.first(), chain.last()) else {
        return;
    };
    let first = &micros[first_index];
    let last = &micros[last_index];
    if last.last_ts - first.first_ts < params.focus_min_len_ms {
        return;
    }
    out.push(Band {
        start_ms: first.first_ts,
        end_ms: last.last_ts,
        id: id.to_string(),
        host: first.recognition.as_ref().and_then(|r| r.host),
        presence_ms: chain.iter().map(|&index| duration(&micros[index])).sum(),
        absorbed_ms: 0,
        frame_ids: Vec::new(),
    });
}

fn owning_band_index(at: i64, bands: &[Band]) -> Option<usize> {
    bands
        .iter()
        .enumerate()
        .filter(|(_, band)| at >= band.start_ms && at <= band.end_ms)
        .max_by(|(_, left), (_, right)| {
            left.presence_ms
                .cmp(&right.presence_ms)
                .then(right.start_ms.cmp(&left.start_ms))
        })
        .map(|(index, _)| index)
}

fn build_bands(micros: &[MicroSpan], params: &SegmentationParams) -> Vec<Band> {
    let mut by_identity: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, span) in micros.iter().enumerate() {
        if let Identity::Meeting(id) = identity_of(span) {
            by_identity.entry(id).or_default().push(index);
        }
    }
    let mut bands = Vec::new();
    for (id, indexes) in by_identity {
        let mut chain: Vec<usize> = Vec::new();
        for index in indexes {
            if let Some(&previous) = chain.last() {
                if micros[index].first_ts - micros[previous].last_ts > params.meeting_gap_ms {
                    push_band_if_qualified(&chain, micros, &id, params, &mut bands);
                    chain.clear();
                }
            }
            chain.push(index);
        }
        push_band_if_qualified(&chain, micros, &id, params, &mut bands);
    }
    bands.sort_by(|left, right| {
        left.start_ms
            .cmp(&right.start_ms)
            .then(left.end_ms.cmp(&right.end_ms))
            .then(left.id.cmp(&right.id))
    });

    // Preserve the validated exclusive-ownership rule: every non-AI micro belongs to at most
    // one containing meeting band, selected by presence; AI micros remain free for their track.
    for span in micros {
        if matches!(identity_of(span), Identity::Ai(_)) {
            continue;
        }
        if let Some(index) = owning_band_index(midpoint(span), &bands) {
            if !matches!(identity_of(span), Identity::Meeting(ref id) if id == &bands[index].id) {
                bands[index].absorbed_ms += duration(span);
            }
            bands[index].frame_ids.extend_from_slice(&span.frame_ids);
        }
    }
    bands
}

#[derive(Debug)]
struct OpenTrack {
    start_ms: i64,
    end_ms: i64,
    first_id: i64,
    frame_ids: Vec<i64>,
    ai_id: Option<String>,
    ai_host: Option<SessionHost>,
    ai_presence_ms: i64,
    absorbed_ms: i64,
    stem_duration: HashMap<String, i64>,
    stem_frames: HashMap<String, usize>,
}

impl OpenTrack {
    fn new_ai(span: &MicroSpan, id: &str, host: Option<SessionHost>) -> Self {
        Self {
            start_ms: span.first_ts,
            end_ms: span.last_ts.max(span.first_ts),
            first_id: span.first_id,
            frame_ids: span.frame_ids.clone(),
            ai_id: Some(id.to_string()),
            ai_host: host,
            ai_presence_ms: duration(span),
            absorbed_ms: 0,
            stem_duration: HashMap::new(),
            stem_frames: HashMap::new(),
        }
    }

    fn new_focus(span: &MicroSpan) -> Self {
        let mut track = Self {
            start_ms: span.first_ts,
            end_ms: span.last_ts.max(span.first_ts),
            first_id: span.first_id,
            frame_ids: span.frame_ids.clone(),
            ai_id: None,
            ai_host: None,
            ai_presence_ms: 0,
            absorbed_ms: 0,
            stem_duration: HashMap::new(),
            stem_frames: HashMap::new(),
        };
        track.accrue_stem(span);
        track
    }

    fn accrue_stem(&mut self, span: &MicroSpan) {
        *self
            .stem_duration
            .entry(stem_of(span).to_string())
            .or_insert(0) += duration(span);
        *self
            .stem_frames
            .entry(stem_of(span).to_string())
            .or_insert(0) += span.frame_ids.len();
    }

    fn extend(&mut self, span: &MicroSpan) {
        self.end_ms = self.end_ms.max(span.last_ts);
        self.frame_ids.extend_from_slice(&span.frame_ids);
    }

    fn absorb_ai(&mut self, span: &MicroSpan) {
        self.ai_presence_ms += duration(span);
        self.extend(span);
    }

    fn absorb_none(&mut self, span: &MicroSpan) {
        self.absorbed_ms += duration(span);
        self.extend(span);
        if self.ai_id.is_none() {
            self.accrue_stem(span);
        }
    }

    fn prepend_ramp(&mut self, ramp: Ramp) {
        if ramp.start_ms < self.start_ms {
            self.start_ms = ramp.start_ms;
            self.first_id = ramp.first_id;
        }
        self.absorbed_ms += ramp.duration_ms;
        let mut ids = ramp.frame_ids;
        ids.extend(std::mem::take(&mut self.frame_ids));
        self.frame_ids = ids;
    }

    fn dominant_stem(&self) -> Option<String> {
        let mut candidates: Vec<(&String, i64, usize)> = self
            .stem_duration
            .iter()
            .map(|(stem, &duration)| {
                (
                    stem,
                    duration,
                    self.stem_frames.get(stem).copied().unwrap_or(0),
                )
            })
            .collect();
        candidates.sort_by(|left, right| {
            right
                .1
                .cmp(&left.1)
                .then(right.2.cmp(&left.2))
                .then(left.0.cmp(right.0))
        });
        candidates
            .first()
            .map(|(stem, _, _)| (*stem).clone())
            .filter(|stem| !stem.is_empty())
    }
}

#[derive(Debug)]
struct Ramp {
    start_ms: i64,
    first_id: i64,
    duration_ms: i64,
    frame_ids: Vec<i64>,
}

fn frame_density(frame_count: usize, duration_ms: i64) -> f64 {
    if duration_ms <= 0 {
        0.0
    } else {
        frame_count as f64 * 3_600_000.0 / duration_ms as f64
    }
}

fn finalize_track(
    mut track: OpenTrack,
    closed_by_gap: bool,
    params: &SegmentationParams,
    out: &mut Vec<SessionDraft>,
) {
    let end = track.end_ms.max(track.start_ms);
    let duration_ms = end - track.start_ms;
    track.frame_ids.sort_unstable();
    track.frame_ids.dedup();

    let (kind, tool, host, context_key, confidence, keep) = match &track.ai_id {
        Some(id) => (
            SessionKind::Ai,
            Some(id.clone()),
            track.ai_host,
            format!("ai:{id}"),
            anchored_confidence(track.ai_presence_ms, track.absorbed_ms),
            duration_ms >= params.min_len_ms && track.ai_presence_ms >= params.identity_qualify_ms,
        ),
        None => {
            let stem = track.dominant_stem();
            let density = frame_density(track.frame_ids.len(), duration_ms);
            let context_key = stem
                .as_ref()
                .map_or_else(|| "focus".to_string(), |stem| format!("focus:{stem}"));
            let keep = duration_ms >= params.focus_min_len_ms
                && (params.focus_min_density_fph <= 0
                    || density >= params.focus_min_density_fph as f64);
            (
                SessionKind::Focus,
                None,
                None,
                context_key,
                focus_confidence(stem.as_deref(), density, params.focus_min_density_fph),
                keep,
            )
        }
    };
    if !keep {
        return;
    }
    let open = !closed_by_gap && params.now_ms.saturating_sub(end) < params.merge_gap_ms;
    out.push(SessionDraft {
        started_at: track.start_ms,
        ended_at: end,
        kind,
        tool,
        host,
        context_key,
        confidence,
        frame_ids: track.frame_ids,
        open,
    });
}

fn extend_or_open_focus(
    focus: &mut Option<OpenTrack>,
    span: &MicroSpan,
    params: &SegmentationParams,
    out: &mut Vec<SessionDraft>,
) {
    if focus
        .as_ref()
        .is_some_and(|track| span.first_ts - track.end_ms >= params.merge_gap_ms)
    {
        finalize_track(focus.take().expect("focus present"), true, params, out);
    }
    match focus {
        Some(track) => track.absorb_none(span),
        None => *focus = Some(OpenTrack::new_focus(span)),
    }
}

fn flush_pending(
    pending: &mut Vec<&MicroSpan>,
    ramp_start: Option<i64>,
    tracks: &mut HashMap<String, OpenTrack>,
    focus: &mut Option<OpenTrack>,
    params: &SegmentationParams,
    out: &mut Vec<SessionDraft>,
) -> Option<Ramp> {
    let mut ramp: Option<Ramp> = None;
    for span in pending.drain(..) {
        let last_touched = tracks
            .iter()
            .filter(|(_, track)| span.first_ts - track.end_ms < params.merge_gap_ms)
            .max_by(|left, right| left.1.end_ms.cmp(&right.1.end_ms).then(left.0.cmp(right.0)))
            .map(|(id, _)| id.clone());
        if let Some(id) = last_touched {
            tracks
                .get_mut(&id)
                .expect("selected track exists")
                .absorb_none(span);
        } else if ramp_start.is_some_and(|start| start - span.last_ts < params.merge_gap_ms) {
            let ramp = ramp.get_or_insert_with(|| Ramp {
                start_ms: span.first_ts,
                first_id: span.first_id,
                duration_ms: 0,
                frame_ids: Vec::new(),
            });
            ramp.start_ms = ramp.start_ms.min(span.first_ts);
            ramp.first_id = ramp.first_id.min(span.first_id);
            ramp.duration_ms += duration(span);
            ramp.frame_ids.extend_from_slice(&span.frame_ids);
        } else {
            extend_or_open_focus(focus, span, params, out);
        }
    }
    ramp
}

fn track_partition(draft: &SessionDraft) -> String {
    if draft.kind == SessionKind::Focus {
        "focus".to_string()
    } else {
        draft.context_key.clone()
    }
}

fn enforce_non_overlap_per_track(drafts: &mut [SessionDraft]) {
    let mut last_end: HashMap<String, i64> = HashMap::new();
    for draft in drafts {
        let partition = track_partition(draft);
        if let Some(&previous_end) = last_end.get(&partition) {
            if draft.started_at < previous_end {
                draft.started_at = previous_end;
                draft.ended_at = draft.ended_at.max(previous_end);
            }
        }
        last_end.insert(partition, draft.ended_at);
    }
}

pub(crate) fn segment_with_taxonomy(
    frames: &[SegmenterFrame],
    taxonomy: &Taxonomy,
    params: &SegmentationParams,
) -> Vec<SessionDraft> {
    let micros = segment_micro(frames, taxonomy, params);
    let mut bands = build_bands(&micros, params);
    let mut out = Vec::new();
    for band in &mut bands {
        band.frame_ids.sort_unstable();
        band.frame_ids.dedup();
        out.push(SessionDraft {
            started_at: band.start_ms,
            ended_at: band.end_ms,
            kind: SessionKind::Meeting,
            tool: None,
            host: band.host,
            context_key: format!("meeting:{}", band.id),
            confidence: anchored_confidence(band.presence_ms, band.absorbed_ms),
            frame_ids: band.frame_ids.clone(),
            open: params.now_ms.saturating_sub(band.end_ms) < params.meeting_gap_ms,
        });
    }

    let free: Vec<&MicroSpan> = micros
        .iter()
        .filter(|span| {
            matches!(identity_of(span), Identity::Ai(_))
                || owning_band_index(midpoint(span), &bands).is_none()
        })
        .collect();
    let mut tracks: HashMap<String, OpenTrack> = HashMap::new();
    let mut focus = None;
    let mut pending: Vec<&MicroSpan> = Vec::new();

    for span in free {
        if pending
            .last()
            .is_some_and(|last| span.first_ts - last.last_ts >= params.merge_gap_ms)
        {
            flush_pending(
                &mut pending,
                None,
                &mut tracks,
                &mut focus,
                params,
                &mut out,
            );
        }
        match free_identity(span) {
            Identity::Ai(id) => {
                if tracks
                    .get(&id)
                    .is_some_and(|track| span.first_ts - track.end_ms >= params.merge_gap_ms)
                {
                    let old = tracks.remove(&id).expect("gapped track exists");
                    finalize_track(old, true, params, &mut out);
                }
                let is_open = tracks.contains_key(&id);
                let ramp = flush_pending(
                    &mut pending,
                    (!is_open).then_some(span.first_ts),
                    &mut tracks,
                    &mut focus,
                    params,
                    &mut out,
                );
                if let Some(track) = tracks.get_mut(&id) {
                    track.absorb_ai(span);
                } else {
                    let host = span.recognition.as_ref().and_then(|r| r.host);
                    let mut track = OpenTrack::new_ai(span, &id, host);
                    if let Some(ramp) = ramp {
                        track.prepend_ramp(ramp);
                    }
                    tracks.insert(id, track);
                }
            }
            Identity::None | Identity::Meeting(_) => {
                if duration(span) > params.absorb_max_ms {
                    flush_pending(
                        &mut pending,
                        None,
                        &mut tracks,
                        &mut focus,
                        params,
                        &mut out,
                    );
                    extend_or_open_focus(&mut focus, span, params, &mut out);
                } else {
                    pending.push(span);
                }
            }
        }
    }

    flush_pending(
        &mut pending,
        None,
        &mut tracks,
        &mut focus,
        params,
        &mut out,
    );
    for track in tracks.into_values() {
        finalize_track(track, false, params, &mut out);
    }
    if let Some(track) = focus {
        finalize_track(track, false, params, &mut out);
    }

    out.sort_by(|left, right| {
        left.started_at
            .cmp(&right.started_at)
            .then(left.frame_ids.first().cmp(&right.frame_ids.first()))
            .then(left.context_key.cmp(&right.context_key))
    });
    enforce_non_overlap_per_track(&mut out);
    out.sort_by(|left, right| {
        left.started_at
            .cmp(&right.started_at)
            .then(left.frame_ids.first().cmp(&right.frame_ids.first()))
            .then(left.context_key.cmp(&right.context_key))
    });

    debug_assert!({
        let mut owners = HashSet::new();
        out.iter()
            .flat_map(|draft| draft.frame_ids.iter())
            .all(|frame_id| owners.insert(*frame_id))
    });
    out
}

/// Convenience entry for the harness referee. Production constructs [`crate::SessionEngine`]
/// once at startup so the taxonomy is not reparsed per scheduler tick.
pub fn segment_concurrent(
    frames: &[SegmenterFrame],
    params: &SegmentationParams,
) -> Vec<SessionDraft> {
    let taxonomy = Taxonomy::seed().expect("bundled sessions taxonomy is valid");
    segment_with_taxonomy(frames, &taxonomy, params)
}

//! The pure candidate segmenter (`specs/03 §7e`): ordered frame metadata in, session spans
//! out. It generalizes the `where_was_i` heuristic (`03 §7b`, `crates/kernel/src/resume.rs`)
//! in two ways:
//!
//! **1. Context key** = `app_hint` (lowercased) ⊕ browser domain ⊕ taxonomy tool id, instead
//! of just `app_hint` ⊕ domain. So two tools in the same app (a `claude`-titled terminal vs a
//! `codex`-titled terminal) read as distinct contexts. The browser-domain term is dormant
//! (production capture stores `browser_url = NULL`, gap #109).
//!
//! **2. Close rule** — a run of one context closes when either the elapsed gap since its last
//! same-context frame reaches `gap_close_ms`, or a *sustained* switch to another context
//! occurs (an interrupting context whose presence-span within the gap reaches `min_len_ms` —
//! the `03 §7b` dwell logic, reusing `min_len_ms` as the single dwell knob). The merge
//! condition to extend a run to a later same-key segment is therefore
//! `(elapsed < gap_close_ms) AND (no sustained interrupter in the gap)`. The elapsed term is
//! what `resume.rs` deliberately lacks (it merges across unbounded gaps to find the one run
//! behind you); adding it is the intentional generalization for partitioning a day. A
//! consequence, tested below: a **keyless stretch longer than `gap_close_ms`** between two
//! same-key segments splits the run (unknown context is not continuity), where `resume.rs`
//! treats keyless frames as transparent without a time limit.
//!
//! Runs shorter than `min_len_ms` are floored out (their frames stay sessionless — the world
//! is additive; not every frame belongs to a session). Recognition emits `kind`/`tool`/`host`
//! per the taxonomy; `tool` is recorded only for `kind = Ai` (the `sessions.tool` CHECK shape,
//! `03 §4`). No model calls anywhere (D3).

use std::collections::HashMap;

use crate::model::{Host, Kind, SegParams, SessionSpan};
use crate::taxonomy::{Recognized, Taxonomy};

/// One exported frame's segmentation inputs (a thin view; `export::ExportFrame` converts in).
#[derive(Debug, Clone)]
pub struct FrameRow {
    pub frame_id: i64,
    pub captured_at: i64,
    pub app_hint: Option<String>,
    pub window_title: Option<String>,
    pub browser_url: Option<String>,
}

impl From<&crate::model::ExportFrame> for FrameRow {
    fn from(f: &crate::model::ExportFrame) -> Self {
        Self {
            frame_id: f.frame_id,
            captured_at: f.captured_at,
            app_hint: f.app_hint.clone(),
            window_title: f.window_title.clone(),
            browser_url: f.browser_url.clone(),
        }
    }
}

/// The browser domain of a URL (scheme + `www.` stripped, host up to the first `/?:#`,
/// lowercased), or `None`. Dormant in production (`browser_url` is always `None`) but kept
/// per the `03 §7b`/`§7e` contract so the domain term activates if URL capture lands.
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

/// The context key + recognition of a frame, or `None` when `app_hint` is absent (a keyless,
/// transparent frame). Key slots are `app|domain|tool` with trailing empty slots trimmed.
fn context_of(f: &FrameRow, tax: &Taxonomy) -> Option<(String, Option<Recognized>)> {
    let app = f.app_hint.as_deref()?.trim();
    if app.is_empty() {
        return None;
    }
    let app = app.to_ascii_lowercase();
    let domain = browser_domain(f.browser_url.as_deref()).unwrap_or_default();
    let recog = tax.recognize(f.app_hint.as_deref(), f.window_title.as_deref());
    let tool = recog.as_ref().map(|r| r.id.as_str()).unwrap_or_default();
    let key = format!("{app}|{domain}|{tool}");
    let key = key.trim_end_matches('|').to_string();
    Some((key, recog))
}

/// A maximal run of consecutive same-key frames (keyless frames transparently absorbed).
struct Seg {
    key: String,
    recog: Option<Recognized>,
    first_ts: i64,
    last_ts: i64,
    first_id: i64,
    last_id: i64,
    frames: usize,
}

/// Whether the gap between two same-key segments holds a *sustained* interrupter — some
/// other context whose presence-span (max last − min first across the gap, so a fragmented
/// interrupter still counts) reaches `min_len_ms`.
fn gap_has_sustained_interrupter(gap: &[Seg], min_len_ms: i64) -> bool {
    let mut spans: HashMap<&str, (i64, i64)> = HashMap::new();
    for s in gap {
        let e = spans
            .entry(s.key.as_str())
            .or_insert((s.first_ts, s.last_ts));
        e.0 = e.0.min(s.first_ts);
        e.1 = e.1.max(s.last_ts);
    }
    spans.values().any(|(f, l)| l - f >= min_len_ms)
}

/// Segment ordered frame metadata into candidate session spans (`03 §7e`).
pub fn segment(frames: &[FrameRow], tax: &Taxonomy, p: &SegParams) -> Vec<SessionSpan> {
    // Chronological, ties by frame id (multi-monitor cycles share captured_at).
    let mut ordered: Vec<&FrameRow> = frames.iter().collect();
    ordered.sort_by_key(|f| (f.captured_at, f.frame_id));

    // 1. Maximal same-key segments; keyless frames transparently extend the current segment.
    //    A same-key frame more than gap_close_ms after the segment's last frame does NOT
    //    extend it (an idle gap, or a keyless stretch, longer than gap_close closes the run) —
    //    it starts a fresh segment, which consolidation will not re-merge (same elapsed test).
    let mut segs: Vec<Seg> = Vec::new();
    for f in &ordered {
        let Some((key, recog)) = context_of(f, tax) else {
            continue;
        };
        match segs.last_mut() {
            Some(s) if s.key == key && f.captured_at - s.last_ts < p.gap_close_ms => {
                s.last_ts = f.captured_at;
                s.last_id = f.frame_id;
                s.frames += 1;
            }
            _ => segs.push(Seg {
                key,
                recog,
                first_ts: f.captured_at,
                last_ts: f.captured_at,
                first_id: f.frame_id,
                last_id: f.frame_id,
                frames: 1,
            }),
        }
    }

    // 2. Consolidate runs: greedily absorb later same-key segments across non-sustained
    //    excursions within gap_close_ms; consume the in-between excursion segments so they
    //    never form (overlapping) sessions of their own.
    let mut consumed = vec![false; segs.len()];
    let mut spans: Vec<SessionSpan> = Vec::new();
    for start in 0..segs.len() {
        if consumed[start] {
            continue;
        }
        consumed[start] = true;
        let key = segs[start].key.clone();
        let recog = segs[start].recog.clone();
        let first_ts = segs[start].first_ts;
        let mut last_ts = segs[start].last_ts;
        let first_id = segs[start].first_id;
        let mut last_id = segs[start].last_id;
        let mut frame_count = segs[start].frames;
        let mut last_seg = start;

        while let Some(next) = (last_seg + 1..segs.len()).find(|&k| segs[k].key == key) {
            let elapsed = segs[next].first_ts - segs[last_seg].last_ts;
            if elapsed >= p.gap_close_ms {
                break; // idle / long absence: the run closed.
            }
            if gap_has_sustained_interrupter(&segs[last_seg + 1..next], p.min_len_ms) {
                break; // a sustained switch to another context closed the run.
            }
            for s in consumed.iter_mut().take(next + 1).skip(last_seg + 1) {
                *s = true; // absorb the non-sustained excursion(s) in the gap.
            }
            last_ts = segs[next].last_ts;
            last_id = segs[next].last_id;
            frame_count += segs[next].frames;
            last_seg = next;
        }

        // 3. Floor: a run shorter than min_len_ms is not a session.
        if last_ts - first_ts < p.min_len_ms {
            continue;
        }

        let (kind, tool, host) = classify(&recog);
        spans.push(SessionSpan {
            start_ms: first_ts,
            end_ms: last_ts,
            context_key: key,
            kind,
            tool,
            host,
            frame_count,
            first_frame_id: first_id,
            last_frame_id: last_id,
        });
    }
    spans.sort_by_key(|s| (s.start_ms, s.first_frame_id));
    spans
}

/// Map a recognition outcome to a span's `(kind, tool, host)`. `tool` is recorded only for
/// `kind = Ai` (the `sessions.tool` CHECK shape); a keyed-but-unrecognized run is `Focus`.
fn classify(recog: &Option<Recognized>) -> (Kind, Option<String>, Option<Host>) {
    match recog {
        Some(r) if r.kind == Kind::Ai => (Kind::Ai, Some(r.id.clone()), r.host),
        Some(r) => (r.kind, None, r.host),
        None => (Kind::Focus, None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAP: i64 = 300_000; // gap_close 300 s
    const MIN: i64 = 120_000; // min_len 120 s (also the dwell knob)

    fn params() -> SegParams {
        SegParams {
            gap_close_ms: GAP,
            min_len_ms: MIN,
        }
    }

    /// Build a frame at `sec` seconds with app/title.
    fn f(id: i64, sec: i64, app: Option<&str>, title: Option<&str>) -> FrameRow {
        FrameRow {
            frame_id: id,
            captured_at: sec * 1000,
            app_hint: app.map(str::to_string),
            window_title: title.map(str::to_string),
            browser_url: None,
        }
    }

    /// A sustained run of `app`/`title` from `start`..=`end` sec, one frame per `step`.
    fn run(id0: i64, app: &str, title: &str, start: i64, end: i64, step: i64) -> Vec<FrameRow> {
        let mut v = Vec::new();
        let mut t = start;
        let mut id = id0;
        while t <= end {
            v.push(f(id, t, Some(app), Some(title)));
            t += step;
            id += 1;
        }
        v
    }

    fn seg(frames: &[FrameRow]) -> Vec<SessionSpan> {
        segment(frames, &Taxonomy::seed(), &params())
    }

    #[test]
    fn empty_and_keyless_produce_no_spans() {
        assert!(seg(&[]).is_empty());
        let keyless = vec![
            f(1, 0, None, None),
            f(2, 30, None, None),
            f(3, 600, None, None),
        ];
        assert!(seg(&keyless).is_empty());
    }

    #[test]
    fn single_sustained_run_is_one_span() {
        let frames = run(1, "Notepad", "notes.txt", 0, 300, 30);
        let s = seg(&frames);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].start_ms, 0);
        assert_eq!(s[0].end_ms, 300_000);
        assert_eq!(s[0].kind, Kind::Focus);
        assert_eq!(s[0].context_key, "notepad");
        assert_eq!(s[0].frame_count, 11);
    }

    #[test]
    fn sub_min_len_run_is_dropped() {
        // 90 s < 120 s floor.
        let frames = run(1, "Notepad", "n", 0, 90, 30);
        assert!(seg(&frames).is_empty());
    }

    #[test]
    fn brief_excursion_is_absorbed_into_one_span() {
        // Notepad, a 30 s keyed blip (Toast), more Notepad. The blip (< min_len) must not
        // split the Notepad run, and must not produce its own (overlapping) span.
        let mut frames = run(1, "Notepad", "n", 0, 120, 30);
        frames.extend(run(100, "Toast", "t", 150, 180, 30)); // 30 s excursion
        frames.extend(run(200, "Notepad", "n", 210, 360, 30));
        let s = seg(&frames);
        assert_eq!(
            s.len(),
            1,
            "one absorbed Notepad span, no Toast span: {s:?}"
        );
        assert_eq!(s[0].context_key, "notepad");
        assert_eq!(s[0].start_ms, 0);
        assert_eq!(s[0].end_ms, 360_000);
    }

    #[test]
    fn sustained_interruption_splits_into_three_spans() {
        // Notepad, a sustained Slack (150 s >= min_len), Notepad again. Slack splits the run.
        let mut frames = run(1, "Notepad", "n", 0, 150, 30);
        frames.extend(run(100, "Slack", "s", 180, 330, 30)); // 150 s sustained
        frames.extend(run(200, "Notepad", "n", 360, 540, 30));
        let s = seg(&frames);
        assert_eq!(s.len(), 3, "Notepad / Slack / Notepad: {s:?}");
        assert_eq!(s[0].context_key, "notepad");
        assert_eq!(s[1].context_key, "slack");
        assert_eq!(s[2].context_key, "notepad");
        assert!(
            s[0].end_ms <= s[1].start_ms && s[1].end_ms <= s[2].start_ms,
            "non-overlapping"
        );
    }

    #[test]
    fn gap_close_splits_same_context_after_idle() {
        // Same Notepad context, but a > gap_close (300 s) idle gap with nothing in between.
        let mut frames = run(1, "Notepad", "n", 0, 150, 30);
        frames.extend(run(100, "Notepad", "n", 600, 750, 30)); // resumes 450 s after last frame
        let s = seg(&frames);
        assert_eq!(
            s.len(),
            2,
            "idle gap > gap_close splits the same context: {s:?}"
        );
        assert_eq!(s[0].end_ms, 150_000);
        assert_eq!(s[1].start_ms, 600_000);
    }

    #[test]
    fn same_context_within_gap_close_stays_one_span() {
        // A 90 s gap (< gap_close) with nothing in between: one continuous session.
        let mut frames = run(1, "Notepad", "n", 0, 120, 30);
        frames.extend(run(100, "Notepad", "n", 210, 360, 30)); // 90 s gap
        let s = seg(&frames);
        assert_eq!(s.len(), 1, "gap < gap_close does not split: {s:?}");
        assert_eq!(s[0].end_ms, 360_000);
    }

    #[test]
    fn keyless_stretch_over_gap_close_splits() {
        // The documented divergence from resume.rs: a keyless stretch longer than gap_close
        // between two same-key segments splits (unknown context is not continuity).
        let mut frames = run(1, "Notepad", "n", 0, 150, 30);
        frames.push(f(50, 300, None, None)); // keyless
        frames.push(f(51, 450, None, None)); // keyless, total keyless gap 450-150 = 300 s
        frames.extend(run(100, "Notepad", "n", 600, 750, 30));
        let s = seg(&frames);
        assert_eq!(s.len(), 2, "keyless gap > gap_close splits: {s:?}");
    }

    #[test]
    fn tool_identity_splits_same_app_into_adjacent_sessions() {
        // Same terminal app, but a claude-titled run then a codex-titled run: the tool id in
        // the context key makes them two adjacent AI sessions with the right tools.
        let mut frames = run(1, "WindowsTerminal", "claude - repo", 0, 150, 30);
        frames.extend(run(100, "WindowsTerminal", "codex - repo", 180, 330, 30));
        let s = seg(&frames);
        assert_eq!(s.len(), 2, "claude vs codex split: {s:?}");
        assert_eq!(s[0].kind, Kind::Ai);
        assert_eq!(s[0].tool.as_deref(), Some("claude-code"));
        assert_eq!(s[0].host, Some(Host::Terminal));
        assert_eq!(s[1].tool.as_deref(), Some("codex"));
        assert!(s[0].context_key.contains("claude-code"));
    }

    #[test]
    fn browser_ai_vs_plain_browser_are_distinct() {
        // A ChatGPT tab (browser-ai) then a plain GitHub tab: distinct keys, distinct kinds.
        let mut frames = run(1, "chrome", "ChatGPT - Google Chrome", 0, 150, 30);
        frames.extend(run(100, "chrome", "GitHub - Google Chrome", 180, 360, 30));
        let s = seg(&frames);
        assert_eq!(s.len(), 2, "{s:?}");
        assert_eq!(s[0].kind, Kind::Ai);
        assert_eq!(s[0].tool.as_deref(), Some("browser-ai"));
        assert_eq!(s[1].kind, Kind::Focus, "plain browser is focus, not ai");
        assert_eq!(s[1].tool, None);
    }

    #[test]
    fn meeting_recognition_sets_kind_without_tool() {
        let frames = run(1, "Zoom", "Zoom Meeting", 0, 300, 30);
        let s = seg(&frames);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].kind, Kind::Meeting);
        assert_eq!(s[0].tool, None, "meetings carry no tool id (schema shape)");
        assert_eq!(s[0].host, Some(Host::Desktop));
    }

    #[test]
    fn multimonitor_equal_timestamps_are_deterministic() {
        // A dual-monitor cycle shares captured_at; the same foreground app_hint is cloned to
        // both frames, so they collapse into one context. Must not panic or double-count time.
        let frames = vec![
            f(1, 0, Some("Code"), Some("a.rs")),
            f(2, 0, Some("Code"), Some("a.rs")), // 2nd monitor, same ts
            f(3, 60, Some("Code"), Some("a.rs")),
            f(4, 150, Some("Code"), Some("a.rs")),
        ];
        let s = seg(&frames);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].start_ms, 0);
        assert_eq!(s[0].end_ms, 150_000);
    }

    #[test]
    fn fragmented_interrupter_reaching_dwell_splits() {
        // Two short Slack blips 130 s apart bracket a brief Toast; Slack's presence-span
        // across the gap (60->190 s = 130 s >= min_len) is a sustained interrupter, so it
        // splits the surrounding Notepad and forms its own Slack session (blips consolidated
        // across the Toast). Mirrors resume.rs's fragmented-interrupter test.
        let frames = vec![
            f(1, 0, Some("Notepad"), Some("a")),
            f(2, 30, Some("Notepad"), Some("a")),
            f(3, 60, Some("Slack"), Some("s")),  // blip 1
            f(4, 90, Some("Toast"), Some("t")),  // brief excursion inside Slack's stretch
            f(5, 190, Some("Slack"), Some("s")), // blip 2 -> Slack span 60..190 = 130 s
            f(6, 220, Some("Notepad"), Some("a")),
            f(7, 250, Some("Notepad"), Some("a")),
            // Extend both Notepad chunks so they'd individually clear the floor if not split.
            f(8, 400, Some("Notepad"), Some("a")),
        ];
        // Use a smaller min_len so the 130 s Slack presence qualifies as sustained.
        let p = SegParams {
            gap_close_ms: GAP,
            min_len_ms: 120_000,
        };
        let s = segment(&frames, &Taxonomy::seed(), &p);
        // Slack (fragmented) becomes a session; it splits Notepad.
        assert!(
            s.iter().any(|x| x.context_key == "slack"),
            "fragmented Slack consolidates into a session: {s:?}"
        );
    }
}

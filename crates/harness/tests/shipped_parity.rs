use harness::group::IDENTITY_QUALIFY_MS;
use harness::model::{GroupParams, SegParams};
use harness::score::{spans_for_algo, Algo};
use harness::segmenter::FrameRow;
use harness::taxonomy::Taxonomy;

fn run_step(id0: i64, app: &str, title: &str, start: i64, end: i64, step: i64) -> Vec<FrameRow> {
    let mut frames = Vec::new();
    let (mut id, mut at) = (id0, start);
    while at <= end {
        frames.push(FrameRow {
            frame_id: id,
            captured_at: at * 1000,
            app_hint: Some(app.to_string()),
            window_title: Some(title.to_string()),
            browser_url: None,
        });
        id += 1;
        at += step;
    }
    frames
}

fn run(id0: i64, app: &str, title: &str, start: i64, end: i64) -> Vec<FrameRow> {
    run_step(id0, app, title, start, end, 30)
}

fn outputs(
    frames: &[FrameRow],
) -> (
    Vec<harness::model::SessionSpan>,
    Vec<harness::model::SessionSpan>,
) {
    let seg = SegParams::default();
    let group = GroupParams {
        merge_gap_ms: 2_700_000,
        absorb_max_ms: 1_800_000,
        ..GroupParams::default()
    };
    let taxonomy = Taxonomy::seed();
    (
        spans_for_algo(
            frames,
            &taxonomy,
            &seg,
            &group,
            IDENTITY_QUALIFY_MS,
            Algo::Concurrent,
        ),
        spans_for_algo(
            frames,
            &taxonomy,
            &seg,
            &group,
            IDENTITY_QUALIFY_MS,
            Algo::Shipped,
        ),
    )
}

fn assert_semantic_parity(frames: &[FrameRow], exact_frame_count: bool) {
    let (baseline, shipped) = outputs(frames);
    assert_eq!(shipped.len(), baseline.len(), "{shipped:?} != {baseline:?}");
    for (actual, expected) in shipped.iter().zip(&baseline) {
        assert_eq!(actual.start_ms, expected.start_ms);
        assert_eq!(actual.end_ms, expected.end_ms);
        assert_eq!(actual.context_key, expected.context_key);
        assert_eq!(actual.kind, expected.kind);
        assert_eq!(actual.tool, expected.tool);
        assert_eq!(actual.host, expected.host);
        assert_eq!(actual.first_frame_id, expected.first_frame_id);
        assert_eq!(actual.last_frame_id, expected.last_frame_id);
        if exact_frame_count {
            assert_eq!(actual.frame_count, expected.frame_count);
        } else {
            // The frozen referee counts only resumed same-key frames when pass-1 consumes a short
            // excursion. Production additionally owns those consumed ids, as §7e requires. Keep
            // the validated harness algorithm byte-untouched and pin this deliberate extension.
            assert!(actual.frame_count >= expected.frame_count);
        }
    }
}

#[test]
fn shipped_matches_frozen_harness_concurrent_on_synthetic_day() {
    let mut frames = run(1, "WindowsTerminal", "claude - repo", 0, 300);
    frames.extend(run(100, "codex", "Codex", 120, 480));
    frames.extend(run(200, "WindowsTerminal", "claude - repo", 510, 720));
    frames.extend(run(300, "chatgpt", "ChatGPT Classic", 4_000, 4_700));
    frames.extend(run(400, "chatgpt", "Codex", 8_000, 8_300));
    assert_semantic_parity(&frames, false);
}

#[test]
fn shipped_parity_covers_none_meetings_gap_edges_density_and_qualification() {
    let mut none_absorption = run(1, "WindowsTerminal", "claude - repo", 0, 180);
    none_absorption.extend(run(100, "Notepad", "notes", 240, 300));
    none_absorption.extend(run(200, "WindowsTerminal", "claude - repo", 360, 540));
    assert_semantic_parity(&none_absorption, false);

    let mut meeting_overlap = run(1, "WindowsTerminal", "claude - repo", 0, 90);
    meeting_overlap.extend(run(50, "WindowsTerminal", "claude - repo", 810, 900));
    meeting_overlap.extend(run(100, "chrome", "Google Meet - Google Chrome", 120, 780));
    assert_semantic_parity(&meeting_overlap, true);

    let mut merge_gap_equality = run(1, "WindowsTerminal", "claude - repo", 0, 180);
    merge_gap_equality.extend(run(100, "WindowsTerminal", "claude - repo", 2_880, 3_060));
    assert_semantic_parity(&merge_gap_equality, true);

    let dense_focus = run(1, "Notepad", "notes", 0, 600);
    assert_semantic_parity(&dense_focus, true);
    let sparse_focus = run_step(1, "Notepad", "notes", 0, 600, 60);
    assert_semantic_parity(&sparse_focus, true);

    let underqualified_ai = run(1, "WindowsTerminal", "claude - repo", 0, 90);
    assert_semantic_parity(&underqualified_ai, true);
}

#[test]
fn shipped_open_projection_keeps_the_parity_span_fields() {
    let frames = run(1, "WindowsTerminal", "claude - repo", 0, 180);
    assert_semantic_parity(&frames, true);

    let production = frames
        .iter()
        .map(|frame| sessions::SegmenterFrame {
            id: frame.frame_id,
            captured_at: frame.captured_at,
            app_hint: frame.app_hint.clone(),
            window_title: frame.window_title.clone(),
            browser_url: frame.browser_url.clone(),
        })
        .collect::<Vec<_>>();
    let params = sessions::SegmentationParams::shipped(180_000, 300_000, 120_000);
    let draft = sessions::segment_concurrent(&production, &params)
        .into_iter()
        .next()
        .unwrap();
    assert!(draft.open);
}

use harness::group::IDENTITY_QUALIFY_MS;
use harness::model::{GroupParams, SegParams};
use harness::score::{spans_for_algo, Algo};
use harness::segmenter::FrameRow;
use harness::taxonomy::Taxonomy;

fn run(id0: i64, app: &str, title: &str, start: i64, end: i64) -> Vec<FrameRow> {
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
        at += 30;
    }
    frames
}

#[test]
fn shipped_matches_frozen_harness_concurrent_on_synthetic_day() {
    let mut frames = run(1, "WindowsTerminal", "claude - repo", 0, 300);
    frames.extend(run(100, "codex", "Codex", 120, 480));
    frames.extend(run(200, "WindowsTerminal", "claude - repo", 510, 720));
    frames.extend(run(300, "chatgpt", "ChatGPT Classic", 4_000, 4_700));
    frames.extend(run(400, "chatgpt", "Codex", 8_000, 8_300));
    let seg = SegParams::default();
    let group = GroupParams {
        merge_gap_ms: 2_700_000,
        absorb_max_ms: 1_800_000,
        ..GroupParams::default()
    };
    let taxonomy = Taxonomy::seed();

    let baseline = spans_for_algo(
        &frames,
        &taxonomy,
        &seg,
        &group,
        IDENTITY_QUALIFY_MS,
        Algo::Concurrent,
    );
    let shipped = spans_for_algo(
        &frames,
        &taxonomy,
        &seg,
        &group,
        IDENTITY_QUALIFY_MS,
        Algo::Shipped,
    );

    assert_eq!(shipped, baseline);
}

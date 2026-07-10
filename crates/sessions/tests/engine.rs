use std::collections::HashSet;

use sessions::{extract_exchanges, segment_concurrent, SessionEngine};
use traits::{
    SegmentationParams, SegmenterFrame, SessionArtifactRole, SessionContent, SessionKind,
};

fn frame(id: i64, sec: i64, app: Option<&str>, title: Option<&str>) -> SegmenterFrame {
    SegmenterFrame {
        id,
        captured_at: sec * 1000,
        app_hint: app.map(str::to_string),
        window_title: title.map(str::to_string),
        browser_url: None,
    }
}

fn run(id0: i64, app: &str, title: &str, start: i64, end: i64, step: i64) -> Vec<SegmenterFrame> {
    let mut out = Vec::new();
    let (mut id, mut at) = (id0, start);
    while at <= end {
        out.push(frame(id, at, Some(app), Some(title)));
        id += 1;
        at += step;
    }
    out
}

fn compact(now_sec: i64) -> SegmentationParams {
    SegmentationParams {
        now_ms: now_sec * 1000,
        gap_close_ms: 300_000,
        min_len_ms: 60_000,
        merge_gap_ms: 600_000,
        absorb_max_ms: 200_000,
        meeting_gap_ms: 150_000,
        focus_min_len_ms: 100_000,
        focus_min_density_fph: 0,
        identity_qualify_ms: 120_000,
        freeze_lookback_ms: 86_400_000,
    }
}

const CLAUDE: (&str, &str) = ("WindowsTerminal", "claude - repo");
const CODEX: (&str, &str) = ("codex", "Codex");
const FOCUS: (&str, &str) = ("Notepad", "notes");

#[test]
fn bundled_taxonomy_v3_parses_at_startup() {
    let engine = SessionEngine::new().expect("bundled taxonomy parses");
    assert_eq!(engine.taxonomy_version(), 3);
}

#[test]
fn spinner_prefix_recognizes_claude_code() {
    let frames = run(1, "pwsh", "\u{2733} Post review comments on PR", 0, 180, 30);
    let sessions = segment_concurrent(&frames, &compact(1000));
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].tool.as_deref(), Some("claude-code"));
}

#[test]
fn browser_ai_requires_browser_stem_and_ai_title() {
    let mut frames = run(1, "chrome", "ChatGPT - Google Chrome", 0, 180, 30);
    frames.extend(run(100, "chrome", "GitHub - Google Chrome", 700, 950, 30));
    let sessions = segment_concurrent(&frames, &compact(2000));
    assert!(sessions
        .iter()
        .any(|s| s.tool.as_deref() == Some("browser-ai")));
    assert!(sessions.iter().any(|s| s.kind == SessionKind::Focus));
}

#[test]
fn chatgpt_renamed_desktop_maps_to_codex_but_classic_does_not() {
    let renamed = run(1, "chatgpt", "ChatGPT", 0, 180, 30);
    let got = segment_concurrent(&renamed, &compact(1000));
    assert_eq!(got[0].tool.as_deref(), Some("codex"));

    let classic = run(100, "chatgpt", "ChatGPT Classic", 0, 180, 30);
    let got = segment_concurrent(&classic, &compact(1000));
    assert_eq!(got[0].kind, SessionKind::Focus);
    assert_eq!(got[0].tool, None);
}

#[test]
fn two_tools_interleaved_form_overlapping_tracks() {
    let mut frames = run(1, CLAUDE.0, CLAUDE.1, 0, 150, 30);
    frames.extend(run(100, CODEX.0, CODEX.1, 180, 330, 30));
    frames.extend(run(200, CLAUDE.0, CLAUDE.1, 360, 510, 30));
    frames.extend(run(300, CODEX.0, CODEX.1, 540, 690, 30));

    let got = segment_concurrent(&frames, &compact(2000));
    assert_eq!(got.len(), 2, "{got:?}");
    let claude = got
        .iter()
        .find(|s| s.tool.as_deref() == Some("claude-code"))
        .unwrap();
    let codex = got
        .iter()
        .find(|s| s.tool.as_deref() == Some("codex"))
        .unwrap();
    assert_eq!((claude.started_at, claude.ended_at), (0, 510_000));
    assert_eq!((codex.started_at, codex.ended_at), (180_000, 690_000));
    assert!(codex.started_at < claude.ended_at);
}

#[test]
fn exclusive_frame_ownership_survives_cross_track_overlap_and_none_absorption() {
    let mut frames = run(1, CLAUDE.0, CLAUDE.1, 0, 150, 30);
    frames.extend(run(100, FOCUS.0, FOCUS.1, 180, 240, 30));
    frames.extend(run(200, CODEX.0, CODEX.1, 270, 420, 30));
    frames.extend(run(300, CLAUDE.0, CLAUDE.1, 450, 600, 30));

    let got = segment_concurrent(&frames, &compact(2000));
    let mut all = HashSet::new();
    for session in &got {
        for id in &session.frame_ids {
            assert!(all.insert(*id), "frame {id} owned twice: {got:?}");
        }
    }
    let claude = got
        .iter()
        .find(|s| s.tool.as_deref() == Some("claude-code"))
        .unwrap();
    assert!(claude.frame_ids.contains(&100));
    assert!(claude.frame_ids.contains(&102));
}

#[test]
fn same_track_splits_only_at_its_own_merge_gap() {
    let mut frames = run(1, CLAUDE.0, CLAUDE.1, 0, 150, 30);
    frames.extend(run(100, CLAUDE.0, CLAUDE.1, 800, 950, 30));
    let got = segment_concurrent(&frames, &compact(2000));
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(got[0].ended_at <= got[1].started_at);
}

#[test]
fn sustained_foreign_identity_does_not_close_incumbent() {
    let mut frames = run(1, CLAUDE.0, CLAUDE.1, 0, 150, 30);
    frames.extend(run(100, CODEX.0, CODEX.1, 180, 430, 30));
    frames.extend(run(200, CLAUDE.0, CLAUDE.1, 460, 710, 30));
    let got = segment_concurrent(&frames, &compact(2000));
    let claude = got
        .iter()
        .find(|s| s.tool.as_deref() == Some("claude-code"))
        .unwrap();
    assert_eq!((claude.started_at, claude.ended_at), (0, 700_000));
}

#[test]
fn sub_qualification_ai_track_is_dropped() {
    let frames = run(1, CODEX.0, CODEX.1, 0, 90, 30);
    assert!(segment_concurrent(&frames, &compact(1000)).is_empty());
}

#[test]
fn long_unrecognized_run_becomes_focus_material() {
    let mut frames = run(1, CLAUDE.0, CLAUDE.1, 0, 200, 30);
    frames.extend(run(100, FOCUS.0, FOCUS.1, 230, 450, 30));
    frames.extend(run(200, CLAUDE.0, CLAUDE.1, 480, 730, 30));
    let got = segment_concurrent(&frames, &compact(2000));
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(got.iter().any(|s| s.context_key == "focus:notepad"));
}

#[test]
fn sparse_focus_is_density_gated_but_ai_is_exempt() {
    let params = SegmentationParams {
        focus_min_density_fph: 90,
        ..compact(2000)
    };
    let sparse_focus = run(1, FOCUS.0, FOCUS.1, 0, 600, 100);
    assert!(segment_concurrent(&sparse_focus, &params).is_empty());

    let sparse_ai = run(100, CLAUDE.0, CLAUDE.1, 0, 200, 100);
    let got = segment_concurrent(&sparse_ai, &params);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].kind, SessionKind::Ai);
}

#[test]
fn meetings_never_absorb_and_can_overlap_ai_and_each_other() {
    let mut frames = run(1, CLAUDE.0, CLAUDE.1, -200, -50, 30);
    frames.extend(run(100, "chrome", "Google Meet", 0, 100, 25));
    frames.extend(run(200, "zoom", "Zoom Meeting", 110, 190, 20));
    frames.extend(run(300, "chrome", "Google Meet", 200, 300, 25));
    frames.push(frame(350, 250, Some(CLAUDE.0), Some(CLAUDE.1)));
    frames.extend(run(400, "zoom", "Zoom Meeting", 320, 445, 25));
    frames.extend(run(500, CLAUDE.0, CLAUDE.1, 500, 650, 30));

    let got = segment_concurrent(&frames, &compact(2000));
    assert_eq!(
        got.iter()
            .filter(|s| s.kind == SessionKind::Meeting)
            .count(),
        2
    );
    let claude = got
        .iter()
        .find(|s| s.tool.as_deref() == Some("claude-code"))
        .unwrap();
    assert!(got
        .iter()
        .filter(|s| s.kind == SessionKind::Meeting)
        .all(|m| { m.started_at < claude.ended_at && claude.started_at < m.ended_at }));
}

#[test]
fn open_flag_tracks_inactivity_at_now() {
    let frames = run(1, CLAUDE.0, CLAUDE.1, 0, 180, 30);
    let open = segment_concurrent(&frames, &compact(200));
    assert!(open[0].open);
    let closed = segment_concurrent(&frames, &compact(1000));
    assert!(!closed[0].open);
}

#[test]
fn confidence_penalizes_absorbed_time_and_keeps_ai_above_focus() {
    let clean = segment_concurrent(&run(1, CLAUDE.0, CLAUDE.1, 0, 300, 30), &compact(2000));
    let mut absorbed_frames = run(1, CLAUDE.0, CLAUDE.1, 0, 150, 30);
    absorbed_frames.extend(run(100, FOCUS.0, FOCUS.1, 180, 300, 30));
    absorbed_frames.extend(run(200, CLAUDE.0, CLAUDE.1, 330, 480, 30));
    let absorbed = segment_concurrent(&absorbed_frames, &compact(2000));
    let focus = segment_concurrent(&run(500, FOCUS.0, FOCUS.1, 0, 300, 30), &compact(2000));

    assert!(clean[0].confidence > absorbed[0].confidence);
    assert!(absorbed[0].confidence > focus[0].confidence);
}

fn content(frame_id: i64, text: &str) -> SessionContent {
    SessionContent {
        frame_id,
        captured_at: frame_id * 1000,
        content_text: text.to_string(),
    }
}

#[test]
fn claude_code_markers_extract_only_explicit_roles() {
    let exchanges = extract_exchanges(
        "claude-code",
        &[
            content(1, "\u{276f} Fix the failing test\nmore detail"),
            content(2, "\u{23fa} I changed the scheduler\nTests now pass"),
        ],
    );
    assert_eq!(exchanges.len(), 2);
    assert_eq!(exchanges[0].role, SessionArtifactRole::User);
    assert_eq!(exchanges[1].role, SessionArtifactRole::Agent);
    assert!(exchanges[0].content.contains("Fix the failing test"));
}

#[test]
fn desktop_and_browser_markers_extract_heading_blocks() {
    let desktop = extract_exchanges(
        "codex",
        &[content(
            1,
            "Codex\nNew task\nScheduled\nPlugins\nChat\nProjects\nQ\nReview this diff\nFile\nWorked for 12s\nThe guard is correct\nView all",
        )],
    );
    assert_eq!(desktop.len(), 2);
    assert_eq!(desktop[0].role, SessionArtifactRole::User);
    assert_eq!(desktop[1].role, SessionArtifactRole::Agent);

    let browser = extract_exchanges(
        "browser-ai",
        &[content(
            2,
            "User: Explain WAL\nChatGPT: WAL permits readers",
        )],
    );
    assert_eq!(browser.len(), 2);
}

#[test]
fn codex_desktop_chrome_is_not_misclassified_as_an_agent_turn() {
    let extracted = extract_exchanges(
        "codex",
        &[content(
            3,
            "Edit\nView Help\nFile\nCodex\nNew task\nScheduled\nPlugins\nChat\nProjects\nQ\nFix login\nHow do I authenticate the MCP?\nFile\nCodex\nTasks\nWorking for 34s\nI'm checking the current configuration guidance.\nThe login state is machine-local.\n@ Ran commands\nAsk for follow-up changes",
        )],
    );

    assert_eq!(extracted.len(), 2);
    assert_eq!(extracted[0].role, SessionArtifactRole::User);
    assert!(extracted[0].content.contains("How do I authenticate"));
    assert_eq!(extracted[1].role, SessionArtifactRole::Agent);
    assert_eq!(
        extracted[1].content,
        "I'm checking the current configuration guidance.\nThe login state is machine-local."
    );
    assert!(extracted
        .iter()
        .all(|exchange| !exchange.content.starts_with("New task")));
}

#[test]
fn no_marker_means_no_exchange_and_duplicates_collapse() {
    assert!(extract_exchanges("codex", &[content(1, "plain captured prose")]).is_empty());
    let same = content(1, "User: Review this diff");
    let got = extract_exchanges("browser-ai", &[same.clone(), same]);
    assert_eq!(got.len(), 1);
}

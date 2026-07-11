//! Dev-only demo-data seeder — **ignored in CI**, run explicitly to populate an
//! isolated store for the README screenshots (`docs/SCREENSHOTS.md`). It writes a real
//! schema-v11 database and copies pre-rendered synthetic capture images into place, so a
//! throwaway dev build pointed at an isolated app-data dir renders fully-populated
//! Timeline / Moment / Insights / Deck / Recall screens. Everything here is **invented**
//! — no personal data ever touches the repo or a screenshot.
//!
//! Run (never against your real app-data dir — use a scratch identifier):
//! ```text
//! DEMO_DB=<dir>/screensearch.db  DEMO_FRAMES_DIR=<dir>/frames \
//! DEMO_SCENE_DIR=<pngs>  DEMO_TODAY_START_MS=<local-midnight-ms> \
//! cargo test -p store --test seed_demo -- --ignored --nocapture
//! ```
//! It seeds entirely through the public [`store::SqliteStore`] API (no raw SQL), so the
//! data is byte-identical in shape to what the live capture + enrichment path writes.

use std::fs;
use std::path::PathBuf;

use store::SqliteStore;
use traits::{
    normalize_text, CaptureTrigger, NewFrame, NewSession, NewSessionArtifact, OcrResult,
    SessionArtifactKind, SessionArtifactRole, SessionHost, SessionKind, TextRole, TextSource,
    TextSpan, VisionAnalysis,
};

const MIN_MS: i64 = 60_000;

/// One synthetic capture. `min` is minutes after the local day's 09:00; `scene` names a
/// pre-rendered PNG under `DEMO_SCENE_DIR`; `session` groups frames into a seeded session.
struct Frame {
    min: i64,
    scene: &'static str,
    app: &'static str,
    title: &'static str,
    url: Option<&'static str>,
    activity: &'static str,
    text: &'static str,
    session: Option<&'static str>,
    mark: Option<&'static str>,
}

/// Compact constructor to keep the table below readable.
#[allow(clippy::too_many_arguments)]
const fn f(
    min: i64,
    scene: &'static str,
    app: &'static str,
    title: &'static str,
    url: Option<&'static str>,
    activity: &'static str,
    text: &'static str,
    session: Option<&'static str>,
    mark: Option<&'static str>,
) -> Frame {
    Frame {
        min,
        scene,
        app,
        title,
        url,
        activity,
        text,
        session,
        mark,
    }
}

/// A plausible knowledge-work day (09:00–16:40 local), every few minutes. The narrative:
/// triage email, a focused coding block pairing with Claude Code, a docs detour, the
/// weekly sync, writing notes, more coding, then reviewing metrics.
#[rustfmt::skip]
fn day() -> Vec<Frame> {
    vec![
        f(2,  "email",     "chrome.exe",          "Inbox — mail.example.com",                 Some("https://mail.example.com/u/0/#inbox"), "Email",      "Inbox: CI passed on docs/synthetic-screenshots. Dependabot bump tokio 1.39 to 1.40. Review requested on pull request 106.", None, None),
        f(9,  "email",     "chrome.exe",          "Inbox — mail.example.com",                 Some("https://mail.example.com/u/0/#inbox"), "Email",      "Weekly digest: three merged pull requests, zero open incidents. Release notes drafted for the next tag.", None, None),
        f(18, "ide",       "Code.exe",            "segmenter.rs — screensearch",              None,                                        "Coding",     "impl Segmenter: idle gap that closes a run, and the rate limiter guarding re-recognition. Pure heuristic, zero model calls.", Some("focus"), None),
        f(24, "ide",       "Code.exe",            "segmenter.rs — screensearch",              None,                                        "Coding",     "for f in frames: let ident = self.recognizer.recognize(f); tracks.accrete(ident, f, self.idle_gap_ms). Exclusive frame ownership per track.", Some("focus"), Some("Wire the rate limiter into the recognizer next")),
        f(31, "ide",       "Code.exe",            "taxonomy.rs — screensearch",               None,                                        "Coding",     "Recognizer taxonomy v3: Claude Code, Codex, Claude desktop, browser AI, and five meeting identities. Match on window title and app hint.", Some("focus"), None),
        f(38, "terminal",  "WindowsTerminal.exe", "Claude Code — screensearch",               None,                                        "AI pairing", "claude code segmentation engine work. Adding the idle-gap rate limiter to the recognizer so a busy track is not re-scanned each frame.", Some("ai"), None),
        f(45, "terminal",  "WindowsTerminal.exe", "Claude Code — screensearch",               None,                                        "AI pairing", "cargo test -p sessions: test result ok, 21 passed, 0 failed. segmenter overlapping tools stay separate, idle gap closes a run.", Some("ai"), None),
        f(52, "ide",       "Code.exe",            "segmenter.rs — screensearch",              None,                                        "Coding",     "let mut tracks = IdentityTracks::new(). The rate limiter bounds how often re-recognition runs on the per-frame hot path.", Some("focus"), None),
        f(59, "terminal",  "WindowsTerminal.exe", "Claude Code — screensearch",               None,                                        "AI pairing", "Running the sessions harness against captured days. Held-out F1 improved; tool recognition accuracy is stable at 1.000.", Some("ai"), None),
        f(66, "ide",       "Code.exe",            "engine.rs — screensearch",                 None,                                        "Coding",     "Segmentation engine test: overlapping AI tools stay separate tracks; each frame is owned by exactly one session.", Some("focus"), None),
        f(74, "docsTokio", "chrome.exe",          "tokio — asynchronous runtime",             Some("https://docs.rs/tokio/latest/tokio/"), "Reading",    "Tokio runtime drives async tasks on a work-stealing scheduler. Blocking work belongs on spawn_blocking. A bounded semaphore is a simple rate limiter.", None, None),
        f(82, "docsTokio", "chrome.exe",          "tokio::sync::Semaphore",                   Some("https://docs.rs/tokio/latest/tokio/sync/"), "Reading",  "A semaphore with N permits caps concurrency to N. Acquire a permit before the call and drop it after to rate limit a downstream service.", None, None),
        f(180, "meeting",  "ms-teams.exe",        "Weekly Sync — Meetings",                   None,                                        "Meeting",    "Weekly sync: sessions arc status, the segmentation engine landed, screenshots pending. Next: rate limiter and the Timeline surface.", Some("meeting"), None),
        f(188, "meeting",  "ms-teams.exe",        "Weekly Sync — Meetings",                   None,                                        "Meeting",    "Team sync discussion: overlapping tool sessions, foreground-only capture ceiling, and the plan for the 0.4.0 release.", Some("meeting"), None),
        f(196, "meeting",  "ms-teams.exe",        "Weekly Sync — Meetings",                   None,                                        "Meeting",    "Action items assigned. Reviewers tagged on the open pull request before any merge to main.", Some("meeting"), Some("Follow up on the release checklist")),
        f(228, "notes",    "Code.exe",            "0.4.0-sessions.md — Notes",                None,                                        "Writing",    "0.4.0 sessions arc plan. Wire the idle-gap rate limiter into the recognizer. Sessions in the Timeline UI. Overlapping tools stay separate.", None, None),
        f(236, "notes",    "Code.exe",            "0.4.0-sessions.md — Notes",                None,                                        "Writing",    "Notes: a rate limiter keeps re-recognition off the per-frame hot path. Each frame owned by exactly one session; cross-track spans may overlap.", None, None),
        f(300, "ide",      "Code.exe",            "segmenter.rs — screensearch",              None,                                        "Coding",     "Back to the segmenter. Hooking the rate limiter to the idle-gap threshold and re-running the validation harness.", Some("focus"), None),
        f(308, "terminal", "WindowsTerminal.exe", "Claude Code — screensearch",               None,                                        "AI pairing", "claude code: refine the recognizer, add a test for the rate limiter, and confirm no regression in tool accuracy.", Some("ai"), None),
        f(316, "terminal", "WindowsTerminal.exe", "Claude Code — screensearch",               None,                                        "AI pairing", "cargo clippy --workspace --all-targets: no warnings. cargo fmt check clean. bindings unchanged.", Some("ai"), None),
        f(360, "docsFts",  "chrome.exe",          "SQLite FTS5 Extension",                    Some("https://www.sqlite.org/fts5.html"),    "Reading",    "FTS5 virtual table for full-text search over content_text. External-content tables mirror an ordinary table, kept in sync by triggers.", None, None),
        f(368, "docsFts",  "chrome.exe",          "SQLite FTS5 Extension",                    Some("https://www.sqlite.org/fts5.html"),    "Reading",    "The porter tokenizer stems terms so a search for run also matches running. snippet() highlights the matched span for the results list.", None, None),
        f(430, "dashboard","chrome.exe",          "Capture metrics — Dashboard",              Some("https://metrics.internal.example/capture"), "Analysis", "Capture metrics: 1,284 frames today, vision throughput 91 per minute, 30-day text index at 2.3 GB. Throughput steady over the last 12 hours.", None, None),
        f(438, "dashboard","chrome.exe",          "Capture metrics — Dashboard",              Some("https://metrics.internal.example/capture"), "Analysis", "Reviewing throughput trend before the release. No regressions; the memory recycle-valve is holding host RAM flat.", None, None),
    ]
}

fn env_path(key: &str) -> PathBuf {
    PathBuf::from(
        std::env::var(key).unwrap_or_else(|_| panic!("{key} must be set for the demo seeder")),
    )
}

/// One content span per frame — enough that `text_spans` is non-empty (the live path
/// writes per-word spans; the exact geometry is irrelevant to the screenshots).
fn spans_for(text: &str) -> Vec<TextSpan> {
    vec![TextSpan {
        normalized_text: normalize_text(text),
        text: text.to_string(),
        source: TextSource::Ocr,
        role: TextRole::Content,
        x: 0.08,
        y: 0.14,
        w: 0.84,
        h: 0.06,
        line_index: 0,
        is_searchable: true,
        suppress_reason: None,
    }]
}

fn ocr_for(text: &str) -> OcrResult {
    OcrResult {
        text: text.to_string(),
        mean_confidence: 0.98,
        engine: "winrt".to_string(),
        spans: spans_for(text),
    }
}

fn vision_for(activity: &str, app: &str, text: &str) -> VisionAnalysis {
    VisionAnalysis {
        description: text.chars().take(140).collect(),
        activity_type: Some(activity.to_string()),
        app_hint: Some(app.to_string()),
        confidence: 0.9,
        model: "demo-seed".to_string(),
    }
}

/// Session metadata keyed by the `session` tag used in [`day`].
fn session_meta(
    tag: &str,
) -> (
    SessionKind,
    Option<&'static str>,
    SessionHost,
    &'static str,
    &'static str,
    &'static str,
) {
    match tag {
        "focus" => (
            SessionKind::Focus,
            None,
            SessionHost::Ide,
            "focus:code",
            "Focused coding — segmenter.rs",
            "A sustained stretch editing the sessions segmentation engine and its taxonomy.",
        ),
        "ai" => (
            SessionKind::Ai,
            Some("claude-code"),
            SessionHost::Terminal,
            "ai:claude-code",
            "Claude Code — segmentation engine",
            "Pairing with Claude Code to add the idle-gap rate limiter and keep tool recognition accurate.",
        ),
        "meeting" => (
            SessionKind::Meeting,
            None,
            SessionHost::Desktop,
            "meeting:weekly-sync",
            "Weekly Sync",
            "The team weekly sync: sessions arc status, the concurrent-tracks model, and release planning.",
        ),
        other => panic!("unknown session tag {other}"),
    }
}

/// Seeded user/agent exchanges for the AI session (best-effort extraction in the live
/// path; hand-written here). Attached to the earliest AI frame for a citation anchor.
fn ai_exchanges(frame_id: i64) -> Vec<NewSessionArtifact> {
    let turns = [
        (SessionArtifactRole::User, "Add the idle-gap rate limiter to the recognizer."),
        (SessionArtifactRole::Agent, "I'll guard re-recognition so a busy track isn't re-scanned each frame, then add a test."),
        (SessionArtifactRole::User, "Run the sessions tests."),
        (SessionArtifactRole::Agent, "cargo test -p sessions: 21 passed, 0 failed. Tool recognition accuracy stayed at 1.000."),
    ];
    turns
        .into_iter()
        .map(|(role, content)| NewSessionArtifact {
            kind: SessionArtifactKind::Exchange,
            role: Some(role),
            frame_id: Some(frame_id),
            content: content.to_string(),
        })
        .collect()
}

#[tokio::test]
#[ignore = "dev-only demo seeder; run explicitly with DEMO_* env vars set"]
async fn seed_demo_db() {
    let db = env_path("DEMO_DB");
    let frames_dir = env_path("DEMO_FRAMES_DIR");
    let scene_dir = env_path("DEMO_SCENE_DIR");
    let day_start: i64 = std::env::var("DEMO_TODAY_START_MS")
        .expect("DEMO_TODAY_START_MS must be set (local midnight, unix ms)")
        .parse()
        .expect("DEMO_TODAY_START_MS must be an integer");
    // Frames start at 09:00 local.
    let base = day_start + 9 * 60 * MIN_MS;

    let frames = day();

    // Copy every referenced scene PNG into <appdata>/frames/demo/ (the asset scope is
    // $APPDATA/frames/**), so the stored relative image_path resolves in the WebView.
    let demo_dir = frames_dir.join("demo");
    fs::create_dir_all(&demo_dir).expect("create frames/demo dir");
    let mut scenes: Vec<&str> = frames.iter().map(|fr| fr.scene).collect();
    scenes.sort_unstable();
    scenes.dedup();
    for scene in &scenes {
        let src = scene_dir.join(format!("{scene}.png"));
        let dst = demo_dir.join(format!("{scene}.png"));
        fs::copy(&src, &dst)
            .unwrap_or_else(|e| panic!("copy scene {} -> {}: {e}", src.display(), dst.display()));
    }

    let store = SqliteStore::open_path(&db).expect("open demo store");

    // Insert frames, remembering each frame's id and its optional session tag.
    let mut session_frames: Vec<(&str, i64, i64)> = Vec::new(); // (tag, frame_id, captured_at)
    let mut prev_scene = "";
    for fr in &frames {
        let captured_at = base + fr.min * MIN_MS;
        let trigger = if fr.scene == prev_scene {
            CaptureTrigger::Timer
        } else {
            CaptureTrigger::ForegroundChange
        };
        prev_scene = fr.scene;

        let frame_id = store
            .insert_frame(NewFrame {
                captured_at,
                monitor_index: 0,
                width: 1600,
                height: 1000,
                image_path: format!("frames/demo/{}.png", fr.scene),
                content_hash: format!("demo-{}", fr.min),
                app_hint: Some(fr.app.to_string()),
                window_title: Some(fr.title.to_string()),
                browser_url: fr.url.map(str::to_string),
                capture_trigger: Some(trigger),
            })
            .await
            .expect("insert frame");

        store
            .insert_ocr(frame_id, ocr_for(fr.text))
            .await
            .expect("insert ocr");
        store
            .insert_vision(frame_id, vision_for(fr.activity, fr.app, fr.text))
            .await
            .expect("insert vision");

        if let Some(note) = fr.mark {
            store
                .insert_mark(frame_id, captured_at + 30_000, Some(note.to_string()))
                .await
                .expect("insert mark");
        }
        if let Some(tag) = fr.session {
            session_frames.push((tag, frame_id, captured_at));
        }
    }

    // Build one session per tag from its owned frames.
    for tag in ["focus", "ai", "meeting"] {
        let mut owned: Vec<(i64, i64)> = session_frames
            .iter()
            .filter(|(t, _, _)| *t == tag)
            .map(|(_, id, at)| (*id, *at))
            .collect();
        owned.sort_by_key(|(_, at)| *at);
        let Some(&(_, started_at)) = owned.first() else {
            continue;
        };
        let ended_at = owned.last().map(|(_, at)| *at + 4 * MIN_MS);
        let (kind, tool, host, context_key, title, summary) = session_meta(tag);

        let session_id = store
            .insert_session(NewSession {
                started_at,
                ended_at,
                kind,
                tool: tool.map(str::to_string),
                host: Some(host),
                context_key: context_key.to_string(),
                confidence: 0.86,
                frozen: false,
            })
            .await
            .expect("insert session");

        let ids: Vec<i64> = owned.iter().map(|(id, _)| *id).collect();
        store
            .assign_frames_session(&ids, Some(session_id))
            .await
            .expect("assign frames to session");
        store
            .set_session_title_summary(session_id, title, summary, "demo-seed")
            .await
            .expect("set session title/summary");

        if tag == "ai" {
            store
                .insert_session_artifacts(session_id, &ai_exchanges(ids[0]))
                .await
                .expect("insert ai exchanges");
        }
    }

    let total = frames.len();
    println!("seeded {total} frames + 3 sessions into {}", db.display());
}

//! Dev-only demo-data seeder — **ignored in CI**, run explicitly to populate an
//! isolated store for the README screenshots (`docs/SCREENSHOTS.md`). It writes a real
//! schema-v11 database and copies pre-rendered synthetic capture images into place, so a
//! throwaway dev build pointed at an isolated app-data dir renders fully-populated
//! Timeline / Moment / Insights / Deck / Recall screens. Everything here is **invented**
//! — no personal data ever touches the repo or a screenshot.
//!
//! Frames are dated a few minutes into the future of the seeding moment (still today) and
//! sessions are seeded **frozen**: the background sessions scheduler only segments the
//! past, and never deletes a frozen row, so the curated overlapping sessions survive
//! untouched (`crates/kernel/src/sessions_scheduler.rs`).
//!
//! Run (never against your real app-data dir — use a scratch identifier):
//! ```text
//! DEMO_DB=<dir>/screensearch.db  DEMO_FRAMES_DIR=<dir>/frames \
//! DEMO_SCENE_DIR=<pngs>  DEMO_NOW_MS=<Date.now()> \
//! cargo test -p store --test seed_demo -- --ignored --nocapture
//! ```
//! It seeds entirely through the public [`store::SqliteStore`] API (no raw SQL).

use std::fs;
use std::path::PathBuf;

use store::SqliteStore;
use traits::{
    normalize_text, CaptureTrigger, NewFrame, NewSession, NewSessionArtifact, OcrResult,
    SessionArtifactKind, SessionArtifactRole, SessionHost, SessionKind, TextRole, TextSource,
    TextSpan, VisionAnalysis,
};

const MIN_MS: i64 = 60_000;

/// One activity block → one frozen, titled session, densely sampled every `step` minutes
/// over `[start, end]` (minutes after the seed base time). Overlapping blocks (a coding
/// block and a Claude Code block at the same time) model concurrent sessions.
struct Block {
    start: i64,
    end: i64,
    step: i64,
    scene: &'static str,
    app: &'static str,
    window: &'static str,
    url: Option<&'static str>,
    activity: &'static str,
    kind: SessionKind,
    tool: Option<&'static str>,
    host: SessionHost,
    context_key: &'static str,
    session_title: &'static str,
    session_summary: &'static str,
    confidence: f64,
    texts: &'static [&'static str],
    exchanges: bool,
    mark: Option<&'static str>,
}

#[allow(clippy::too_many_arguments)]
const fn b(
    start: i64,
    end: i64,
    step: i64,
    scene: &'static str,
    app: &'static str,
    window: &'static str,
    url: Option<&'static str>,
    activity: &'static str,
    kind: SessionKind,
    tool: Option<&'static str>,
    host: SessionHost,
    context_key: &'static str,
    session_title: &'static str,
    session_summary: &'static str,
    confidence: f64,
    texts: &'static [&'static str],
    exchanges: bool,
    mark: Option<&'static str>,
) -> Block {
    Block {
        start,
        end,
        step,
        scene,
        app,
        window,
        url,
        activity,
        kind,
        tool,
        host,
        context_key,
        session_title,
        session_summary,
        confidence,
        texts,
        exchanges,
        mark,
    }
}

/// The seeded day: a knowledge-work session with two concurrent coding+Claude-Code stretches.
#[rustfmt::skip]
fn blocks() -> Vec<Block> {
    vec![
        b(0, 12, 3, "email", "chrome.exe", "Inbox — mail.example.com", Some("https://mail.example.com/u/0/#inbox"), "Email",
          SessionKind::Focus, None, SessionHost::Browser, "focus:email", "Inbox triage",
          "Morning email triage before starting the sessions work.", 0.78,
          &["Inbox: CI passed on docs/synthetic-screenshots. Dependabot bump tokio 1.39 to 1.40.",
            "Review requested on pull request 106. Weekly digest: three merged PRs, zero incidents.",
            "Release notes drafted for the next tag. Reviewers tagged before any merge to main."], false, None),

        b(15, 69, 3, "ide", "Code.exe", "segmenter.rs — screensearch", None, "Coding",
          SessionKind::Focus, None, SessionHost::Ide, "focus:code", "Focused coding — segmenter.rs",
          "A sustained stretch editing the sessions segmentation engine and its taxonomy.", 0.91,
          &["impl Segmenter: idle gap that closes a run, and the rate limiter guarding re-recognition. Pure heuristic, zero model calls.",
            "for f in frames: let ident = self.recognizer.recognize(f); tracks.accrete(ident, f, self.idle_gap_ms). Exclusive frame ownership per track.",
            "Recognizer taxonomy v3: Claude Code, Codex, Claude desktop, browser AI, and five meeting identities. Match on window title and app hint.",
            "let mut tracks = IdentityTracks::new(). The rate limiter bounds how often re-recognition runs on the per-frame hot path.",
            "Segmentation engine test: overlapping AI tools stay separate tracks; each frame is owned by exactly one session."],
          false, Some("Wire the rate limiter into the recognizer next")),

        b(30, 93, 3, "terminal", "WindowsTerminal.exe", "Claude Code — screensearch", None, "AI pairing",
          SessionKind::Ai, Some("claude-code"), SessionHost::Terminal, "ai:claude-code", "Claude Code — segmentation engine",
          "Pairing with Claude Code to add the idle-gap rate limiter and keep tool recognition accurate.", 0.88,
          &["claude code segmentation engine work. Adding the idle-gap rate limiter to the recognizer so a busy track is not re-scanned each frame.",
            "cargo test -p sessions: test result ok, 21 passed, 0 failed. segmenter overlapping tools stay separate, idle gap closes a run.",
            "Running the sessions harness against captured days. Held-out F1 improved; tool recognition accuracy is stable at 1.000.",
            "Refine the recognizer, add a test for the rate limiter, and confirm no regression in tool accuracy."],
          true, None),

        b(99, 117, 3, "docsTokio", "chrome.exe", "tokio — asynchronous runtime", Some("https://docs.rs/tokio/latest/tokio/"), "Reading",
          SessionKind::Focus, None, SessionHost::Browser, "focus:docs", "Reading — Tokio runtime",
          "Reading the Tokio runtime and semaphore docs while designing the rate limiter.", 0.72,
          &["Tokio runtime drives async tasks on a work-stealing scheduler. Blocking work belongs on spawn_blocking. A bounded semaphore is a simple rate limiter.",
            "A semaphore with N permits caps concurrency to N. Acquire a permit before the call and drop it after to rate limit a downstream service."],
          false, None),

        b(126, 162, 4, "meeting", "ms-teams.exe", "Weekly Sync — Meetings", None, "Meeting",
          SessionKind::Meeting, None, SessionHost::Desktop, "meeting:weekly-sync", "Weekly Sync",
          "The team weekly sync: sessions arc status, the concurrent-tracks model, and release planning.", 0.83,
          &["Weekly sync: sessions arc status, the segmentation engine landed, screenshots pending. Next: rate limiter and the Timeline surface.",
            "Team sync discussion: overlapping tool sessions, foreground-only capture ceiling, and the plan for the 0.4.0 release.",
            "Action items assigned. Reviewers tagged on the open pull request before any merge to main."],
          false, Some("Follow up on the release checklist")),

        b(168, 192, 3, "notes", "Code.exe", "0.4.0-sessions.md — Notes", None, "Writing",
          SessionKind::Focus, None, SessionHost::Ide, "focus:notes", "Writing — 0.4.0 plan",
          "Writing up the plan: wire the rate limiter, then sessions in the Timeline UI.", 0.8,
          &["0.4.0 sessions arc plan. Wire the idle-gap rate limiter into the recognizer. Sessions in the Timeline UI. Overlapping tools stay separate.",
            "Notes: a rate limiter keeps re-recognition off the per-frame hot path. Each frame owned by exactly one session; cross-track spans may overlap."],
          false, None),

        b(201, 249, 3, "ide", "Code.exe", "segmenter.rs — screensearch", None, "Coding",
          SessionKind::Focus, None, SessionHost::Ide, "focus:code", "Coding — recognizer rate limiter",
          "Back to the segmenter: hooking the rate limiter to the idle-gap threshold.", 0.9,
          &["Back to the segmenter. Hooking the rate limiter to the idle-gap threshold and re-running the validation harness.",
            "The rate limiter bounds how often re-recognition runs on the per-frame hot path. Idempotent and resumable.",
            "impl Segmenter: idle gap that closes a run, and the rate limiter guarding re-recognition."],
          false, None),

        b(210, 264, 3, "terminal", "WindowsTerminal.exe", "Claude Code — screensearch", None, "AI pairing",
          SessionKind::Ai, Some("claude-code"), SessionHost::Terminal, "ai:claude-code", "Claude Code — verify and lint",
          "Second Claude Code stretch: add the rate-limiter test, run clippy and fmt.", 0.87,
          &["claude code: refine the recognizer, add a test for the rate limiter, and confirm no regression in tool accuracy.",
            "cargo clippy --workspace --all-targets: no warnings. cargo fmt check clean. bindings unchanged.",
            "cargo test -p sessions: 21 passed, 0 failed. Tool recognition accuracy stayed at 1.000."],
          true, None),

        b(270, 288, 3, "docsFts", "chrome.exe", "SQLite FTS5 Extension", Some("https://www.sqlite.org/fts5.html"), "Reading",
          SessionKind::Focus, None, SessionHost::Browser, "focus:docs", "Reading — SQLite FTS5",
          "Refreshing FTS5 snippet and tokenizer behavior for the search results.", 0.71,
          &["FTS5 virtual table for full-text search over content_text. External-content tables mirror an ordinary table, kept in sync by triggers.",
            "The porter tokenizer stems terms so a search for run also matches running. snippet() highlights the matched span for the results list."],
          false, None),

        b(294, 312, 3, "dashboard", "chrome.exe", "Capture metrics — Dashboard", Some("https://metrics.internal.example/capture"), "Analysis",
          SessionKind::Focus, None, SessionHost::Browser, "focus:metrics", "Reviewing capture metrics",
          "Reviewing throughput and index size before the release.", 0.75,
          &["Capture metrics: 1,284 frames today, vision throughput 91 per minute, 30-day text index at 2.3 GB. Throughput steady over the last 12 hours.",
            "Reviewing throughput trend before the release. No regressions; the memory recycle-valve is holding host RAM flat."],
          false, None),
    ]
}

fn env_path(key: &str) -> PathBuf {
    PathBuf::from(
        std::env::var(key).unwrap_or_else(|_| panic!("{key} must be set for the demo seeder")),
    )
}

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
    let now_ms: i64 = std::env::var("DEMO_NOW_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_millis() as i64
        });
    // Frames start a few minutes ahead of now so the scheduler (which only segments the
    // past) never touches them.
    let base = now_ms + 3 * MIN_MS;

    let blocks = blocks();

    // Copy every referenced scene PNG into <appdata>/frames/demo/ (asset scope $APPDATA/frames/**).
    let demo_dir = frames_dir.join("demo");
    fs::create_dir_all(&demo_dir).expect("create frames/demo dir");
    let mut scenes: Vec<&str> = blocks.iter().map(|bl| bl.scene).collect();
    scenes.sort_unstable();
    scenes.dedup();
    for scene in &scenes {
        let src = scene_dir.join(format!("{scene}.png"));
        let dst = demo_dir.join(format!("{scene}.png"));
        fs::copy(&src, &dst)
            .unwrap_or_else(|e| panic!("copy scene {} -> {}: {e}", src.display(), dst.display()));
    }

    let store = SqliteStore::open_path(&db).expect("open demo store");

    let mut prev_scene = "";
    let mut hash = 0i64;
    let mut total_frames = 0usize;

    for bl in &blocks {
        // Dense frames across the block.
        let mut ids: Vec<i64> = Vec::new();
        let mut first_at = i64::MAX;
        let mut last_at = i64::MIN;
        let mut i = 0usize;
        let mut t = bl.start;
        while t <= bl.end {
            let captured_at = base + t * MIN_MS;
            first_at = first_at.min(captured_at);
            last_at = last_at.max(captured_at);
            let trigger = if bl.scene == prev_scene {
                CaptureTrigger::Timer
            } else {
                CaptureTrigger::ForegroundChange
            };
            prev_scene = bl.scene;
            hash += 1;
            let text = bl.texts[i % bl.texts.len()];

            let frame_id = store
                .insert_frame(NewFrame {
                    captured_at,
                    monitor_index: 0,
                    width: 1600,
                    height: 1000,
                    image_path: format!("frames/demo/{}.png", bl.scene),
                    content_hash: format!("demo-{hash}"),
                    app_hint: Some(bl.app.to_string()),
                    window_title: Some(bl.window.to_string()),
                    browser_url: bl.url.map(str::to_string),
                    capture_trigger: Some(trigger),
                })
                .await
                .expect("insert frame");
            store
                .insert_ocr(frame_id, ocr_for(text))
                .await
                .expect("insert ocr");
            store
                .insert_vision(frame_id, vision_for(bl.activity, bl.app, text))
                .await
                .expect("insert vision");

            // Mark the first frame of a block that carries one.
            if i == 0 {
                if let Some(note) = bl.mark {
                    store
                        .insert_mark(frame_id, captured_at + 30_000, Some(note.to_string()))
                        .await
                        .expect("insert mark");
                }
            }

            ids.push(frame_id);
            total_frames += 1;
            i += 1;
            t += bl.step;
        }

        // One frozen session owning the whole block.
        let session_id = store
            .insert_session(NewSession {
                started_at: first_at,
                ended_at: Some(last_at),
                kind: bl.kind,
                tool: bl.tool.map(str::to_string),
                host: Some(bl.host),
                context_key: bl.context_key.to_string(),
                confidence: bl.confidence,
                frozen: true,
            })
            .await
            .expect("insert session");
        store
            .assign_frames_session(&ids, Some(session_id))
            .await
            .expect("assign frames to session");
        store
            .set_session_title_summary(
                session_id,
                bl.session_title,
                bl.session_summary,
                "demo-seed",
            )
            .await
            .expect("set session title/summary");
        if bl.exchanges {
            store
                .insert_session_artifacts(session_id, &ai_exchanges(ids[0]))
                .await
                .expect("insert ai exchanges");
        }
    }

    // Resolve one mark so the Intentions strip shows both open and resolved states.
    let marks = store.list_marks().await.expect("list marks");
    if let Some(oldest) = marks.iter().min_by_key(|m| m.created_at) {
        store
            .resolve_mark(oldest.mark_id, base + 200 * MIN_MS)
            .await
            .expect("resolve mark");
    }

    println!(
        "seeded {total_frames} frames + {} frozen sessions into {}",
        blocks.len(),
        db.display()
    );
}

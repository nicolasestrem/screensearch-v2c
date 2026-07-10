use store::SqliteStore;
use traits::{
    NewFrame, NewSession, NewSessionArtifact, OcrResult, SessionArtifactKind, SessionArtifactRole,
    SessionFilter, SessionHost, SessionKind,
};

fn frame(at: i64, app: &str, title: &str) -> NewFrame {
    NewFrame {
        captured_at: at,
        monitor_index: 0,
        width: 1920,
        height: 1080,
        image_path: format!("frames/{at}.webp"),
        content_hash: format!("hash-{at}"),
        app_hint: Some(app.to_string()),
        window_title: Some(title.to_string()),
        browser_url: None,
        capture_trigger: None,
    }
}

fn session(started_at: i64, ended_at: Option<i64>, key: &str) -> NewSession {
    NewSession {
        started_at,
        ended_at,
        kind: SessionKind::Ai,
        tool: Some("claude-code".to_string()),
        host: Some(SessionHost::Terminal),
        context_key: key.to_string(),
        confidence: 0.9,
        frozen: false,
    }
}

async fn seed_frame(store: &SqliteStore, at: i64, text: &str) -> i64 {
    let id = store
        .insert_frame(frame(at, "WindowsTerminal", "claude - repo"))
        .await
        .unwrap();
    store
        .insert_ocr(
            id,
            OcrResult {
                text: text.to_string(),
                mean_confidence: 0.8,
                engine: "test".to_string(),
                spans: Vec::new(),
            },
        )
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn session_crud_is_unfrozen_only_and_ids_stay_stable() {
    let store = SqliteStore::open_in_memory().unwrap();
    let id = store
        .insert_session(session(100, Some(200), "ai:claude-code"))
        .await
        .unwrap();
    let mut changed = session(90, Some(250), "ai:claude-code");
    changed.confidence = 0.8;
    assert!(store
        .update_unfrozen_session(id, changed.clone())
        .await
        .unwrap());
    assert_eq!(store.get_session(id).await.unwrap().unwrap().started_at, 90);

    assert_eq!(store.freeze_sessions(300).await.unwrap(), 1);
    assert!(store.get_session(id).await.unwrap().unwrap().frozen);
    changed.started_at = 1;
    assert!(!store.update_unfrozen_session(id, changed).await.unwrap());
    assert!(!store.delete_unfrozen_session(id).await.unwrap());
    assert_eq!(store.get_session(id).await.unwrap().unwrap().started_at, 90);
}

#[tokio::test]
async fn frozen_session_frames_cannot_be_cleared_or_reassigned() {
    let store = SqliteStore::open_in_memory().unwrap();
    let frame_id = seed_frame(&store, 100, "content").await;
    let session_id = store
        .insert_session(session(100, Some(200), "ai:claude-code"))
        .await
        .unwrap();
    assert_eq!(
        store
            .assign_frames_session(&[frame_id], Some(session_id))
            .await
            .unwrap(),
        1
    );
    store.freeze_sessions(300).await.unwrap();
    assert_eq!(
        store
            .assign_frames_session(&[frame_id], None)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        store.session_frames_meta(session_id).await.unwrap().len(),
        1
    );
}

#[tokio::test]
async fn deleting_unfrozen_sessions_preserves_frames_text_and_marks() {
    let store = SqliteStore::open_in_memory().unwrap();
    let frame_id = seed_frame(&store, 100, "durable filtered content").await;
    let mark_id = store.insert_mark(frame_id, 150, None).await.unwrap();
    let session_id = store
        .insert_session(session(100, Some(200), "ai:claude-code"))
        .await
        .unwrap();
    store
        .assign_frames_session(&[frame_id], Some(session_id))
        .await
        .unwrap();
    store
        .insert_session_artifacts(
            session_id,
            &[NewSessionArtifact {
                kind: SessionArtifactKind::Exchange,
                role: Some(SessionArtifactRole::User),
                frame_id: Some(frame_id),
                content: "do the work".to_string(),
            }],
        )
        .await
        .unwrap();

    assert!(store.delete_unfrozen_session(session_id).await.unwrap());
    assert!(store.get_frame(frame_id).await.unwrap().is_some());
    assert_eq!(store.list_marks().await.unwrap()[0].mark_id, mark_id);
    assert_eq!(
        store.content_texts_for_frames(&[frame_id]).await.unwrap()[0].content_text,
        "durable filtered content"
    );
    assert!(store
        .list_session_artifacts(session_id)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn session_queries_use_half_open_overlap_and_request_time_for_open_rows() {
    let store = SqliteStore::open_in_memory().unwrap();
    let closed = store
        .insert_session(session(100, Some(300), "ai:claude-code"))
        .await
        .unwrap();
    let open = store
        .insert_session(session(400, None, "ai:claude-code"))
        .await
        .unwrap();

    let got = store
        .sessions_in_range(SessionFilter {
            from_ms: Some(250),
            to_ms: Some(450),
            now_ms: 500,
            ..SessionFilter::default()
        })
        .await
        .unwrap();
    let ids: Vec<i64> = got.iter().map(|session| session.id).collect();
    assert_eq!(ids, vec![open, closed]);

    let touching = store
        .sessions_in_range(SessionFilter {
            from_ms: Some(300),
            to_ms: Some(400),
            now_ms: 500,
            ..SessionFilter::default()
        })
        .await
        .unwrap();
    assert!(touching.is_empty(), "half-open boundaries do not overlap");
}

#[tokio::test]
async fn frame_metadata_and_content_reads_are_chronological() {
    let store = SqliteStore::open_in_memory().unwrap();
    let later = seed_frame(&store, 200, "later").await;
    let earlier = seed_frame(&store, 100, "earlier").await;
    let frames = store.frames_meta_in_range(50, 250).await.unwrap();
    assert_eq!(
        frames.iter().map(|frame| frame.id).collect::<Vec<_>>(),
        vec![earlier, later]
    );
    let contents = store
        .content_texts_for_frames(&[later, earlier])
        .await
        .unwrap();
    assert_eq!(
        contents
            .iter()
            .map(|content| content.content_text.as_str())
            .collect::<Vec<_>>(),
        vec!["earlier", "later"]
    );
}

#[tokio::test]
async fn artifact_checks_and_delete_by_kind_are_enforced() {
    let store = SqliteStore::open_in_memory().unwrap();
    let frame_id = seed_frame(&store, 100, "content").await;
    let session_id = store
        .insert_session(session(100, Some(200), "ai:claude-code"))
        .await
        .unwrap();
    let invalid = store
        .insert_session_artifacts(
            session_id,
            &[NewSessionArtifact {
                kind: SessionArtifactKind::Exchange,
                role: None,
                frame_id: Some(frame_id),
                content: "roleless".to_string(),
            }],
        )
        .await;
    assert!(invalid.is_err(), "schema requires an exchange role");

    let ids = store
        .insert_session_artifacts(
            session_id,
            &[NewSessionArtifact {
                kind: SessionArtifactKind::Exchange,
                role: Some(SessionArtifactRole::Agent),
                frame_id: Some(frame_id),
                content: "answer".to_string(),
            }],
        )
        .await
        .unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(
        store
            .list_session_artifacts(session_id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        store
            .delete_session_artifacts_by_kind(session_id, SessionArtifactKind::Exchange)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn title_summary_cache_updates_the_row_without_touching_boundaries() {
    let store = SqliteStore::open_in_memory().unwrap();
    let session_id = store
        .insert_session(session(100, Some(200), "ai:claude-code"))
        .await
        .unwrap();
    assert!(store
        .set_session_title_summary(
            session_id,
            "Fix scheduler",
            "Changed the loop.",
            "model.gguf"
        )
        .await
        .unwrap());
    let got = store.get_session(session_id).await.unwrap().unwrap();
    assert_eq!(got.title.as_deref(), Some("Fix scheduler"));
    assert_eq!(got.summary.as_deref(), Some("Changed the loop."));
    assert_eq!(got.summary_model.as_deref(), Some("model.gguf"));
    assert_eq!((got.started_at, got.ended_at), (100, Some(200)));
}

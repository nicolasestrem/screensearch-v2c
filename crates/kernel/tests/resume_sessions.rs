use kernel::resume::where_was_i;
use store::SqliteStore;
use traits::{NewFrame, NewSession, SessionHost, SessionKind, Settings};

async fn frame(store: &SqliteStore, at: i64, app: &str) -> i64 {
    store
        .insert_frame(NewFrame {
            captured_at: at,
            monitor_index: 0,
            width: 1920,
            height: 1080,
            image_path: format!("frames/{at}.webp"),
            content_hash: format!("hash-{at}"),
            app_hint: Some(app.to_string()),
            window_title: Some(app.to_string()),
            browser_url: None,
            capture_trigger: None,
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn resume_context_hydrates_session_and_omits_a_deleted_session() {
    let store = SqliteStore::open_in_memory().unwrap();
    let first = frame(&store, 0, "Code").await;
    let representative = frame(&store, 120_000, "Code").await;
    frame(&store, 200_000, "Firefox").await;
    frame(&store, 320_000, "Firefox").await;
    let session_id = store
        .insert_session(NewSession {
            started_at: 0,
            ended_at: Some(120_000),
            kind: SessionKind::Focus,
            tool: None,
            host: Some(SessionHost::Ide),
            context_key: "focus:code".to_string(),
            confidence: 0.75,
            frozen: false,
        })
        .await
        .unwrap();
    store
        .assign_frames_session(&[first, representative], Some(session_id))
        .await
        .unwrap();

    let settings = Settings::default();
    let joined = where_was_i(&store, &settings).await.unwrap().unwrap();
    assert_eq!(joined.frame_id, representative);
    assert_eq!(
        joined.session.as_ref().map(|session| session.id),
        Some(session_id)
    );

    assert!(store.delete_unfrozen_session(session_id).await.unwrap());
    let deleted = where_was_i(&store, &settings).await.unwrap().unwrap();
    assert_eq!(deleted.frame_id, representative);
    assert!(deleted.session.is_none());
}

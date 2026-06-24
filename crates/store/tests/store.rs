//! Integration tests for the `store` crate — the data spine (`03 §4/§5/§10`).
//!
//! All tests run against an in-memory SQLite DB (`03 §10`). Because the store
//! keeps a single connection for its lifetime, `:memory:` persists across calls.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use store::{SqliteStore, EMBEDDING_DIM};
use traits::{
    ChunkSource, Embedding, EmbeddingProvider, FrameMeta, JobKind, JobState, NewFrame, NewJob,
    OcrResult, SearchQuery, TimeRange, TimelineBucket, VisionAnalysis,
};

/// A job of the given kind with sensible defaults (immediately runnable).
fn job(kind: JobKind, priority: i64, max_attempts: i64, not_before: i64) -> NewJob {
    NewJob {
        kind,
        frame_id: None,
        priority,
        max_attempts,
        not_before,
    }
}

/// A deterministic stand-in for fastembed: it maps each query string to a
/// pre-registered vector, so the vector arm of `hybrid_search` is fully exercised
/// without the real model (the P3 wiring just swaps this for the fastembed impl).
struct FakeEmbedder {
    by_text: HashMap<String, Embedding>,
}

#[async_trait::async_trait]
impl EmbeddingProvider for FakeEmbedder {
    fn dim(&self) -> usize {
        EMBEDDING_DIM
    }
    async fn embed_texts(&self, inputs: &[String]) -> traits::Result<Vec<Embedding>> {
        inputs
            .iter()
            .map(|s| {
                self.by_text
                    .get(s)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("fake embedder has no vector for {s:?}"))
            })
            .collect()
    }
    async fn embed_image(&self, _image: &image::RgbaImage) -> traits::Result<Embedding> {
        anyhow::bail!("image embedding unused in these tests")
    }
}

/// A unit basis vector of the embedding dimension with `1.0` at `hot`. Cosine
/// distance between two distinct one-hot vectors is 1.0; between identical ones
/// it is ~0 — enough to make KNN ordering deterministic in tests.
fn one_hot(hot: usize) -> Embedding {
    let mut v = vec![0.0_f32; EMBEDDING_DIM];
    v[hot] = 1.0;
    Embedding(v)
}

/// A throwaway frame with the given content marker; `image_path`/`content_hash`
/// derive from it so fixtures are distinct.
fn frame_at(captured_at: i64) -> NewFrame {
    NewFrame {
        captured_at,
        monitor_index: 0,
        width: 1920,
        height: 1080,
        image_path: format!("frames/{captured_at}.jpg"),
        content_hash: format!("hash-{captured_at}"),
        app_hint: Some("Firefox".to_string()),
        window_title: Some("Inbox".to_string()),
        browser_url: Some("https://mail.example.com".to_string()),
    }
}

/// Opening a fresh store applies the forward-only migrations and lands at the
/// latest schema version (`03 §4`, `12`).
#[tokio::test]
async fn open_in_memory_migrates_to_latest_schema_version() {
    let store = SqliteStore::open_in_memory().expect("open in-memory store");
    assert_eq!(
        store.schema_version().expect("read schema_version"),
        store::LATEST_SCHEMA_VERSION,
    );
}

#[test]
fn open_path_rejects_future_schema_version() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("screensearch.db");

    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE schema_version (version INTEGER NOT NULL);")
            .unwrap();
        conn.execute(
            "INSERT INTO schema_version (version) VALUES (?1)",
            [store::LATEST_SCHEMA_VERSION + 1],
        )
        .unwrap();
    }

    let err = match SqliteStore::open_path(&db) {
        Ok(_) => panic!("future schema opened successfully"),
        Err(e) => e.to_string(),
    };
    assert!(
        err.contains("newer schema_version"),
        "future schema should be rejected, got {err:?}"
    );
}

#[tokio::test]
async fn settings_round_trip_and_overwrite() {
    let store = SqliteStore::open_in_memory().unwrap();

    assert_eq!(
        store.get_setting("capture.interval_ms").await.unwrap(),
        None
    );

    store
        .set_setting("capture.interval_ms", "3000")
        .await
        .unwrap();
    assert_eq!(
        store.get_setting("capture.interval_ms").await.unwrap(),
        Some("3000".to_string())
    );

    // set is an upsert — second write overwrites
    store
        .set_setting("capture.interval_ms", "5000")
        .await
        .unwrap();
    assert_eq!(
        store.get_setting("capture.interval_ms").await.unwrap(),
        Some("5000".to_string())
    );
}

#[tokio::test]
async fn insert_frame_then_get_frame_returns_context() {
    let store = SqliteStore::open_in_memory().unwrap();
    let id = store.insert_frame(frame_at(1_000)).await.unwrap();

    let detail = store.get_frame(id).await.unwrap().expect("frame exists");
    assert_eq!(detail.frame_id, id);
    assert_eq!(detail.captured_at, 1_000);
    assert_eq!(detail.width, 1920);
    assert_eq!(detail.app_hint.as_deref(), Some("Firefox"));
    assert_eq!(detail.window_title.as_deref(), Some("Inbox"));
    // no OCR / vision / tags yet
    assert_eq!(detail.text, None);
    assert!(detail.vision.is_none());
    assert!(detail.tags.is_empty());

    // a missing id is None, not an error
    assert!(store.get_frame(999).await.unwrap().is_none());
}

#[tokio::test]
async fn insert_ocr_then_get_frame_has_text() {
    let store = SqliteStore::open_in_memory().unwrap();
    let id = store.insert_frame(frame_at(2_000)).await.unwrap();
    store
        .insert_ocr(
            id,
            OcrResult {
                text: "quarterly invoice total due".to_string(),
                mean_confidence: 0.94,
                engine: "winrt".to_string(),
            },
        )
        .await
        .unwrap();

    let detail = store.get_frame(id).await.unwrap().unwrap();
    assert_eq!(detail.text.as_deref(), Some("quarterly invoice total due"));
}

#[tokio::test]
async fn insert_vision_then_get_frame_has_analysis() {
    let store = SqliteStore::open_in_memory().unwrap();
    let id = store.insert_frame(frame_at(3_000)).await.unwrap();
    store
        .insert_vision(
            id,
            VisionAnalysis {
                description: "a code editor showing Rust".to_string(),
                activity_type: Some("coding".to_string()),
                app_hint: Some("VS Code".to_string()),
                confidence: 0.81,
                model: "qwen3-vl-4b".to_string(),
            },
        )
        .await
        .unwrap();

    let detail = store.get_frame(id).await.unwrap().unwrap();
    let vision = detail.vision.expect("vision present");
    assert_eq!(vision.description, "a code editor showing Rust");
    assert_eq!(vision.activity_type.as_deref(), Some("coding"));
    // 03 §4: frames.activity_type is "filled by vision" — insert_vision mirrors the
    // classification onto the frame for fast timeline filtering.
    assert_eq!(detail.activity_type.as_deref(), Some("coding"));
}

#[tokio::test]
async fn text_embedding_knn_orders_by_cosine_distance() {
    let store = SqliteStore::open_in_memory().unwrap();
    let a = store.insert_frame(frame_at(10)).await.unwrap();
    let b = store.insert_frame(frame_at(20)).await.unwrap();
    store
        .upsert_text_embedding(a, 0, "alpha", ChunkSource::Ocr, &one_hot(0), "gemma")
        .await
        .unwrap();
    store
        .upsert_text_embedding(b, 0, "beta", ChunkSource::Ocr, &one_hot(1), "gemma")
        .await
        .unwrap();

    // querying near A's vector ranks A before B
    assert_eq!(
        store.nearest_text_frames(&one_hot(0), 5).await.unwrap(),
        vec![a, b]
    );
    // querying near B's vector ranks B before A
    assert_eq!(
        store.nearest_text_frames(&one_hot(1), 5).await.unwrap(),
        vec![b, a]
    );
}

#[tokio::test]
async fn upsert_text_embedding_replaces_vector_in_place() {
    let store = SqliteStore::open_in_memory().unwrap();
    let a = store.insert_frame(frame_at(10)).await.unwrap();
    let b = store.insert_frame(frame_at(20)).await.unwrap();
    store
        .upsert_text_embedding(a, 0, "v1", ChunkSource::Ocr, &one_hot(0), "gemma")
        .await
        .unwrap();
    store
        .upsert_text_embedding(b, 0, "fixed", ChunkSource::Ocr, &one_hot(0), "gemma")
        .await
        .unwrap();

    // replace A's chunk-0 vector; same (frame, chunk) key → no duplicate row
    store
        .upsert_text_embedding(a, 0, "v2", ChunkSource::Ocr, &one_hot(5), "gemma")
        .await
        .unwrap();
    assert_eq!(store.text_embedding_count().await.unwrap(), 2);

    // A now sits at one_hot(5); near one_hot(0) B comes first, A trails
    assert_eq!(
        store.nearest_text_frames(&one_hot(0), 5).await.unwrap(),
        vec![b, a]
    );
    assert_eq!(
        store.nearest_text_frames(&one_hot(5), 5).await.unwrap(),
        vec![a, b]
    );
}

#[tokio::test]
async fn image_embedding_knn_returns_frame() {
    let store = SqliteStore::open_in_memory().unwrap();
    let a = store.insert_frame(frame_at(10)).await.unwrap();
    let b = store.insert_frame(frame_at(20)).await.unwrap();
    store
        .upsert_image_embedding(a, &one_hot(3), "nomic-vision")
        .await
        .unwrap();
    store
        .upsert_image_embedding(b, &one_hot(7), "nomic-vision")
        .await
        .unwrap();

    assert_eq!(
        store.nearest_image_frames(&one_hot(3), 5).await.unwrap(),
        vec![a, b]
    );
    assert_eq!(store.image_embedding_count().await.unwrap(), 2);
}

#[tokio::test]
async fn wrong_dimension_embedding_is_rejected() {
    let store = SqliteStore::open_in_memory().unwrap();
    let f = store.insert_frame(frame_at(10)).await.unwrap();
    let bad = Embedding(vec![0.0_f32; 10]); // not 768
    assert!(store
        .upsert_text_embedding(f, 0, "x", ChunkSource::Ocr, &bad, "gemma")
        .await
        .is_err());
}

#[tokio::test]
async fn delete_frame_cascades_and_purges_vectors() {
    let store = SqliteStore::open_in_memory().unwrap();
    let f = store.insert_frame(frame_at(10)).await.unwrap();
    store
        .insert_ocr(
            f,
            OcrResult {
                text: "doomed".to_string(),
                mean_confidence: 0.9,
                engine: "winrt".to_string(),
            },
        )
        .await
        .unwrap();
    store
        .upsert_text_embedding(f, 0, "doomed", ChunkSource::Ocr, &one_hot(0), "gemma")
        .await
        .unwrap();
    store
        .upsert_image_embedding(f, &one_hot(0), "nomic-vision")
        .await
        .unwrap();

    store.delete_frame(f).await.unwrap();

    assert!(store.get_frame(f).await.unwrap().is_none());
    // the AFTER DELETE triggers must have purged the vec0 shadow rows
    assert_eq!(store.text_embedding_count().await.unwrap(), 0);
    assert_eq!(store.image_embedding_count().await.unwrap(), 0);
    assert!(store
        .nearest_text_frames(&one_hot(0), 5)
        .await
        .unwrap()
        .is_empty());
    assert!(store
        .nearest_image_frames(&one_hot(0), 5)
        .await
        .unwrap()
        .is_empty());
}

/// Seeds a frame with OCR text and (optionally) a text embedding; returns its id.
async fn seed(store: &SqliteStore, at: i64, text: &str, emb: Option<&Embedding>) -> i64 {
    let id = store.insert_frame(frame_at(at)).await.unwrap();
    store
        .insert_ocr(
            id,
            OcrResult {
                text: text.to_string(),
                mean_confidence: 0.9,
                engine: "winrt".to_string(),
            },
        )
        .await
        .unwrap();
    if let Some(e) = emb {
        store
            .upsert_text_embedding(id, 0, text, ChunkSource::Ocr, e, "gemma")
            .await
            .unwrap();
    }
    id
}

fn query(text: &str, limit: u32) -> SearchQuery {
    SearchQuery {
        text: text.to_string(),
        limit,
        time_range: None,
    }
}

#[tokio::test]
async fn hybrid_search_fts_only_without_embedder() {
    let store = SqliteStore::open_in_memory().unwrap();
    let a = seed(&store, 100, "quarterly invoice total", None).await;
    let _b = seed(&store, 200, "vacation photos beach", None).await;

    let hits = store.hybrid_search(&query("invoice", 10)).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].frame_id, a);
    assert!(
        hits[0].snippet.contains("invoice"),
        "snippet should include the matched term: {:?}",
        hits[0].snippet
    );
}

#[tokio::test]
async fn hybrid_search_empty_query_returns_nothing() {
    let store = SqliteStore::open_in_memory().unwrap();
    seed(&store, 100, "anything at all", None).await;
    assert!(store
        .hybrid_search(&query("   ", 10))
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn hybrid_search_fuses_fts_and_vector_arms_via_rrf() {
    // A matches both arms (FTS term + nearest vector) → must rank first.
    let store_a_vec = one_hot(0);
    let store = SqliteStore::open_in_memory().unwrap();
    let a = seed(&store, 100, "alpha apple", Some(&one_hot(0))).await;
    let b = seed(&store, 200, "beta banana", Some(&one_hot(1))).await;
    let c = seed(&store, 300, "alpha cherry", Some(&one_hot(2))).await;

    // query "alpha": FTS matches A and C; the fake embeds "alpha" -> A's vector.
    let mut by_text = HashMap::new();
    by_text.insert("alpha".to_string(), store_a_vec);
    let store = store.with_embedder(Arc::new(FakeEmbedder { by_text }));

    let hits = store.hybrid_search(&query("alpha", 10)).await.unwrap();
    let ids: Vec<i64> = hits.iter().map(|h| h.frame_id).collect();

    assert_eq!(hits[0].frame_id, a, "A is in both arms → top; got {ids:?}");
    assert!(ids.contains(&c), "C matches FTS → present; got {ids:?}");
    assert!(
        ids.contains(&b),
        "B is reachable via the vector arm; got {ids:?}"
    );
    // scores are descending
    assert!(hits.windows(2).all(|w| w[0].score >= w[1].score));
}

#[tokio::test]
async fn hybrid_search_honors_time_range() {
    let store = SqliteStore::open_in_memory().unwrap();
    let _old = seed(&store, 1_000, "status report", None).await;
    let recent = seed(&store, 5_000, "status report", None).await;

    let q = SearchQuery {
        text: "report".to_string(),
        limit: 10,
        time_range: Some(TimeRange {
            start: 4_000,
            end: 6_000,
        }),
    };
    let hits = store.hybrid_search(&q).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].frame_id, recent);
}

#[tokio::test]
async fn hybrid_search_respects_limit() {
    let store = SqliteStore::open_in_memory().unwrap();
    for i in 0..5 {
        seed(&store, 100 + i, "common keyword here", None).await;
    }
    let hits = store.hybrid_search(&query("keyword", 3)).await.unwrap();
    assert_eq!(hits.len(), 3);

    let one = store.hybrid_search(&query("keyword", 0)).await.unwrap();
    assert_eq!(one.len(), 1);
}

#[tokio::test]
async fn hybrid_search_clamps_excessive_limit() {
    let store = SqliteStore::open_in_memory().unwrap();
    for i in 0..150 {
        seed(&store, 10_000 + i, "common keyword here", None).await;
    }

    let hits = store
        .hybrid_search(&query("keyword", u32::MAX))
        .await
        .unwrap();
    assert_eq!(hits.len(), 100);
}

#[tokio::test]
async fn claim_returns_highest_priority_first_and_marks_running() {
    let store = SqliteStore::open_in_memory().unwrap();
    store
        .enqueue_job(job(JobKind::EmbedText, 0, 3, 0))
        .await
        .unwrap();
    store
        .enqueue_job(job(JobKind::EmbedText, 10, 3, 0))
        .await
        .unwrap();

    let claimed = store
        .claim_jobs(&[JobKind::EmbedText], 10, 100)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 2);
    assert_eq!(claimed[0].priority, 10); // higher priority first
    assert_eq!(claimed[1].priority, 0);
    assert!(claimed.iter().all(|j| j.state == JobState::Running));

    // a second claim finds nothing pending
    assert!(store
        .claim_jobs(&[JobKind::EmbedText], 10, 100)
        .await
        .unwrap()
        .is_empty());

    let stats = store.job_stats().await.unwrap();
    assert_eq!(stats.running, 2);
    assert_eq!(stats.pending, 0);
}

#[tokio::test]
async fn claim_honors_not_before_schedule() {
    let store = SqliteStore::open_in_memory().unwrap();
    store
        .enqueue_job(job(JobKind::EmbedText, 0, 3, 200))
        .await
        .unwrap();

    assert!(store
        .claim_jobs(&[JobKind::EmbedText], 10, 100)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        store
            .claim_jobs(&[JobKind::EmbedText], 10, 300)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn claim_filters_by_kind() {
    let store = SqliteStore::open_in_memory().unwrap();
    store
        .enqueue_job(job(JobKind::EmbedText, 0, 3, 0))
        .await
        .unwrap();
    store
        .enqueue_job(job(JobKind::VisionTag, 0, 3, 0))
        .await
        .unwrap();

    let claimed = store
        .claim_jobs(&[JobKind::EmbedText], 10, 100)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].kind, JobKind::EmbedText);
    // no enabled kinds → claims nothing
    assert!(store.claim_jobs(&[], 10, 100).await.unwrap().is_empty());
}

#[tokio::test]
async fn complete_job_moves_to_done() {
    let store = SqliteStore::open_in_memory().unwrap();
    let id = store
        .enqueue_job(job(JobKind::EmbedText, 0, 3, 0))
        .await
        .unwrap();
    store
        .claim_jobs(&[JobKind::EmbedText], 10, 100)
        .await
        .unwrap();

    store.complete_job(id).await.unwrap();
    let stats = store.job_stats().await.unwrap();
    assert_eq!(stats.done, 1);
    assert_eq!(stats.running, 0);
}

#[tokio::test]
async fn complete_job_requires_running_state() {
    let store = SqliteStore::open_in_memory().unwrap();
    let id = store
        .enqueue_job(job(JobKind::EmbedText, 0, 3, 0))
        .await
        .unwrap();

    let pending_err = store.complete_job(id).await.unwrap_err().to_string();
    assert!(
        pending_err.contains("not running"),
        "pending job completion should be rejected, got {pending_err:?}"
    );
    assert_eq!(store.job_stats().await.unwrap().pending, 1);

    store
        .claim_jobs(&[JobKind::EmbedText], 10, 100)
        .await
        .unwrap();
    store.complete_job(id).await.unwrap();
    assert_eq!(store.job_stats().await.unwrap().done, 1);

    let done_err = store.complete_job(id).await.unwrap_err().to_string();
    assert!(
        done_err.contains("not running"),
        "completed job should not be completed again, got {done_err:?}"
    );
}

#[tokio::test]
async fn fail_retries_with_backoff_then_dead_letters_at_max_attempts() {
    let store = SqliteStore::open_in_memory().unwrap();
    let id = store
        .enqueue_job(job(JobKind::EmbedText, 0, 2, 0))
        .await
        .unwrap();

    // attempt 1 fails with a retry scheduled at t=500
    store
        .claim_jobs(&[JobKind::EmbedText], 10, 100)
        .await
        .unwrap();
    store.fail_job(id, "boom", Some(500)).await.unwrap();
    assert_eq!(store.job_stats().await.unwrap().pending, 1);
    // not yet runnable (backoff), runnable at/after 500
    assert!(store
        .claim_jobs(&[JobKind::EmbedText], 10, 100)
        .await
        .unwrap()
        .is_empty());
    let again = store
        .claim_jobs(&[JobKind::EmbedText], 10, 500)
        .await
        .unwrap();
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].attempts, 1);
    assert_eq!(again[0].last_error.as_deref(), Some("boom"));

    // attempt 2 fails → max_attempts reached → dead-letter (never silently dropped)
    store.fail_job(id, "boom again", Some(900)).await.unwrap();
    let stats = store.job_stats().await.unwrap();
    assert_eq!(stats.dead, 1);
    assert_eq!(stats.pending, 0);
    assert!(store
        .claim_jobs(&[JobKind::EmbedText], 10, 100_000)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn fail_without_retry_at_dead_letters_immediately() {
    let store = SqliteStore::open_in_memory().unwrap();
    let id = store
        .enqueue_job(job(JobKind::EmbedText, 0, 5, 0))
        .await
        .unwrap();
    store
        .claim_jobs(&[JobKind::EmbedText], 10, 100)
        .await
        .unwrap();

    // retry_at = None → terminal even though attempts remain
    store.fail_job(id, "fatal", None).await.unwrap();
    assert_eq!(store.job_stats().await.unwrap().dead, 1);
}

#[tokio::test]
async fn fail_job_requires_running_state() {
    let store = SqliteStore::open_in_memory().unwrap();
    let id = store
        .enqueue_job(job(JobKind::EmbedText, 0, 2, 0))
        .await
        .unwrap();

    let pending_err = store
        .fail_job(id, "not claimed", Some(500))
        .await
        .unwrap_err()
        .to_string();
    assert!(
        pending_err.contains("not running"),
        "pending job failure should be rejected, got {pending_err:?}"
    );
    let stats = store.job_stats().await.unwrap();
    assert_eq!(stats.pending, 1);
    assert_eq!(stats.dead, 0);

    store
        .claim_jobs(&[JobKind::EmbedText], 10, 100)
        .await
        .unwrap();
    store.fail_job(id, "retryable", Some(500)).await.unwrap();
    assert_eq!(store.job_stats().await.unwrap().pending, 1);

    let retry_pending_err = store
        .fail_job(id, "still not claimed", Some(700))
        .await
        .unwrap_err()
        .to_string();
    assert!(
        retry_pending_err.contains("not running"),
        "retry-pending job failure should be rejected, got {retry_pending_err:?}"
    );

    store
        .claim_jobs(&[JobKind::EmbedText], 10, 500)
        .await
        .unwrap();
    store.fail_job(id, "fatal", None).await.unwrap();
    assert_eq!(store.job_stats().await.unwrap().dead, 1);

    let dead_err = store
        .fail_job(id, "already dead", None)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        dead_err.contains("not running"),
        "dead job failure should be rejected, got {dead_err:?}"
    );
}

#[tokio::test]
async fn completing_or_failing_an_unknown_job_is_an_error() {
    // a write that changes no rows is a programming error, not a silent no-op
    let store = SqliteStore::open_in_memory().unwrap();
    assert!(store.complete_job(999).await.is_err());
    assert!(store.fail_job(999, "boom", Some(10)).await.is_err());
}

// Proves the *production* concurrency model: many async callers share one
// `Mutex<Connection>`, so claims serialize through it and the atomic
// `UPDATE … RETURNING` hands each job to exactly one caller — none lost, none
// double-claimed. It does NOT exercise multi-connection WAL contention (the store
// is single-connection by design; see `05` "Still risky" and `07`).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_claims_never_double_claim() {
    let store = Arc::new(SqliteStore::open_in_memory().unwrap());
    const N: usize = 40;
    for _ in 0..N {
        store
            .enqueue_job(job(JobKind::EmbedText, 0, 3, 0))
            .await
            .unwrap();
    }

    let mut handles = Vec::new();
    for _ in 0..4 {
        let s = store.clone();
        handles.push(tokio::spawn(async move {
            let mut got = Vec::new();
            loop {
                let batch = s.claim_jobs(&[JobKind::EmbedText], 3, 1_000).await.unwrap();
                if batch.is_empty() {
                    break;
                }
                got.extend(batch.into_iter().map(|j| j.id));
            }
            got
        }));
    }

    let mut all = Vec::new();
    for h in handles {
        all.extend(h.await.unwrap());
    }
    let unique: HashSet<i64> = all.iter().copied().collect();
    assert_eq!(all.len(), N, "every job claimed exactly once (no loss)");
    assert_eq!(unique.len(), N, "no job claimed twice (no double-claim)");
}

#[tokio::test]
async fn works_through_the_store_trait_object() {
    use traits::Store;

    // the composition root will hold the store as `Arc<dyn Store>`; prove the
    // trait impl forwards to the inherent methods across the boundary.
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open_in_memory().unwrap());
    let id = store.insert_frame(frame_at(42)).await.unwrap();
    store
        .insert_ocr(
            id,
            OcrResult {
                text: "trait object path".to_string(),
                mean_confidence: 0.9,
                engine: "winrt".to_string(),
            },
        )
        .await
        .unwrap();

    let hits = store.hybrid_search(&query("trait", 10)).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].frame_id, id);

    let jid = store
        .enqueue_job(job(JobKind::EmbedText, 0, 3, 0))
        .await
        .unwrap();
    assert!(jid > 0);
    assert_eq!(store.job_stats().await.unwrap().pending, 1);

    store.set_setting("k", "v").await.unwrap();
    assert_eq!(store.get_setting("k").await.unwrap(), Some("v".to_string()));
}

// --- enrichment-input read + stale-job recovery (P3, `03 §5/§6`) ---------------

/// The embedding worker's lightweight read returns the JPEG path and, once OCR has
/// run, the text — and `None` for a frame that no longer exists.
#[tokio::test]
async fn frame_enrichment_input_reads_path_and_optional_text() {
    let store = SqliteStore::open_in_memory().unwrap();
    let fid = store.insert_frame(frame_at(1_000)).await.unwrap();

    // before OCR: image path present, text absent
    let pre = store
        .frame_enrichment_input(fid)
        .await
        .unwrap()
        .expect("frame exists");
    assert_eq!(pre.image_path, "frames/1000.jpg");
    assert_eq!(pre.ocr_text, None);

    // after OCR: text present
    store
        .insert_ocr(
            fid,
            OcrResult {
                text: "hello world".to_string(),
                mean_confidence: -1.0,
                engine: "test".to_string(),
            },
        )
        .await
        .unwrap();
    let post = store.frame_enrichment_input(fid).await.unwrap().unwrap();
    assert_eq!(post.ocr_text.as_deref(), Some("hello world"));

    // missing frame → None
    assert!(store.frame_enrichment_input(9_999).await.unwrap().is_none());
}

/// The startup sweep (`older_than == 0`) requeues a `running` job a dead worker
/// left behind, *without* consuming an attempt — it is reclaimable again (`03 §6`).
#[tokio::test]
async fn reset_stale_running_jobs_requeues_running() {
    let store = SqliteStore::open_in_memory().unwrap();
    let id = store
        .enqueue_job(job(JobKind::EmbedText, 0, 3, 0))
        .await
        .unwrap();
    // claim → running, stamped at a past time
    let claimed = store
        .claim_jobs(&[JobKind::EmbedText], 10, 1_000)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);

    assert_eq!(store.reset_stale_running_jobs(0).await.unwrap(), 1);

    let again = store
        .claim_jobs(&[JobKind::EmbedText], 10, 2_000)
        .await
        .unwrap();
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].id, id);
    assert_eq!(
        again[0].attempts, 0,
        "a crash sweep must not consume an attempt"
    );
}

/// A job claimed `now` is within the visibility window — a 5-minute periodic sweep
/// must not requeue it out from under a live worker.
#[tokio::test]
async fn reset_stale_running_jobs_spares_fresh_running() {
    let store = SqliteStore::open_in_memory().unwrap();
    store
        .enqueue_job(job(JobKind::EmbedText, 0, 3, 0))
        .await
        .unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    store
        .claim_jobs(&[JobKind::EmbedText], 10, now)
        .await
        .unwrap();

    assert_eq!(store.reset_stale_running_jobs(5 * 60_000).await.unwrap(), 0);
}

/// `untagged_frame_ids` returns frames with no vision row, oldest first, honoring the
/// limit and the optional time window (the timer/idle batch + `enqueue_vision` range
/// source, `03 §5`).
#[tokio::test]
async fn untagged_frame_ids_excludes_tagged_and_honors_range() {
    let store = SqliteStore::open_in_memory().unwrap();
    let f1 = store.insert_frame(frame_at(100)).await.unwrap();
    let f2 = store.insert_frame(frame_at(200)).await.unwrap();
    let f3 = store.insert_frame(frame_at(300)).await.unwrap();

    // Tag the middle frame; it must drop out of the untagged set.
    store
        .insert_vision(
            f2,
            VisionAnalysis {
                description: "tagged".to_string(),
                activity_type: None,
                app_hint: None,
                confidence: 0.5,
                model: "m".to_string(),
            },
        )
        .await
        .unwrap();

    // All untagged, oldest first.
    assert_eq!(
        store.untagged_frame_ids(10, None).await.unwrap(),
        vec![f1, f3]
    );
    // Limit caps the result.
    assert_eq!(store.untagged_frame_ids(1, None).await.unwrap(), vec![f1]);
    // Range [150, 350) excludes f1 (too early) and f2 (tagged) → only f3.
    assert_eq!(
        store
            .untagged_frame_ids(10, Some((150, 350)))
            .await
            .unwrap(),
        vec![f3]
    );
}

/// `untagged_frame_ids` also excludes frames with an in-flight (`pending`/`running`)
/// `vision_tag` job, so a slow batch isn't re-enqueued on the next tick and the
/// timer/idle lanes don't double-queue the same frames. A finished (`done`) vision_tag
/// job, or a pending job of another kind, leaves a still-untagged frame eligible.
#[tokio::test]
async fn untagged_frame_ids_excludes_in_flight_vision_jobs() {
    let store = SqliteStore::open_in_memory().unwrap();
    let f1 = store.insert_frame(frame_at(100)).await.unwrap();
    let f2 = store.insert_frame(frame_at(200)).await.unwrap();
    let f3 = store.insert_frame(frame_at(300)).await.unwrap();
    let f4 = store.insert_frame(frame_at(400)).await.unwrap();
    let f5 = store.insert_frame(frame_at(500)).await.unwrap();

    let frame_job = |kind, frame_id| NewJob {
        kind,
        frame_id: Some(frame_id),
        priority: 0,
        max_attempts: 3,
        not_before: 0,
    };

    // f3: a vision_tag job that ran to completion — it still has no vision_analysis
    // row, so the frame stays eligible. (Claim then complete; it's the only pending
    // vision_tag job at this point.)
    store
        .enqueue_job(frame_job(JobKind::VisionTag, f3))
        .await
        .unwrap();
    let done = store
        .claim_jobs(&[JobKind::VisionTag], 10, 100)
        .await
        .unwrap();
    assert_eq!(done.len(), 1);
    store.complete_job(done[0].id).await.unwrap();

    // f2: a vision_tag job left running (claimed, not completed) — excluded.
    store
        .enqueue_job(frame_job(JobKind::VisionTag, f2))
        .await
        .unwrap();
    let running = store
        .claim_jobs(&[JobKind::VisionTag], 10, 100)
        .await
        .unwrap();
    assert_eq!(running.len(), 1);
    assert_eq!(running[0].frame_id, Some(f2));

    // f1: a vision_tag job left pending — excluded.
    store
        .enqueue_job(frame_job(JobKind::VisionTag, f1))
        .await
        .unwrap();

    // f4: a pending job of a *different* kind — does not exclude the frame.
    store
        .enqueue_job(frame_job(JobKind::EmbedText, f4))
        .await
        .unwrap();

    // f5: no job at all.

    // Eligible, oldest first: f3 (done), f4 (other kind), f5 (no job).
    assert_eq!(
        store.untagged_frame_ids(10, None).await.unwrap(),
        vec![f3, f4, f5]
    );
}

/// `ocr_texts` bulk-fetches non-empty OCR text for many frames in one query (the `ask`
/// grounding hydrate). Frames without text — or with empty text — are omitted.
#[tokio::test]
async fn ocr_texts_bulk_fetches_nonempty_only() {
    let store = SqliteStore::open_in_memory().unwrap();
    let f1 = store.insert_frame(frame_at(100)).await.unwrap();
    let f2 = store.insert_frame(frame_at(200)).await.unwrap();
    let f3 = store.insert_frame(frame_at(300)).await.unwrap();
    let ocr = |text: &str| OcrResult {
        text: text.to_string(),
        mean_confidence: -1.0,
        engine: "winrt".to_string(),
    };
    store.insert_ocr(f1, ocr("login screen")).await.unwrap();
    store.insert_ocr(f2, ocr("")).await.unwrap(); // empty → omitted
                                                  // f3 has no OCR row at all → omitted.

    let map = store.ocr_texts(&[f1, f2, f3]).await.unwrap();
    assert_eq!(map.len(), 1);
    assert_eq!(map.get(&f1).map(String::as_str), Some("login screen"));
    assert!(!map.contains_key(&f2));
    assert!(!map.contains_key(&f3));

    // Empty input is a no-op (no query).
    assert!(store.ocr_texts(&[]).await.unwrap().is_empty());
}

/// `timeline_buckets` groups frames into fixed-width, half-open buckets and returns
/// only the **occupied** ones (sparse), ascending by time. Backs `get_timeline`.
#[tokio::test]
async fn timeline_buckets_are_sparse_and_half_open() {
    let store = SqliteStore::open_in_memory().unwrap();
    // bucket width = ceil(200 / 4) = 50 → [0,50) [50,100) [100,150) [150,200).
    for ts in [10, 20, 120] {
        store.insert_frame(frame_at(ts)).await.unwrap();
    }
    // Out of range: a frame before `start` and one exactly at the exclusive `end`
    // are both excluded.
    store.insert_frame(frame_at(-5)).await.unwrap();
    store.insert_frame(frame_at(200)).await.unwrap();

    let buckets = store.timeline_buckets(0, 200, 4).await.unwrap();
    assert_eq!(
        buckets,
        vec![
            TimelineBucket {
                start: 0,
                end: 50,
                count: 2
            },
            TimelineBucket {
                start: 100,
                end: 150,
                count: 1
            },
        ],
    );

    // Invalid / degenerate ranges yield nothing.
    assert!(store.timeline_buckets(200, 0, 4).await.unwrap().is_empty());
    assert!(store.timeline_buckets(0, 200, 0).await.unwrap().is_empty());
}

/// `insights_summary` returns real aggregates over the half-open window: total and
/// vision-tagged counts, the top foreground apps, and the activity-type breakdown.
#[tokio::test]
async fn insights_summary_aggregates_truthfully() {
    let store = SqliteStore::open_in_memory().unwrap();
    let firefox = |ts: i64| {
        let mut f = frame_at(ts);
        f.app_hint = Some("Firefox".to_string());
        f
    };
    // 3 Firefox, 1 Code, 1 with no app hint — all inside [0, 1000).
    let f1 = store.insert_frame(firefox(100)).await.unwrap();
    store.insert_frame(firefox(200)).await.unwrap();
    store.insert_frame(firefox(300)).await.unwrap();
    let mut code = frame_at(400);
    code.app_hint = Some("Code".to_string());
    let f_code = store.insert_frame(code).await.unwrap();
    let mut anon = frame_at(500);
    anon.app_hint = None;
    store.insert_frame(anon).await.unwrap();

    // Tag two frames with vision activity types (writes frames.activity_type).
    let vision = |activity: &str| VisionAnalysis {
        description: "desc".to_string(),
        activity_type: Some(activity.to_string()),
        app_hint: None,
        confidence: 0.9,
        model: "test".to_string(),
    };
    store.insert_vision(f1, vision("browsing")).await.unwrap();
    store.insert_vision(f_code, vision("coding")).await.unwrap();

    let s = store.insights_summary(0, 1000, 48).await.unwrap();
    assert_eq!(s.total_frames, 5);
    assert_eq!(s.tagged_frames, 2);
    // Most-captured app is Firefox (3), ordered by count desc.
    let top = s.top_apps.first().expect("a top app");
    assert_eq!(top.app.as_deref(), Some("Firefox"));
    assert_eq!(top.count, 3);
    // Both tagged activities appear, each once.
    let activities: Vec<(Option<String>, u32)> = s
        .activity_breakdown
        .iter()
        .map(|a| (a.activity.clone(), a.count))
        .collect();
    assert!(activities.contains(&(Some("browsing".to_string()), 1)));
    assert!(activities.contains(&(Some("coding".to_string()), 1)));
    assert!(!s.captures.is_empty(), "capture density buckets present");

    // A window with no frames → honest-empty summary, never fabricated.
    let empty = store.insights_summary(10_000, 20_000, 48).await.unwrap();
    assert_eq!(empty.total_frames, 0);
    assert!(empty.top_apps.is_empty());
    assert!(empty.activity_breakdown.is_empty());
}

/// `timeline_buckets` must not panic on hostile/extreme timestamps (the values come
/// straight from the frontend). An unrepresentable span yields an empty result, and
/// a huge-but-representable span — which the old `(span + n - 1)` ceil would have
/// overflowed — is handled cleanly.
#[tokio::test]
async fn timeline_buckets_survives_extreme_ranges() {
    let store = SqliteStore::open_in_memory().unwrap();
    store.insert_frame(frame_at(0)).await.unwrap();

    // `end - start` overflows i64 → empty window, no panic.
    assert!(store
        .timeline_buckets(i64::MIN, i64::MAX, 4)
        .await
        .unwrap()
        .is_empty());

    // Span is exactly i64::MAX (representable). The previous `(span + n - 1) / n`
    // ceil would overflow here; the current form does not. The single frame at 0
    // lands in bucket 0.
    let buckets = store.timeline_buckets(0, i64::MAX, 4).await.unwrap();
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].start, 0);
    assert_eq!(buckets[0].count, 1);
    // A forward bucket: a `checked_add` overflow would wrap the end negative.
    assert!(buckets[0].end > buckets[0].start);
}

/// `frames_in_range` lists frames in the half-open window, most-recent-first, capped
/// at `limit` — the Timeline thumbnails / Deck recents source. Backs `get_frames`.
#[tokio::test]
async fn frames_in_range_lists_window_recent_first() {
    let store = SqliteStore::open_in_memory().unwrap();
    let f1 = store.insert_frame(frame_at(100)).await.unwrap();
    let f2 = store.insert_frame(frame_at(200)).await.unwrap();
    let f3 = store.insert_frame(frame_at(300)).await.unwrap();
    // Out of [150, 350): one before `start`, one exactly at the exclusive `end`.
    store.insert_frame(frame_at(100)).await.unwrap(); // same ts as f1, still < start
    store.insert_frame(frame_at(350)).await.unwrap();

    // Window [150, 350): f2 and f3 only, newest first.
    let metas = store.frames_in_range(150, 350, 10).await.unwrap();
    let ids: Vec<i64> = metas.iter().map(|m| m.frame_id).collect();
    assert_eq!(ids, vec![f3, f2]);
    // FrameMeta carries the lightweight browsing fields.
    assert_eq!(metas[0].captured_at, 300);
    assert_eq!(metas[0].image_path, "frames/300.jpg");
    assert_eq!(metas[0].app_hint.as_deref(), Some("Firefox"));

    // `limit` caps the result; newest-first means the most recent (@350) is kept.
    let capped = store.frames_in_range(0, 1_000, 1).await.unwrap();
    assert_eq!(capped.len(), 1);
    assert_eq!(capped[0].captured_at, 350);

    // Degenerate windows / zero limit yield nothing — never the whole table.
    assert!(store
        .frames_in_range(350, 150, 10)
        .await
        .unwrap()
        .is_empty());
    assert!(store.frames_in_range(0, 1_000, 0).await.unwrap().is_empty());
    // f1 is reachable in a window that includes it.
    assert!(store
        .frames_in_range(0, 150, 10)
        .await
        .unwrap()
        .iter()
        .any(|m| m.frame_id == f1));
}

/// `nearest_frame` resolves a timestamp to the closest frame on either side, with
/// the at-or-after frame winning an exact tie. Backs `get_nearest_frame` (the
/// Timeline scan-head → concrete frame id). `None` only when the DB is empty.
#[tokio::test]
async fn nearest_frame_picks_closest_with_after_winning_ties() {
    let store = SqliteStore::open_in_memory().unwrap();
    // Empty DB → no nearest frame.
    assert!(store.nearest_frame(1_000).await.unwrap().is_none());

    let f100 = store.insert_frame(frame_at(100)).await.unwrap();
    let f200 = store.insert_frame(frame_at(200)).await.unwrap();
    let f400 = store.insert_frame(frame_at(400)).await.unwrap();

    let id_at = |m: Option<FrameMeta>| m.expect("a frame").frame_id;
    // Before the first frame → the first frame.
    assert_eq!(id_at(store.nearest_frame(0).await.unwrap()), f100);
    // After the last frame → the last frame.
    assert_eq!(id_at(store.nearest_frame(10_000).await.unwrap()), f400);
    // Closer to f200 than f100.
    assert_eq!(id_at(store.nearest_frame(170).await.unwrap()), f200);
    // Closer to f200 than f400.
    assert_eq!(id_at(store.nearest_frame(260).await.unwrap()), f200);
    // Exact midpoint between f200 and f400 (300) → the later frame wins the tie.
    assert_eq!(id_at(store.nearest_frame(300).await.unwrap()), f400);
    // Exactly on a frame → that frame.
    assert_eq!(id_at(store.nearest_frame(200).await.unwrap()), f200);
}

/// Range-scoped nearest-frame lookup must never return a frame outside the visible
/// Timeline window. Regression: the P5 command looked across the whole DB, so opening
/// a sparse Today/7d/30d window could jump to a capture outside the selected range.
#[tokio::test]
async fn nearest_frame_in_range_ignores_frames_outside_window() {
    let store = SqliteStore::open_in_memory().unwrap();
    let outside_before = store.insert_frame(frame_at(100)).await.unwrap();
    let inside = store.insert_frame(frame_at(500)).await.unwrap();
    let outside_after = store.insert_frame(frame_at(900)).await.unwrap();

    let nearest = store
        .nearest_frame_in_range(150, 400, 800)
        .await
        .unwrap()
        .expect("frame inside range");
    assert_eq!(nearest.frame_id, inside);

    let nearest_after = store
        .nearest_frame_in_range(850, 400, 800)
        .await
        .unwrap()
        .expect("frame inside range");
    assert_eq!(nearest_after.frame_id, inside);

    assert!(store
        .nearest_frame_in_range(150, 200, 400)
        .await
        .unwrap()
        .is_none());

    let global = store.nearest_frame(150).await.unwrap().unwrap();
    assert_eq!(
        global.frame_id, outside_before,
        "global nearest remains available for non-windowed callers"
    );
    assert_ne!(global.frame_id, outside_after);
}

/// `neighbour_frames` brackets the anchor with the *closest* captures on each side —
/// not the window edges — so a Moment's prev/next + context strip always point at the
/// adjacent frames. Backs `get_frame_context`. Regression: `frames_in_range`'s
/// newest-first cap would return only the latest frames in a wide forward window and
/// silently drop the anchor's true neighbours (and the anchor itself).
#[tokio::test]
async fn neighbour_frames_brackets_anchor_with_closest_each_side() {
    let store = SqliteStore::open_in_memory().unwrap();
    for t in [100, 200, 300, 400, 500, 600, 700] {
        store.insert_frame(frame_at(t)).await.unwrap();
    }

    // Anchor @400, ±1000 covers every frame, two per side. Closest two before (@300,
    // @200) + closest two after (@500, @600), ascending; anchor (@400) excluded and the
    // far edges (@100/@700) dropped by the 2-each cap — NOT the window's newest frames.
    let metas = store.neighbour_frames(400, 1_000, 2).await.unwrap();
    let times: Vec<i64> = metas.iter().map(|m| m.captured_at).collect();
    assert_eq!(times, vec![200, 300, 500, 600]);

    // prev/next as the Moment screen derives them: closest below / closest above.
    let prev = metas.iter().rfind(|m| m.captured_at < 400).unwrap();
    let next = metas.iter().find(|m| m.captured_at > 400).unwrap();
    assert_eq!(prev.captured_at, 300);
    assert_eq!(next.captured_at, 500);

    // Anchor at the earliest frame → only the after-side; no before frames, no panic.
    let only_after = store.neighbour_frames(100, 1_000, 3).await.unwrap();
    assert!(only_after.iter().all(|m| m.captured_at > 100));
    assert_eq!(only_after.first().map(|m| m.captured_at), Some(200));

    // The half-window clips reach: ±100 keeps only the immediately adjacent @300/@500.
    let near = store.neighbour_frames(400, 100, 5).await.unwrap();
    let near_times: Vec<i64> = near.iter().map(|m| m.captured_at).collect();
    assert_eq!(near_times, vec![300, 500]);

    // Degenerate window / zero limit → empty (never the whole table).
    assert!(store.neighbour_frames(400, 0, 5).await.unwrap().is_empty());
    assert!(store
        .neighbour_frames(400, 1_000, 0)
        .await
        .unwrap()
        .is_empty());
}

/// `set_settings_batch` upserts every pair in one transaction: all keys are present
/// afterward and existing keys are overwritten. (Atomicity-on-failure comes from the
/// single `BEGIN … COMMIT` — a failed write `?`-returns before `commit`, so nothing
/// lands; that path is exercised by `save_settings`' build-then-commit ordering.)
#[tokio::test]
async fn set_settings_batch_writes_all_and_overwrites() {
    let store = SqliteStore::open_in_memory().unwrap();
    // Pre-existing value the batch must overwrite.
    store.set_setting("a", "old").await.unwrap();

    store
        .set_settings_batch(&[
            ("a".to_string(), "new".to_string()),
            ("b".to_string(), "1".to_string()),
            ("c".to_string(), "2".to_string()),
        ])
        .await
        .unwrap();

    assert_eq!(
        store.get_setting("a").await.unwrap().as_deref(),
        Some("new")
    );
    assert_eq!(store.get_setting("b").await.unwrap().as_deref(), Some("1"));
    assert_eq!(store.get_setting("c").await.unwrap().as_deref(), Some("2"));

    // An empty batch is a no-op (opens and commits an empty transaction).
    store.set_settings_batch(&[]).await.unwrap();
}

/// Retention needs a bounded list of purge candidates before deleting files. It
/// should return only frames older than the cutoff, oldest first, capped at `limit`.
#[tokio::test]
async fn frames_older_than_lists_bounded_retention_candidates() {
    let store = SqliteStore::open_in_memory().unwrap();
    let f100 = store.insert_frame(frame_at(100)).await.unwrap();
    let f200 = store.insert_frame(frame_at(200)).await.unwrap();
    store.insert_frame(frame_at(300)).await.unwrap();

    let candidates = store.frames_older_than(250, 1).await.unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].frame_id, f100);
    assert_eq!(candidates[0].image_path, "frames/100.jpg");

    let candidates = store.frames_older_than(250, 10).await.unwrap();
    let ids: Vec<i64> = candidates.iter().map(|m| m.frame_id).collect();
    assert_eq!(ids, vec![f100, f200]);

    assert!(store.frames_older_than(250, 0).await.unwrap().is_empty());
}

/// Insights bucket density is presentation-driven just like Timeline density; callers
/// can request a coarser/finer chart without a fixed 48-bucket backend ceiling.
#[tokio::test]
async fn insights_summary_uses_requested_bucket_count() {
    let store = SqliteStore::open_in_memory().unwrap();
    for ts in [10, 60, 120, 180] {
        store.insert_frame(frame_at(ts)).await.unwrap();
    }

    let two = store.insights_summary(0, 200, 2).await.unwrap();
    assert_eq!(
        two.captures,
        vec![
            TimelineBucket {
                start: 0,
                end: 100,
                count: 2
            },
            TimelineBucket {
                start: 100,
                end: 200,
                count: 2
            },
        ]
    );

    let four = store.insights_summary(0, 200, 4).await.unwrap();
    assert_eq!(four.captures.len(), 4);
}

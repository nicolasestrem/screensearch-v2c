//! Integration tests for the local API (0.3.0 PR7; `03 §7c`, DoD `§13b` item 6).
//!
//! A `TestHost` implements [`api::ApiHost`] over a real in-memory `SqliteStore` (plus a
//! scripted answer provider), so every endpoint is exercised against a populated DB. The
//! server binds an ephemeral loopback port; requests go out over `reqwest` with a bearer
//! token, exactly as a real client would.

use std::sync::Arc;

use api::{ApiHost, ApiServer, FrameImage, SharedToken};
use async_trait::async_trait;
use store::SqliteStore;
use tokio::sync::mpsc::Sender;
use traits::{
    AnswerDelta, AnswerOpts, AnswerProvider, ExportFrameRow, FrameDetail, NewFrame, OcrResult,
    ResumeContext, RetrievedChunk, Settings, Store,
};

const TEST_TOKEN: &str = "test-token-000000000000000000000000000000000000000000000000000000";

/// A scripted answer provider: streams a fixed set of deltas, then `Done`.
struct ScriptedAnswer;

#[async_trait]
impl AnswerProvider for ScriptedAnswer {
    async fn answer(
        &self,
        _query: &str,
        context: &[RetrievedChunk],
        _opts: AnswerOpts,
        tx: Sender<AnswerDelta>,
    ) -> anyhow::Result<()> {
        let _ = tx
            .send(AnswerDelta::Token {
                text: "grounded answer".to_string(),
            })
            .await;
        // Cite the first grounded frame so the SSE carries a Citation too.
        if let Some(chunk) = context.first() {
            let _ = tx
                .send(AnswerDelta::Citation {
                    frame_id: chunk.frame_id,
                })
                .await;
        }
        let _ = tx.send(AnswerDelta::Done).await;
        Ok(())
    }
}

/// The test seam over a real store. `answer` toggles whether an answer provider is
/// available (for the 503 path).
struct TestHost {
    store: Arc<SqliteStore>,
    answer: Option<Arc<dyn AnswerProvider>>,
    started_at_ms: i64,
}

#[async_trait]
impl ApiHost for TestHost {
    fn version(&self) -> String {
        "test-0.3.0".to_string()
    }
    fn started_at_ms(&self) -> i64 {
        self.started_at_ms
    }
    async fn is_capturing(&self) -> bool {
        false
    }
    fn store(&self) -> Arc<dyn Store> {
        self.store.clone()
    }
    async fn settings(&self) -> Settings {
        Settings::default()
    }
    async fn frame_detail(&self, frame_id: i64) -> anyhow::Result<Option<FrameDetail>> {
        self.store.get_frame(frame_id).await
    }
    async fn frame_image(&self, frame_id: i64) -> anyhow::Result<Option<FrameImage>> {
        // The fixture doesn't write real image files; a frame with image_purged=false and
        // a known path returns synthetic bytes, a purged one returns None.
        match self.store.get_frame(frame_id).await? {
            Some(detail) if !detail.image_purged => Ok(Some(FrameImage {
                bytes: b"webp-bytes".to_vec(),
                content_type: "image/webp",
            })),
            _ => Ok(None),
        }
    }
    async fn ask_context(&self, query: &str, top_k: u32) -> anyhow::Result<Vec<RetrievedChunk>> {
        let hits = self
            .store
            .hybrid_search(&traits::SearchQuery {
                text: query.to_string(),
                limit: top_k,
                time_range: None,
                include_chrome: false,
            })
            .await?;
        Ok(hits
            .into_iter()
            .map(|h| RetrievedChunk {
                frame_id: h.frame_id,
                text: h.snippet,
                score: h.score,
                captured_at: h.captured_at,
            })
            .collect())
    }
    async fn answer_provider(&self) -> Option<Arc<dyn AnswerProvider>> {
        self.answer.clone()
    }
    async fn add_mark(
        &self,
        frame_id: Option<i64>,
        capture_now: bool,
        note: Option<String>,
    ) -> anyhow::Result<i64> {
        match (frame_id, capture_now) {
            (Some(id), false) => self.store.insert_mark(id, 1_000, note).await,
            // The test host has no capture worker, so `now` is honestly unavailable (503).
            (None, true) => anyhow::bail!("capture is off — mark not saved"),
            _ => anyhow::bail!("exactly one of frame_id or capture_now"),
        }
    }
    async fn resolve_mark(&self, mark_id: i64) -> anyhow::Result<()> {
        self.store.resolve_mark(mark_id, 2_000).await
    }
    async fn where_was_i(&self) -> anyhow::Result<Option<ResumeContext>> {
        Ok(None)
    }
    async fn export_frames_page(
        &self,
        after_id: i64,
        from: Option<i64>,
        to: Option<i64>,
        limit: u32,
    ) -> anyhow::Result<Vec<ExportFrameRow>> {
        self.store
            .export_frames_page(after_id, from, to, limit)
            .await
    }
}

/// Live manual-verification harness (ignored by default). Serves the real API over an
/// on-disk store (from `SCREENSEARCH_LIVE_DB`, else a fixture) on port 43210 with a known
/// token, then stays up so an external `curl` can exercise every endpoint — a genuine
/// out-of-process live check without disturbing the running desktop app.
/// Run: `cargo test -p api --test http_api -- --ignored --nocapture live_server_for_curl`
#[tokio::test]
#[ignore]
async fn live_server_for_curl() {
    let token: SharedToken = Arc::new(std::sync::RwLock::new(TEST_TOKEN.to_string()));
    let host: Arc<TestHost> = match std::env::var("SCREENSEARCH_LIVE_DB") {
        Ok(path) => {
            let store = Arc::new(SqliteStore::open_path(std::path::Path::new(&path)).unwrap());
            Arc::new(TestHost {
                store,
                answer: None,
                started_at_ms: 0,
            })
        }
        Err(_) => fixture_host(false).await,
    };
    let server = ApiServer::start(host, token, 43210)
        .await
        .expect("bind 43210");
    println!(
        "LIVE_API_READY addr={} token={TEST_TOKEN}",
        server.local_addr()
    );
    tokio::time::sleep(std::time::Duration::from_secs(90)).await;
    server.stop().await;
}

/// A frame with content text at the given capture time; returns its id.
async fn seed_frame(store: &SqliteStore, captured_at: i64, text: &str) -> i64 {
    let id = store
        .insert_frame(NewFrame {
            captured_at,
            monitor_index: 0,
            width: 1920,
            height: 1080,
            image_path: format!("frames/{captured_at}.webp"),
            content_hash: format!("hash-{captured_at}"),
            app_hint: Some("VS Code".to_string()),
            window_title: Some("main.rs".to_string()),
            browser_url: None,
            capture_trigger: None,
        })
        .await
        .expect("insert frame");
    store
        .insert_ocr(
            id,
            OcrResult {
                text: text.to_string(),
                mean_confidence: 0.9,
                engine: "winrt".to_string(),
                spans: vec![],
            },
        )
        .await
        .expect("insert ocr");
    id
}

async fn fixture_host(with_answer: bool) -> Arc<TestHost> {
    let store = Arc::new(SqliteStore::open_in_memory().expect("open store"));
    seed_frame(&store, 1_000, "quarterly invoice total").await;
    seed_frame(&store, 2_000, "vacation beach photos").await;
    Arc::new(TestHost {
        store,
        answer: with_answer.then(|| Arc::new(ScriptedAnswer) as Arc<dyn AnswerProvider>),
        started_at_ms: 500,
    })
}

/// Starts a server over the fixture host on an ephemeral loopback port; returns the base
/// URL and the running server (kept alive by the caller).
async fn start_server(host: Arc<TestHost>) -> (String, ApiServer, SharedToken) {
    let token: SharedToken = Arc::new(std::sync::RwLock::new(TEST_TOKEN.to_string()));
    let server = ApiServer::start(host, token.clone(), 0)
        .await
        .expect("bind ephemeral loopback");
    let base = format!("http://{}", server.local_addr());
    (base, server, token)
}

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

fn bearer(token: &str) -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    h.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    h
}

// ---- D11: bind is loopback-only, by construction ----

#[tokio::test]
async fn binds_loopback_only() {
    let (_base, server, _t) = start_server(fixture_host(false).await).await;
    assert!(
        server.local_addr().ip().is_loopback(),
        "the server binds 127.0.0.1 only (D11)"
    );
    // And specifically IPv4 loopback (BIND_IP = 127.0.0.1).
    assert_eq!(server.local_addr().ip().to_string(), "127.0.0.1");
    server.stop().await;
}

// ---- auth: 401 without / with a wrong token; health reports state ----

#[tokio::test]
async fn health_requires_token_and_reports_state() {
    let (base, server, _t) = start_server(fixture_host(false).await).await;

    // No token → 401.
    let r = client()
        .get(format!("{base}/v1/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["error"], "unauthorized");

    // Wrong token → 401.
    let r = client()
        .get(format!("{base}/v1/health"))
        .headers(bearer("nope"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);

    // Right token → 200 + state.
    let r = client()
        .get(format!("{base}/v1/health"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["version"], "test-0.3.0");
    assert_eq!(body["capturing"], false);
    assert!(body["uptime_secs"].is_number());

    server.stop().await;
}

/// Regenerating the token (swapping the shared value) takes effect immediately — no
/// restart. Proves the middleware reads the live token per request.
#[tokio::test]
async fn token_swap_takes_effect_without_restart() {
    let (base, server, token) = start_server(fixture_host(false).await).await;

    // Old token works.
    let r = client()
        .get(format!("{base}/v1/health"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    // Rotate the token in place.
    let new_token = "rotated-000000000000000000000000000000000000000000000000000000000";
    *token.write().unwrap() = new_token.to_string();

    // Old token now 401, new token 200 — same running server.
    let r = client()
        .get(format!("{base}/v1/health"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
    let r = client()
        .get(format!("{base}/v1/health"))
        .headers(bearer(new_token))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    server.stop().await;
}

// ---- every read endpoint vs the fixture DB ----

#[tokio::test]
async fn search_returns_hits_from_fixture() {
    let (base, server, _t) = start_server(fixture_host(false).await).await;
    let r = client()
        .get(format!("{base}/v1/search?q=invoice&limit=10"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let hits: serde_json::Value = r.json().await.unwrap();
    assert_eq!(
        hits.as_array().unwrap().len(),
        1,
        "only the invoice frame matches"
    );

    // Missing `q` → 400 on the JSON error contract, not axum's plaintext rejection.
    let r = client()
        .get(format!("{base}/v1/search?limit=10"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
    assert!(r
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("application/json"));
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["error"], "bad_request");
    assert!(body["message"].is_string());

    server.stop().await;
}

#[tokio::test]
async fn frame_detail_image_and_not_found() {
    let (base, server, _t) = start_server(fixture_host(false).await).await;

    // Detail for a known frame (id 1 = first seeded).
    let r = client()
        .get(format!("{base}/v1/frames/1"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let detail: serde_json::Value = r.json().await.unwrap();
    assert_eq!(detail["frame_id"], 1);

    // `?image=1` returns the synthetic WebP bytes with the right content type.
    let r = client()
        .get(format!("{base}/v1/frames/1?image=1"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(
        r.headers().get("content-type").unwrap().to_str().unwrap(),
        "image/webp"
    );
    assert_eq!(r.bytes().await.unwrap().as_ref(), b"webp-bytes");

    // Unknown frame → 404.
    let r = client()
        .get(format!("{base}/v1/frames/9999"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["error"], "not_found");

    // A non-integer id → 400 on the JSON error contract (path rejection mapped).
    let r = client()
        .get(format!("{base}/v1/frames/not-a-number"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["error"], "bad_request");

    server.stop().await;
}

#[tokio::test]
async fn where_was_i_returns_null_when_nothing_qualifies() {
    let (base, server, _t) = start_server(fixture_host(false).await).await;
    let r = client()
        .get(format!("{base}/v1/context/where-was-i"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.text().await.unwrap(), "null");
    server.stop().await;
}

// ---- marks: the only write surface (create / list / resolve / validation) ----

#[tokio::test]
async fn marks_crud_roundtrip() {
    let (base, server, _t) = start_server(fixture_host(false).await).await;
    let c = client();

    // Create a mark on an existing frame → 201 with a mark_id.
    let r = c
        .post(format!("{base}/v1/marks"))
        .headers(bearer(TEST_TOKEN))
        .json(&serde_json::json!({ "frame_id": 1, "note": "check this" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 201);
    let created: serde_json::Value = r.json().await.unwrap();
    let mark_id = created["mark_id"].as_i64().unwrap();

    // List → the mark is present.
    let r = c
        .get(format!("{base}/v1/marks"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let marks: serde_json::Value = r.json().await.unwrap();
    assert_eq!(marks.as_array().unwrap().len(), 1);

    // `now:true` with no capture worker → 503 (honest, not a silent no-op).
    let r = c
        .post(format!("{base}/v1/marks"))
        .headers(bearer(TEST_TOKEN))
        .json(&serde_json::json!({ "now": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 503);

    // Both frame_id and now → 400.
    let r = c
        .post(format!("{base}/v1/marks"))
        .headers(bearer(TEST_TOKEN))
        .json(&serde_json::json!({ "frame_id": 1, "now": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);

    // Create on an unknown frame → 404.
    let r = c
        .post(format!("{base}/v1/marks"))
        .headers(bearer(TEST_TOKEN))
        .json(&serde_json::json!({ "frame_id": 9999 }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);

    // Resolve the real mark → 200.
    let r = c
        .post(format!("{base}/v1/marks/{mark_id}/resolve"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    // Resolve an unknown mark → 404.
    let r = c
        .post(format!("{base}/v1/marks/9999/resolve"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);

    server.stop().await;
}

// ---- SSE ask ----

#[tokio::test]
async fn ask_streams_sse_deltas() {
    let (base, server, _t) = start_server(fixture_host(true).await).await;
    let r = client()
        .post(format!("{base}/v1/ask"))
        .headers(bearer(TEST_TOKEN))
        .json(&serde_json::json!({ "query": "invoice" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert!(r
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("text/event-stream"));
    let body = r.text().await.unwrap();
    assert!(
        body.contains("grounded answer"),
        "token delta streamed: {body}"
    );
    assert!(
        body.contains("\"type\":\"done\""),
        "terminal Done streamed: {body}"
    );
    server.stop().await;
}

#[tokio::test]
async fn ask_without_answer_model_is_503() {
    let (base, server, _t) = start_server(fixture_host(false).await).await;
    let r = client()
        .post(format!("{base}/v1/ask"))
        .headers(bearer(TEST_TOKEN))
        .json(&serde_json::json!({ "query": "invoice" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 503);
    server.stop().await;
}

// ---- export over HTTP + the shared file path ----

#[tokio::test]
async fn export_over_http_is_valid_json() {
    let (base, server, _t) = start_server(fixture_host(false).await).await;
    let r = client()
        .get(format!("{base}/v1/export?format=json"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let doc: serde_json::Value = r.json().await.unwrap();
    assert_eq!(doc["schema"], "screensearch.export.v1");
    assert_eq!(doc["frames"].as_array().unwrap().len(), 2);
    assert!(doc["marks"].is_array());

    // A non-JSON format is 400.
    let r = client()
        .get(format!("{base}/v1/export?format=csv"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);

    server.stop().await;
}

#[tokio::test]
async fn export_window_excludes_out_of_range_frames() {
    let (base, server, _t) = start_server(fixture_host(false).await).await;
    // [1500, 3000) includes only the 2000 frame.
    let r = client()
        .get(format!("{base}/v1/export?from=1500&to=3000"))
        .headers(bearer(TEST_TOKEN))
        .send()
        .await
        .unwrap();
    let doc: serde_json::Value = r.json().await.unwrap();
    let frames = doc["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0]["captured_at"], 2000);
    server.stop().await;
}

/// The Settings "Export…" path: streams the same document to a file **with no server** —
/// export works with the API disabled (`03 §7c`, DoD `§13b`).
#[tokio::test]
async fn export_to_file_writes_valid_json_without_a_server() {
    let host = fixture_host(false).await;
    let dir = tempfile::tempdir().unwrap();
    let result = api::export::export_to_file(host, None, None, dir.path())
        .await
        .expect("export to file");

    assert_eq!(result.frames, 2);
    let contents = std::fs::read_to_string(&result.path).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert_eq!(doc["schema"], "screensearch.export.v1");
    assert_eq!(doc["frames"].as_array().unwrap().len(), 2);

    // No leftover `.partial` file.
    let partials: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".partial"))
        .collect();
    assert!(partials.is_empty(), "no leftover .partial after success");
}

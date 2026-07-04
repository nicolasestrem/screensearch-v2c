//! Spawned-binary stdio integration tests for `screensearch-mcp` (0.3.0 PR8; `03 §13b.7`).
//!
//! Each test starts the **real** `crates/api` server in-process over a fixture store, then
//! spawns the compiled `screensearch-mcp` binary and drives it over stdio exactly as an MCP
//! client would — proving the handshake, tool listing, per-tool round-trips, the error
//! paths, and the API-off guidance end to end. Dev-deps (`api`/`store`/`traits`) never enter
//! the shipped binary, so D13's "no store, no app coupling" still holds for what ships.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Duration;

use api::{ApiHost, ApiServer, FrameImage, SharedToken};
use async_trait::async_trait;
use serde_json::{json, Value};
use store::SqliteStore;
use tokio::sync::mpsc::Sender;
use traits::{
    AnswerDelta, AnswerOpts, AnswerProvider, ExportFrameRow, FrameDetail, NewFrame, OcrResult,
    ResumeContext, RetrievedChunk, Settings, Store,
};

const TEST_TOKEN: &str = "test-token-000000000000000000000000000000000000000000000000000000";
/// How long to wait for one child response before failing (rather than hanging the suite).
const RECV_TIMEOUT: Duration = Duration::from_secs(20);

// ---------------------------------------------------------------------------
// The API test host (a trimmed clone of `crates/api/tests/http_api.rs`, plus a
// retention-purged frame so the `get_moment` purged-image path is exercised).
// ---------------------------------------------------------------------------

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
            // No capture worker in the fixture, so `now` is honestly unavailable (503).
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

/// Seeds two normal frames + one retention-purged frame (id 3). Returns the host.
async fn fixture_host(with_answer: bool) -> Arc<TestHost> {
    let store = Arc::new(SqliteStore::open_in_memory().expect("open store"));
    seed_frame(&store, 1_000, "quarterly invoice total").await;
    seed_frame(&store, 2_000, "vacation beach photos").await;
    let purged = seed_frame(&store, 3_000, "receipt from the hardware store").await;
    store.purge_frame_image(purged).await.expect("purge image");
    Arc::new(TestHost {
        store,
        answer: with_answer.then(|| Arc::new(ScriptedAnswer) as Arc<dyn AnswerProvider>),
        started_at_ms: 500,
    })
}

/// Starts the real API server over the fixture host on an ephemeral loopback port.
async fn start_api(with_answer: bool) -> (String, ApiServer, String) {
    let token: SharedToken = Arc::new(std::sync::RwLock::new(TEST_TOKEN.to_string()));
    let server = ApiServer::start(fixture_host(with_answer).await, token, 0)
        .await
        .expect("bind ephemeral loopback");
    let base = format!("http://{}", server.local_addr());
    (base, server, TEST_TOKEN.to_string())
}

// ---------------------------------------------------------------------------
// The MCP child-process driver.
// ---------------------------------------------------------------------------

struct McpChild {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<String>,
    next_id: i64,
}

impl McpChild {
    /// Spawns the compiled binary with the given URL and optional token.
    fn spawn(url: &str, token: Option<&str>) -> McpChild {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_screensearch-mcp"));
        cmd.env("SCREENSEARCH_API_URL", url)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        match token {
            Some(t) => {
                cmd.env("SCREENSEARCH_API_TOKEN", t);
            }
            // Ensure no stray token leaks in from the test environment.
            None => {
                cmd.env_remove("SCREENSEARCH_API_TOKEN");
            }
        }
        let mut child = cmd.spawn().expect("spawn screensearch-mcp");
        let stdin = child.stdin.take().expect("child stdin");
        let stdout = child.stdout.take().expect("child stdout");

        // A dedicated reader thread pumps stdout lines into a channel, so a hung child
        // becomes a recv timeout instead of a deadlocked test.
        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(l) if !l.trim().is_empty() => {
                        if tx.send(l).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        });

        McpChild {
            child,
            stdin,
            rx,
            next_id: 1,
        }
    }

    fn send(&mut self, msg: &Value) {
        let line = serde_json::to_string(msg).unwrap();
        self.stdin.write_all(line.as_bytes()).expect("write stdin");
        self.stdin.write_all(b"\n").expect("write newline");
        self.stdin.flush().expect("flush stdin");
    }

    fn recv(&mut self) -> Value {
        let line = self
            .rx
            .recv_timeout(RECV_TIMEOUT)
            .expect("a response line within the timeout");
        serde_json::from_str(&line).expect("response is valid JSON")
    }

    /// Sends a raw line (used to exercise the parse-error path).
    fn send_raw(&mut self, line: &str) {
        self.stdin.write_all(line.as_bytes()).expect("write stdin");
        self.stdin.write_all(b"\n").expect("write newline");
        self.stdin.flush().expect("flush stdin");
    }

    /// Sends a request and returns its response, asserting the id round-trips.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let resp = self.recv();
        assert_eq!(resp["id"], json!(id), "response id must match request id");
        resp
    }

    fn notify(&mut self, method: &str) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method }));
    }

    /// initialize + notifications/initialized. Returns the initialize result.
    fn handshake(&mut self) -> Value {
        let resp = self.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "integration-test", "version": "0" }
            }),
        );
        self.notify("notifications/initialized");
        resp["result"].clone()
    }

    /// Calls a tool and returns the `result` object (`{ content, isError? }`).
    fn call_tool(&mut self, name: &str, args: Value) -> Value {
        let resp = self.request("tools/call", json!({ "name": name, "arguments": args }));
        assert!(
            resp.get("error").is_none(),
            "unexpected protocol error: {resp}"
        );
        resp["result"].clone()
    }

    /// Drops stdin (EOF) and waits for the child to exit, returning its code.
    fn close_and_wait(mut self) -> i32 {
        drop(self.stdin);
        let status = self.child.wait().expect("child exit");
        status.code().unwrap_or(-1)
    }
}

/// The first text content block of a tool result.
fn first_text(result: &Value) -> String {
    result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn handshake_and_tools_list_over_stdio() {
    let (base, server, token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, Some(&token));

    let init = mcp.handshake();
    assert_eq!(init["protocolVersion"], "2025-06-18");
    assert!(init["capabilities"]["tools"].is_object());
    assert_eq!(init["serverInfo"]["name"], "screensearch-mcp");

    let resp = mcp.request("tools/list", json!({}));
    let names: Vec<String> = resp["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        names,
        vec![
            "search_screen_history",
            "ask_screen_history",
            "get_moment",
            "where_was_i",
            "list_marks",
            "add_mark",
        ]
    );
    for t in resp["result"]["tools"].as_array().unwrap() {
        assert_eq!(t["inputSchema"]["type"], "object");
    }

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ping_returns_empty_result() {
    let (base, server, token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, Some(&token));
    mcp.handshake();
    let resp = mcp.request("ping", json!({}));
    assert_eq!(resp["result"], json!({}));
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn search_tool_roundtrips_fixture() {
    let (base, server, token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, Some(&token));
    mcp.handshake();

    let result = mcp.call_tool("search_screen_history", json!({ "query": "invoice" }));
    let hits: Value = serde_json::from_str(&first_text(&result)).expect("hits are JSON");
    assert_eq!(hits.as_array().unwrap().len(), 1);
    assert_eq!(hits[0]["frame_id"], 1);

    // A query with no matches returns the empty sentinel, not an error.
    let empty = mcp.call_tool("search_screen_history", json!({ "query": "zzz-nope-zzz" }));
    assert_eq!(first_text(&empty), "No results.");

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ask_tool_aggregates_answer_and_citation() {
    let (base, server, token) = start_api(true).await;
    let mut mcp = McpChild::spawn(&base, Some(&token));
    mcp.handshake();

    let result = mcp.call_tool("ask_screen_history", json!({ "query": "invoice" }));
    assert_ne!(
        result["isError"],
        json!(true),
        "ask should succeed: {result}"
    );
    let text = first_text(&result);
    assert!(text.contains("grounded answer"), "answer text: {text}");
    assert!(text.contains("Cited frames: 1"), "citations: {text}");

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ask_without_model_is_tool_error() {
    let (base, server, token) = start_api(false).await; // no answer provider
    let mut mcp = McpChild::spawn(&base, Some(&token));
    mcp.handshake();

    let result = mcp.call_tool("ask_screen_history", json!({ "query": "invoice" }));
    assert_eq!(result["isError"], json!(true));
    assert!(
        first_text(&result).contains("answer model not loaded")
            || first_text(&result).to_lowercase().contains("unavailable"),
        "message: {}",
        first_text(&result)
    );

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn get_moment_text_only_and_include_image() {
    let (base, server, token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, Some(&token));
    mcp.handshake();

    // Default: one text block, no image.
    let text_only = mcp.call_tool("get_moment", json!({ "frame_id": 1 }));
    assert_eq!(text_only["content"].as_array().unwrap().len(), 1);
    assert_eq!(text_only["content"][0]["type"], "text");

    // include_image: a second block, an image, base64 of the fixture bytes.
    let with_image = mcp.call_tool(
        "get_moment",
        json!({ "frame_id": 1, "include_image": true }),
    );
    let blocks = with_image["content"].as_array().unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[1]["type"], "image");
    assert_eq!(blocks[1]["mimeType"], "image/webp");
    use base64::Engine as _;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(blocks[1]["data"].as_str().unwrap())
        .unwrap();
    assert_eq!(decoded, b"webp-bytes");

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn get_moment_purged_image_notes_purge_without_error() {
    let (base, server, token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, Some(&token));
    mcp.handshake();

    // Frame 3 is retention-purged; include_image must NOT error, and must note the purge.
    let result = mcp.call_tool(
        "get_moment",
        json!({ "frame_id": 3, "include_image": true }),
    );
    assert_ne!(result["isError"], json!(true));
    let blocks = result["content"].as_array().unwrap();
    // Text detail + a purge note; no image block.
    assert!(blocks.iter().all(|b| b["type"] == "text"));
    assert!(
        blocks
            .iter()
            .any(|b| b["text"].as_str().unwrap_or("").contains("purged")),
        "expected a purge note: {result}"
    );

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn get_moment_unknown_frame_is_tool_error() {
    let (base, server, token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, Some(&token));
    mcp.handshake();

    let result = mcp.call_tool("get_moment", json!({ "frame_id": 9999 }));
    assert_eq!(result["isError"], json!(true));
    assert!(first_text(&result).contains("not_found"));

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn where_was_i_null_returns_human_message() {
    let (base, server, token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, Some(&token));
    mcp.handshake();

    let result = mcp.call_tool("where_was_i", json!({}));
    assert_ne!(result["isError"], json!(true));
    assert!(first_text(&result).contains("Nothing to resume"));

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn add_mark_frame_id_then_list_marks_roundtrip() {
    let (base, server, token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, Some(&token));
    mcp.handshake();

    // No marks yet.
    let empty = mcp.call_tool("list_marks", json!({}));
    assert_eq!(first_text(&empty), "No marks.");

    // Create one on an existing frame.
    let created = mcp.call_tool("add_mark", json!({ "frame_id": 1, "note": "from mcp" }));
    assert_ne!(created["isError"], json!(true), "add_mark: {created}");
    assert!(first_text(&created).contains("Created mark"));

    // Now it lists.
    let listed = mcp.call_tool("list_marks", json!({}));
    let marks: Value = serde_json::from_str(&first_text(&listed)).expect("marks are JSON");
    assert_eq!(marks.as_array().unwrap().len(), 1);
    assert_eq!(marks[0]["frame_id"], 1);

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn add_mark_now_surfaces_unavailable() {
    let (base, server, token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, Some(&token));
    mcp.handshake();

    // No frame_id → capture now; the fixture has no capture worker → 503 surfaced.
    let result = mcp.call_tool("add_mark", json!({ "note": "capture me" }));
    assert_eq!(result["isError"], json!(true));
    assert!(
        first_text(&result).contains("capture is off"),
        "message: {}",
        first_text(&result)
    );

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unknown_tool_is_protocol_error() {
    let (base, server, token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, Some(&token));
    mcp.handshake();

    let resp = mcp.request(
        "tools/call",
        json!({ "name": "no_such_tool", "arguments": {} }),
    );
    assert_eq!(resp["error"]["code"], -32602);

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unknown_method_is_method_not_found() {
    let (base, server, token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, Some(&token));
    mcp.handshake();

    let resp = mcp.request("resources/list", json!({}));
    assert_eq!(resp["error"]["code"], -32601);

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn malformed_line_is_parse_error() {
    let (base, server, token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, Some(&token));
    mcp.handshake();

    mcp.send_raw("this is not json");
    let resp = mcp.recv();
    assert_eq!(resp["id"], Value::Null);
    assert_eq!(resp["error"]["code"], -32700);

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn batch_line_is_invalid_request() {
    let (base, server, token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, Some(&token));
    mcp.handshake();

    mcp.send_raw(r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#);
    let resp = mcp.recv();
    assert_eq!(resp["error"]["code"], -32600);

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn api_off_tool_calls_return_guided_error() {
    // Reserve a free loopback port and HOLD the listener bound through process spawn and the
    // handshake, so no other process can steal the port during the slow setup window. (The
    // old bind-then-immediately-drop left the port free for the tens of ms it takes to spawn
    // the child — a TOCTOU race where another process binding that port would turn the
    // expected connection-refused into a connection to a foreign server and fail the assert.)
    // The child connects to the API lazily on its first tool call, never during the handshake,
    // so holding the listener bound here is safe.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let base = format!("http://127.0.0.1:{port}");
    let mut mcp = McpChild::spawn(&base, Some(TEST_TOKEN));

    // Handshake + tools/list still work (they never touch the API).
    mcp.handshake();
    let list = mcp.request("tools/list", json!({}));
    assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 6);

    // Release the port now — immediately before the call that must hit connection-refused —
    // shrinking the reuse window to the microseconds between here and the child's connect.
    drop(listener);

    // But a tool call surfaces the guided "enable the API" error.
    let result = mcp.call_tool("search_screen_history", json!({ "query": "x" }));
    assert_eq!(result["isError"], json!(true));
    assert!(
        first_text(&result).contains("enable the API in ScreenSearch Settings"),
        "message: {}",
        first_text(&result)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn wrong_token_returns_guided_401_message() {
    let (base, server, _token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, Some("the-wrong-token"));
    mcp.handshake();

    let result = mcp.call_tool("search_screen_history", json!({ "query": "invoice" }));
    assert_eq!(result["isError"], json!(true));
    assert!(
        first_text(&result).contains("401"),
        "message: {}",
        first_text(&result)
    );

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn missing_token_still_serves_tools_list_but_calls_are_guided() {
    let (base, server, _token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, None); // no token at all
    mcp.handshake();

    // Listing works.
    let list = mcp.request("tools/list", json!({}));
    assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 6);

    // A call is guided (the no-token message also carries the contract phrase).
    let result = mcp.call_tool("list_marks", json!({}));
    assert_eq!(result["isError"], json!(true));
    assert!(first_text(&result).contains("enable the API in ScreenSearch Settings"));

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn child_exits_zero_on_stdin_close() {
    let (base, server, token) = start_api(false).await;
    let mut mcp = McpChild::spawn(&base, Some(&token));
    mcp.handshake();
    server.stop().await;
    // Closing stdin is the MCP shutdown signal — the child must exit cleanly.
    assert_eq!(mcp.close_and_wait(), 0);
}

//! `SidecarClient` against a mock of the sidecar's OpenAI-compatible HTTP API
//! (`03 §6`). Exercises the two shapes P4 uses — non-streaming vision completion and
//! streaming (SSE) answers — without a real `llama-server` (that is the gated
//! `#[ignore]` smoke). Verifies request shape, response parsing, and that a
//! `reasoning_content` delta and ordinary `content` deltas surface as the right
//! ordered [`StreamPiece`]s ending in `Done`.

use inference::client::{ChatMessage, ClientTimeouts, SidecarClient, StreamPiece};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn health_reports_success() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;

    let client = SidecarClient::new(server.uri());
    assert!(client.health().await, "health should be true on 200");
}

#[tokio::test]
async fn health_false_when_unavailable() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let client = SidecarClient::new(server.uri());
    assert!(!client.health().await, "health should be false on 503");
}

#[tokio::test]
async fn health_times_out_quickly_when_sidecar_hangs() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(5))
                .set_body_string("{}"),
        )
        .mount(&server)
        .await;

    let client = SidecarClient::with_timeouts(
        server.uri(),
        Duration::from_millis(50),
        Duration::from_secs(5),
        Duration::from_secs(5),
    );
    let started = Instant::now();

    assert!(
        !client.health().await,
        "hung health endpoint should be treated as unhealthy"
    );
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "health timeout should be bounded, elapsed {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn vision_completion_returns_message_content() {
    let server = MockServer::start().await;
    // Non-streaming completion (the vision lane): stream=false in the body.
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_string_contains("\"stream\":false"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"choices":[{"message":{"role":"assistant","content":"a VS Code editor with Rust code"}}]}"#,
        ))
        .mount(&server)
        .await;

    let client = SidecarClient::new(server.uri());
    let msg = ChatMessage::image("Describe this screenshot.", "data:image/jpeg;base64,AAAA");
    let out = client
        .complete(vec![msg], 256, None)
        .await
        .expect("completion");
    assert_eq!(out, "a VS Code editor with Rust code");
}

#[tokio::test]
async fn completion_times_out_when_sidecar_hangs() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(5))
                .set_body_string(
                    r#"{"choices":[{"message":{"role":"assistant","content":"late"}}]}"#,
                ),
        )
        .mount(&server)
        .await;

    let client = SidecarClient::with_timeouts(
        server.uri(),
        Duration::from_secs(5),
        Duration::from_millis(50),
        Duration::from_secs(5),
    );
    let msg = ChatMessage::image("Describe this screenshot.", "data:image/jpeg;base64,AAAA");
    let started = Instant::now();
    let err = client
        .complete(vec![msg], 256, None)
        .await
        .expect_err("hung completion should time out");

    assert!(err.to_string().contains("timed out"));
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "completion timeout should be bounded, elapsed {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn answer_stream_yields_ordered_pieces() {
    let server = MockServer::start().await;
    // SSE: one reasoning delta, two content deltas, then [DONE].
    let sse = "\
data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"the user asked about X\"}}]}\n\
\n\
data: {\"choices\":[{\"delta\":{\"content\":\"The answer\"}}]}\n\
\n\
data: {\"choices\":[{\"delta\":{\"content\":\" is 42.\"}}]}\n\
\n\
data: [DONE]\n\
\n";
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_string_contains("\"stream\":true"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(sse.as_bytes().to_vec(), "text/event-stream"),
        )
        .mount(&server)
        .await;

    let client = SidecarClient::new(server.uri());
    let (tx, mut rx) = mpsc::channel(16);
    client
        .stream(vec![ChatMessage::text("user", "What is 6*7?")], 128, &tx)
        .await
        .expect("stream");
    drop(tx);

    let mut pieces = Vec::new();
    while let Some(p) = rx.recv().await {
        pieces.push(p);
    }

    assert_eq!(
        pieces,
        vec![
            StreamPiece::Reasoning("the user asked about X".to_string()),
            StreamPiece::Content("The answer".to_string()),
            StreamPiece::Content(" is 42.".to_string()),
            StreamPiece::Done,
        ]
    );
}

#[tokio::test]
async fn stream_connect_times_out_when_initial_post_hangs() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(5))
                .set_body_raw(b"data: [DONE]\n\n".to_vec(), "text/event-stream"),
        )
        .mount(&server)
        .await;

    let client = SidecarClient::with_client_timeouts(
        server.uri(),
        ClientTimeouts {
            health: Duration::from_secs(5),
            completion: Duration::from_secs(5),
            stream_connect: Duration::from_millis(50),
            stream_idle: Duration::from_secs(5),
        },
    );
    let (tx, _rx) = mpsc::channel(16);
    let started = Instant::now();
    let err = client
        .stream(vec![ChatMessage::text("user", "What is 6*7?")], 128, &tx)
        .await
        .expect_err("hung stream connection should time out");

    assert!(err.to_string().contains("timed out"));
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "stream connect timeout should be bounded, elapsed {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn stream_times_out_when_no_sse_chunk_arrives() {
    let base = sse_server_that_sends_headers_then_hangs().await;
    let client = SidecarClient::with_client_timeouts(
        base,
        ClientTimeouts {
            health: Duration::from_secs(5),
            completion: Duration::from_secs(5),
            stream_connect: Duration::from_secs(5),
            stream_idle: Duration::from_millis(50),
        },
    );
    let (tx, _rx) = mpsc::channel(16);
    let started = Instant::now();
    let err = client
        .stream(vec![ChatMessage::text("user", "What is 6*7?")], 128, &tx)
        .await
        .expect_err("hung SSE stream should time out after headers");

    assert!(err.to_string().contains("SSE data"));
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "stream idle timeout should be bounded, elapsed {:?}",
        started.elapsed()
    );
}

async fn sse_server_that_sends_headers_then_hangs() -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind local test server");
    let addr = listener.local_addr().expect("test server local addr");
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept test request");
        let mut request = vec![0; 4096];
        let _ = socket.read(&mut request).await;
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n",
            )
            .await
            .expect("write SSE headers");
        tokio::time::sleep(Duration::from_secs(5)).await;
    });
    format!("http://{addr}")
}

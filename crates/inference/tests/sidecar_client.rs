//! `SidecarClient` against a mock of the sidecar's OpenAI-compatible HTTP API
//! (`03 §6`). Exercises the two shapes P4 uses — non-streaming vision completion and
//! streaming (SSE) answers — without a real `llama-server` (that is the gated
//! `#[ignore]` smoke). Verifies request shape, response parsing, and that a
//! `reasoning_content` delta and ordinary `content` deltas surface as the right
//! ordered [`StreamPiece`]s ending in `Done`.

use inference::client::{ChatMessage, SidecarClient, StreamPiece};
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

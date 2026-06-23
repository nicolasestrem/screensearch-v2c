//! HTTP client for the `llama-server` sidecar's OpenAI-compatible API (`03 §6`).
//!
//! The sidecar serves plain HTTP on `127.0.0.1:<ephemeral>`; this module speaks the
//! two shapes P4 needs: a **non-streaming** chat completion for vision tagging (image
//! in, description out) and a **streaming** (SSE) chat completion for answers. The
//! streaming side normalizes both ways a llama.cpp build surfaces a reasoning trace —
//! a dedicated `reasoning_content` delta field, or `content` text that the answer
//! provider later splits on `<think>` tags — into a neutral [`StreamPiece`] flow.

use anyhow::{Context, Result};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
use tokio::time::{timeout, Duration};

/// Bounded waits for the localhost sidecar. The defaults are intentionally long
/// enough for real model work while still preventing an unresponsive process from
/// pinning a worker or answer stream forever.
#[derive(Debug, Clone, Copy)]
pub struct ClientTimeouts {
    pub health: Duration,
    pub completion: Duration,
    pub stream_connect: Duration,
    pub stream_idle: Duration,
}

impl Default for ClientTimeouts {
    fn default() -> Self {
        Self {
            health: Duration::from_secs(2),
            completion: Duration::from_secs(120),
            stream_connect: Duration::from_secs(30),
            stream_idle: Duration::from_secs(30),
        }
    }
}

/// One message in a chat request. `content` is either a plain string (answer lane) or
/// a multimodal parts array (vision lane: text + an image data URL).
#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: &'static str,
    pub content: MessageContent,
}

impl ChatMessage {
    /// A text-only message.
    pub fn text(role: &'static str, text: impl Into<String>) -> Self {
        Self {
            role,
            content: MessageContent::Text(text.into()),
        }
    }

    /// A user message pairing a prompt with one image (a `data:` URL).
    pub fn image(prompt: impl Into<String>, image_data_url: impl Into<String>) -> Self {
        Self {
            role: "user",
            content: MessageContent::Parts(vec![
                ContentPart::Text {
                    text: prompt.into(),
                },
                ContentPart::ImageUrl {
                    image_url: ImageUrl {
                        url: image_data_url.into(),
                    },
                },
            ]),
        }
    }
}

/// Either a plain string or a multimodal parts array (serialized untagged so it
/// matches the OpenAI wire format).
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

/// One part of a multimodal message.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageUrl {
    pub url: String,
}

/// A neutral streamed piece — the client's view of an SSE delta, before the answer
/// provider maps it to a typed `AnswerDelta`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamPiece {
    /// A token of the model's reasoning trace (from a `reasoning_content` delta).
    Reasoning(String),
    /// A token of normal assistant output.
    Content(String),
    /// The stream terminated (`data: [DONE]`).
    Done,
}

#[derive(Serialize)]
struct ChatRequest {
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    /// Optional OpenAI-style structured-output constraint. `llama-server` converts a
    /// `json_schema` here into a sampling grammar, so the vision lane can force a
    /// well-shaped object (enum `activity_type`, numeric `confidence`) rather than
    /// trusting the prompt alone (`07` #20). Omitted from the body when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}
#[derive(Deserialize)]
struct Choice {
    message: RespMessage,
}
#[derive(Deserialize)]
struct RespMessage {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}
#[derive(Deserialize)]
struct StreamChoice {
    delta: Delta,
}
#[derive(Deserialize, Default)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

/// A thin client bound to one sidecar base URL (e.g. `http://127.0.0.1:51234`).
/// Cheap to clone — `reqwest::Client` is internally reference-counted — so the
/// supervisor can hand a clone to each in-flight request.
#[derive(Clone)]
pub struct SidecarClient {
    http: reqwest::Client,
    base: String,
    timeouts: ClientTimeouts,
}

impl SidecarClient {
    /// Builds a client for `base` (scheme + host + port, no trailing slash).
    pub fn new(base: impl Into<String>) -> Self {
        Self::with_client_timeouts(base, ClientTimeouts::default())
    }

    /// Builds a client with custom timeout values. Tests use very short durations;
    /// production uses [`ClientTimeouts::default`] via [`Self::new`]. The stream
    /// value is applied to both initial stream connection and per-chunk idle waits;
    /// use [`Self::with_client_timeouts`] when those phases need different budgets.
    pub fn with_timeouts(
        base: impl Into<String>,
        health: Duration,
        completion: Duration,
        stream_idle: Duration,
    ) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: base.into(),
            timeouts: ClientTimeouts {
                health,
                completion,
                stream_connect: stream_idle,
                stream_idle,
            },
        }
    }

    /// Builds a client with fully specified timeout values.
    pub fn with_client_timeouts(base: impl Into<String>, timeouts: ClientTimeouts) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: base.into(),
            timeouts,
        }
    }

    /// `GET /health` — true once the model is loaded and the server is serving.
    pub async fn health(&self) -> bool {
        let url = format!("{}/health", self.base);
        matches!(
            timeout(self.timeouts.health, self.http.get(url).send()).await,
            Ok(Ok(r)) if r.status().is_success()
        )
    }

    /// Non-streaming chat completion; returns the assistant message text. Used by the
    /// vision lane (one image → one description). `response_format`, when set, constrains
    /// the reply to a structured shape (the vision lane passes a JSON schema so the model
    /// must emit a well-formed object — `07` #20); pass `None` for an unconstrained reply.
    pub async fn complete(
        &self,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
        response_format: Option<serde_json::Value>,
    ) -> Result<String> {
        timeout(self.timeouts.completion, async {
            let req = ChatRequest {
                messages,
                max_tokens,
                stream: false,
                temperature: Some(0.2),
                response_format,
            };
            let url = format!("{}/v1/chat/completions", self.base);
            let resp = self
                .http
                .post(url)
                .json(&req)
                .send()
                .await
                .context("sidecar chat request failed")?
                .error_for_status()
                .context("sidecar returned an error status")?;
            let body: ChatResponse = resp.json().await.context("decode chat response")?;
            let content = body
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.message.content)
                .context("chat response had no message content")?;
            Ok(content)
        })
        .await
        .context("sidecar completion timed out")?
    }

    /// Streaming chat completion; forwards each SSE delta to `tx` as a [`StreamPiece`]
    /// and finishes with [`StreamPiece::Done`]. Used by the answer lane.
    pub async fn stream(
        &self,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
        tx: &Sender<StreamPiece>,
    ) -> Result<()> {
        let req = ChatRequest {
            messages,
            max_tokens,
            stream: true,
            temperature: Some(0.6),
            response_format: None,
        };
        let url = format!("{}/v1/chat/completions", self.base);
        let resp = timeout(self.timeouts.stream_connect, async {
            self.http
                .post(url)
                .json(&req)
                .send()
                .await
                .context("sidecar stream request failed")?
                .error_for_status()
                .context("sidecar returned an error status")
        })
        .await
        .context("sidecar stream timed out")??;

        let mut bytes = resp.bytes_stream();
        // Buffer raw bytes (not a lossy string): a chunk boundary can fall in the
        // middle of a multi-byte UTF-8 character, so only convert *complete lines* —
        // delimited by `\n`, which is ASCII and so always a safe split point.
        let mut buf: Vec<u8> = Vec::new();
        loop {
            let next = timeout(self.timeouts.stream_idle, bytes.next())
                .await
                .context("sidecar stream timed out waiting for SSE data")?;
            let Some(chunk) = next else {
                break;
            };
            let chunk = chunk.context("sidecar stream chunk failed")?;
            buf.extend_from_slice(&chunk);
            // Process complete lines; keep any partial trailing line in `buf`.
            while let Some(nl) = buf.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = buf.drain(..=nl).collect();
                let line = String::from_utf8_lossy(&line_bytes);
                if let Some(done) = self.handle_sse_line(line.trim_end(), tx).await? {
                    if done {
                        return Ok(());
                    }
                }
            }
        }
        // Stream ended without an explicit [DONE]; still signal completion.
        let _ = tx.send(StreamPiece::Done).await;
        Ok(())
    }

    /// Parses one SSE line; returns `Some(true)` when `[DONE]` ended the stream.
    async fn handle_sse_line(&self, line: &str, tx: &Sender<StreamPiece>) -> Result<Option<bool>> {
        let Some(data) = line.strip_prefix("data:") else {
            return Ok(None); // comments / blank separators / event: lines
        };
        let data = data.trim();
        if data.is_empty() {
            return Ok(None);
        }
        if data == "[DONE]" {
            let _ = tx.send(StreamPiece::Done).await;
            return Ok(Some(true));
        }
        let chunk: StreamChunk = match serde_json::from_str(data) {
            Ok(c) => c,
            Err(e) => {
                // A malformed frame shouldn't abort a long answer; log and skip.
                tracing::debug!(error = %e, "sidecar: skipping unparsable SSE frame");
                return Ok(None);
            }
        };
        if let Some(delta) = chunk.choices.into_iter().next().map(|c| c.delta) {
            if let Some(r) = delta.reasoning_content.filter(|s| !s.is_empty()) {
                let _ = tx.send(StreamPiece::Reasoning(r)).await;
            }
            if let Some(c) = delta.content.filter(|s| !s.is_empty()) {
                let _ = tx.send(StreamPiece::Content(c)).await;
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(response_format: Option<serde_json::Value>) -> ChatRequest {
        ChatRequest {
            messages: vec![ChatMessage::text("user", "hi")],
            max_tokens: 16,
            stream: false,
            temperature: Some(0.2),
            response_format,
        }
    }

    #[test]
    fn response_format_is_omitted_from_body_when_none() {
        let json = serde_json::to_string(&req(None)).unwrap();
        assert!(
            !json.contains("response_format"),
            "an unconstrained request must not carry a response_format key: {json}"
        );
    }

    #[test]
    fn response_format_is_serialized_when_set() {
        let json = serde_json::to_string(&req(Some(serde_json::json!({ "type": "json_object" }))))
            .unwrap();
        assert!(json.contains("\"response_format\""), "missing key: {json}");
        assert!(
            json.contains("json_object"),
            "schema not serialized: {json}"
        );
    }
}

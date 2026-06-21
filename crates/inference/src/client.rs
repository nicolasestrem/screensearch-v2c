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
}

impl SidecarClient {
    /// Builds a client for `base` (scheme + host + port, no trailing slash).
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: base.into(),
        }
    }

    /// `GET /health` — true once the model is loaded and the server is serving.
    pub async fn health(&self) -> bool {
        let url = format!("{}/health", self.base);
        matches!(self.http.get(url).send().await, Ok(r) if r.status().is_success())
    }

    /// Non-streaming chat completion; returns the assistant message text. Used by the
    /// vision lane (one image → one description).
    pub async fn complete(&self, messages: Vec<ChatMessage>, max_tokens: u32) -> Result<String> {
        let req = ChatRequest {
            messages,
            max_tokens,
            stream: false,
            temperature: Some(0.2),
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
        };
        let url = format!("{}/v1/chat/completions", self.base);
        let resp = self
            .http
            .post(url)
            .json(&req)
            .send()
            .await
            .context("sidecar stream request failed")?
            .error_for_status()
            .context("sidecar returned an error status")?;

        let mut bytes = resp.bytes_stream();
        // Buffer raw bytes (not a lossy string): a chunk boundary can fall in the
        // middle of a multi-byte UTF-8 character, so only convert *complete lines* —
        // delimited by `\n`, which is ASCII and so always a safe split point.
        let mut buf: Vec<u8> = Vec::new();
        while let Some(chunk) = bytes.next().await {
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

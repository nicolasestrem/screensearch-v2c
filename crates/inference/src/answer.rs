//! [`AnswerProvider`] over the llama.cpp sidecar (`03 §3/§6/§13.5`). Builds a grounded
//! RAG prompt from retrieved chunks, streams the model's reply, and maps it to the
//! typed [`AnswerDelta`] flow: reasoning → `Thinking`, answer text → `Token`, one
//! `Citation` per grounding frame, then `Done` (or `Error`).
//!
//! Reasoning is surfaced two ways depending on the build: a `reasoning_content` SSE
//! field (handled by the client as [`StreamPiece::Reasoning`]) or inline `<think>…
//! </think>` tags in the content (split here by [`ThinkSplitter`]). Citations are the
//! provided context frames (a reliable grounding set), not parsed from the prose.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::sync::mpsc::{self, Sender};
use traits::{AnswerDelta, AnswerOpts, AnswerProvider, ModelTier, RetrievedChunk};

use crate::client::{ChatMessage, StreamPiece};
use crate::download;
use crate::models::{self, ModelLane, ModelSpec};
use crate::supervisor::ModelSupervisor;

const SYSTEM_PROMPT: &str = "You answer questions about the user's screen history. \
Use ONLY the provided context snippets, each tagged with its source frame id. Ground \
your answer in them and be concise. If the context does not contain the answer, say so \
plainly rather than guessing.";

/// The answer lane provider. Like the vision provider, it owns the active tier and
/// lazily downloads the model on first use.
pub struct AnswerSidecar {
    supervisor: Arc<ModelSupervisor>,
    models_root: PathBuf,
    tier: RwLock<ModelTier>,
    launch: RwLock<LaunchOptions>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LaunchOptions {
    ngl: u32,
    device: Option<String>,
}

impl AnswerSidecar {
    pub fn new(
        supervisor: Arc<ModelSupervisor>,
        models_root: PathBuf,
        tier: ModelTier,
        ngl: u32,
        device: Option<String>,
    ) -> Self {
        Self {
            supervisor,
            models_root,
            tier: RwLock::new(tier),
            launch: RwLock::new(LaunchOptions { ngl, device }),
        }
    }

    /// Updates the active answer tier (next request switches the sidecar model).
    pub fn set_tier(&self, tier: ModelTier) {
        *self.tier.write().expect("answer tier lock") = tier;
    }

    /// Updates launch options for the next request (or the next model restart if a
    /// sidecar is already serving the same spec).
    pub fn set_launch_options(&self, ngl: u32, device: Option<String>) {
        *self.launch.write().expect("answer launch lock") = LaunchOptions { ngl, device };
    }

    async fn ensure_spec(&self) -> Result<ModelSpec> {
        let tier = *self.tier.read().expect("answer tier lock");
        let launch = self.launch.read().expect("answer launch lock").clone();
        if let Some(spec) = models::resolve_spec(
            &self.models_root,
            ModelLane::Answer,
            tier,
            launch.ngl,
            launch.device.clone(),
        ) {
            return Ok(spec);
        }
        download::ensure_model(&self.models_root, ModelLane::Answer, tier)
            .await
            .context("download answer model")?;
        models::resolve_spec(
            &self.models_root,
            ModelLane::Answer,
            tier,
            launch.ngl,
            launch.device,
        )
        .context("answer model files missing after download")
    }

    /// Runs the request to completion, sending a terminal delta either way. Setup
    /// failures surface as an `AnswerDelta::Error` rather than an `Err`, so the UI
    /// always receives a terminal event.
    async fn run(
        &self,
        query: &str,
        context: &[RetrievedChunk],
        opts: AnswerOpts,
        tx: &Sender<AnswerDelta>,
    ) -> Result<()> {
        let spec = self.ensure_spec().await?;
        let lease = self.supervisor.acquire(spec).await?;
        let messages = build_messages(query, context);

        // Bridge the client's low-level SSE pieces onto the typed AnswerDelta stream.
        let (ptx, mut prx) = mpsc::channel::<StreamPiece>(64);
        let client = lease.client().clone();
        let max_tokens = opts.max_tokens;
        let stream_task =
            tokio::spawn(async move { client.stream(messages, max_tokens, &ptx).await });

        let mut splitter = ThinkSplitter::default();
        while let Some(piece) = prx.recv().await {
            match piece {
                StreamPiece::Reasoning(text) => {
                    if opts.thinking {
                        let _ = tx.send(AnswerDelta::Thinking { text }).await;
                    }
                }
                StreamPiece::Content(text) => {
                    for (is_thinking, chunk) in splitter.push(&text) {
                        emit_segment(tx, is_thinking, chunk, opts.thinking).await;
                    }
                }
                StreamPiece::Done => break,
            }
        }
        if let Some((is_thinking, rest)) = splitter.flush() {
            emit_segment(tx, is_thinking, rest, opts.thinking).await;
        }

        let stream_result = stream_task
            .await
            .unwrap_or_else(|e| Err(anyhow::anyhow!("answer stream task panicked: {e}")));

        if let Err(e) = stream_result {
            let _ = tx
                .send(AnswerDelta::Error {
                    message: e.to_string(),
                })
                .await;
            return Ok(());
        }

        // Grounding citations: one per unique context frame (reliable, not parsed).
        let mut seen = Vec::new();
        for chunk in context {
            if !seen.contains(&chunk.frame_id) {
                seen.push(chunk.frame_id);
                let _ = tx
                    .send(AnswerDelta::Citation {
                        frame_id: chunk.frame_id,
                    })
                    .await;
            }
        }
        let _ = tx.send(AnswerDelta::Done).await;
        Ok(())
    }
}

#[async_trait]
impl AnswerProvider for AnswerSidecar {
    async fn answer(
        &self,
        query: &str,
        context: &[RetrievedChunk],
        opts: AnswerOpts,
        tx: Sender<AnswerDelta>,
    ) -> Result<()> {
        if let Err(e) = self.run(query, context, opts, &tx).await {
            // A setup failure (model resolve / sidecar spawn) still gets a terminal
            // delta so the UI never hangs waiting.
            let _ = tx
                .send(AnswerDelta::Error {
                    message: e.to_string(),
                })
                .await;
        }
        Ok(())
    }
}

async fn emit_segment(
    tx: &Sender<AnswerDelta>,
    is_thinking: bool,
    text: String,
    thinking_on: bool,
) {
    if text.is_empty() {
        return;
    }
    let delta = if is_thinking {
        if !thinking_on {
            return; // thinking suppressed by the request
        }
        AnswerDelta::Thinking { text }
    } else {
        AnswerDelta::Token { text }
    };
    let _ = tx.send(delta).await;
}

/// Builds the chat messages: a grounding system prompt + a user message that lists the
/// context snippets (tagged with their frame ids) and the question.
fn build_messages(query: &str, context: &[RetrievedChunk]) -> Vec<ChatMessage> {
    let mut user = String::from("Context snippets from my screen history:\n");
    if context.is_empty() {
        user.push_str("(no relevant snippets found)\n");
    } else {
        for chunk in context {
            user.push_str(&format!(
                "[frame {}] {}\n",
                chunk.frame_id,
                chunk.text.trim()
            ));
        }
    }
    user.push_str(&format!("\nQuestion: {query}"));
    vec![
        ChatMessage::text("system", SYSTEM_PROMPT),
        ChatMessage::text("user", user),
    ]
}

/// Splits a streamed content sequence into thinking vs. answer segments by tracking
/// `<think>…</think>` tags across chunk boundaries. Text that could be the *start* of a
/// tag is held back until the next chunk so a tag split across SSE frames isn't missed.
#[derive(Default)]
pub struct ThinkSplitter {
    in_think: bool,
    buf: String,
}

impl ThinkSplitter {
    const OPEN: &'static str = "<think>";
    const CLOSE: &'static str = "</think>";

    /// Feeds more content; returns `(is_thinking, text)` segments ready to emit.
    pub fn push(&mut self, text: &str) -> Vec<(bool, String)> {
        self.buf.push_str(text);
        let mut out = Vec::new();
        loop {
            let marker = if self.in_think {
                Self::CLOSE
            } else {
                Self::OPEN
            };
            if let Some(idx) = self.buf.find(marker) {
                let before: String = self.buf[..idx].to_string();
                if !before.is_empty() {
                    out.push((self.in_think, before));
                }
                self.buf.drain(..idx + marker.len());
                self.in_think = !self.in_think;
            } else {
                // No full marker. Emit all but a trailing tail that might begin one.
                let keep = partial_marker_suffix(&self.buf, marker);
                let emit_len = self.buf.len() - keep;
                if emit_len > 0 {
                    let chunk: String = self.buf.drain(..emit_len).collect();
                    out.push((self.in_think, chunk));
                }
                break;
            }
        }
        out
    }

    /// Emits any buffered remainder at end of stream.
    pub fn flush(&mut self) -> Option<(bool, String)> {
        if self.buf.is_empty() {
            None
        } else {
            Some((self.in_think, std::mem::take(&mut self.buf)))
        }
    }
}

/// Length of the longest suffix of `buf` that is a (proper) prefix of `marker` — the
/// tail to hold back in case the marker is split across chunks.
fn partial_marker_suffix(buf: &str, marker: &str) -> usize {
    let max = marker.len().saturating_sub(1).min(buf.len());
    for k in (1..=max).rev() {
        // `marker` is ASCII, so byte-prefix slicing is valid; guard the buf boundary.
        if buf.is_char_boundary(buf.len() - k) && buf[buf.len() - k..] == marker[..k] {
            return k;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(splitter: &mut ThinkSplitter, parts: &[&str]) -> Vec<(bool, String)> {
        let mut out = Vec::new();
        for p in parts {
            out.extend(splitter.push(p));
        }
        out.extend(splitter.flush());
        out
    }

    #[test]
    fn splits_inline_think_tags() {
        let mut s = ThinkSplitter::default();
        let segs = collect(&mut s, &["<think>reasoning here</think>The answer."]);
        assert_eq!(
            segs,
            vec![
                (true, "reasoning here".to_string()),
                (false, "The answer.".to_string()),
            ]
        );
    }

    #[test]
    fn handles_tag_split_across_chunks() {
        let mut s = ThinkSplitter::default();
        // "<think>" arrives split as "<thi" + "nk>", and "</think>" as "</thin" + "k>".
        let segs = collect(&mut s, &["<thi", "nk>step</thin", "k>done"]);
        assert_eq!(
            segs,
            vec![(true, "step".to_string()), (false, "done".to_string())]
        );
    }

    #[test]
    fn plain_content_is_all_tokens() {
        let mut s = ThinkSplitter::default();
        let segs = collect(&mut s, &["Hello ", "world"]);
        assert_eq!(
            segs,
            vec![(false, "Hello ".to_string()), (false, "world".to_string()),]
        );
    }

    #[test]
    fn builds_grounded_prompt_with_frame_tags() {
        let ctx = vec![
            RetrievedChunk {
                frame_id: 7,
                text: "login page".to_string(),
                score: 1.0,
                captured_at: 0,
            },
            RetrievedChunk {
                frame_id: 9,
                text: "dashboard".to_string(),
                score: 0.5,
                captured_at: 0,
            },
        ];
        let msgs = build_messages("what did I see?", &ctx);
        assert_eq!(msgs.len(), 2);
        // The user message must reference both frames for grounding.
        let user = serde_json::to_string(&msgs[1]).unwrap();
        assert!(user.contains("[frame 7]"));
        assert!(user.contains("[frame 9]"));
        assert!(user.contains("what did I see?"));
    }
}

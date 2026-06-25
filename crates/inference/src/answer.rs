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
use traits::{AnswerDelta, AnswerOpts, AnswerProvider, ModelTier, RetrievedChunk, SidecarParams};

use crate::client::{ChatMessage, StreamPiece};
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
    downloader: Arc<crate::download::ModelDownloader>,
    models_root: PathBuf,
    tier: RwLock<ModelTier>,
    launch: RwLock<SidecarParams>,
}

impl AnswerSidecar {
    pub fn new(
        supervisor: Arc<ModelSupervisor>,
        downloader: Arc<crate::download::ModelDownloader>,
        models_root: PathBuf,
        tier: ModelTier,
        params: SidecarParams,
    ) -> Self {
        Self {
            supervisor,
            downloader,
            models_root,
            tier: RwLock::new(tier),
            launch: RwLock::new(params),
        }
    }

    /// Updates the active answer tier (next request switches the sidecar model).
    pub fn set_tier(&self, tier: ModelTier) {
        *self.tier.write().expect("answer tier lock") = tier;
    }

    /// Updates launch options for the next request (or the next model restart if a
    /// sidecar is already serving the same spec). A change to any tuning field makes the
    /// next `resolve_spec` differ, so the supervisor relaunches.
    pub fn set_launch_options(&self, params: SidecarParams) {
        *self.launch.write().expect("answer launch lock") = params;
    }

    async fn ensure_spec(&self) -> Result<ModelSpec> {
        let tier = *self.tier.read().expect("answer tier lock");
        let params = self.launch.read().expect("answer launch lock").clone();
        if let Some(spec) =
            models::resolve_spec(&self.models_root, ModelLane::Answer, tier, params.clone())
        {
            return Ok(spec);
        }
        self.downloader
            .ensure(ModelLane::Answer, tier)
            .await
            .context("download answer model")?;
        models::resolve_spec(&self.models_root, ModelLane::Answer, tier, params)
            .context("answer model files missing after download")
    }

    /// Eagerly loads the current answer model into the sidecar (the manual "Load" control)
    /// so the next Ask is instant. Downloads on first use, then keeps it resident until the
    /// idle-TTL or a manual unload reclaims it.
    pub async fn preload(&self) -> Result<()> {
        let spec = self.ensure_spec().await?;
        self.supervisor.preload(spec).await
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
        let ctx_size = spec.ctx_size;
        let lease = self.supervisor.acquire(spec).await?;
        // Cap the reply budget so a large requested `max_tokens` can't consume the whole
        // context window and leave nothing for grounding snippets when `ctx_size` is small
        // (the UI sends a fixed 2048, but Settings allows ctx down to 512). (Codex review.)
        let max_tokens = effective_reply_budget(opts.max_tokens, ctx_size);
        let (messages, cited) = build_messages(query, context, ctx_size, max_tokens);

        // Bridge the client's low-level SSE pieces onto the typed AnswerDelta stream.
        let (ptx, mut prx) = mpsc::channel::<StreamPiece>(64);
        let client = lease.client().clone();
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

        // Grounding citations: one per included context frame (already deduped, in order).
        // Only frames that fit the context budget are cited, so a citation always
        // corresponds to text the model actually saw.
        for frame_id in &cited {
            let _ = tx
                .send(AnswerDelta::Citation {
                    frame_id: *frame_id,
                })
                .await;
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

    /// One report summarization pass (`03 §8b`): resolves + acquires the answer model
    /// like [`Self::run`], builds bounded report messages with the caller's
    /// `system_prompt`, and collects the streamed reply (thinking dropped). Returns the
    /// summary text + the frame ids actually read. Setup/stream errors propagate as
    /// `Err` (the orchestrator/command decides how to surface them), unlike `answer`'s
    /// terminal-delta swallow.
    async fn summarize(
        &self,
        system_prompt: &str,
        instruction: &str,
        context: &[RetrievedChunk],
        opts: AnswerOpts,
    ) -> Result<(String, Vec<i64>)> {
        let spec = self.ensure_spec().await?;
        let ctx_size = spec.ctx_size;
        let lease = self.supervisor.acquire(spec).await?;
        let max_tokens = effective_reply_budget(opts.max_tokens, ctx_size);
        let (messages, cited) =
            build_summary_messages(system_prompt, instruction, context, ctx_size, max_tokens);
        let client = lease.client().clone();
        let text = collect_stream(&client, messages, max_tokens).await?;
        Ok((text, cited))
    }

    /// The active answer model's GGUF filename for the report footer — resolved
    /// without downloading (`None` if the files aren't present yet).
    async fn answer_model_label(&self) -> Option<String> {
        let tier = *self.tier.read().expect("answer tier lock");
        let params = self.launch.read().expect("answer launch lock").clone();
        models::resolve_spec(&self.models_root, ModelLane::Answer, tier, params)
            .map(|s| report_model_label(&s.gguf_path))
    }

    /// The answer-lane context window the next `summarize` will run with, resolved from
    /// the launch options exactly as [`models::resolve_spec`] would (an explicit
    /// `ctx_size`, or the per-lane auto default when `0`). Lets the report planner budget
    /// against the user's real `sidecar.ctx_size` instead of assuming the default.
    fn answer_context_budget(&self) -> Option<u32> {
        let params = self.launch.read().expect("answer launch lock");
        Some(if params.ctx_size == 0 {
            models::default_ctx_for(ModelLane::Answer)
        } else {
            params.ctx_size
        })
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

/// Chat-template + role-tag overhead reserved on top of the system prompt and question,
/// so the assembled prompt leaves headroom for llama.cpp's own template tokens.
const TEMPLATE_OVERHEAD_TOKENS: usize = 96;
/// Per-snippet framing cost (`[frame <id>] ` + newline), in estimated tokens.
const ID_FRAMING_TOKENS: usize = 6;

/// Conservative UTF-8 **bytes**-per-token lower bound used to estimate prompt length
/// without a real tokenizer. A *chars*-based ratio under-counts dense scripts (a CJK
/// character is ~3 bytes yet ~1 token for well-merging tokenizers, and up to ~1.5 tokens
/// for Mistral-family ones like the default Ministral answer model) and would re-trigger
/// the `exceed_context_size_error` this budgeting prevents. At 2 bytes/token the estimate
/// stays an *upper* bound on tokens for both scripts — English (~4 bytes/token) is
/// over-reserved (safe, with ample context still admitted) and worst-case CJK is covered.
/// (Gemini/Claude/Codex review, PR #26.)
const BYTES_PER_TOKEN: usize = 2;

/// The reply token budget actually used. Caps a large requested `max_tokens` to half the
/// context window so it can never reserve the *entire* window and force `build_messages` to
/// drop all grounding snippets (the symptom: every Ask answers "(no relevant snippets
/// found)"). For the normal 4K/8K windows the UI's 2048 is well under half and passes
/// through unchanged; only a small `ctx_size` (Settings allows down to 512) shrinks it.
fn effective_reply_budget(requested: u32, ctx_size: u32) -> u32 {
    requested.min((ctx_size / 2).max(1))
}

/// Rough token estimate. Deliberately over-counts (see [`BYTES_PER_TOKEN`]) so the
/// assembled prompt stays under the model's context window.
fn estimate_tokens(text: &str) -> usize {
    text.len() / BYTES_PER_TOKEN + 1
}

/// Truncates `text` to roughly `budget_tokens` worth of UTF-8 bytes, snapped down to a
/// char boundary (so multibyte characters are never split). Mirrors [`estimate_tokens`].
fn truncate_to_tokens(text: &str, budget_tokens: usize) -> String {
    let mut max_bytes = budget_tokens
        .saturating_mul(BYTES_PER_TOKEN)
        .min(text.len());
    while max_bytes > 0 && !text.is_char_boundary(max_bytes) {
        max_bytes -= 1;
    }
    text[..max_bytes].to_string()
}

/// Packs context chunks best-first into `intro`, spending at most `budget_tokens`. Each
/// chunk costs its estimated tokens plus [`ID_FRAMING_TOKENS`]; chunks are appended as
/// `[frame <id>] <text>` until the budget is spent, the rest dropped. If the most relevant
/// chunk alone exceeds the budget it is truncated (grounding on a head beats dropping
/// everything). When nothing fit, a `(no relevant snippets found)` line is appended.
/// Returns the assembled user text and the frame ids actually included — the shared core
/// of [`build_messages`] (Ask) and [`build_summary_messages`] (reports), so citations
/// always cover context the model really saw and the budgeting is bounded once.
fn pack_context(
    intro: &str,
    context: &[&RetrievedChunk],
    budget_tokens: usize,
) -> (String, Vec<i64>) {
    let mut budget = budget_tokens;
    let mut user = String::from(intro);
    let mut included: Vec<i64> = Vec::new();
    for chunk in context {
        if budget == 0 {
            break;
        }
        let text = chunk.text.trim();
        if text.is_empty() {
            continue;
        }
        let cost = estimate_tokens(text) + ID_FRAMING_TOKENS;
        let snippet = if cost <= budget {
            budget -= cost;
            text.to_string()
        } else if included.is_empty() {
            // The most relevant chunk alone exceeds the budget: ground on a truncated head
            // rather than dropping all context, then stop.
            let s = truncate_to_tokens(text, budget.saturating_sub(ID_FRAMING_TOKENS));
            budget = 0;
            s
        } else {
            break;
        };
        if snippet.is_empty() {
            break;
        }
        user.push_str(&format!("[frame {}] {}\n", chunk.frame_id, snippet));
        if !included.contains(&chunk.frame_id) {
            included.push(chunk.frame_id);
        }
    }
    if included.is_empty() {
        user.push_str("(no relevant snippets found)\n");
    }
    (user, included)
}

/// Builds the chat messages: a grounding system prompt + a user message listing the
/// context snippets (tagged with their frame ids) and the question — **bounded** to the
/// model's context window via [`pack_context`] (the reserve is `ctx_size` minus the reply
/// `max_tokens`, the system prompt, the question, and template overhead). Without this the
/// prompt could exceed `n_ctx` and llama-server returns a 400 `exceed_context_size_error`
/// (verified). Returns the messages plus the frame ids actually included, so citations only
/// cover context the model really saw.
fn build_messages(
    query: &str,
    context: &[RetrievedChunk],
    ctx_size: u32,
    max_tokens: u32,
) -> (Vec<ChatMessage>, Vec<i64>) {
    let reserve = max_tokens as usize
        + estimate_tokens(SYSTEM_PROMPT)
        + estimate_tokens(query)
        + TEMPLATE_OVERHEAD_TOKENS;
    let budget = (ctx_size as usize).saturating_sub(reserve);
    let refs: Vec<&RetrievedChunk> = context.iter().collect();
    let (mut user, included) =
        pack_context("Context snippets from my screen history:\n", &refs, budget);
    user.push_str(&format!("\nQuestion: {query}"));
    (
        vec![
            ChatMessage::text("system", SYSTEM_PROMPT),
            ChatMessage::text("user", user),
        ],
        included,
    )
}

/// Builds the chat messages for one report summarization pass (`03 §8b`): the
/// caller-supplied `system_prompt` (map / reduce / final) + a user message of the
/// content snippets (bounded via [`pack_context`]) followed by the `instruction` (e.g.
/// the period label or the user's steering prompt). Returns the frame ids actually
/// included so the orchestrator can carry citations through the map → reduce tree.
fn build_summary_messages(
    system_prompt: &str,
    instruction: &str,
    context: &[RetrievedChunk],
    ctx_size: u32,
    max_tokens: u32,
) -> (Vec<ChatMessage>, Vec<i64>) {
    let reserve = max_tokens as usize
        + estimate_tokens(system_prompt)
        + estimate_tokens(instruction)
        + TEMPLATE_OVERHEAD_TOKENS;
    let budget = (ctx_size as usize).saturating_sub(reserve);
    let refs: Vec<&RetrievedChunk> = context.iter().collect();
    let (mut user, included) = pack_context("Content from my screen history:\n", &refs, budget);
    if !instruction.trim().is_empty() {
        user.push_str(&format!("\n{instruction}"));
    }
    (
        vec![
            ChatMessage::text("system", system_prompt),
            ChatMessage::text("user", user),
        ],
        included,
    )
}

/// Collects a streamed completion into a single `String`, dropping the model's
/// `<think>` reasoning (reports have no streaming UI to show it). Mirrors
/// [`AnswerSidecar::run`]'s stream plumbing minus the [`AnswerDelta`] bridge: the
/// per-chunk idle timeout in [`SidecarClient::stream`] keeps a slow-but-progressing
/// map-reduce pass from spuriously timing out.
async fn collect_stream(
    client: &crate::client::SidecarClient,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
) -> Result<String> {
    let (ptx, mut prx) = mpsc::channel::<StreamPiece>(64);
    let c = client.clone();
    let task = tokio::spawn(async move { c.stream(messages, max_tokens, &ptx).await });
    let mut splitter = ThinkSplitter::default();
    let mut out = String::new();
    while let Some(piece) = prx.recv().await {
        match piece {
            StreamPiece::Reasoning(_) => {}
            StreamPiece::Content(text) => {
                for (is_thinking, chunk) in splitter.push(&text) {
                    if !is_thinking {
                        out.push_str(&chunk);
                    }
                }
            }
            StreamPiece::Done => break,
        }
    }
    if let Some((is_thinking, rest)) = splitter.flush() {
        if !is_thinking {
            out.push_str(&rest);
        }
    }
    task.await
        .unwrap_or_else(|e| Err(anyhow::anyhow!("report stream task panicked: {e}")))?;
    Ok(out)
}

/// The report footer's model provenance: the answer-lane GGUF filename
/// (`ModelSpec` has no display-name field). Falls back to `"answer-model"`.
fn report_model_label(gguf_path: &std::path::Path) -> String {
    gguf_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("answer-model")
        .to_string()
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

    fn chunk(frame_id: i64, text: &str) -> RetrievedChunk {
        RetrievedChunk {
            frame_id,
            text: text.to_string(),
            score: 1.0,
            captured_at: 0,
        }
    }

    #[test]
    fn builds_grounded_prompt_with_frame_tags() {
        let ctx = vec![chunk(7, "login page"), chunk(9, "dashboard")];
        let (msgs, cited) = build_messages("what did I see?", &ctx, 8192, 512);
        assert_eq!(msgs.len(), 2);
        // The user message must reference both frames for grounding.
        let user = serde_json::to_string(&msgs[1]).unwrap();
        assert!(user.contains("[frame 7]"));
        assert!(user.contains("[frame 9]"));
        assert!(user.contains("what did I see?"));
        assert_eq!(cited, vec![7, 9], "both frames fit the budget → both cited");
    }

    #[test]
    fn drops_chunks_that_exceed_the_context_budget() {
        // Many large chunks into a tiny ctx: only a prefix fits, and only those are cited —
        // this is the fix for the verified 400 `exceed_context_size_error`.
        let big = "lorem ipsum dolor sit amet ".repeat(50); // ~1350 chars ≈ 450 tokens
        let ctx: Vec<RetrievedChunk> = (0..20).map(|i| chunk(i, &big)).collect();
        let (msgs, cited) = build_messages("q", &ctx, 1024, 256);
        assert_eq!(msgs.len(), 2);
        assert!(!cited.is_empty(), "at least the top chunk is grounded");
        assert!(
            cited.len() < ctx.len(),
            "the budget must drop the chunks that don't fit (cited {})",
            cited.len()
        );
        // The included frames are exactly the leading ones (best-first order preserved).
        assert_eq!(cited, (0..cited.len() as i64).collect::<Vec<_>>());
    }

    #[test]
    fn truncates_an_oversized_top_chunk_instead_of_dropping_everything() {
        let huge = "x".repeat(100_000);
        let (msgs, cited) = build_messages("q", &[chunk(3, &huge)], 2048, 256);
        assert_eq!(
            cited,
            vec![3],
            "the sole chunk is still grounded (truncated)"
        );
        let user = serde_json::to_string(&msgs[1]).unwrap();
        assert!(
            user.len() < huge.len(),
            "the oversized chunk must be truncated"
        );
    }

    #[test]
    fn reply_budget_leaves_room_for_grounding_in_a_small_context() {
        // Ample window: the UI's 2048 is under half, so it passes through unchanged.
        assert_eq!(effective_reply_budget(2048, 8192), 2048);
        // Small window: capped to half so the prompt/context still has room.
        assert_eq!(effective_reply_budget(2048, 2048), 1024);
        assert!(effective_reply_budget(2048, 512) <= 256);
        // With the cap, a small ctx still grounds instead of dropping every chunk.
        let budget = effective_reply_budget(2048, 2048);
        let (_, cited) = build_messages("q", &[chunk(1, "hello world")], 2048, budget);
        assert_eq!(cited, vec![1], "grounding survives a small context window");
    }

    #[test]
    fn estimate_tokens_does_not_undercount_cjk() {
        // 40 CJK chars = 120 UTF-8 bytes, tokenizing ~1 token/char. A chars/3 ratio would
        // estimate ~14 and overflow the context; the byte ratio must stay >= the char count.
        let cjk = "你好世界".repeat(10);
        assert_eq!(cjk.chars().count(), 40);
        assert!(
            estimate_tokens(&cjk) >= cjk.chars().count(),
            "CJK estimate {} must not undercount {} chars",
            estimate_tokens(&cjk),
            cjk.chars().count()
        );
    }

    #[test]
    fn report_summary_messages_use_the_given_system_prompt_and_tag_frames() {
        let ctx = vec![
            chunk(11, "edited the report draft"),
            chunk(13, "ran the build"),
        ];
        let (msgs, cited) = build_summary_messages(
            "SUMMARIZE PROMPT",
            "Summarize Tuesday (Jun 24).",
            &ctx,
            8192,
            512,
        );
        assert_eq!(msgs.len(), 2);
        let system = serde_json::to_string(&msgs[0]).unwrap();
        assert!(
            system.contains("SUMMARIZE PROMPT"),
            "report system prompt, not Ask's"
        );
        let user = serde_json::to_string(&msgs[1]).unwrap();
        assert!(user.contains("[frame 11]") && user.contains("[frame 13]"));
        assert!(
            user.contains("Summarize Tuesday (Jun 24)."),
            "instruction appended"
        );
        assert_eq!(cited, vec![11, 13], "both frames fit → both cited");
    }

    #[test]
    fn report_summary_drops_overflow_and_cites_only_what_fit() {
        // A tiny ctx: only a prefix of frames fits, and only those are cited.
        let big = "lorem ipsum dolor sit amet ".repeat(40);
        let ctx: Vec<RetrievedChunk> = (0..20).map(|i| chunk(i, &big)).collect();
        let (_msgs, cited) = build_summary_messages("S", "label", &ctx, 1024, 256);
        assert!(!cited.is_empty() && cited.len() < ctx.len());
        assert_eq!(cited, (0..cited.len() as i64).collect::<Vec<_>>());
    }

    #[test]
    fn report_model_label_extracts_the_gguf_filename() {
        use std::path::Path;
        assert_eq!(
            report_model_label(Path::new("/models/answer/Ministral-3-3B-Q4_K_M.gguf")),
            "Ministral-3-3B-Q4_K_M.gguf"
        );
        assert_eq!(report_model_label(Path::new("")), "answer-model");
    }

    #[test]
    fn truncate_to_tokens_never_splits_a_multibyte_char() {
        // 3-byte chars; a byte budget that lands mid-character must snap back to a boundary
        // (a naive byte slice would panic).
        let cjk = "世".repeat(100);
        let out = truncate_to_tokens(&cjk, 10);
        assert!(cjk.starts_with(&out));
        assert!(!out.is_empty() && out.len() <= 10 * BYTES_PER_TOKEN);
        assert!(
            out.chars().all(|c| c == '世'),
            "no split / replacement chars"
        );
    }
}

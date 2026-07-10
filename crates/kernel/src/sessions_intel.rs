//! Lazy in-app session title/summary generation (D3). API/MCP reads never call this module.

use std::sync::Arc;

use traits::{AnswerOpts, AnswerProvider, RetrievedChunk, Session, Store};

const MAX_CONTEXT_FRAMES: usize = 24;

/// Generates title + summary in exactly one `summarize` call, caches both with model
/// provenance, and returns the cached row. A fully cached row performs no model call.
pub async fn generate_session_title_summary(
    store: Arc<dyn Store>,
    answer: Arc<dyn AnswerProvider>,
    session_id: i64,
) -> anyhow::Result<Session> {
    let current = store
        .get_session(session_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("session {session_id} not found"))?;
    if current.title.is_some() && current.summary.is_some() && current.summary_model.is_some() {
        return Ok(current);
    }

    let frames = store.session_frames_meta(session_id).await?;
    let sampled_ids = evenly_sample_ids(&frames, MAX_CONTEXT_FRAMES);
    let contents = store.content_texts_for_frames(&sampled_ids).await?;
    if contents.is_empty() {
        anyhow::bail!("session {session_id} has no filtered content_text to summarize");
    }
    let context: Vec<RetrievedChunk> = contents
        .into_iter()
        .map(|content| RetrievedChunk {
            frame_id: content.frame_id,
            text: content.content_text,
            score: 1.0,
            captured_at: content.captured_at,
        })
        .collect();
    let (generated, _) = answer
        .summarize(
            "Create a neutral recall label from the supplied ScreenSearch session frames. Do not invent facts.",
            "Return exactly two lines: `Title: <short title>` then `Summary: <concise factual summary>`.",
            &context,
            AnswerOpts {
                thinking: false,
                max_tokens: 384,
            },
        )
        .await?;
    let (title, summary) = parse_generated(&generated)?;
    let model = answer
        .answer_model_label()
        .await
        .ok_or_else(|| anyhow::anyhow!("answer provider omitted session summary provenance"))?;
    if !store
        .set_session_title_summary(session_id, &title, &summary, &model)
        .await?
    {
        anyhow::bail!("session {session_id} disappeared before its summary could be cached");
    }
    store
        .get_session(session_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("session {session_id} disappeared after summary cache"))
}

fn evenly_sample_ids(frames: &[traits::SegmenterFrame], limit: usize) -> Vec<i64> {
    if frames.len() <= limit {
        return frames.iter().map(|frame| frame.id).collect();
    }
    (0..limit)
        .map(|index| frames[index * frames.len() / limit].id)
        .collect()
}

fn parse_generated(value: &str) -> anyhow::Result<(String, String)> {
    let mut title = None;
    let mut summary = None;
    for line in value.lines() {
        if let Some(value) = line.trim().strip_prefix("Title:") {
            title = nonempty(value);
        } else if let Some(value) = line.trim().strip_prefix("Summary:") {
            summary = nonempty(value);
        }
    }
    match (title, summary) {
        (Some(title), Some(summary)) => Ok((title, summary)),
        _ => anyhow::bail!("answer model returned an invalid session title/summary shape"),
    }
}

fn nonempty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

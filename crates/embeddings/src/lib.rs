//! `embeddings` — [`EmbeddingProvider`] via fastembed (in-process ONNX, **no
//! Python**, `01 §5`, `03 §3`).
//!
//! Text: EmbeddingGemma-300M (768-dim, quantized) — *cannot batch*, so the impl
//! embeds one input at a time (`01 §6`, `MODEL_REGISTRY §3/§5`). Text is the only
//! embedding lane; the optional image lane was removed in 0.3.0 PR4
//! (`docs/0.3.0.md`, `MODEL_REGISTRY §3`).
//!
//! ## Concurrency
//! fastembed's `TextEmbedding` is a plain `Send` ONNX handle with no thread affinity
//! (unlike the COM-STA-bound WinRT OCR), but `embed` takes `&mut self` and blocks the
//! CPU. The lane is therefore an `Arc<Mutex<…>>` whose lock is taken *inside* a
//! [`tokio::task::spawn_blocking`] closure — never across an `.await` (same discipline
//! as `store::with_conn`). The model loads eagerly in [`FastEmbedProvider::new`] (call
//! it off the launch thread; first run downloads from HuggingFace into `cache_dir`).

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use traits::{Embedding, EmbeddingProvider};

/// Vector dimensionality the text model produces, and the `vec0` schema's `FLOAT[768]`
/// (`03 §4`). Mirrors `store::EMBEDDING_DIM` without a cross-crate dep (the store
/// validates length on upsert).
pub const EMBED_DIM: usize = 768;

/// Provenance label written to `embeddings.model` (`03 §4`).
const TEXT_MODEL_NAME: &str = "embeddinggemma-300m-q";

/// In-process fastembed provider wrapping the text embedding lane. Satisfies the
/// [`EmbeddingProvider`] contract (`03 §3`).
pub struct FastEmbedProvider {
    text: Arc<Mutex<TextEmbedding>>,
}

impl FastEmbedProvider {
    /// Eagerly loads the text model, downloading to `cache_dir` on first use.
    /// **Blocking** — invoke inside `spawn_blocking` so the launch thread isn't held on
    /// a multi-hundred-MB download (`03 §5`).
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        let text = TextEmbedding::try_new(
            TextInitOptions::new(EmbeddingModel::EmbeddingGemma300MQ)
                .with_cache_dir(cache_dir)
                .with_show_download_progress(false),
        )
        .map_err(|e| anyhow!("failed to load text embedding model: {e}"))?;

        tracing::info!("fastembed provider loaded");
        Ok(Self {
            text: Arc::new(Mutex::new(text)),
        })
    }
}

#[async_trait]
impl EmbeddingProvider for FastEmbedProvider {
    fn dim(&self) -> usize {
        EMBED_DIM
    }

    async fn embed_texts(&self, inputs: &[String]) -> Result<Vec<Embedding>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let model = self.text.clone();
        let inputs = inputs.to_vec();
        let vectors = tokio::task::spawn_blocking(move || -> Result<Vec<Vec<f32>>> {
            let mut guard = model
                .lock()
                .map_err(|_| anyhow!("text embed model lock poisoned"))?;
            let mut out = Vec::with_capacity(inputs.len());
            for text in &inputs {
                // One input per call: the quantized EmbeddingGemma model must not
                // batch (MODEL_REGISTRY §5). The hot path embeds a single query.
                let mut batch = guard
                    .embed(vec![text.as_str()], None)
                    .map_err(|e| anyhow!("text embedding failed: {e}"))?;
                out.push(
                    batch
                        .pop()
                        .ok_or_else(|| anyhow!("text embedder returned no vector"))?,
                );
            }
            Ok(out)
        })
        .await
        .map_err(|e| anyhow!("embed_texts task failed: {e}"))??;

        Ok(vectors.into_iter().map(Embedding).collect())
    }

    fn text_model_name(&self) -> &str {
        TEXT_MODEL_NAME
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vector dimension matches the `vec0` schema (`03 §4`) — pure, no model.
    #[test]
    fn embed_dim_is_768() {
        assert_eq!(EMBED_DIM, 768);
    }

    /// Loads the real EmbeddingGemma model and embeds text: 768-dim output,
    /// identical input ⇒ identical vector, different input ⇒ different vector.
    /// Downloads the model (hundreds of MB) — run locally:
    /// `cargo test -p embeddings -- --ignored`.
    #[tokio::test]
    #[ignore = "downloads the EmbeddingGemma model; run locally with --ignored"]
    async fn loads_and_embeds_text() {
        let cache = std::env::temp_dir().join(format!("ssv2c-embed-{}", std::process::id()));
        let provider = {
            let cache = cache.clone();
            tokio::task::spawn_blocking(move || FastEmbedProvider::new(cache))
                .await
                .unwrap()
                .expect("load text model")
        };

        assert_eq!(provider.dim(), 768);
        assert_eq!(provider.text_model_name(), TEXT_MODEL_NAME);

        let a = provider
            .embed_texts(&["hello world".to_string()])
            .await
            .unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].len(), 768);

        let b = provider
            .embed_texts(&["hello world".to_string()])
            .await
            .unwrap();
        assert_eq!(a[0].0, b[0].0, "identical input ⇒ identical vector");

        let c = provider
            .embed_texts(&["a completely unrelated sentence".to_string()])
            .await
            .unwrap();
        assert_ne!(a[0].0, c[0].0, "different input ⇒ different vector");

        let _ = std::fs::remove_dir_all(&cache);
    }
}

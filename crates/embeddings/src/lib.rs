//! `embeddings` — `EmbeddingProvider` via fastembed (in-process ONNX, **no
//! Python**, `01 §5`, `03 §3`).
//!
//! Text: EmbeddingGemma-300M (768-dim) — *cannot batch*, embed one input at a
//! time (`01 §6`, `MODEL_REGISTRY §3`). Image (optional): nomic-embed-vision-v1.5.
//!
//! **Status:** P0 scaffold — no implementation yet. Lands in **P3** (`02 §5`).

#![forbid(unsafe_code)]

/// The contract this crate implements (impl arrives in P3).
pub use traits::EmbeddingProvider;

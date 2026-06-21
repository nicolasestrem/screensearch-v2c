//! `store` — the durable data spine: frames, OCR, embeddings, hybrid retrieval,
//! the job queue, and settings, on SQLite (WAL) + sqlite-vec + FTS5 (`03 §4/§5`).
//!
//! **Status:** P0 scaffold — no implementation yet. The schema, migrations, and
//! the [`Store`] impl land in **P1** (`02 §5`). *Everything writes here*, so it is
//! built before any producer.

#![forbid(unsafe_code)]

/// The contract this crate implements (impl arrives in P1).
pub use traits::Store;

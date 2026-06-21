//! Text and image embeddings: metadata rows plus the synchronized sqlite-vec
//! (`vec0`) shadow tables, and the cosine-KNN building blocks the vector arm of
//! [`crate::SqliteStore::hybrid_search`] reuses (`03 §3/§4`).
//!
//! Each embedding lives in two places that must stay in lock-step: the metadata
//! table (`embeddings` / `image_embeddings`) and its `vec0` shadow keyed by the
//! same id. Upserts do both inside one transaction; deletes are handled by the
//! schema's `AFTER DELETE` triggers (incl. the `frames` cascade).

use anyhow::bail;
use rusqlite::{params, OptionalExtension};
use traits::{ChunkSource, Embedding, Result};

use crate::{SqliteStore, EMBEDDING_DIM};

/// Packs an f32 vector into the little-endian byte blob `vec0` stores/queries.
pub(crate) fn f32_blob(v: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(v.len() * 4);
    for f in v {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
    bytes
}

/// DB `source` token for an embedded chunk (`03 §4`).
fn source_token(source: ChunkSource) -> &'static str {
    match source {
        ChunkSource::Ocr => "ocr",
        ChunkSource::VisionDescription => "vision_description",
    }
}

/// De-duplicates frame ids preserving first-seen (nearest-first) order.
pub(crate) fn dedup_keep_order(ids: Vec<i64>) -> Vec<i64> {
    let mut seen = std::collections::HashSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

impl SqliteStore {
    /// Inserts or replaces the embedding for `(frame_id, chunk_index)` and its
    /// vec0 shadow, atomically (`03 §3/§4`). Errors if the vector's length is not
    /// [`EMBEDDING_DIM`].
    pub async fn upsert_text_embedding(
        &self,
        frame_id: i64,
        chunk_index: i32,
        chunk_text: &str,
        source: ChunkSource,
        emb: &Embedding,
        model: &str,
    ) -> Result<()> {
        if emb.len() != EMBEDDING_DIM {
            bail!(
                "text embedding has {} dims, expected {EMBEDDING_DIM}",
                emb.len()
            );
        }
        let blob = f32_blob(&emb.0);
        let content_hash = blake3::hash(chunk_text.as_bytes()).to_hex().to_string();
        let (chunk_text, model, source) = (
            chunk_text.to_string(),
            model.to_string(),
            source_token(source),
        );

        self.with_conn(move |conn| {
            let tx = conn.unchecked_transaction()?;
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT id FROM embeddings WHERE frame_id = ?1 AND chunk_index = ?2",
                    params![frame_id, chunk_index],
                    |r| r.get(0),
                )
                .optional()?;

            let id = match existing {
                Some(id) => {
                    tx.execute(
                        "UPDATE embeddings SET chunk_text = ?1, source = ?2, model = ?3,
                                              dim = ?4, content_hash = ?5 WHERE id = ?6",
                        params![
                            chunk_text,
                            source,
                            model,
                            EMBEDDING_DIM as i64,
                            content_hash,
                            id
                        ],
                    )?;
                    tx.execute(
                        "DELETE FROM embedding_vectors WHERE embedding_id = ?1",
                        params![id],
                    )?;
                    id
                }
                None => {
                    tx.execute(
                        "INSERT INTO embeddings
                           (frame_id, chunk_index, chunk_text, source, model, dim, content_hash)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            frame_id,
                            chunk_index,
                            chunk_text,
                            source,
                            model,
                            EMBEDDING_DIM as i64,
                            content_hash
                        ],
                    )?;
                    tx.last_insert_rowid()
                }
            };
            tx.execute(
                "INSERT INTO embedding_vectors (embedding_id, embedding) VALUES (?1, ?2)",
                params![id, blob],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    /// Inserts or replaces a frame's image embedding and its vec0 shadow (`03 §4`).
    pub async fn upsert_image_embedding(
        &self,
        frame_id: i64,
        emb: &Embedding,
        model: &str,
    ) -> Result<()> {
        if emb.len() != EMBEDDING_DIM {
            bail!(
                "image embedding has {} dims, expected {EMBEDDING_DIM}",
                emb.len()
            );
        }
        let blob = f32_blob(&emb.0);
        let model = model.to_string();

        self.with_conn(move |conn| {
            let tx = conn.unchecked_transaction()?;
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT id FROM image_embeddings WHERE frame_id = ?1",
                    params![frame_id],
                    |r| r.get(0),
                )
                .optional()?;
            let id = match existing {
                Some(id) => {
                    tx.execute(
                        "UPDATE image_embeddings SET model = ?1, dim = ?2 WHERE id = ?3",
                        params![model, EMBEDDING_DIM as i64, id],
                    )?;
                    tx.execute(
                        "DELETE FROM image_embedding_vectors WHERE image_embedding_id = ?1",
                        params![id],
                    )?;
                    id
                }
                None => {
                    tx.execute(
                        "INSERT INTO image_embeddings (frame_id, model, dim) VALUES (?1, ?2, ?3)",
                        params![frame_id, model, EMBEDDING_DIM as i64],
                    )?;
                    tx.last_insert_rowid()
                }
            };
            tx.execute(
                "INSERT INTO image_embedding_vectors (image_embedding_id, embedding)
                 VALUES (?1, ?2)",
                params![id, blob],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    /// Frame ids of the text chunks nearest `query` by cosine distance,
    /// nearest-first and de-duplicated by frame. Building block for the vector
    /// arm of hybrid search.
    pub async fn nearest_text_frames(&self, query: &Embedding, k: u32) -> Result<Vec<i64>> {
        self.knn_frames("embedding_vectors", "embeddings", "embedding_id", query, k)
            .await
    }

    /// Frame ids of the images nearest `query`, nearest-first (`03 §4`,
    /// optional visual recall).
    pub async fn nearest_image_frames(&self, query: &Embedding, k: u32) -> Result<Vec<i64>> {
        self.knn_frames(
            "image_embedding_vectors",
            "image_embeddings",
            "image_embedding_id",
            query,
            k,
        )
        .await
    }

    /// Shared cosine-KNN over a `(vec0 shadow, metadata)` pair. `id_col` is the
    /// shadow's primary-key column that equals `meta.id`.
    async fn knn_frames(
        &self,
        vec_table: &'static str,
        meta_table: &'static str,
        id_col: &'static str,
        query: &Embedding,
        k: u32,
    ) -> Result<Vec<i64>> {
        if query.len() != EMBEDDING_DIM {
            bail!(
                "query embedding has {} dims, expected {EMBEDDING_DIM}",
                query.len()
            );
        }
        let blob = f32_blob(&query.0);
        self.with_conn(move |conn| {
            // KNN runs in a subquery (the `k = ?` constraint must stand alone),
            // then joins the metadata table to resolve frame ids.
            let sql = format!(
                "SELECT m.frame_id FROM (
                     SELECT {id_col} AS vid, distance FROM {vec_table}
                     WHERE embedding MATCH ?1 AND k = ?2 ORDER BY distance
                 ) knn JOIN {meta_table} m ON m.id = knn.vid
                 ORDER BY knn.distance"
            );
            let mut stmt = conn.prepare(&sql)?;
            let ids = stmt
                .query_map(params![blob, k as i64], |r| r.get::<_, i64>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(dedup_keep_order(ids))
        })
        .await
    }

    /// Count of text-embedding rows (diagnostics / tests).
    pub async fn text_embedding_count(&self) -> Result<u64> {
        self.count_rows("embeddings").await
    }

    /// Count of image-embedding rows (diagnostics / tests).
    pub async fn image_embedding_count(&self) -> Result<u64> {
        self.count_rows("image_embeddings").await
    }

    async fn count_rows(&self, table: &'static str) -> Result<u64> {
        self.with_conn(move |conn| {
            let n: i64 =
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
            Ok(n as u64)
        })
        .await
    }

    /// Deletes a frame and everything that hangs off it (OCR/FTS, vision,
    /// embeddings + vec0 shadows, tags, jobs) via FK cascade + the vec-cleanup
    /// triggers. The retention-purge primitive (`storage.retention_days`).
    pub async fn delete_frame(&self, frame_id: i64) -> Result<()> {
        self.with_conn(move |conn| {
            conn.execute("DELETE FROM frames WHERE id = ?1", params![frame_id])?;
            Ok(())
        })
        .await
    }
}

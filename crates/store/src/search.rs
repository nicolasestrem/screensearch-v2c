//! Hybrid retrieval: an FTS5 (BM25) arm and a sqlite-vec (cosine KNN) arm fused
//! with Reciprocal Rank Fusion (`02 §4`, `03 §4/§13`).
//!
//! The vector arm is only active when an `Arc<dyn EmbeddingProvider>` was injected
//! (`SqliteStore::with_embedder`); otherwise search degrades to FTS-only. The query
//! text is embedded once (async) up front, then both arms run as DB queries.

use std::collections::HashMap;

use rusqlite::{params, params_from_iter};
use traits::{Result, SearchHit, SearchQuery};

use crate::embeddings::{dedup_keep_order, f32_blob};
use crate::SqliteStore;

/// RRF damping constant (the conventional value). A larger `k` flattens the
/// contribution of top ranks; 60 is the de-facto standard.
const RRF_K: f64 = 60.0;
/// Backend ceiling for one search response, matching the Recall UI's current max.
const MAX_SEARCH_LIMIT: usize = 100;
const MAX_CANDIDATE_POOL: usize = MAX_SEARCH_LIMIT * 5;

fn normalized_limit(limit: u32) -> usize {
    (limit as usize).clamp(1, MAX_SEARCH_LIMIT)
}

/// Per-arm candidate pool. We over-fetch beyond `limit` so fusion (and the vector
/// arm's time-range post-filter) have material to work with.
fn candidate_pool(limit: usize) -> u32 {
    limit.saturating_mul(5).clamp(50, MAX_CANDIDATE_POOL) as u32
}

/// Builds a safe FTS5 MATCH expression from free user text: each whitespace term
/// is quoted (so FTS operators in the input can't inject), terms AND together.
/// Returns `None` for blank input.
fn fts_match_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

/// First ~160 chars of `text` on a char boundary, with an ellipsis if truncated.
/// Fallback snippet for vector-only hits (which have no FTS highlight).
fn truncate_snippet(text: &str) -> String {
    const MAX: usize = 160;
    if text.chars().count() <= MAX {
        return text.to_string();
    }
    let mut s: String = text.chars().take(MAX).collect();
    s.push('…');
    s
}

/// Reciprocal Rank Fusion over per-arm ranked id lists. Returns ids with fused
/// scores, highest first; ties break toward the newer (larger) id.
fn rrf_fuse(arms: &[Vec<i64>], limit: usize) -> Vec<(i64, f64)> {
    let mut scores: HashMap<i64, f64> = HashMap::new();
    for arm in arms {
        for (rank, &id) in arm.iter().enumerate() {
            *scores.entry(id).or_insert(0.0) += 1.0 / (RRF_K + (rank + 1) as f64);
        }
    }
    let mut fused: Vec<(i64, f64)> = scores.into_iter().collect();
    fused.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.0.cmp(&a.0))
    });
    fused.truncate(limit);
    fused
}

impl SqliteStore {
    /// Hybrid search over OCR text + (optional) text embeddings, fused via RRF
    /// (`03 §3/§13`).
    pub async fn hybrid_search(&self, q: &SearchQuery) -> Result<Vec<SearchHit>> {
        let limit = normalized_limit(q.limit);
        let pool = candidate_pool(limit);
        // Half-open `[start, end)` per the `TimeRange` contract: both arms filter
        // `captured_at >= start AND captured_at < end`. No range → the full line.
        let (start, end) = q
            .time_range
            .map(|t| (t.start, t.end))
            .unwrap_or((i64::MIN, i64::MAX));

        // --- content FTS arm (default retrieval text; carries the highlighted snippets) ---
        let fts = self
            .fts_arm("frame_text_fts", &q.text, start, end, pool)
            .await?;
        let fts_ids: Vec<i64> = fts.iter().map(|(id, _)| *id).collect();
        let mut snippets: HashMap<i64, String> = fts.into_iter().collect();

        // --- raw FTS arm (only when the caller opts into chrome/raw text, `03 §3b`) ---
        // Searches `frame_text.raw_text` so static chrome the content filter drops is
        // still reachable. The content snippet wins; raw snippets fill only ids the
        // content arm didn't match.
        let raw_ids: Vec<i64> = if q.include_chrome {
            let raw = self
                .fts_arm("frame_text_raw_fts", &q.text, start, end, pool)
                .await?;
            let ids: Vec<i64> = raw.iter().map(|(id, _)| *id).collect();
            for (id, snip) in raw {
                snippets.entry(id).or_insert(snip);
            }
            ids
        } else {
            Vec::new()
        };

        // --- vector arm (only when an embedder is present and the query is non-blank) ---
        // Clone the Arc out from under the lock first — the read guard must never be
        // held across the `.await` on `embed_texts`.
        let embedder = self
            .embedder
            .read()
            .expect("store embedder lock poisoned")
            .clone();
        let vec_ids = match (embedder, q.text.trim().is_empty()) {
            (Some(embedder), false) => {
                let mut embs = embedder.embed_texts(std::slice::from_ref(&q.text)).await?;
                let query_emb = embs
                    .pop()
                    .ok_or_else(|| anyhow::anyhow!("embedder returned no vector for the query"))?;
                self.text_knn_in_range(query_emb.0, pool, start, end)
                    .await?
            }
            _ => Vec::new(),
        };

        // Fuse the active arms (the raw arm only participates when opted in).
        let mut arms: Vec<Vec<i64>> = vec![fts_ids];
        if q.include_chrome {
            arms.push(raw_ids);
        }
        arms.push(vec_ids);
        let fused = rrf_fuse(&arms, limit);
        self.hydrate(fused, snippets).await
    }

    /// BM25-ranked FTS hits within the time window, with highlighted snippets, over
    /// the given external-content FTS5 table (`frame_text_fts` for content text,
    /// `frame_text_raw_fts` for raw text). `table` is a fixed internal identifier, not
    /// user input, so interpolating it is injection-safe.
    async fn fts_arm(
        &self,
        table: &str,
        text: &str,
        start: i64,
        end: i64,
        pool: u32,
    ) -> Result<Vec<(i64, String)>> {
        let Some(match_q) = fts_match_query(text) else {
            return Ok(Vec::new());
        };
        let sql = format!(
            "SELECT fts.rowid,
                    snippet({table}, 0, '[', ']', '…', 12) AS snip,
                    bm25({table}) AS rank
             FROM {table} fts
             JOIN frames fr ON fr.id = fts.rowid
             WHERE {table} MATCH ?1 AND fr.captured_at >= ?2 AND fr.captured_at < ?3
             ORDER BY rank
             LIMIT ?4"
        );
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![match_q, start, end, pool as i64], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .await
    }

    /// Text-embedding cosine KNN, nearest-first, de-duped by frame, restricted to
    /// the time window. (vec0 can't filter inside MATCH, so we over-fetch `pool`
    /// vectors and post-filter on the join.)
    async fn text_knn_in_range(
        &self,
        query: Vec<f32>,
        pool: u32,
        start: i64,
        end: i64,
    ) -> Result<Vec<i64>> {
        let blob = f32_blob(&query);
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT m.frame_id FROM (
                     SELECT embedding_id AS vid, distance FROM embedding_vectors
                     WHERE embedding MATCH ?1 AND k = ?2 ORDER BY distance
                 ) knn
                 JOIN embeddings m ON m.id = knn.vid
                 JOIN frames fr ON fr.id = m.frame_id
                 WHERE fr.captured_at >= ?3 AND fr.captured_at < ?4
                 ORDER BY knn.distance",
            )?;
            let ids = stmt
                .query_map(params![blob, pool as i64, start, end], |r| {
                    r.get::<_, i64>(0)
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(dedup_keep_order(ids))
        })
        .await
    }

    /// Resolves fused `(frame_id, score)` rows into [`SearchHit`]s, preferring the
    /// FTS snippet and falling back to a truncated OCR-text snippet. Frame context
    /// and fallback snippets are fetched in two bulk `IN` queries (not per-row), so
    /// hydration is at most two round-trips regardless of result count.
    async fn hydrate(
        &self,
        fused: Vec<(i64, f64)>,
        snippets: HashMap<i64, String>,
    ) -> Result<Vec<SearchHit>> {
        if fused.is_empty() {
            return Ok(Vec::new());
        }
        self.with_conn(move |conn| {
            let ids: Vec<i64> = fused.iter().map(|(id, _)| *id).collect();

            // bulk-fetch frame context for every candidate
            let frames_sql = format!(
                "SELECT id, captured_at, image_path, app_hint FROM frames WHERE id IN ({})",
                placeholders(ids.len())
            );
            let frames: HashMap<i64, (i64, String, Option<String>)> = conn
                .prepare(&frames_sql)?
                .query_map(params_from_iter(ids.iter()), |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        (
                            r.get::<_, i64>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, Option<String>>(3)?,
                        ),
                    ))
                })?
                .collect::<rusqlite::Result<_>>()?;

            // bulk-fetch content text only for hits lacking an FTS snippet (the fallback)
            let need_text: Vec<i64> = ids
                .iter()
                .copied()
                .filter(|id| !snippets.contains_key(id))
                .collect();
            let texts: HashMap<i64, String> = if need_text.is_empty() {
                HashMap::new()
            } else {
                let ocr_sql = format!(
                    "SELECT frame_id, content_text FROM frame_text WHERE frame_id IN ({})",
                    placeholders(need_text.len())
                );
                conn.prepare(&ocr_sql)?
                    .query_map(params_from_iter(need_text.iter()), |r| {
                        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                    })?
                    .collect::<rusqlite::Result<_>>()?
            };

            // assemble in fused (RRF) order; skip frames that vanished between arms
            let mut hits = Vec::with_capacity(fused.len());
            for (frame_id, score) in fused {
                let Some((captured_at, image_path, app_hint)) = frames.get(&frame_id) else {
                    continue;
                };
                let snippet = match snippets.get(&frame_id) {
                    Some(s) => s.clone(),
                    None => texts
                        .get(&frame_id)
                        .map(|t| truncate_snippet(t))
                        .unwrap_or_default(),
                };
                hits.push(SearchHit {
                    frame_id,
                    captured_at: *captured_at,
                    snippet,
                    score: score as f32,
                    image_path: image_path.clone(),
                    app_hint: app_hint.clone(),
                });
            }
            Ok(hits)
        })
        .await
    }
}

/// `?,?,…,?` — a comma-joined run of `n` positional placeholders for an `IN (…)`.
fn placeholders(n: usize) -> String {
    let mut s = String::with_capacity(n * 2);
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        s.push('?');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use traits::{NewFrame, OcrResult};

    fn frame(at: i64) -> NewFrame {
        NewFrame {
            captured_at: at,
            monitor_index: 0,
            width: 1920,
            height: 1080,
            image_path: format!("frames/{at}.jpg"),
            content_hash: format!("h{at}"),
            app_hint: None,
            window_title: None,
            browser_url: None,
        }
    }

    fn q(text: &str, include_chrome: bool) -> SearchQuery {
        SearchQuery {
            text: text.to_string(),
            limit: 10,
            time_range: None,
            include_chrome,
        }
    }

    /// `include_chrome` searches `raw_text` via the raw FTS arm, independent of
    /// `content_text` (`03 §3b`). PR2's populator writes content == raw, so we widen
    /// `raw_text` directly to simulate PR3's filter dropping a term to the chrome
    /// layer: the term is then reachable only with `include_chrome = true`, while
    /// content text stays searchable in both modes.
    #[tokio::test]
    async fn include_chrome_searches_raw_text_independently_of_content() {
        let store = crate::SqliteStore::open_in_memory().unwrap();
        let fid = store.insert_frame(frame(1_000)).await.unwrap();
        store
            .insert_ocr(
                fid,
                OcrResult {
                    text: "alpha".to_string(),
                    mean_confidence: -1.0,
                    engine: "test".to_string(),
                    spans: Vec::new(),
                },
            )
            .await
            .unwrap();
        // Diverge raw from content: "bravo" now lives only in raw_text. The raw FTS
        // update trigger keeps frame_text_raw_fts in sync.
        store
            .with_conn(move |conn| {
                conn.execute(
                    "UPDATE frame_text SET raw_text = ?2 WHERE frame_id = ?1",
                    params![fid, "alpha bravo"],
                )?;
                Ok(())
            })
            .await
            .unwrap();

        // Default (content only): "bravo" is not in content_text → no hit.
        assert!(store
            .hybrid_search(&q("bravo", false))
            .await
            .unwrap()
            .is_empty());
        // include_chrome: the raw arm finds it.
        let hits = store.hybrid_search(&q("bravo", true)).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].frame_id, fid);
        // content text stays searchable regardless of the flag.
        assert_eq!(
            store.hybrid_search(&q("alpha", false)).await.unwrap().len(),
            1
        );
        assert_eq!(
            store.hybrid_search(&q("alpha", true)).await.unwrap().len(),
            1
        );
    }
}

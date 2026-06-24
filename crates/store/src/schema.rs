//! Forward-only schema migrations (`03 §4/§12`).
//!
//! The authoritative DDL lives here. Migrations are an append-only list of
//! `(version, sql)` pairs applied in order; the current version is tracked in the
//! `schema_version` table (created by the bootstrap in [`crate`], not by a
//! migration). To evolve the schema, **append** a new `(n, "...")` entry — never
//! edit a shipped one (no schema drift).

/// The highest migration version this build knows how to reach.
pub const LATEST_SCHEMA_VERSION: i32 = 2;

/// Vector dimensionality for every embedding lane (`03 §3/§4`,
/// [`traits::EmbeddingProvider::dim`]).
pub const EMBEDDING_DIM: usize = 768;

/// Ordered, forward-only migrations. Each is applied in its own transaction when
/// the DB's tracked version is below it.
pub const MIGRATIONS: &[(i32, &str)] = &[(1, MIGRATION_V1), (2, MIGRATION_V2)];

/// v1 — the full data spine (`03 §4`, transcribed verbatim, plus the FTS5 and
/// vector-sync triggers the spec describes in prose).
const MIGRATION_V1: &str = r#"
-- frames: one row per stored (changed) capture
CREATE TABLE frames (
  id            INTEGER PRIMARY KEY,
  captured_at   INTEGER NOT NULL,          -- unix ms
  monitor_index INTEGER NOT NULL,
  width         INTEGER NOT NULL,
  height        INTEGER NOT NULL,
  image_path    TEXT    NOT NULL,          -- relative path to JPEG on disk
  content_hash  TEXT    NOT NULL,
  app_hint      TEXT, window_title TEXT, browser_url TEXT,  -- context (nullable)
  activity_type TEXT,                       -- filled by vision (nullable)
  created_at    INTEGER NOT NULL DEFAULT (unixepoch()*1000)
);
CREATE INDEX idx_frames_captured_at ON frames(captured_at);

-- OCR text (one row per frame) + FTS5 mirror
CREATE TABLE ocr_text (
  frame_id        INTEGER PRIMARY KEY REFERENCES frames(id) ON DELETE CASCADE,
  text            TEXT NOT NULL,
  mean_confidence REAL NOT NULL,
  engine          TEXT NOT NULL
);
CREATE VIRTUAL TABLE ocr_text_fts USING fts5(text, content='ocr_text', content_rowid='frame_id',
                                             tokenize='porter');
-- external-content sync triggers (standard FTS5 pattern)
CREATE TRIGGER ocr_text_ai AFTER INSERT ON ocr_text BEGIN
  INSERT INTO ocr_text_fts(rowid, text) VALUES (new.frame_id, new.text);
END;
CREATE TRIGGER ocr_text_ad AFTER DELETE ON ocr_text BEGIN
  INSERT INTO ocr_text_fts(ocr_text_fts, rowid, text) VALUES('delete', old.frame_id, old.text);
END;
CREATE TRIGGER ocr_text_au AFTER UPDATE ON ocr_text BEGIN
  INSERT INTO ocr_text_fts(ocr_text_fts, rowid, text) VALUES('delete', old.frame_id, old.text);
  INSERT INTO ocr_text_fts(rowid, text) VALUES (new.frame_id, new.text);
END;

-- vision analysis (deferred, optional, one row per analyzed frame)
CREATE TABLE vision_analysis (
  frame_id     INTEGER PRIMARY KEY REFERENCES frames(id) ON DELETE CASCADE,
  description  TEXT NOT NULL, activity_type TEXT, app_hint TEXT,
  confidence   REAL NOT NULL, model TEXT NOT NULL,
  created_at   INTEGER NOT NULL DEFAULT (unixepoch()*1000)
);

-- text embeddings: metadata + sqlite-vec index
CREATE TABLE embeddings (
  id           INTEGER PRIMARY KEY,
  frame_id     INTEGER NOT NULL REFERENCES frames(id) ON DELETE CASCADE,
  chunk_index  INTEGER NOT NULL,
  chunk_text   TEXT NOT NULL,
  source       TEXT NOT NULL,               -- 'ocr' | 'vision_description'
  model        TEXT NOT NULL, dim INTEGER NOT NULL,
  content_hash TEXT NOT NULL,               -- skip re-embed if unchanged
  UNIQUE(frame_id, chunk_index)
);
CREATE INDEX idx_embeddings_frame ON embeddings(frame_id);
CREATE VIRTUAL TABLE embedding_vectors USING vec0(
  embedding_id INTEGER PRIMARY KEY,         -- == embeddings.id
  embedding    FLOAT[768] distance_metric=cosine
);
-- keep the vec0 shadow in sync when an embeddings row is removed (incl. via the
-- frames ON DELETE CASCADE; recursive_triggers is enabled per-connection)
CREATE TRIGGER embeddings_ad AFTER DELETE ON embeddings BEGIN
  DELETE FROM embedding_vectors WHERE embedding_id = old.id;
END;

-- image embeddings (optional visual recall): metadata + sqlite-vec index
CREATE TABLE image_embeddings (
  id        INTEGER PRIMARY KEY,
  frame_id  INTEGER NOT NULL REFERENCES frames(id) ON DELETE CASCADE,
  model     TEXT NOT NULL, dim INTEGER NOT NULL,
  UNIQUE(frame_id)
);
CREATE VIRTUAL TABLE image_embedding_vectors USING vec0(
  image_embedding_id INTEGER PRIMARY KEY,   -- == image_embeddings.id
  embedding          FLOAT[768] distance_metric=cosine
);
CREATE TRIGGER image_embeddings_ad AFTER DELETE ON image_embeddings BEGIN
  DELETE FROM image_embedding_vectors WHERE image_embedding_id = old.id;
END;

-- durable job queue (the heart of enrich-deferred) — see 03 §5
CREATE TABLE jobs (
  id           INTEGER PRIMARY KEY,
  kind         TEXT NOT NULL,               -- 'embed_text' | 'embed_image' | 'vision_tag'
  frame_id     INTEGER REFERENCES frames(id) ON DELETE CASCADE,
  state        TEXT NOT NULL DEFAULT 'pending', -- pending|running|done|failed|dead
  priority     INTEGER NOT NULL DEFAULT 0,  -- higher first
  attempts     INTEGER NOT NULL DEFAULT 0,
  max_attempts INTEGER NOT NULL DEFAULT 3,
  not_before   INTEGER NOT NULL DEFAULT 0,  -- unix ms (scheduling + backoff)
  last_error   TEXT,
  created_at   INTEGER NOT NULL DEFAULT (unixepoch()*1000),
  updated_at   INTEGER NOT NULL DEFAULT (unixepoch()*1000)
);
CREATE INDEX idx_jobs_ready ON jobs(state, not_before, priority DESC, id);

-- tagging, settings
CREATE TABLE tags (id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL);
CREATE TABLE frame_tags (frame_id INTEGER REFERENCES frames(id) ON DELETE CASCADE,
                         tag_id INTEGER REFERENCES tags(id) ON DELETE CASCADE,
                         PRIMARY KEY(frame_id, tag_id));
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
"#;

/// v2 — index supporting the vision scheduler's pending-job dedup. The
/// `untagged_frame_ids` query runs a correlated `NOT EXISTS` over `jobs` keyed by
/// `frame_id` (+ `kind`/`state`) for every candidate frame; without this the subquery
/// scans the whole `jobs` table per candidate, which degrades as completed/dead rows
/// accumulate (no purge yet). Index-only, no data change.
const MIGRATION_V2: &str = r#"
CREATE INDEX IF NOT EXISTS idx_jobs_frame_kind_state ON jobs(frame_id, kind, state);
"#;

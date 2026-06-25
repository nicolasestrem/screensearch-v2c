//! Forward-only schema migrations (`03 §4/§12`).
//!
//! The authoritative DDL lives here. Migrations are an append-only list of
//! `(version, sql)` pairs applied in order; the current version is tracked in the
//! `schema_version` table (created by the bootstrap in [`crate`], not by a
//! migration). To evolve the schema, **append** a new `(n, "...")` entry — never
//! edit a shipped one (no schema drift).

/// The highest migration version this build knows how to reach.
pub const LATEST_SCHEMA_VERSION: i32 = 4;

/// Vector dimensionality for every embedding lane (`03 §3/§4`,
/// [`traits::EmbeddingProvider::dim`]).
pub const EMBEDDING_DIM: usize = 768;

/// `frame_text.filter_version` written by PR2's interim passthrough populator
/// (`07` #51): `0` marks "no attention filter applied — `content_text` is a raw copy".
/// PR3's classifier writes [`FILTER_VERSION`] and is bumpable to recompute the chrome
/// catalog (`03 §3b`).
pub const UNFILTERED_FILTER_VERSION: i32 = 0;

/// `frame_text.filter_version` written by PR3's attention filter. Bumping this is the
/// "recompute the chrome catalog" lever (`03 §3b`): on startup the store wipes
/// `chrome_text_catalog` when the active version changes so signatures rebuild from new
/// captures. No backfill — old frames keep their old `content_text`/version (clean-DB,
/// `07` #51/#52).
pub const FILTER_VERSION: i32 = 1;

/// Ordered, forward-only migrations. Each is applied in its own transaction when
/// the DB's tracked version is below it.
pub const MIGRATIONS: &[(i32, &str)] = &[
    (1, MIGRATION_V1),
    (2, MIGRATION_V2),
    (3, MIGRATION_V3),
    (4, MIGRATION_V4),
];

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

/// v3 — 0.2.x text signal (`03 §3b`/`§4`, `docs/0.2.0.md` PR2). Splits the single
/// unfiltered OCR string into a preserved `raw_text` layer and a filtered
/// `content_text` layer, adds per-word `text_spans` (normalized geometry + role) and
/// the `chrome_text_catalog` PR3's static-chrome suppression will drive.
///
/// Clean-DB assumption (`07` #51): `ocr_text` is empty in every install reaching v3,
/// so it is dropped — `frame_text.raw_text` becomes the single raw store
/// (`03 §4` "legacy ocr_text … not required going forward"). The `DROP … IF EXISTS`
/// form keeps the migration idempotent. `include_chrome=true` searches raw via a
/// dedicated raw FTS5 mirror (`frame_text_raw_fts`); roles aren't populated until PR3,
/// so a role-filtered spans FTS would be meaningless now (`05`).
///
/// FTS5 + trigger style mirrors v1's `ocr_text_fts` exactly: external-content over
/// `frame_text` keyed on `frame_id`, kept in sync by AFTER INSERT/DELETE/UPDATE
/// triggers (the `'delete'` command form purges the old row).
const MIGRATION_V3: &str = r#"
-- clean-DB: retire the legacy single-string OCR store (frame_text replaces it)
DROP TRIGGER IF EXISTS ocr_text_au;
DROP TRIGGER IF EXISTS ocr_text_ad;
DROP TRIGGER IF EXISTS ocr_text_ai;
DROP TABLE   IF EXISTS ocr_text_fts;
DROP TABLE   IF EXISTS ocr_text;

-- frame_text: preserved raw text + filtered default-retrieval text, one row per frame
CREATE TABLE frame_text (
  frame_id            INTEGER PRIMARY KEY REFERENCES frames(id) ON DELETE CASCADE,
  raw_text            TEXT    NOT NULL,          -- full unfiltered OCR/UIA text (preserved)
  content_text        TEXT    NOT NULL,          -- filtered text (NOT vision); default retrieval input
  primary_source      TEXT    NOT NULL CHECK (primary_source IN ('ocr','uia')),
  filter_version      INTEGER NOT NULL,          -- bump to recompute the chrome catalog
  suppressed_count    INTEGER NOT NULL,          -- spans dropped from content_text (suppression-rate metric)
  target_window_title TEXT,                      -- foreground window title (metadata, nullable)
  target_app_hint     TEXT,                      -- foreground app hint (metadata, nullable)
  created_at          INTEGER NOT NULL DEFAULT (unixepoch()*1000)
);

-- default search FTS mirrors content_text (porter), external-content over frame_text
CREATE VIRTUAL TABLE frame_text_fts USING fts5(content_text, content='frame_text',
                                               content_rowid='frame_id', tokenize='porter');
CREATE TRIGGER frame_text_ai AFTER INSERT ON frame_text BEGIN
  INSERT INTO frame_text_fts(rowid, content_text) VALUES (new.frame_id, new.content_text);
END;
CREATE TRIGGER frame_text_ad AFTER DELETE ON frame_text BEGIN
  INSERT INTO frame_text_fts(frame_text_fts, rowid, content_text) VALUES('delete', old.frame_id, old.content_text);
END;
CREATE TRIGGER frame_text_au AFTER UPDATE ON frame_text BEGIN
  INSERT INTO frame_text_fts(frame_text_fts, rowid, content_text) VALUES('delete', old.frame_id, old.content_text);
  INSERT INTO frame_text_fts(rowid, content_text) VALUES (new.frame_id, new.content_text);
END;

-- raw FTS mirror (porter) — searched only when include_chrome=true (03 §3b/§4)
CREATE VIRTUAL TABLE frame_text_raw_fts USING fts5(raw_text, content='frame_text',
                                                   content_rowid='frame_id', tokenize='porter');
CREATE TRIGGER frame_text_raw_ai AFTER INSERT ON frame_text BEGIN
  INSERT INTO frame_text_raw_fts(rowid, raw_text) VALUES (new.frame_id, new.raw_text);
END;
CREATE TRIGGER frame_text_raw_ad AFTER DELETE ON frame_text BEGIN
  INSERT INTO frame_text_raw_fts(frame_text_raw_fts, rowid, raw_text) VALUES('delete', old.frame_id, old.raw_text);
END;
CREATE TRIGGER frame_text_raw_au AFTER UPDATE ON frame_text BEGIN
  INSERT INTO frame_text_raw_fts(frame_text_raw_fts, rowid, raw_text) VALUES('delete', old.frame_id, old.raw_text);
  INSERT INTO frame_text_raw_fts(rowid, raw_text) VALUES (new.frame_id, new.raw_text);
END;

-- text_spans: per-frame OCR/UIA spans with normalized geometry + classified role
CREATE TABLE text_spans (
  frame_id        INTEGER NOT NULL REFERENCES frames(id) ON DELETE CASCADE,
  span_index      INTEGER NOT NULL,
  text            TEXT    NOT NULL,
  normalized_text TEXT    NOT NULL,
  source          TEXT    NOT NULL CHECK (source IN ('ocr','uia')),
  role            TEXT    NOT NULL CHECK (role IN ('content','chrome','background','system','unknown')),
  x REAL NOT NULL, y REAL NOT NULL, w REAL NOT NULL, h REAL NOT NULL,  -- normalized [0,1] bbox
  is_searchable   INTEGER NOT NULL CHECK (is_searchable IN (0,1)),
  suppress_reason TEXT CHECK (suppress_reason IS NULL
                              OR suppress_reason IN ('static_chrome','system_ui','background_window')),
  PRIMARY KEY (frame_id, span_index)
);

-- chrome_text_catalog: signature counter that drives static-chrome suppression (PR3)
CREATE TABLE chrome_text_catalog (
  signature       TEXT PRIMARY KEY,              -- app_hint + normalized_text + region_bucket
  app_hint        TEXT,
  region_bucket   TEXT,
  normalized_text TEXT    NOT NULL,
  seen_count      INTEGER NOT NULL,
  first_seen_at   INTEGER NOT NULL,
  last_seen_at    INTEGER NOT NULL,
  suppressed      INTEGER NOT NULL DEFAULT 0 CHECK (suppressed IN (0,1))  -- 0/1; marked chrome after a configurable threshold (§8)
);
"#;

/// v4 — PR3 attention filter (`03 §3b`, `docs/0.2.0.md` PR3). Carries the OCR
/// `line_index` onto each span so the classifier groups words into lines exactly
/// (the engine already computed line boundaries; PR2 flattened spans to words and
/// dropped them). Clean-DB: `text_spans` is empty at v4, so the `DEFAULT 0` only
/// shapes the column, not existing rows. Index-light: the `(frame_id, span_index)`
/// PK already covers the per-frame read PR3's filter does.
const MIGRATION_V4: &str = r#"
ALTER TABLE text_spans ADD COLUMN line_index INTEGER NOT NULL DEFAULT 0;
"#;

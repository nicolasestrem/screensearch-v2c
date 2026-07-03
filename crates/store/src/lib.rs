//! `store` — the durable data spine: frames, OCR, vision, embeddings, hybrid
//! retrieval, the job queue, and settings, on SQLite (WAL) + sqlite-vec + FTS5
//! (`03 §4/§5`). *Everything writes here*, so it is built before any producer.
//!
//! ## Concurrency model
//! One [`rusqlite::Connection`] is kept for the store's lifetime behind a
//! [`std::sync::Mutex`]; every async [`traits::Store`] method runs its SQL inside
//! [`tokio::task::spawn_blocking`]. SQLite allows a single writer, so serializing
//! through one connection is correct and simple, and an in-memory DB persists for
//! the store's lifetime (clean `:memory:` tests, `03 §10`). The Mutex guard is
//! only ever held *inside* a blocking task — never across an `.await`.
//!
//! ## Retrieval & the embedder
//! [`SqliteStore::hybrid_search`] fuses an FTS5 arm with a sqlite-vec KNN arm via
//! RRF. The KNN arm needs the *query* embedded, but the store must not depend on
//! the embeddings impl — so it optionally holds an `Arc<dyn `[`EmbeddingProvider`]`>`
//! (a trait; modularity holds). With no embedder injected the search degrades to
//! FTS-only; P3 injects the real fastembed provider. (`07` engineering note.)

use std::path::Path;
use std::sync::{Arc, Mutex, Once, RwLock};

use async_trait::async_trait;
use rusqlite::Connection;
use traits::{
    AppSuppression, ChunkSource, Embedding, EmbeddingProvider, FrameEnrichmentInput, FrameMeta,
    InsightsSummary, Job, JobKind, JobStats, NewFrame, NewJob, OcrResult, Result, SearchHit,
    SearchQuery, TextFilterContext, TimelineBucket, VisionAnalysis,
};

mod embeddings;
mod frames;
mod insights;
mod jobs;
mod records;
mod schema;
mod search;
mod settings;
mod timeline;

pub use schema::{EMBEDDING_DIM, FILTER_VERSION, LATEST_SCHEMA_VERSION};
/// Re-export of the contract this crate implements (`03 §3`).
pub use traits::Store;

/// The durable data spine over SQLite + sqlite-vec + FTS5.
///
/// Cheap to [`Clone`] (it is a pair of `Arc`s); clones share the same underlying
/// connection and embedder.
#[derive(Clone)]
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
    /// Runtime-settable (`03 §5`): the composition root attaches the embedder *after*
    /// the model finishes loading off the launch thread. Behind an `Arc<RwLock>` so
    /// every clone shares it — injecting on one handle lights up every clone's vector
    /// arm. `None` ⇒ search degrades to FTS-only.
    embedder: Arc<RwLock<Option<Arc<dyn EmbeddingProvider>>>>,
}

/// Registers the sqlite-vec (`vec0`) loadable extension exactly once for the
/// process. `sqlite3_auto_extension` applies to every connection opened *after*
/// the call, so this must run before any [`Connection::open`].
fn register_vec_extension() {
    static VEC_INIT: Once = Once::new();
    VEC_INIT.call_once(|| {
        // The auto-extension entry point has the C ABI sqlite expects; sqlite-vec
        // exposes it as `sqlite3_vec_init`. The transmute is the documented
        // registration pattern (annotated to satisfy `missing_transmute_annotations`).
        type ExtInit = unsafe extern "C" fn(
            *mut rusqlite::ffi::sqlite3,
            *mut *mut std::os::raw::c_char,
            *const rusqlite::ffi::sqlite3_api_routines,
        ) -> std::os::raw::c_int;
        // SAFETY: `sqlite3_vec_init` is a valid C function with the signature
        // `sqlite3_auto_extension` expects; we register it once via the `Once`.
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<*const (), ExtInit>(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    });
}

/// Opens a connection, applies the connection-scoped pragmas, and runs migrations.
fn open_connection(conn: Connection) -> Result<Connection> {
    // WAL is a no-op on `:memory:` (returns "memory"); foreign_keys and
    // recursive_triggers are per-connection and must be set every open so the
    // cascade + vec-cleanup triggers fire (`03 §4`).
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA foreign_keys=ON;
         PRAGMA recursive_triggers=ON;
         PRAGMA busy_timeout=5000;",
    )?;
    let mut conn = conn;
    bootstrap_and_migrate(&mut conn)?;
    Ok(conn)
}

/// Ensures `schema_version` exists, then applies every pending migration in its
/// own transaction, advancing the tracked version forward-only (`03 §4/§12`).
fn bootstrap_and_migrate(conn: &mut Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);")?;
    let rows: i64 = conn.query_row("SELECT COUNT(*) FROM schema_version", [], |r| r.get(0))?;
    if rows == 0 {
        conn.execute("INSERT INTO schema_version (version) VALUES (0)", [])?;
    }
    let mut current: i32 =
        conn.query_row("SELECT version FROM schema_version", [], |r| r.get(0))?;
    let max_version = schema::MIGRATIONS
        .iter()
        .map(|(version, _)| *version)
        .max()
        .unwrap_or(0);
    debug_assert_eq!(
        schema::LATEST_SCHEMA_VERSION,
        max_version,
        "LATEST_SCHEMA_VERSION is out of sync with MIGRATIONS"
    );
    if current > max_version {
        anyhow::bail!(
            "database has newer schema_version {current}; this build supports up to {max_version}"
        );
    }
    // A migration may rebuild a table that has inbound foreign keys (v6 widens the
    // `frames.capture_trigger` CHECK, which SQLite can only do by recreating `frames`).
    // Such a rebuild is only safe with foreign keys OFF — otherwise `DROP TABLE frames`
    // fires an implicit cascade DELETE that wipes every child row. `PRAGMA foreign_keys`
    // is a no-op inside a transaction, so toggle it here, around the whole loop, per
    // SQLite's documented schema-change recipe. Only when there is work to do (the common
    // already-migrated open keeps foreign keys ON untouched).
    let had_pending = current < max_version;
    if had_pending {
        conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
    }
    for (version, sql) in schema::MIGRATIONS {
        if *version > current {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.execute("UPDATE schema_version SET version = ?1", [version])?;
            tx.commit()?;
            current = *version;
            tracing::info!(schema_version = current, "applied store migration");
        }
    }
    if had_pending {
        // Surface any integrity breakage loudly rather than silently shipping a corrupt
        // DB, then re-enable enforcement for the connection's lifetime.
        {
            let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
            let mut rows = stmt.query([])?;
            if rows.next()?.is_some() {
                anyhow::bail!("post-migration foreign_key_check found integrity violations");
            }
        }
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    }
    Ok(())
}

impl SqliteStore {
    /// Opens an in-memory store (tests / ephemeral use, `03 §10`).
    pub fn open_in_memory() -> Result<Self> {
        register_vec_extension();
        let conn = open_connection(Connection::open_in_memory()?)?;
        Ok(Self::from_conn(conn))
    }

    /// Opens (creating if needed) the store at `path` — the on-disk
    /// `screensearch.db` (`03 §4`).
    pub fn open_path(path: &Path) -> Result<Self> {
        register_vec_extension();
        let conn = open_connection(Connection::open(path)?)?;
        Ok(Self::from_conn(conn))
    }

    fn from_conn(conn: Connection) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
            embedder: Arc::new(RwLock::new(None)),
        }
    }

    /// Builder form of [`Self::set_embedder`] — injects the query-embedding provider
    /// that lights up the vector arm of [`Self::hybrid_search`]. Without it, search
    /// is FTS-only (P1 default; P3 injects fastembed once it has loaded).
    #[must_use]
    pub fn with_embedder(self, embedder: Arc<dyn EmbeddingProvider>) -> Self {
        self.set_embedder(embedder);
        self
    }

    /// Injects (or replaces) the query-embedding provider at runtime (`03 §5`). Takes
    /// effect immediately for every clone sharing this store — the composition root
    /// calls this once the fastembed model has loaded off the launch thread.
    pub fn set_embedder(&self, embedder: Arc<dyn EmbeddingProvider>) {
        *self.embedder.write().expect("store embedder lock poisoned") = Some(embedder);
    }

    /// The DB's current (tracked) schema version (`03 §4`). Useful for the
    /// readiness/diagnostics panel.
    pub fn schema_version(&self) -> Result<i32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let v = conn.query_row("SELECT version FROM schema_version", [], |r| r.get(0))?;
        Ok(v)
    }

    /// Runs `f` against the (exclusively locked) connection inside a blocking
    /// task, so async callers never block the runtime and the Mutex guard is
    /// never held across an `.await`. All data-access methods funnel through here.
    pub(crate) async fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let guard = conn.lock().expect("store mutex poisoned");
            f(&guard)
        })
        .await
        .map_err(|e| anyhow::anyhow!("store blocking task failed: {e}"))?
    }
}

/// The [`traits::Store`] contract, forwarding to the inherent methods (which the
/// module files implement and the test-suite exercises directly). The composition
/// root depends on `dyn Store`, so this is the seam that keeps it impl-agnostic
/// (`03 §2`). Inherent methods win method resolution, so the qualified
/// `SqliteStore::…` calls are non-recursive.
#[async_trait]
impl Store for SqliteStore {
    async fn insert_frame(&self, f: NewFrame) -> Result<i64> {
        SqliteStore::insert_frame(self, f).await
    }
    async fn insert_ocr(&self, frame_id: i64, ocr: OcrResult) -> Result<()> {
        SqliteStore::insert_ocr(self, frame_id, ocr).await
    }
    async fn insert_ocr_filtered(
        &self,
        frame_id: i64,
        ocr: OcrResult,
        ctx: TextFilterContext,
    ) -> Result<()> {
        SqliteStore::insert_ocr_filtered(self, frame_id, ocr, ctx).await
    }
    async fn insert_vision(&self, frame_id: i64, v: VisionAnalysis) -> Result<()> {
        SqliteStore::insert_vision(self, frame_id, v).await
    }
    async fn upsert_text_embedding(
        &self,
        frame_id: i64,
        chunk_index: i32,
        chunk_text: &str,
        source: ChunkSource,
        emb: &Embedding,
        model: &str,
    ) -> Result<()> {
        SqliteStore::upsert_text_embedding(
            self,
            frame_id,
            chunk_index,
            chunk_text,
            source,
            emb,
            model,
        )
        .await
    }
    async fn upsert_image_embedding(
        &self,
        frame_id: i64,
        emb: &Embedding,
        model: &str,
    ) -> Result<()> {
        SqliteStore::upsert_image_embedding(self, frame_id, emb, model).await
    }
    async fn hybrid_search(&self, q: &SearchQuery) -> Result<Vec<SearchHit>> {
        SqliteStore::hybrid_search(self, q).await
    }
    async fn timeline_buckets(
        &self,
        start: i64,
        end: i64,
        bucket_count: u32,
    ) -> Result<Vec<TimelineBucket>> {
        SqliteStore::timeline_buckets(self, start, end, bucket_count).await
    }
    async fn insights_summary(
        &self,
        start: i64,
        end: i64,
        bucket_count: u32,
    ) -> Result<InsightsSummary> {
        SqliteStore::insights_summary(self, start, end, bucket_count).await
    }
    async fn get_enrichment_input(&self, frame_id: i64) -> Result<Option<FrameEnrichmentInput>> {
        SqliteStore::frame_enrichment_input(self, frame_id).await
    }
    async fn untagged_frame_ids(&self, limit: u32, range: Option<(i64, i64)>) -> Result<Vec<i64>> {
        SqliteStore::untagged_frame_ids(self, limit, range).await
    }
    async fn ocr_texts(&self, frame_ids: &[i64]) -> Result<std::collections::HashMap<i64, String>> {
        SqliteStore::ocr_texts(self, frame_ids).await
    }
    async fn sample_frames_in_range(
        &self,
        start: i64,
        end: i64,
        limit: u32,
    ) -> Result<Vec<FrameMeta>> {
        SqliteStore::sample_frames_in_range(self, start, end, limit).await
    }
    async fn enqueue_job(&self, job: NewJob) -> Result<i64> {
        SqliteStore::enqueue_job(self, job).await
    }
    async fn claim_jobs(&self, kinds: &[JobKind], limit: u32, now: i64) -> Result<Vec<Job>> {
        SqliteStore::claim_jobs(self, kinds, limit, now).await
    }
    async fn complete_job(&self, id: i64) -> Result<()> {
        SqliteStore::complete_job(self, id).await
    }
    async fn fail_job(&self, id: i64, err: &str, retry_at: Option<i64>) -> Result<()> {
        SqliteStore::fail_job(self, id, err, retry_at).await
    }
    async fn job_stats(&self) -> Result<JobStats> {
        SqliteStore::job_stats(self).await
    }
    async fn pending_vision_job_count(&self) -> Result<u64> {
        SqliteStore::pending_vision_job_count(self).await
    }
    async fn reset_stale_running_jobs(&self, older_than_ms: i64) -> Result<u64> {
        SqliteStore::reset_stale_running_jobs(self, older_than_ms).await
    }
    async fn text_filter_stats(&self, filter_version: i32) -> Result<Vec<AppSuppression>> {
        SqliteStore::text_filter_stats(self, filter_version).await
    }
    async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        SqliteStore::get_setting(self, key).await
    }
    async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        SqliteStore::set_setting(self, key, value).await
    }
    async fn set_settings_batch(&self, kvs: &[(String, String)]) -> Result<()> {
        SqliteStore::set_settings_batch(self, kvs).await
    }
    async fn delete_settings(&self, keys: &[&str]) -> Result<Vec<String>> {
        SqliteStore::delete_settings(self, keys).await
    }
    fn set_embedder(&self, embedder: Arc<dyn EmbeddingProvider>) {
        SqliteStore::set_embedder(self, embedder);
    }
}

#[cfg(test)]
mod migration_tests {
    use super::{bootstrap_and_migrate, register_vec_extension, schema};
    use rusqlite::Connection;

    /// Applies migrations `v1..=through` to a fresh connection exactly as an older build
    /// would (each in its own transaction, foreign keys ON), leaving the DB at
    /// `schema_version = through` so the production runner can be exercised across the
    /// next upgrade with realistic pre-existing data.
    fn open_at_version(through: i32) -> Connection {
        register_vec_extension();
        let mut conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=ON;")
            .expect("pragmas");
        conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);")
            .expect("schema_version table");
        conn.execute("INSERT INTO schema_version (version) VALUES (0)", [])
            .expect("seed version row");
        for (version, sql) in schema::MIGRATIONS.iter().filter(|(v, _)| *v <= through) {
            let tx = conn.transaction().expect("begin");
            tx.execute_batch(sql).expect("apply migration");
            tx.execute("UPDATE schema_version SET version = ?1", [version])
                .expect("bump version");
            tx.commit().expect("commit");
        }
        conn
    }

    fn fk_violation_count(conn: &Connection) -> i64 {
        let mut stmt = conn
            .prepare("PRAGMA foreign_key_check")
            .expect("prepare check");
        let mut rows = stmt.query([]).expect("run check");
        let mut n = 0;
        while rows.next().expect("row").is_some() {
            n += 1;
        }
        n
    }

    /// v6 rebuilds the `frames` table to widen the `capture_trigger` CHECK (adding the
    /// `click` / `scroll_stop` tokens). A naive rebuild with foreign keys ON would fire an
    /// implicit cascade DELETE on every child row when the old `frames` is dropped — so
    /// this proves the runner disables foreign keys around the migration: child rows must
    /// survive, the new tokens must be accepted, a bogus token must still be rejected, FK
    /// integrity must hold, and ON DELETE CASCADE must still be wired afterward.
    #[test]
    fn migration_v6_widens_capture_trigger_check_without_dropping_children() {
        let mut conn = open_at_version(5);
        // A frame at v5 plus a child row (frame_text CASCADEs on frames).
        conn.execute(
            "INSERT INTO frames (id, captured_at, monitor_index, width, height, image_path, content_hash, capture_trigger)
             VALUES (1, 100, 0, 10, 10, 'p', 'h', 'timer')",
            [],
        )
        .expect("insert frame");
        conn.execute(
            "INSERT INTO frame_text (frame_id, raw_text, content_text, primary_source, filter_version, suppressed_count)
             VALUES (1, 'r', 'c', 'ocr', 0, 0)",
            [],
        )
        .expect("insert child frame_text");

        // Upgrade to LATEST via the real runner.
        bootstrap_and_migrate(&mut conn).expect("migrate to latest");

        let version: i32 = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .expect("read version");
        assert_eq!(version, schema::LATEST_SCHEMA_VERSION);
        assert_eq!(
            version, 8,
            "latest schema is v8 (degrade-to-text retention: image_purged + sweep index)"
        );

        let frame_rows: i64 = conn
            .query_row("SELECT count(*) FROM frames WHERE id = 1", [], |r| r.get(0))
            .expect("count frames");
        assert_eq!(frame_rows, 1, "the frame row must survive the rebuild");
        let child_rows: i64 = conn
            .query_row(
                "SELECT count(*) FROM frame_text WHERE frame_id = 1",
                [],
                |r| r.get(0),
            )
            .expect("count children");
        assert_eq!(
            child_rows, 1,
            "child rows must NOT be cascade-deleted by the frames rebuild"
        );

        // New tokens accepted.
        conn.execute(
            "UPDATE frames SET capture_trigger = 'click' WHERE id = 1",
            [],
        )
        .expect("'click' must satisfy the widened CHECK");
        conn.execute(
            "UPDATE frames SET capture_trigger = 'scroll_stop' WHERE id = 1",
            [],
        )
        .expect("'scroll_stop' must satisfy the widened CHECK");
        // A bogus token is still rejected loudly.
        assert!(
            conn.execute(
                "UPDATE frames SET capture_trigger = 'bogus' WHERE id = 1",
                []
            )
            .is_err(),
            "an unknown token must still violate the CHECK"
        );

        // FK integrity intact, and ON DELETE CASCADE still wired post-rebuild.
        assert_eq!(
            fk_violation_count(&conn),
            0,
            "no FK violations after rebuild"
        );
        conn.execute("DELETE FROM frames WHERE id = 1", [])
            .expect("delete frame");
        let child_after: i64 = conn
            .query_row(
                "SELECT count(*) FROM frame_text WHERE frame_id = 1",
                [],
                |r| r.get(0),
            )
            .expect("count children after delete");
        assert_eq!(
            child_after, 0,
            "ON DELETE CASCADE must still fire after rebuild"
        );
    }

    /// v7 adds `frames.image_purged` for degrade-to-text retention. An existing frame
    /// (carried up from v5, through the v6 rebuild) must read back as `0` ("image
    /// present"); the column must accept `1` and reject anything else via its CHECK.
    #[test]
    fn migration_v7_adds_image_purged_present_by_default() {
        let mut conn = open_at_version(6);
        conn.execute(
            "INSERT INTO frames (id, captured_at, monitor_index, width, height, image_path, content_hash, capture_trigger)
             VALUES (1, 100, 0, 10, 10, 'p', 'h', 'timer')",
            [],
        )
        .expect("insert v6 frame");

        bootstrap_and_migrate(&mut conn).expect("migrate to latest");

        let version: i32 = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .expect("read version");
        assert_eq!(version, schema::LATEST_SCHEMA_VERSION);

        // Pre-existing row defaults to "image present".
        let purged: i64 = conn
            .query_row("SELECT image_purged FROM frames WHERE id = 1", [], |r| {
                r.get(0)
            })
            .expect("read image_purged");
        assert_eq!(purged, 0, "existing frames keep their image (purged = 0)");

        // The degrade write is accepted; a bogus value is rejected by the CHECK.
        conn.execute("UPDATE frames SET image_purged = 1 WHERE id = 1", [])
            .expect("marking image purged must satisfy the CHECK");
        assert!(
            conn.execute("UPDATE frames SET image_purged = 2 WHERE id = 1", [])
                .is_err(),
            "image_purged outside {{0,1}} must violate the CHECK"
        );
    }

    /// v8 adds a **partial** index for the degrade-to-text retention sweep. The candidate
    /// query (`frames_with_image_older_than`) must use it instead of scanning past an
    /// ever-growing prefix of already-purged rows — so the planner must pick
    /// `idx_frames_image_retention`, not the plain `captured_at` index, for that predicate.
    #[test]
    fn migration_v8_indexes_image_retention_sweep() {
        let mut conn = open_at_version(7);
        bootstrap_and_migrate(&mut conn).expect("migrate to latest");

        // The partial index exists.
        let idx: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_frames_image_retention'",
                [],
                |r| r.get(0),
            )
            .expect("read sqlite_master");
        assert_eq!(idx, 1, "v8 creates idx_frames_image_retention");

        // The sweep query actually uses it (the whole point of the index).
        let plan: String = conn
            .query_row(
                "EXPLAIN QUERY PLAN
                 SELECT id, captured_at, image_path, image_purged, app_hint FROM frames
                 WHERE captured_at < 1 AND image_purged = 0
                 ORDER BY captured_at ASC LIMIT 1",
                [],
                |r| r.get(3),
            )
            .expect("explain query plan");
        assert!(
            plan.contains("idx_frames_image_retention"),
            "sweep must use the partial retention index, got plan: {plan}"
        );
    }
}

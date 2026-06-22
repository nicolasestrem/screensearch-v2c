//! Key/value settings persistence (`03 §8`). Values are opaque strings; the typed
//! [`traits::Settings`] mapping is assembled by the composition root.

use rusqlite::{params, OptionalExtension};
use traits::Result;

use crate::SqliteStore;

impl SqliteStore {
    /// Reads a single setting, or `None` if it has never been written.
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let key = key.to_string();
        self.with_conn(move |conn| {
            let value = conn
                .query_row(
                    "SELECT value FROM settings WHERE key = ?1",
                    params![key],
                    |r| r.get::<_, String>(0),
                )
                .optional()?;
            Ok(value)
        })
        .await
    }

    /// Upserts a setting (write-then-read is stable; a second write overwrites).
    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let (key, value) = (key.to_string(), value.to_string());
        self.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
            Ok(())
        })
        .await
    }

    /// Atomically upserts many settings in **one transaction** — either every pair
    /// lands or none do. This is what makes `save_settings` crash-safe: a failure
    /// (or a kill) part-way through rolls back, so `load_settings` never sees a mix of
    /// new and stale keys. `unchecked_transaction` is sound here because `with_conn`
    /// holds the store mutex exclusively for the duration of the closure — no other
    /// borrow of the connection can exist.
    pub async fn set_settings_batch(&self, kvs: &[(String, String)]) -> Result<()> {
        let kvs = kvs.to_vec();
        self.with_conn(move |conn| {
            let tx = conn.unchecked_transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO settings (key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                )?;
                for (key, value) in &kvs {
                    stmt.execute(params![key, value])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }
}

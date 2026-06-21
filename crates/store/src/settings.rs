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
}

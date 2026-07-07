//! Read-only export of frame metadata + marks from the live SQLite DB, the `suggest-days`
//! survey, and the D5 `VACUUM INTO` backup. Opens `SQLITE_OPEN_READ_ONLY` + `PRAGMA
//! query_only`; never mutates the source. Filled in Task 5.

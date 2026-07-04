//! Streaming JSON export (`GET /v1/export` + the `export_data` command; `03 §7c`, D12).
//!
//! One code path, two consumers: the HTTP route streams the bytes to the client, and
//! [`export_to_file`] drains the same stream to a file (the Settings "Export…" button, so
//! export works with the API off). Frames are paged through the store cursor so memory
//! stays flat over an arbitrarily large history; marks (user-created, few) are appended in
//! one query. Content text only — never `raw_text`, never images (D12).

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::State;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use futures_util::stream::Stream;
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use traits::ExportResult;

use crate::error::ApiError;
use crate::extract::ApiQuery;
use crate::{now_ms, ApiHost, ApiState};

/// The export document's `schema` tag — a version marker so consumers can detect the v1
/// shape (`03 §7c`).
const SCHEMA: &str = "screensearch.export.v1";

/// Frames per store page. Bounded so the single store connection is taken and released
/// per page (`03 §3`), never held across the whole response.
const PAGE_SIZE: u32 = 500;

/// Running counts, filled as the stream is drained; read into [`ExportResult`] afterward.
#[derive(Default)]
pub struct ExportStats {
    pub frames: AtomicU64,
    pub marks: AtomicU64,
}

/// The marks shape in the export — the durable columns only, stripped of the display
/// fields `list_marks` joins for the UI (D12: frames + content text + marks).
#[derive(Serialize)]
struct ExportMark {
    mark_id: i64,
    frame_id: i64,
    created_at: i64,
    note: Option<String>,
    resolved_at: Option<i64>,
}

enum Phase {
    Header,
    Frames { after_id: i64, first: bool },
    Done,
}

struct ExportState {
    phase: Phase,
    host: Arc<dyn ApiHost>,
    from: Option<i64>,
    to: Option<i64>,
    stats: Arc<ExportStats>,
}

fn opt_num(v: Option<i64>) -> String {
    v.map(|n| n.to_string())
        .unwrap_or_else(|| "null".to_string())
}

/// The incremental JSON byte stream:
/// `{"schema":…,"exported_at":…,"from":…,"to":…,"frames":[…],"marks":[…]}`.
/// One `export_frames_page` call per step (memory flat); marks appended once at the end.
pub fn export_json_stream(
    host: Arc<dyn ApiHost>,
    from: Option<i64>,
    to: Option<i64>,
    stats: Arc<ExportStats>,
) -> impl Stream<Item = anyhow::Result<Vec<u8>>> + Send + 'static {
    let init = ExportState {
        phase: Phase::Header,
        host,
        from,
        to,
        stats,
    };
    futures_util::stream::try_unfold(init, |mut st| async move {
        match st.phase {
            Phase::Header => {
                let header = format!(
                    "{{\"schema\":\"{SCHEMA}\",\"exported_at\":{},\"from\":{},\"to\":{},\"frames\":[",
                    now_ms(),
                    opt_num(st.from),
                    opt_num(st.to),
                );
                st.phase = Phase::Frames {
                    after_id: 0,
                    first: true,
                };
                Ok(Some((header.into_bytes(), st)))
            }
            Phase::Frames { after_id, first } => {
                let page = st
                    .host
                    .export_frames_page(after_id, st.from, st.to, PAGE_SIZE)
                    .await?;
                if page.is_empty() {
                    // Close the frames array, append marks (one query), close the object.
                    let marks = collect_marks(st.host.as_ref(), st.from, st.to, &st.stats).await?;
                    let mut tail = Vec::with_capacity(marks.len() + 16);
                    tail.extend_from_slice(b"],\"marks\":[");
                    tail.extend_from_slice(&marks);
                    tail.extend_from_slice(b"]}");
                    st.phase = Phase::Done;
                    Ok(Some((tail, st)))
                } else {
                    let mut buf = Vec::new();
                    let mut is_first = first;
                    let mut last_id = after_id;
                    for row in &page {
                        if !is_first {
                            buf.push(b',');
                        }
                        serde_json::to_writer(&mut buf, row)?;
                        is_first = false;
                        last_id = row.frame_id;
                    }
                    st.stats
                        .frames
                        .fetch_add(page.len() as u64, Ordering::Relaxed);
                    st.phase = Phase::Frames {
                        after_id: last_id,
                        first: false,
                    };
                    Ok(Some((buf, st)))
                }
            }
            Phase::Done => Ok(None),
        }
    })
}

/// Serializes the marks that fall in `[from, to)` (by their frame's capture time). Marks
/// are few, so one query + one buffer is fine — the flat-memory concern is frames only.
async fn collect_marks(
    host: &dyn ApiHost,
    from: Option<i64>,
    to: Option<i64>,
    stats: &ExportStats,
) -> anyhow::Result<Vec<u8>> {
    let marks = host.store().list_marks().await?;
    let mut buf = Vec::new();
    let mut first = true;
    for m in marks {
        if from.is_some_and(|f| m.captured_at < f) || to.is_some_and(|t| m.captured_at >= t) {
            continue;
        }
        if !first {
            buf.push(b',');
        }
        serde_json::to_writer(
            &mut buf,
            &ExportMark {
                mark_id: m.mark_id,
                frame_id: m.frame_id,
                created_at: m.created_at,
                note: m.note,
                resolved_at: m.resolved_at,
            },
        )?;
        first = false;
        stats.marks.fetch_add(1, Ordering::Relaxed);
    }
    Ok(buf)
}

/// `GET /v1/export?from=&to=&format=json` — streams the export as the response body.
/// Only `format=json` in v1; anything else is 400. A mid-stream store error truncates the
/// body and closes the connection (the honest HTTP failure mode; documented in API.md).
#[derive(Deserialize)]
pub struct ExportParams {
    from: Option<i64>,
    to: Option<i64>,
    format: Option<String>,
}

pub async fn export(
    State(state): State<ApiState>,
    ApiQuery(params): ApiQuery<ExportParams>,
) -> Result<Response, ApiError> {
    if params.format.as_deref().unwrap_or("json") != "json" {
        return Err(ApiError::BadRequest(
            "only `format=json` is supported in v1".to_string(),
        ));
    }
    validate_window(params.from, params.to)?;
    let stats = Arc::new(ExportStats::default());
    let stream = export_json_stream(state.host.clone(), params.from, params.to, stats)
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.to_string().into() });
    let body = axum::body::Body::from_stream(stream);
    Ok(([(header::CONTENT_TYPE, "application/json")], body).into_response())
}

/// Rejects an inverted `[from, to)` window early so a client gets a clean `400` instead of
/// a silently empty export. Shared by the HTTP route and the store cursor's callers.
pub fn validate_window(from: Option<i64>, to: Option<i64>) -> Result<(), ApiError> {
    if let (Some(from), Some(to)) = (from, to) {
        if from > to {
            return Err(ApiError::BadRequest(
                "`from` must be less than or equal to `to`".to_string(),
            ));
        }
    }
    Ok(())
}

/// Drains the export stream to `path`, returning the byte count. Owns the file handle for
/// its whole lifetime and **drops it on return** (Ok or Err) — so the caller can safely
/// delete the `.partial` file on failure, which on Windows would fail while a write handle
/// is still open (sharing violation).
async fn drain_to_file(
    path: &Path,
    host: Arc<dyn ApiHost>,
    from: Option<i64>,
    to: Option<i64>,
    stats: Arc<ExportStats>,
) -> anyhow::Result<u64> {
    let mut file = tokio::fs::File::create(path).await?;
    let stream = export_json_stream(host, from, to, stats);
    futures_util::pin_mut!(stream);
    let mut bytes: u64 = 0;
    while let Some(chunk) = stream.try_next().await? {
        file.write_all(&chunk).await?;
        bytes += chunk.len() as u64;
    }
    file.flush().await?;
    Ok(bytes)
}

/// Drains the same export stream to `<dir>/screensearch-export-<UTC stamp>.json`, writing
/// through a `.partial` temp name and renaming on success so a failed export never leaves
/// a plausible-looking file. Backs the Settings "Export…" command; works with the API off.
pub async fn export_to_file(
    host: Arc<dyn ApiHost>,
    from: Option<i64>,
    to: Option<i64>,
    dir: &Path,
) -> anyhow::Result<ExportResult> {
    tokio::fs::create_dir_all(dir).await?;
    let filename = format!("screensearch-export-{}.json", utc_stamp(now_ms()));
    let final_path = dir.join(&filename);
    let partial_path = dir.join(format!("{filename}.partial"));

    let stats = Arc::new(ExportStats::default());
    // `drain_to_file` drops the file handle before returning, so removing the partial on
    // failure (or after a failed rename) succeeds even on Windows.
    let bytes = match drain_to_file(&partial_path, host, from, to, stats.clone()).await {
        Ok(bytes) => bytes,
        Err(e) => {
            let _ = tokio::fs::remove_file(&partial_path).await;
            return Err(e);
        }
    };
    if let Err(e) = tokio::fs::rename(&partial_path, &final_path).await {
        let _ = tokio::fs::remove_file(&partial_path).await;
        return Err(e.into());
    }

    Ok(ExportResult {
        path: final_path.to_string_lossy().into_owned(),
        frames: stats.frames.load(Ordering::Relaxed) as u32,
        marks: stats.marks.load(Ordering::Relaxed) as u32,
        bytes,
    })
}

/// Formats a unix-epoch-ms instant as `YYYYMMDD-HHMMSS` (UTC), for the export filename.
/// UTC keeps it dependency-free (no `chrono`/timezone db) and sortable; the toast shows
/// the full path, so the exact TZ label doesn't matter. Uses Hinnant's civil-from-days.
fn utc_stamp(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{year:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}")
}

#[cfg(test)]
mod tests {
    use super::utc_stamp;

    #[test]
    fn utc_stamp_matches_known_instants() {
        assert_eq!(utc_stamp(0), "19700101-000000");
        // 2021-01-01T00:00:00Z = 1_609_459_200_000 ms.
        assert_eq!(utc_stamp(1_609_459_200_000), "20210101-000000");
        // A time component: 2021-01-01T13:37:42Z.
        assert_eq!(
            utc_stamp(1_609_459_200_000 + (13 * 3600 + 37 * 60 + 42) * 1000),
            "20210101-133742"
        );
    }
}

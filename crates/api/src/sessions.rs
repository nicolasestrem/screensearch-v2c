//! The read-only sessions surface (0.4.0 PR6; `03 §7c`/`§7e`, D12).
//!
//! `GET /v1/sessions` lists sessions overlapping a time window; `GET /v1/sessions/{id}`
//! returns one session's detail plus its `exchange` artifacts. Both are strictly read-only:
//! a GET never starts inference or writes a row, so a cached `summary` is reflected but never
//! *generated* here (generation is an in-app IPC action — `kernel::sessions_intel`, `§7e`).
//! Every session read reuses the store methods PR4/PR5 already ship; no new `ApiHost` method
//! is needed (precedent: `routes::list_marks`).

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use traits::{
    Session, SessionArtifact, SessionArtifactKind, SessionFilter, SessionHost, SessionKind,
};

use crate::error::ApiError;
use crate::extract::{ApiPath, ApiQuery};
use crate::{now_ms, ApiState};

/// Default `limit` for `GET /v1/sessions` — mirrors the in-app `list_sessions` IPC surface
/// (`§7c` calls the endpoint "the list surface behind `list_sessions`"). Sessions are sparse
/// rows, so this covers months of history in one pull for an agent.
const SESSIONS_LIMIT_DEFAULT: u32 = 1000;
/// Hard ceiling on `limit`, matching the IPC clamp. (Maintainer-decided, 2026-07-11.)
const SESSIONS_LIMIT_MAX: u32 = 1000;

/// A session as served over the read-only API. Distinct from the ts-rs `traits::Session` so
/// the summary-visibility rule (`§7c`, below) and the `open` marker can be applied without
/// touching the shared IPC type. `kind`/`host` reuse the `traits` enums, which already
/// serialize as the lowercase strings the taxonomy and MCP consumers expect.
#[derive(Serialize)]
pub struct ApiSession {
    pub id: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    /// The `§7c` non-final marker: `true` while `ended_at` is null (the session is still open).
    /// Always present so a consumer never has to infer openness from a missing field.
    pub open: bool,
    pub kind: SessionKind,
    pub tool: Option<String>,
    pub host: Option<SessionHost>,
    pub context_key: String,
    /// Always included — cheap identity a caller wants on every row.
    pub title: Option<String>,
    /// `null` unless the caller asked (`include_summary=1` on detail). The API never generates
    /// a summary (D12); it only reflects what the in-app path already cached.
    pub summary: Option<String>,
    pub summary_model: Option<String>,
    pub confidence: f64,
    pub frozen: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Projects a stored [`Session`] onto the API shape. `reveal_summary=false` blanks the cached
/// `summary`/`summary_model` (the list surface, and detail without `include_summary=1`); the
/// row is never mutated and no generation is triggered.
fn api_session(s: Session, reveal_summary: bool) -> ApiSession {
    ApiSession {
        id: s.id,
        started_at: s.started_at,
        ended_at: s.ended_at,
        open: s.ended_at.is_none(),
        kind: s.kind,
        tool: s.tool,
        host: s.host,
        context_key: s.context_key,
        title: s.title,
        summary: if reveal_summary { s.summary } else { None },
        summary_model: if reveal_summary {
            s.summary_model
        } else {
            None
        },
        confidence: s.confidence,
        frozen: s.frozen,
        created_at: s.created_at,
        updated_at: s.updated_at,
    }
}

/// Parses the optional `kind` filter, returning a clean 400 (naming the valid values) on an
/// unknown value rather than silently ignoring it.
fn parse_kind(kind: Option<&str>) -> Result<Option<SessionKind>, ApiError> {
    match kind {
        None => Ok(None),
        Some("focus") => Ok(Some(SessionKind::Focus)),
        Some("meeting") => Ok(Some(SessionKind::Meeting)),
        Some("ai") => Ok(Some(SessionKind::Ai)),
        Some("other") => Ok(Some(SessionKind::Other)),
        Some(other) => Err(ApiError::BadRequest(format!(
            "unknown `kind` {other:?}; valid values are focus, meeting, ai, other"
        ))),
    }
}

/// `GET /v1/sessions?kind=&tool=&from=&to=&limit=` — sessions **overlapping** the requested
/// window (`§7c`). `from`/`to` are independently optional (unlike `/v1/search`'s both-or-neither
/// pairing): the store predicate is `started_at < to AND COALESCE(ended_at, now) > from`, so an
/// open or long session that began before `from` still surfaces. Summaries are never included on
/// this surface.
#[derive(Deserialize)]
pub struct SessionsParams {
    kind: Option<String>,
    tool: Option<String>,
    from: Option<i64>,
    to: Option<i64>,
    limit: Option<u32>,
}

pub async fn list_sessions(
    State(state): State<ApiState>,
    ApiQuery(params): ApiQuery<SessionsParams>,
) -> Result<Json<Vec<ApiSession>>, ApiError> {
    let kind = parse_kind(params.kind.as_deref())?;
    if let (Some(from), Some(to)) = (params.from, params.to) {
        if from > to {
            return Err(ApiError::BadRequest(
                "`from` must be less than or equal to `to`".to_string(),
            ));
        }
    }
    // An empty/whitespace `tool` means "no tool filter" (mirrors the IPC normalization).
    let tool = params.tool.and_then(|t| {
        let t = t.trim().to_string();
        (!t.is_empty()).then_some(t)
    });
    let filter = SessionFilter {
        from_ms: params.from,
        to_ms: params.to,
        kind,
        tool,
        limit: Some(
            params
                .limit
                .unwrap_or(SESSIONS_LIMIT_DEFAULT)
                .clamp(1, SESSIONS_LIMIT_MAX),
        ),
        // Open sessions (`ended_at` null) are tested against request time for the overlap.
        now_ms: now_ms(),
    };
    let sessions = state
        .host
        .store()
        .sessions_in_range(filter)
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(
        sessions
            .into_iter()
            .map(|s| api_session(s, false))
            .collect(),
    ))
}

/// Session detail plus its extracted user/agent exchanges. `transcript`/`note` artifacts are
/// reserved for later arcs and never surface here (`§7e`, D8).
#[derive(Serialize)]
pub struct SessionDetailResponse {
    pub session: ApiSession,
    pub exchanges: Vec<SessionArtifact>,
}

#[derive(Deserialize)]
pub struct DetailParams {
    include_summary: Option<u8>,
}

pub async fn session_detail(
    State(state): State<ApiState>,
    ApiPath(id): ApiPath<i64>,
    ApiQuery(params): ApiQuery<DetailParams>,
) -> Result<Json<SessionDetailResponse>, ApiError> {
    let store = state.host.store();
    let session = store
        .get_session(id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(format!("session {id} not found")))?;
    // `include_summary=1` reveals the *cached* summary (or null); it never generates one (D12).
    let reveal_summary = params.include_summary == Some(1);
    let exchanges = store
        .list_session_artifacts(id)
        .await
        .map_err(ApiError::Internal)?
        .into_iter()
        .filter(|a| a.kind == SessionArtifactKind::Exchange)
        .collect();
    Ok(Json(SessionDetailResponse {
        session: api_session(session, reveal_summary),
        exchanges,
    }))
}

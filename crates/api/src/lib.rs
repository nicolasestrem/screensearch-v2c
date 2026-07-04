//! The opt-in local HTTP API (0.3.0 PR7; `03 §7c`, D11/D12).
//!
//! A small [`axum`] server that exposes the same core queries the UI uses — search,
//! ask, frames, where-was-i, marks — plus a streamed JSON export, over
//! `127.0.0.1:<port>` for local scripts and agents. It is **off by default** and, when
//! enabled, binds loopback **only** (the bind IP is a `const`, never a setting — a
//! `0.0.0.0` bind is impossible by construction, D11) and requires a **bearer token** on
//! every request.
//!
//! The crate depends on [`traits`] only (`03 §2`): everything it needs from the app is
//! behind the [`ApiHost`] trait, which the composition root implements over the concrete
//! store/kernel and wires in. When the API is disabled the server is simply never
//! constructed — but the crate (and its export code path) is still linked, so the
//! Settings "Export…" button works with the API off.

pub mod auth;
pub mod error;
pub mod export;
pub mod routes;

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use axum::routing::{get, post};
use axum::Router;
use tokio::sync::watch;
use tokio::task::{AbortHandle, JoinHandle};
use traits::{
    AnswerProvider, ExportFrameRow, FrameDetail, ResumeContext, RetrievedChunk, SearchQuery,
    Settings, Store,
};

/// The default port for the local API (`api.port`, configurable; `03 §8`, D11).
pub const DEFAULT_PORT: u16 = 43210;

/// The **only** address the server ever binds (D11). Not a parameter of [`ApiServer::start`]
/// and never sourced from settings, so a `0.0.0.0`/LAN bind cannot happen by construction —
/// asserted by the `binds_loopback_only` integration test.
const BIND_IP: Ipv4Addr = Ipv4Addr::LOCALHOST;

/// The live bearer token, shared with the auth middleware. Held behind an `RwLock` and
/// read per request, so regenerating the token takes effect immediately without
/// restarting the server (`regenerate_api_token`, `03 §7c`).
pub type SharedToken = Arc<RwLock<String>>;

/// A stored frame image ready to serve as the `?image=1` response body (`GET
/// /v1/frames/{id}`). `content_type` is the MIME derived from the stored file's
/// extension (WebP for current frames; JPEG for pre-WebP ones).
pub struct FrameImage {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}

/// Everything the HTTP surface needs from the app, behind one trait so `crates/api`
/// depends on `traits` only (`03 §2`). The composition root implements it over the
/// concrete `SqliteStore` + `Kernel`, reusing the same helpers the Tauri commands use so
/// the API and the UI answer identically. Integration tests implement it over a fixture
/// store.
#[async_trait]
pub trait ApiHost: Send + Sync + 'static {
    /// The app version string for `GET /v1/health`.
    fn version(&self) -> String;
    /// When the API host was constructed, unix epoch ms — the health uptime anchor.
    fn started_at_ms(&self) -> i64;
    /// Whether the capture loop is currently running (health `capturing`).
    async fn is_capturing(&self) -> bool;

    /// The store, for the trait-level reads the API reuses verbatim (`hybrid_search`,
    /// `list_marks`).
    fn store(&self) -> Arc<dyn Store>;

    /// Effective settings (retrieval depth, chrome default, thinking) — the API mirrors
    /// the UI's defaults when a request omits a parameter.
    async fn settings(&self) -> Settings;

    /// Full detail for a frame, or `None` if it does not exist (`GET /v1/frames/{id}`).
    async fn frame_detail(&self, frame_id: i64) -> anyhow::Result<Option<FrameDetail>>;

    /// The stored image bytes for a frame, or `None` if the frame does not exist **or**
    /// its image was retention-purged (`?image=1`). The route distinguishes the two.
    async fn frame_image(&self, frame_id: i64) -> anyhow::Result<Option<FrameImage>>;

    /// Grounding context for `/v1/ask` — the same hybrid-search + OCR-hydrate path the
    /// `ask` command uses.
    async fn ask_context(&self, query: &str, top_k: u32) -> anyhow::Result<Vec<RetrievedChunk>>;

    /// The active answer provider, or `None` when no answer model is loaded (`/v1/ask`
    /// then returns 503).
    async fn answer_provider(&self) -> Option<Arc<dyn AnswerProvider>>;

    /// Create a mark (`POST /v1/marks`). Exactly one of `frame_id` or `capture_now`;
    /// `capture_now` bypasses the diff gate for one frame (`03 §7b`, D8).
    async fn add_mark(
        &self,
        frame_id: Option<i64>,
        capture_now: bool,
        note: Option<String>,
    ) -> anyhow::Result<i64>;

    /// Resolve (or dismiss) a mark (`POST /v1/marks/{id}/resolve`).
    async fn resolve_mark(&self, mark_id: i64) -> anyhow::Result<()>;

    /// The where-was-i heuristic result, or `None` when nothing qualifies
    /// (`GET /v1/context/where-was-i`).
    async fn where_was_i(&self) -> anyhow::Result<Option<ResumeContext>>;

    /// One keyset page of the streaming export (`GET /v1/export`); see the store method
    /// of the same name.
    async fn export_frames_page(
        &self,
        after_id: i64,
        from: Option<i64>,
        to: Option<i64>,
        limit: u32,
    ) -> anyhow::Result<Vec<ExportFrameRow>>;
}

/// The axum handler state: the app seam + the live bearer token.
#[derive(Clone)]
pub struct ApiState {
    pub host: Arc<dyn ApiHost>,
    pub token: SharedToken,
}

/// A running local API server. Dropping the handle does **not** stop it — call
/// [`ApiServer::stop`]. The composition root keeps it in an `Option` slot so start/stop
/// on the settings toggle is idempotent (mirrors the capture handle, `03 §6`).
pub struct ApiServer {
    stop: watch::Sender<bool>,
    join: JoinHandle<()>,
    local_addr: SocketAddr,
}

impl ApiServer {
    /// Binds `127.0.0.1:<port>` **before** spawning the serve task, so a port conflict is
    /// a synchronous `Err` the caller can surface loudly (`ApiStatus.last_error`,
    /// port-in-use guided change — `03 §7c`). The bind IP is [`BIND_IP`], never a
    /// parameter — a non-loopback bind is impossible by construction (D11).
    pub async fn start(
        host: Arc<dyn ApiHost>,
        token: SharedToken,
        port: u16,
    ) -> std::io::Result<ApiServer> {
        let listener = tokio::net::TcpListener::bind((BIND_IP, port)).await?;
        let local_addr = listener.local_addr()?;
        let router = build_router(ApiState { host, token });
        let (stop, mut stop_rx) = watch::channel(false);
        let join = tokio::spawn(async move {
            let shutdown = async move {
                // Resolve when someone flips the stop flag.
                let _ = stop_rx.wait_for(|stopped| *stopped).await;
            };
            if let Err(e) = axum::serve(listener, router.into_make_service())
                .with_graceful_shutdown(shutdown)
                .await
            {
                tracing::error!(error = %e, "local API server exited with error");
            }
        });
        tracing::info!(%local_addr, "local API listening");
        Ok(ApiServer {
            stop,
            join,
            local_addr,
        })
    }

    /// The bound socket address (always loopback). Used by tests and to report the live
    /// port.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Graceful shutdown, bounded. Signals the serve task to stop accepting and drain,
    /// then waits up to 3 s; a still-open connection (a long `/v1/ask` SSE) that outlasts
    /// the grace window is force-aborted so app exit never wedges.
    pub async fn stop(self) {
        let _ = self.stop.send(true);
        let abort: AbortHandle = self.join.abort_handle();
        if tokio::time::timeout(Duration::from_secs(3), self.join)
            .await
            .is_err()
        {
            tracing::warn!("local API graceful shutdown timed out; aborting");
            abort.abort();
        }
    }
}

/// Builds the v1 router with the bearer-auth layer applied to **every** route (health
/// included — the whole surface is private). The ask + export routes are added by the
/// PR7 SSE/export module.
pub fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/v1/health", get(routes::health))
        .route("/v1/search", get(routes::search))
        .route("/v1/frames/{id}", get(routes::frame))
        .route("/v1/context/where-was-i", get(routes::where_was_i))
        .route(
            "/v1/marks",
            get(routes::list_marks).post(routes::create_mark),
        )
        .route("/v1/marks/{id}/resolve", post(routes::resolve_mark))
        .route("/v1/ask", post(routes::ask))
        .route("/v1/export", get(export::export))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_bearer,
        ))
        .with_state(state)
}

/// Generates a fresh bearer token: 32 CSPRNG bytes as 64 lowercase hex chars (`03 §7c`,
/// D11). Panics only if the OS RNG is unavailable (a machine that can't seed randomness
/// can't be trusted to serve a private API anyway).
pub fn generate_token() -> String {
    use std::fmt::Write;
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("OS RNG unavailable for API token generation");
    let mut token = String::with_capacity(64);
    for b in bytes {
        let _ = write!(token, "{b:02x}");
    }
    token
}

/// Wall-clock unix epoch ms (the kernel clock unit). Local to this crate so it doesn't
/// reach into the composition root for a one-liner.
pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Shared query params for search-like reads. Kept here so `routes` and any future
/// module reuse one definition.
pub(crate) fn build_search_query(
    text: String,
    from: Option<i64>,
    to: Option<i64>,
    limit: Option<u32>,
    include_chrome: Option<bool>,
    default_include_chrome: bool,
) -> Result<SearchQuery, error::ApiError> {
    let time_range = match (from, to) {
        (Some(start), Some(end)) => Some(traits::TimeRange { start, end }),
        (None, None) => None,
        _ => {
            return Err(error::ApiError::BadRequest(
                "`from` and `to` must be provided together".to_string(),
            ))
        }
    };
    Ok(SearchQuery {
        text,
        // Clamp to a sane ceiling; the store enforces its own hard cap too.
        limit: limit.unwrap_or(20).clamp(1, 100),
        time_range,
        include_chrome: include_chrome.unwrap_or(default_include_chrome),
    })
}

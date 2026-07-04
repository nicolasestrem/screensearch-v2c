//! Bearer-token authentication for the local API (`03 §7c`, D11).
//!
//! Applied to **every** `/v1` route, health included — the whole surface is private. A
//! request must carry `Authorization: Bearer <token>` matching the current token exactly;
//! anything else is 401. The token is compared in constant time so a timing side channel
//! can't recover it byte by byte.

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::ApiError;
use crate::ApiState;

/// Middleware: reject any request lacking a valid bearer token. Reads the live token
/// from the shared lock per request (so `regenerate_api_token` takes effect immediately),
/// releasing the guard before the compare's `.await`-free path.
pub async fn require_bearer(State(state): State<ApiState>, req: Request, next: Next) -> Response {
    let presented = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let authorized = match presented {
        Some(token) => {
            let current = state.token.read().expect("api token lock poisoned").clone();
            !current.is_empty() && constant_time_eq(token.as_bytes(), current.as_bytes())
        }
        None => false,
    };

    if authorized {
        next.run(req).await
    } else {
        ApiError::Unauthorized.into_response()
    }
}

/// Constant-time byte-slice equality — no new dependency. Accumulates the XOR of every
/// byte so the running time does not depend on where the first difference is. A length
/// mismatch returns early: the token length is fixed (64 hex chars), so length is not a
/// secret and leaking it reveals nothing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn constant_time_eq_matches_only_identical_slices() {
        assert!(constant_time_eq(b"abc123", b"abc123"));
        assert!(!constant_time_eq(b"abc123", b"abc124"));
        assert!(!constant_time_eq(b"abc123", b"abc12"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }
}

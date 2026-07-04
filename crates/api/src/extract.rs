//! Extractor wrappers that keep malformed requests on the JSON error contract (`03 §7c`).
//!
//! Axum's stock `Query`/`Json`/`Path` extractors reject *before* a handler runs, returning
//! their own plaintext body — bypassing [`ApiError`](crate::error::ApiError) and the
//! documented `{ "error", "message" }` shape. These thin wrappers delegate to the stock
//! extractors and convert any rejection into `ApiError::BadRequest`, so a bad query string,
//! body, or path segment yields the same 400 JSON shape as every other error.

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::error::ApiError;

/// `Query<T>` that reports a malformed query string as an `ApiError::BadRequest`.
pub struct ApiQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for ApiQuery<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Query::<T>::from_request_parts(parts, state)
            .await
            .map(|q| ApiQuery(q.0))
            .map_err(|e: QueryRejection| ApiError::BadRequest(e.body_text()))
    }
}

/// `Path<T>` that reports a malformed path segment (e.g. a non-integer id) as a
/// `BadRequest` rather than axum's plaintext rejection.
pub struct ApiPath<T>(pub T);

impl<T, S> FromRequestParts<S> for ApiPath<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map(|p| ApiPath(p.0))
            .map_err(|e: PathRejection| ApiError::BadRequest(e.body_text()))
    }
}

/// `Json<T>` that reports a malformed body (bad JSON, missing/typed field, wrong
/// content-type) as a `BadRequest` on the JSON error contract.
pub struct ApiJson<T>(pub T);

impl<T, S> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        axum::Json::<T>::from_request(req, state)
            .await
            .map(|j| ApiJson(j.0))
            .map_err(|e: JsonRejection| ApiError::BadRequest(e.body_text()))
    }
}

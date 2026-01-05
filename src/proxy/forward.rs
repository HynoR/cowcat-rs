use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, Request, Response, StatusCode, Uri};
use axum::response::IntoResponse;
use crate::handlers::pow::POW_PREFIX;
use crate::state::{AppState, ProxyTarget};

pub async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
) -> impl IntoResponse {
    if req.uri().path().starts_with(POW_PREFIX) {
        return StatusCode::NOT_FOUND.into_response();
    }

    *req.uri_mut() = build_target_uri(&state.proxy_target.uri, req.uri());
    rewrite_headers(req.headers_mut(), &state.proxy_target);

    match state.proxy_client.request(req).await {
        Ok(resp) => {
            let status = resp.status();
            tracing::debug!(status = %status, "proxy response");
            let (parts, body) = resp.into_parts();
            Response::from_parts(parts, Body::new(body))
        }
        Err(err) => {
            tracing::debug!(error = %err, "proxy request failed");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}

pub fn build_target_uri(target: &Uri, original: &Uri) -> Uri {
    let mut parts = original.clone().into_parts();
    parts.scheme = target.scheme().cloned();
    parts.authority = target.authority().cloned();
    Uri::from_parts(parts).unwrap_or_else(|_| target.clone())
}

pub fn rewrite_headers(headers: &mut HeaderMap, target: &ProxyTarget) {
    headers.insert(header::HOST, target.host_value.clone());
    headers
        .entry(header::HeaderName::from_static("x-forwarded-host"))
        .or_insert_with(|| target.x_forwarded_host.clone());
    headers
        .entry(header::HeaderName::from_static("x-forwarded-proto"))
        .or_insert_with(|| target.x_forwarded_proto.clone());
}

use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, Request, Response, StatusCode, Uri};
use axum::response::IntoResponse;
use crate::handlers::pow::POW_PREFIX;
use crate::middleware::pow::PowVerified;
use crate::state::{AppState, HostProxyTarget, ProxyTarget};

pub async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
) -> impl IntoResponse {
    if req.uri().path().starts_with(POW_PREFIX) {
        return StatusCode::NOT_FOUND.into_response();
    }

    let target = resolve_proxy_target(&state, &req);
    *req.uri_mut() = build_target_uri(&target.uri, req.uri());
    rewrite_headers(req.headers_mut(), target);

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

fn resolve_proxy_target<'a>(state: &'a AppState, req: &Request<Body>) -> &'a ProxyTarget {
    if req.extensions().get::<PowVerified>().is_none() {
        return &state.proxy_target;
    }
    let host = match req.headers().get(header::HOST).and_then(|v| v.to_str().ok()) {
        Some(value) => value,
        None => return &state.proxy_target,
    };
    let normalized = normalize_host(host);
    if normalized.is_empty() {
        return &state.proxy_target;
    }
    match find_host_target(&state.proxy_host_targets, &normalized) {
        Some(target) => target,
        None => &state.proxy_target,
    }
}

fn find_host_target<'a>(
    targets: &'a [HostProxyTarget],
    host: &str,
) -> Option<&'a ProxyTarget> {
    targets
        .iter()
        .find(|entry| entry.host == host)
        .map(|entry| &entry.target)
}

fn normalize_host(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Ok(authority) = trimmed.parse::<axum::http::uri::Authority>() {
        return authority.host().to_ascii_lowercase();
    }
    trimmed.to_ascii_lowercase()
}

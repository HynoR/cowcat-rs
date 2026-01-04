use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, Request, Response, StatusCode, Uri};
use axum::response::IntoResponse;
use crate::handlers::pow::POW_PREFIX;
use crate::state::AppState;

pub async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
) -> impl IntoResponse {
    if req.uri().path().starts_with(POW_PREFIX) {
        return StatusCode::NOT_FOUND.into_response();
    }

    let target = match state.config.proxy.target.parse::<Uri>() {
        Ok(uri) => uri,
        Err(err) => {
            tracing::error!(error = %err, "invalid proxy target");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    let host = match target.authority() {
        Some(auth) => auth.to_string(),
        None => {
            tracing::error!("proxy target missing authority");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };
    let scheme = target.scheme_str().unwrap_or("http").to_string();

    *req.uri_mut() = build_target_uri(&target, req.uri());
    *req.headers_mut() = rewrite_headers(req.headers(), &host, &scheme);

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

fn build_target_uri(target: &Uri, original: &Uri) -> Uri {
    let mut parts = original.clone().into_parts();
    parts.scheme = target.scheme().cloned();
    parts.authority = target.authority().cloned();
    Uri::from_parts(parts).unwrap_or_else(|_| target.clone())
}

fn rewrite_headers(headers: &HeaderMap, host: &str, scheme: &str) -> HeaderMap {
    let mut out = headers.clone();
    if let Ok(host_value) = header::HeaderValue::from_str(host) {
        out.insert(header::HOST, host_value.clone());
        out.entry(header::HeaderName::from_static("x-forwarded-host"))
            .or_insert_with(|| host_value.clone());
    }
    if let Ok(scheme_value) = header::HeaderValue::from_str(scheme) {
        out.entry(header::HeaderName::from_static("x-forwarded-proto"))
            .or_insert_with(|| scheme_value.clone());
    }
    out
}

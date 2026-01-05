use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, Response, StatusCode, Uri};
use axum::response::IntoResponse;
use http_body_util::BodyExt;

use crate::proxy::forward::{build_target_uri, rewrite_headers};
use crate::state::{AppState, FaviconCache};

pub async fn favicon_handler(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
) -> impl IntoResponse {
    // 检查缓存
    {
        let cache = state.favicon_cache.read().await;
        if let Some(cached) = cache.as_ref() {
            if cached.is_valid() {
                tracing::debug!("returning cached favicon");
                let mut response = Response::builder()
                    .status(cached.status)
                    .body(Body::from(cached.body.clone()))
                    .unwrap();
                *response.headers_mut() = cached.headers.clone();
                return response;
            }
        }
    }

    // 构建目标 URI（固定为 /favicon.ico）
    let mut target_uri_parts = req.uri().clone().into_parts();
    target_uri_parts.path_and_query = Some("/favicon.ico".parse().unwrap());
    let target_uri = Uri::from_parts(target_uri_parts).unwrap();
    *req.uri_mut() = build_target_uri(&state.proxy_target.uri, &target_uri);
    rewrite_headers(req.headers_mut(), &state.proxy_target);

    // 请求上游
    let resp = match state.proxy_client.request(req).await {
        Ok(resp) => resp,
        Err(err) => {
            tracing::debug!(error = %err, "favicon proxy request failed");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    let status = resp.status();
    let (parts, body) = resp.into_parts();

    // 读取 body 到内存
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(err) => {
            tracing::error!(error = %err, "failed to read favicon body");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    // 如果状态码是 2xx，缓存响应
    if status.is_success() {
        let cache = FaviconCache {
            status,
            headers: parts.headers.clone(),
            body: body_bytes.clone(),
            cached_at: std::time::Instant::now(),
        };
        *state.favicon_cache.write().await = Some(cache);
        tracing::debug!("cached favicon response");
    }

    // 构建响应
    let mut response = Response::builder()
        .status(status)
        .body(Body::from(body_bytes))
        .unwrap();
    *response.headers_mut() = parts.headers;

    response
}


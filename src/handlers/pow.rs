use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, Request, Response, StatusCode};
use axum::response::IntoResponse;
use base64::Engine;
use http_body_util::BodyExt;
use serde::Deserialize;
use time::OffsetDateTime;

use crate::config::IpPolicy;
use crate::crypto::{compute_ip_hash, compute_ua_hash, generate_cookie};
use crate::protocol::frame::{
    decode_frame, decode_task_request, decode_verify_request, encode_error_frame,
    encode_task_response, encode_verify_response, deobfuscate_frame, BinaryTaskResponse,
    BinaryVerifyResponse, FRAME_TYPE_TASK_REQUEST, FRAME_TYPE_VERIFY_REQUEST, XOR_KEY,
};
use crate::rules::clamp_difficulty;
use crate::state::AppState;
use crate::storage::Task;
use crate::{crypto, protocol};

pub const POW_PREFIX: &str = "/__cowcatwaf";
pub const POW_COOKIE_NAME: &str = "cowcat.waf.token";

#[derive(Debug, Deserialize)]
pub struct ChallengeQuery {
    redirect: Option<String>,
}

pub async fn challenge_page(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ChallengeQuery>,
    req: Request<axum::body::Body>,
) -> impl IntoResponse {
    let redirect = query.redirect.unwrap_or_else(|| "/".to_string());
    build_challenge_response(&state, req.headers(), req.extensions(), &redirect, state.config.pow.difficulty)
}

pub async fn pow_task(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
) -> impl IntoResponse {
    let (parts, body) = req.into_parts();
    let body = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return error_frame(StatusCode::BAD_REQUEST, "Invalid request"),
    };
    if !body.is_empty() {
        let (frame_type, payload) = match decode_frame(&body) {
            Ok(res) => res,
            Err(_) => return error_frame(StatusCode::BAD_REQUEST, "Invalid request"),
        };
        if frame_type != FRAME_TYPE_TASK_REQUEST {
            return error_frame(StatusCode::BAD_REQUEST, "Invalid request");
        }
        if decode_task_request(payload).is_err() {
            return error_frame(StatusCode::BAD_REQUEST, "Invalid request");
        }
    }

    let task = match build_task(&state, &parts.headers, &parts.extensions, state.config.pow.difficulty) {
        Ok(task) => task,
        Err(err) => {
            tracing::error!(error = %err, "failed to build task");
            return error_frame(StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate task");
        }
    };
    tracing::debug!(
        task_id = %task.task_id,
        bits = task.bits,
        scope = %task.scope,
        "pow task created"
    );
    state.task_store.set(task.clone());

    let resp = BinaryTaskResponse {
        task_id: task.task_id.clone(),
        seed: task.seed.clone(),
        bits: task.bits,
        exp: task.exp,
        scope: task.scope.clone(),
        ua_hash: task.ua_hash.clone(),
        ip_hash: task.ip_hash.clone(),
        workers: state.config.pow.workers,
        worker_type: state.config.pow.worker_type.clone(),
    };
    let mut frame = protocol::frame::encode_frame(protocol::frame::FRAME_TYPE_TASK_RESPONSE, encode_task_response(resp));
    deobfuscate_frame(&mut frame, XOR_KEY);

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/octet-stream"));
    (headers, frame).into_response()
}

pub async fn pow_verify(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
) -> impl IntoResponse {
    let (parts, body) = req.into_parts();
    let body = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return error_frame(StatusCode::BAD_REQUEST, "Invalid request"),
    };
    if body.is_empty() {
        return error_frame(StatusCode::BAD_REQUEST, "Invalid request");
    }

    let mut deobfuscated = body.to_vec();
    deobfuscate_frame(&mut deobfuscated, XOR_KEY);
    let (frame_type, payload) = match decode_frame(&deobfuscated) {
        Ok(res) => res,
        Err(_) => return error_frame(StatusCode::BAD_REQUEST, "Invalid request"),
    };
    if frame_type != FRAME_TYPE_VERIFY_REQUEST {
        return error_frame(StatusCode::BAD_REQUEST, "Invalid request");
    }

    let verify_req = match decode_verify_request(payload) {
        Ok(req) => req,
        Err(_) => return error_frame(StatusCode::BAD_REQUEST, "Invalid request"),
    };

    let task = match state.task_store.get(&verify_req.task_id) {
        Some(task) => task,
        None => {
            tracing::warn!(task_id = %verify_req.task_id, "task not found or expired");
            return error_frame(StatusCode::BAD_REQUEST, "Task not found or expired");
        }
    };

    if task.used {
        tracing::warn!(task_id = %task.task_id, "task already used");
        return error_frame(StatusCode::BAD_REQUEST, "Task already used");
    }
    if task.exp < OffsetDateTime::now_utc().unix_timestamp() {
        tracing::warn!(task_id = %task.task_id, "task expired");
        return error_frame(StatusCode::BAD_REQUEST, "Task expired");
    }

    let ua_hash = compute_ua_hash(headers_user_agent(&parts.headers));
    if task.ua_hash != ua_hash {
        tracing::warn!(task_id = %task.task_id, "user agent mismatch");
        return error_frame(StatusCode::BAD_REQUEST, "User agent mismatch");
    }

    if state.config.pow.ip_policy != IpPolicy::None {
        let current_ip = crypto::extract_client_ip(&parts.headers, &parts.extensions, state.config.pow.ip_policy);
        let ip_hash = compute_ip_hash(&current_ip);
        if task.ip_hash != ip_hash {
            tracing::warn!(task_id = %task.task_id, "ip address mismatch");
            return error_frame(StatusCode::BAD_REQUEST, "IP address mismatch");
        }
    }

    if !crypto::verify_pow(&task, &verify_req.nonce) {
        tracing::warn!(task_id = %task.task_id, "invalid proof of work");
        return error_frame(StatusCode::BAD_REQUEST, "Invalid proof of work");
    }

    state.task_store.mark_used(&task.task_id);

    let expire_seconds = state.config.pow.cookie_expire_hours * 3600;
    let cookie_value = generate_cookie(
        &state.server_secret,
        task.bits,
        &task.scope,
        &task.ua_hash,
        &task.ip_hash,
        &verify_req.nonce,
        expire_seconds,
    );

    let redirect = if state.config.pow.test_mode {
        format!("{}/ok", POW_PREFIX)
    } else if verify_req.redirect.is_empty() {
        "/".to_string()
    } else {
        verify_req.redirect.clone()
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/octet-stream"));
    let set_cookie = cookie::Cookie::build((POW_COOKIE_NAME, cookie_value))
        .path("/")
        .http_only(true)
        .max_age(time::Duration::seconds(expire_seconds))
        .build()
        .to_string();
    if let Ok(value) = header::HeaderValue::from_str(&set_cookie) {
        headers.insert(header::SET_COOKIE, value);
    }

    tracing::info!(task_id = %task.task_id, redirect = %redirect, "pow verified");
    let resp = BinaryVerifyResponse { redirect };
    let frame = protocol::frame::encode_frame(protocol::frame::FRAME_TYPE_VERIFY_RESPONSE, encode_verify_response(resp));
    (headers, frame).into_response()
}

pub async fn serve_asset(
    State(_state): State<Arc<AppState>>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    let file_path = format!("assets/{}", path.trim_start_matches('/'));
    let Some(bytes) = crate::static_files::get_asset(&file_path) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let content_type = content_type_for(&file_path);
    let cache_control = cache_control_for(&file_path);

    let mut headers = HeaderMap::new();
    if let Ok(value) = header::HeaderValue::from_str(content_type) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    if let Ok(value) = header::HeaderValue::from_str(cache_control) {
        headers.insert(header::CACHE_CONTROL, value);
    }
    (headers, bytes).into_response()
}

pub async fn health_ok() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

pub fn build_challenge_response(
    state: &AppState,
    headers: &HeaderMap,
    extensions: &axum::http::Extensions,
    redirect: &str,
    difficulty: i32,
) -> Response<axum::body::Body> {
    let task = match build_task(state, headers, extensions, difficulty) {
        Ok(task) => task,
        Err(err) => {
            tracing::error!(error = %err, "failed to build task");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let task_frame = match protocol::frame::encode_task_response_frame(&task, state.config.pow.workers, &state.config.pow.worker_type) {
        Ok(frame) => frame,
        Err(err) => {
            tracing::error!(error = %err, "failed to encode task response frame");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    state.task_store.set(task.clone());

    let task_b64 = base64::engine::general_purpose::STANDARD.encode(task_frame);
    let rendered = render_template(
        &state.template,
        &task_b64,
        redirect,
        &state.cowcat_image1,
        &state.cowcat_image2,
    );

    let mut headers = HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, header::HeaderValue::from_static("private, max-age=0, no-store, no-cache, must-revalidate"));
    headers.insert(header::PRAGMA, header::HeaderValue::from_static("no-cache"));
    headers.insert(header::EXPIRES, header::HeaderValue::from_static("0"));
    headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("text/html; charset=utf-8"));

    (StatusCode::FORBIDDEN, headers, rendered).into_response()
}

fn render_template(
    template: &str,
    task_data: &str,
    redirect_url: &str,
    cowcat_image1: &str,
    cowcat_image2: &str,
) -> String {
    template
        .replace("{{ TaskData }}", task_data)
        .replace("{{ RedirectURL }}", redirect_url)
        .replace("{{ CowcatImage1 }}", cowcat_image1)
        .replace("{{ CowcatImage2 }}", cowcat_image2)
}

fn build_task(
    state: &AppState,
    headers: &HeaderMap,
    extensions: &axum::http::Extensions,
    difficulty: i32,
) -> anyhow::Result<Task> {
    let ua_hash = compute_ua_hash(headers_user_agent(headers));
    let ip = crypto::extract_client_ip(headers, extensions, state.config.pow.ip_policy);
    let ip_hash = compute_ip_hash(&ip);

    let task_id = crypto::generate_random_id()?;
    let seed = crypto::generate_random_seed()?;
    let bits = clamp_difficulty(difficulty) * 4;
    let exp = OffsetDateTime::now_utc().unix_timestamp() + 120;
    let scope = headers_host(headers).unwrap_or_else(|| "unknown".to_string());

    Ok(Task {
        task_id,
        seed,
        bits,
        exp,
        scope,
        ua_hash,
        ip_hash,
        used: false,
        created_at: std::time::Instant::now(),
    })
}

fn error_frame(status: StatusCode, message: &str) -> Response<axum::body::Body> {
    let frame = encode_error_frame(message);
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/octet-stream"));
    (status, headers, frame).into_response()
}

fn headers_user_agent(headers: &HeaderMap) -> &str {
    headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
}

fn headers_host(headers: &HeaderMap) -> Option<String> {
    headers.get(header::HOST).and_then(|v| v.to_str().ok()).map(|s| s.to_string())
}

fn content_type_for(path: &str) -> &'static str {
    if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".wasm") {
        "application/wasm"
    } else if path.ends_with(".webp") {
        "image/webp"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".gif") {
        "image/gif"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".html") || path.ends_with(".htm") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if path.ends_with(".woff") || path.ends_with(".woff2") {
        "font/woff2"
    } else if path.ends_with(".ttf") {
        "font/ttf"
    } else if path.ends_with(".eot") {
        "application/vnd.ms-fontobject"
    } else {
        "application/octet-stream"
    }
}

fn cache_control_for(path: &str) -> &'static str {
    if path.contains("catpaw.worker.js")
        || path.contains("catpaw.js")
        || path.contains("catpaw.min.js")
        || path.contains("catpaw.worker.min.js")
        || path.contains("cowcat-embed.js")
        || path.contains("catpaw.html")
        || path.contains("catpaw.wasm")
    {
        "private, max-age=0, no-store, no-cache, must-revalidate, post-check=0, pre-check=0"
    } else if path.contains("webp") {
        "public, max-age=86400"
    } else {
        "public, no-cache"
    }
}

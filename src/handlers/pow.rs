use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, Request, Response, StatusCode, Uri};
use axum::response::IntoResponse;
use base64::Engine;
use http_body_util::BodyExt;
use serde::Deserialize;
use time::OffsetDateTime;

use crate::config::IpPolicy;
use crate::crypto::{compute_ip_hash, compute_ua_hash, generate_cookie};
use crate::handlers::message::*;
use crate::protocol::frame::{
    decode_frame, decode_task_request, decode_verify_request, encode_error_frame,
    encode_task_response, encode_verify_response, deobfuscate_frame, BinaryTaskResponse,
    BinaryVerifyResponse, FRAME_TYPE_TASK_REQUEST, FRAME_TYPE_VERIFY_REQUEST, XOR_KEY,
};
use crate::protocol::http::HeaderMapExt;
use crate::rules::clamp_difficulty;
use crate::state::AppState;
use crate::storage::{ConsumeError, IpHash, Scope, Seed, Task, TaskId, UaHash};
use crate::{crypto, protocol};
use crate::ip_source::ip::resolve_request_ip;

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
    build_challenge_response(&state, req.headers(), req.extensions(), &redirect, state.config.pow.difficulty).await
}

pub async fn pow_task(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
) -> impl IntoResponse {
    let (parts, body) = req.into_parts();
    let body = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return error_frame(StatusCode::BAD_REQUEST, MSG_INVALID_REQUEST),
    };
    if !body.is_empty() {
        let (frame_type, payload) = match decode_frame(&body) {
            Ok(res) => res,
            Err(_) => return error_frame(StatusCode::BAD_REQUEST, MSG_INVALID_REQUEST),
        };
        if frame_type != FRAME_TYPE_TASK_REQUEST {
            return error_frame(StatusCode::BAD_REQUEST, MSG_INVALID_REQUEST);
        }
        if decode_task_request(payload).is_err() {
            return error_frame(StatusCode::BAD_REQUEST, MSG_INVALID_REQUEST);
        }
    }

    let task = match build_task(&state, &parts.headers, &parts.extensions, state.config.pow.difficulty) {
        Ok(task) => task,
        Err(err) => {
            tracing::error!(error = %err, "{}", MSG_FAILED_TO_GENERATE_TASK);
            return error_frame(StatusCode::INTERNAL_SERVER_ERROR, MSG_FAILED_TO_GENERATE_TASK);
        }
    };
    tracing::debug!(
        task_id = %task.task_id.short_id(),
        bits = task.bits,
        scope = %task.scope,
        "{}",
        MSG_POW_TASK_CREATED
    );
    state.task_store.insert(task.clone()).await;

    let resp = BinaryTaskResponse {
        task_id: task.task_id.0.to_string(),
        seed: task.seed.0.clone(),
        bits: task.bits as i32,
        exp: task.exp,
        scope: task.scope.0.clone(),
        ua_hash: task.ua_hash.0.clone(),
        ip_hash: task.ip_hash.0.clone(),
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
        Err(_) => return error_frame(StatusCode::BAD_REQUEST, MSG_INVALID_REQUEST),
    };
    if body.is_empty() {
        return error_frame(StatusCode::BAD_REQUEST, MSG_INVALID_REQUEST);
    }

    let mut deobfuscated = body.to_vec();
    deobfuscate_frame(&mut deobfuscated, XOR_KEY);
    let (frame_type, payload) = match decode_frame(&deobfuscated) {
        Ok(res) => res,
        Err(_) => return error_frame(StatusCode::BAD_REQUEST, MSG_INVALID_REQUEST),
    };
    if frame_type != FRAME_TYPE_VERIFY_REQUEST {
        return error_frame(StatusCode::BAD_REQUEST, MSG_INVALID_REQUEST);
    }

    let verify_req = match decode_verify_request(payload) {
        Ok(req) => req,
        Err(_) => return error_frame(StatusCode::BAD_REQUEST, MSG_INVALID_REQUEST),
    };

    let ua_hash = compute_ua_hash(headers_user_agent(&parts.headers));
    let ip_for_verify = if state.config.pow.ip_policy != IpPolicy::None {
        crypto::extract_client_ip(&parts.headers, &parts.extensions, state.config.pow.ip_policy)
    } else {
        String::new()
    };
    let ip_hash = if state.config.pow.ip_policy != IpPolicy::None {
        compute_ip_hash(&ip_for_verify)
    } else {
        String::new()
    };

    let task = match state.task_store.consume_if(&verify_req.task_id, |task| {
        if task.ua_hash.0 != ua_hash {
            tracing::warn!(task_id = %task.task_id.short_id(), "{}", MSG_USER_AGENT_MISMATCH);
            return Err(ConsumeError::ValidationFailed(MSG_USER_AGENT_MISMATCH));
        }
        if state.config.pow.ip_policy != IpPolicy::None && task.ip_hash.0 != ip_hash {
            tracing::warn!(task_id = %task.task_id.short_id(), "{}", MSG_IP_ADDRESS_MISMATCH);
            return Err(ConsumeError::ValidationFailed(MSG_IP_ADDRESS_MISMATCH));
        }
        if !crypto::verify_pow(task, &verify_req.nonce) {
            tracing::warn!(task_id = %task.task_id.short_id(), "{}", MSG_INVALID_PROOF_OF_WORK);
            return Err(ConsumeError::ValidationFailed(MSG_INVALID_PROOF_OF_WORK));
        }
        Ok(())
    }).await {
        Ok(task) => task,
        Err(ConsumeError::NotFound) => {
            tracing::warn!(task_id = %TaskId::from(verify_req.task_id.as_str()).short_id(), "{}", MSG_TASK_NOT_FOUND_OR_EXPIRED);
            return error_frame(StatusCode::BAD_REQUEST, MSG_TASK_NOT_FOUND_OR_EXPIRED);
        }
        Err(ConsumeError::Expired) => {
            tracing::warn!(task_id = %TaskId::from(verify_req.task_id.as_str()).short_id(), "{}", MSG_TASK_EXPIRED);
            return error_frame(StatusCode::BAD_REQUEST, MSG_TASK_EXPIRED);
        }
        Err(ConsumeError::ValidationFailed(msg)) => {
            return error_frame(StatusCode::BAD_REQUEST, msg);
        }
    };

    let expire_seconds = state.config.pow.cookie_expire_hours * 3600;
    let cookie_value = generate_cookie(
        &state.server_secret,
        task.bits as i32,
        &task.scope.0,
        &task.ua_hash.0,
        &task.ip_hash.0,
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
    let set_cookie = if state.config.pow.secure {
        cookie::Cookie::build((POW_COOKIE_NAME, cookie_value))
            .path("/")
            .http_only(true)
            .secure(true)
            .same_site(cookie::SameSite::None)
            .max_age(time::Duration::seconds(expire_seconds))
            .build()
            .to_string()
    } else {
        cookie::Cookie::build((POW_COOKIE_NAME, cookie_value))
            .path("/")
            .http_only(true)
            .max_age(time::Duration::seconds(expire_seconds))
            .build()
            .to_string()
    };
    
    if let Ok(value) = header::HeaderValue::from_str(&set_cookie) {
        headers.insert(header::SET_COOKIE, value);
    }
    // 收集和打印用户信息
    let user_agent = headers_user_agent(&parts.headers);
    let accept_language = parts.headers.get_string_or_default(header::ACCEPT_LANGUAGE);
    //let path = parts.uri.path();
    let host = headers_host(&parts.headers).unwrap_or_default();
    
    // 提取并格式化计算时间
    let elapsed = extract_and_format_compute_time(&parts.uri);

    let final_ip = resolve_request_ip(&parts.headers, &parts.extensions);
    
    // 根据是否有计算时间，使用不同的日志格式
    if let Some(time_str) = &elapsed {
        tracing::info!(
            task_id = %task.task_id.short_id(),
            client_ip = %final_ip.0,
            ip_source = %final_ip.1.get_string(),
            accept_language = %accept_language,
            user_agent = %user_agent,
            host = %host,
            redirect = %redirect,
            elapsed = %time_str,
            "{}",
            MSG_POW_VERIFIED
        );
    } else {
        tracing::info!(
            task_id = %task.task_id.short_id(),
            client_ip = %final_ip.0,
            ip_source = %final_ip.1.get_string(),
            accept_language = %accept_language,
            user_agent = %user_agent,
            host = %host,
            redirect = %redirect,
            "{}",
            MSG_POW_VERIFIED
        );
    }
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

pub async fn build_challenge_response(
    state: &AppState,
    headers: &HeaderMap,
    extensions: &axum::http::Extensions,
    redirect: &str,
    difficulty: i32,
) -> Response<axum::body::Body> {
    let task = match build_task(state, headers, extensions, difficulty) {
        Ok(task) => task,
        Err(err) => {
            tracing::error!(error = %err, "{}", MSG_FAILED_TO_GENERATE_TASK);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let task_frame = match protocol::frame::encode_task_response_frame(&task, state.config.pow.workers, &state.config.pow.worker_type) {
        Ok(frame) => frame,
        Err(err) => {
            tracing::error!(error = %err, "{}", MSG_FAILED_TO_ENCODE_TASK_RESPONSE_FRAME);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    state.task_store.insert(task.clone()).await;

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
    let ip_for_verify = if state.config.pow.ip_policy != IpPolicy::None {
        crypto::extract_client_ip(headers, extensions, state.config.pow.ip_policy)
    } else {
        String::new()
    };
    let ip_hash = if state.config.pow.ip_policy != IpPolicy::None {
        compute_ip_hash(&ip_for_verify)
    } else {
        String::new()
    };

    let task_id = crypto::generate_random_id()?;
    let seed = crypto::generate_random_seed()?;
    let bits = (clamp_difficulty(difficulty) * 4) as u32;
    let exp = OffsetDateTime::now_utc().unix_timestamp() + 120;
    let scope = headers_host(headers).unwrap_or_else(|| "unknown".to_string());

    Ok(Task {
        task_id: TaskId::from(task_id),
        seed: Seed(seed),
        bits,
        exp,
        scope: Scope(scope),
        ua_hash: UaHash(ua_hash),
        ip_hash: IpHash(ip_hash),
    })
}

fn error_frame(status: StatusCode, message: &str) -> Response<axum::body::Body> {
    let frame = encode_error_frame(message);
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/octet-stream"));
    (status, headers, frame).into_response()
}

fn headers_user_agent(headers: &HeaderMap) -> &str {
    headers.get_str(header::USER_AGENT).unwrap_or_default()
}

fn headers_host(headers: &HeaderMap) -> Option<String> {
    headers.get_string(header::HOST)
}

fn extract_and_format_compute_time(uri: &Uri) -> Option<String> {
    let query = uri.query()?;
    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            if key == "compute_time" {
                if let Ok(ms) = value.parse::<u64>() {
                    return Some(format_compute_time(ms));
                }
            }
        }
    }
    None
}

fn format_compute_time(ms: u64) -> String {
    if ms<1000 {
        return format!("{}ms", ms);
    }
    format!("{:.2}s", ms as f64 / 1000.0)
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

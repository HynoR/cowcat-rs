use std::sync::Arc;

use axum::http::{header, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::config::IpPolicy;
use crate::crypto::{compute_ip_hash, compute_ua_hash};
use crate::handlers::pow::{build_challenge_response, POW_COOKIE_NAME, POW_PREFIX};
use crate::rules::{RuleAction, RuleDecision};
use crate::state::AppState;

pub async fn pow_gate(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    tracing::debug!(method = %req.method(), path = %req.uri().path(), "pow gate check");
    if state.config.pow.difficulty == 0 {
        tracing::debug!("pow disabled (difficulty=0)");
        return next.run(req).await;
    }

    if is_pow_path(req.uri().path()) {
        tracing::debug!("pow bypass for internal route");
        return next.run(req).await;
    }

    if is_service_worker_request(&req) {
        tracing::debug!("pow bypass for service worker request");
        return next.run(req).await;
    }

    if state.config.pow.test_mode {
        tracing::info!("pow test mode enabled: forcing challenge");
        return build_challenge_response(
            &state,
            req.headers(),
            req.extensions(),
            redirect_target(&req),
            state.config.pow.difficulty,
        );
    }

    if let Some(cookie) = extract_cookie(req.headers()) {
        if verify_cookie(&state, &req, &cookie) {
            tracing::debug!("pow cookie verified");
            return next.run(req).await;
        }
        tracing::debug!("pow cookie invalid");
    }

    if let Some(decision) = evaluate_rules(&state, &req) {
        return match decision.action {
            RuleAction::Allow => {
                tracing::info!("rule decision: allow");
                next.run(req).await
            }
            RuleAction::Block => {
                tracing::info!("rule decision: block");
                StatusCode::FORBIDDEN.into_response()
            }
            RuleAction::Challenge => {
                let base = state.config.pow.difficulty;
                let effective = crate::rules::clamp_difficulty(base + decision.difficulty_delta);
                tracing::info!(base, delta = decision.difficulty_delta, effective, "rule decision: challenge");
                if effective == 0 {
                    next.run(req).await
                } else {
                    build_challenge_response(
                        &state,
                        req.headers(),
                        req.extensions(),
                        redirect_target(&req),
                        effective,
                    )
                }
            }
        };
    }

    tracing::info!(difficulty = state.config.pow.difficulty, "pow challenge (default)");
    build_challenge_response(
        &state,
        req.headers(),
        req.extensions(),
        redirect_target(&req),
        state.config.pow.difficulty,
    )
}

fn evaluate_rules(state: &AppState, req: &Request<axum::body::Body>) -> Option<RuleDecision> {
    let ip = crate::crypto::extract_client_ip(req.headers(), req.extensions(), state.config.pow.ip_policy);
    let ip_addr = crate::crypto::parse_ip(&ip);
    state.rules.evaluate(req.uri().path(), req.headers(), ip_addr)
}

fn is_pow_path(path: &str) -> bool {
    path.starts_with(POW_PREFIX)
}

fn is_service_worker_request(req: &Request<axum::body::Body>) -> bool {
    let method = req.method();
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return false;
    }
    let dest = req
        .headers()
        .get("sec-fetch-dest")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let sw = req
        .headers()
        .get("service-worker")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if dest != "serviceworker" && sw != "script" {
        return false;
    }
    let path = req.uri().path().to_ascii_lowercase();
    path.ends_with(".js") || path.ends_with(".mjs")
}

fn redirect_target(req: &Request<axum::body::Body>) -> &str {
    req.uri()
        .path_and_query()
        .map(|p| p.as_str())
        .unwrap_or_else(|| req.uri().path())
}

fn extract_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for cookie in cookie::Cookie::split_parse(raw).flatten() {
        if cookie.name() == POW_COOKIE_NAME {
            return Some(cookie.value().to_string());
        }
    }
    None
}

fn verify_cookie(state: &AppState, req: &Request<axum::body::Body>, value: &str) -> bool {
    tracing::debug!("verifying pow cookie: {}", value);
    let payload = match crate::crypto::verify_cookie(&state.server_secret, value) {
        Some(payload) => payload,
        None => {
            tracing::debug!("pow cookie signature/expiry invalid");
            return false;
        }
    };
    let ua_hash = compute_ua_hash(
        req.headers()
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default(),
    );
    if payload.ua != ua_hash {
        tracing::debug!(
            payload_ua = %payload.ua,
            request_ua = %ua_hash,
            "pow cookie user agent mismatch"
        );
        return false;
    }
    if state.config.pow.ip_policy != IpPolicy::None {
        let ip = crate::crypto::extract_client_ip(req.headers(), req.extensions(), state.config.pow.ip_policy);
        let ip_hash = compute_ip_hash(&ip);
        if ip.is_empty() {
            tracing::debug!("pow cookie missing client ip under ip_policy");
        }
        let payload_ip = payload.ip.as_deref().unwrap_or_default();
        if payload_ip != ip_hash {
            tracing::debug!(
                payload_ip = %payload_ip,
                request_ip = %ip_hash,
                "pow cookie ip mismatch"
            );
            return false;
        }
    }
    true
}

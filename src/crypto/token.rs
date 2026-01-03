use base64::Engine;
use ring::hmac;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenPayload {
    pub v: String,
    pub exp: i64,
    pub bits: i32,
    pub scope: String,
    pub ua: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    pub nonce: String,
}

pub fn generate_cookie(
    secret: &str,
    bits: i32,
    scope: &str,
    ua_hash: &str,
    ip_hash: &str,
    nonce: &str,
    duration_seconds: i64,
) -> String {
    let exp = OffsetDateTime::now_utc().unix_timestamp() + duration_seconds;
    let ip_value = if ip_hash.is_empty() { None } else { Some(ip_hash.to_string()) };
    let payload = TokenPayload {
        v: "v1".to_string(),
        exp,
        bits,
        scope: scope.to_string(),
        ua: ua_hash.to_string(),
        ip: ip_value,
        nonce: nonce.to_string(),
    };

    let payload_json = match serde_json::to_vec(&payload) {
        Ok(data) => data,
        Err(_) => return String::new(),
    };

    let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json);
    let sig = sign(secret, payload_b64.as_bytes());
    format!("{payload_b64}.{sig}")
}

pub fn verify_cookie(secret: &str, token: &str) -> Option<TokenPayload> {
    let token = token.trim().trim_matches('"');
    let (payload_b64_raw, sig_raw) = split_token(token)?;
    let payload_b64 = payload_b64_raw.trim_end_matches('=');
    let sig = sig_raw.trim_end_matches('=');
    let expected = sign(secret, payload_b64.as_bytes());
    tracing::debug!("expected: {}", expected);
    tracing::debug!("sig: {}", sig);
    if sig != expected {
        tracing::debug!("pow cookie signature mismatch");
        return None;
    }
    let payload_json = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload_b64) {
        Ok(data) => data,
        Err(err) => {
            tracing::debug!(error = %err, payload_len = payload_b64.len(), "pow cookie payload base64 decode failed");
            return None;
        }
    };
    let payload: TokenPayload = match serde_json::from_slice(&payload_json) {
        Ok(value) => value,
        Err(err) => {
            tracing::debug!(error = %err, "pow cookie payload json decode failed");
            return None;
        }
    };
    if payload.v != "v1" {
        tracing::debug!("pow cookie version mismatch");
        return None;
    }
    if payload.exp < OffsetDateTime::now_utc().unix_timestamp() {
        tracing::debug!("pow cookie expired");
        return None;
    }
    tracing::debug!("pow cookie verified: {:?}", payload);
    if payload.nonce.is_empty() {
        tracing::debug!("pow cookie nonce is empty");
        return None;
    }
    Some(payload)
}

fn split_token(token: &str) -> Option<(&str, &str)> {
    let mut iter = token.splitn(2, '.');
    let payload = iter.next()?;
    let sig = iter.next()?;
    if payload.is_empty() || sig.is_empty() {
        None
    } else {
        Some((payload, sig))
    }
}

fn sign(secret: &str, message: &[u8]) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
    let tag = hmac::sign(&key, message);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(tag.as_ref())
}

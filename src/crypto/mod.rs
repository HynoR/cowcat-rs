mod pow;
mod token;

use std::net::IpAddr;

use axum::http::HeaderMap;
use axum::http::header;
use axum::http::Extensions;
use axum::extract::connect_info::ConnectInfo;
use ring::rand::{SecureRandom, SystemRandom};
use base64::Engine;

use crate::config::IpPolicy;

pub use pow::verify_pow;
pub use token::{generate_cookie, verify_cookie};

pub fn compute_ua_hash(user_agent: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, user_agent.as_bytes());
    base64::engine::general_purpose::URL_SAFE.encode(&digest.as_ref()[..8])
}

pub fn compute_ip_hash(ip: &str) -> String {
    if ip.is_empty() {
        return String::new();
    }
    let digest = ring::digest::digest(&ring::digest::SHA256, ip.as_bytes());
    base64::engine::general_purpose::URL_SAFE.encode(&digest.as_ref()[..8])
}

pub fn generate_random_id() -> anyhow::Result<String> {
    let rng = SystemRandom::new();
    let mut buf = vec![0u8; 16];
    rng.fill(&mut buf).map_err(|_| anyhow::anyhow!("random id failed"))?;
    Ok(hex::encode(buf))
}

pub fn generate_random_seed() -> anyhow::Result<String> {
    let rng = SystemRandom::new();
    let mut buf = vec![0u8; 32];
    rng.fill(&mut buf)
        .map_err(|_| anyhow::anyhow!("random seed failed"))?;
    Ok(base64::engine::general_purpose::URL_SAFE.encode(buf))
}

pub fn extract_client_ip(headers: &HeaderMap, extensions: &Extensions, policy: IpPolicy) -> String {
    match policy {
        IpPolicy::None => String::new(),
        IpPolicy::Enable => {
            if let Some(ip) = header_ip(headers, header::HeaderName::from_static("x-forwarded-for")) {
                return ip;
            }
            if let Some(ip) = header_ip(headers, header::HeaderName::from_static("x-real-ip")) {
                return ip;
            }
            remote_ip(extensions).unwrap_or_default()
        }
        IpPolicy::Strict => remote_ip(extensions).unwrap_or_default(),
    }
}

fn header_ip(headers: &HeaderMap, name: header::HeaderName) -> Option<String> {
    let value = headers.get(name)?;
    let value = value.to_str().ok()?;
    let first = value.split(',').next()?.trim();
    if first.is_empty() {
        None
    } else {
        Some(first.to_string())
    }
}

fn remote_ip(extensions: &Extensions) -> Option<String> {
    let info = extensions.get::<ConnectInfo<std::net::SocketAddr>>()?;
    Some(info.0.ip().to_string())
}

pub fn parse_ip(ip: &str) -> Option<IpAddr> {
    let trimmed = ip.trim();
    if trimmed.is_empty() {
        None
    } else {
        trimmed.parse::<IpAddr>().ok()
    }
}

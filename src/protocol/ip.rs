use axum::extract::connect_info::ConnectInfo;
use axum::http::{header, HeaderMap};
use axum::http::Extensions;

use crate::protocol::http::HeaderMapExt;

pub enum IpSource {
    ClientIp,
    XForwardedFor,
    XRealIp,
}

impl IpSource {
    pub fn get_string(&self) -> String {
        match self {
            IpSource::ClientIp => "client_ip".to_string(),
            IpSource::XForwardedFor => "x_forwarded_for".to_string(),
            IpSource::XRealIp => "x_real_ip".to_string(),
        }
    }
}

pub fn resolve_request_ip(headers: &HeaderMap, extensions: &Extensions) -> (String, IpSource) {
    if let Some(ip) = headers.get_ip(header::HeaderName::from_static("x-real-ip")) {
        return (ip, IpSource::XRealIp);
    }
    if let Some(ip) = headers.get_ip(header::HeaderName::from_static("x-forwarded-for")) {
        return (ip, IpSource::XForwardedFor);
    }
    let ip = remote_ip(extensions).unwrap_or_default();
    (ip, IpSource::ClientIp)
}

fn remote_ip(extensions: &Extensions) -> Option<String> {
    let info = extensions.get::<ConnectInfo<std::net::SocketAddr>>()?;
    Some(info.0.ip().to_string())
}

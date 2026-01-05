use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use bytes::Bytes;
use ring::rand::{SecureRandom, SystemRandom};

use crate::bot::BotState;
use crate::config::Config;
use crate::rules::RulesEngine;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;

use crate::storage::TaskStore;

#[derive(Clone)]
pub struct ProxyTarget {
    pub uri: Uri,
    pub host_value: HeaderValue,
    pub x_forwarded_host: HeaderValue,
    pub x_forwarded_proto: HeaderValue,
}

#[derive(Clone)]
pub struct FaviconCache {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub cached_at: Instant,
}

impl FaviconCache {
    pub fn is_valid(&self) -> bool {
        self.cached_at.elapsed() < Duration::from_secs(3600) // 1 hour
    }
}

pub struct AppState {
    pub config: Config,
    pub rules: RulesEngine,
    pub bot: BotState,
    pub task_store: Arc<TaskStore>,
    pub server_secret: String,
    pub template: String,
    pub cowcat_image1: String,
    pub cowcat_image2: String,
    pub proxy_client: Client<HttpConnector, axum::body::Body>,
    pub favicon_cache: Arc<tokio::sync::RwLock<Option<FaviconCache>>>,
    pub proxy_target: ProxyTarget,
}

impl AppState {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        let rules = RulesEngine::from_config(&config.rules)?;
        let bot = BotState::new(&config.rules)?;
        let task_store = TaskStore::new();
        let server_secret = build_server_secret(&config.pow.salt)?;
        tracing::debug!("server secret: {}", server_secret);
        let (template, cowcat_image1, cowcat_image2) = crate::static_files::load_template_assets()?;

        let proxy_client = Client::builder(TokioExecutor::new()).build(HttpConnector::new());

        // 预解析代理目标配置
        let proxy_target = {
            let target_uri = config.proxy.target.parse::<Uri>()
                .map_err(|err| anyhow::anyhow!("invalid proxy target: {err}"))?;
            let host_string = target_uri.authority()
                .ok_or_else(|| anyhow::anyhow!("proxy target missing authority"))?
                .to_string();
            let scheme = target_uri.scheme_str().unwrap_or("http").to_string();
            
            let host_value = HeaderValue::from_str(&host_string)
                .map_err(|err| anyhow::anyhow!("invalid host header value: {err}"))?;
            let scheme_value = HeaderValue::from_str(&scheme)
                .map_err(|err| anyhow::anyhow!("invalid scheme header value: {err}"))?;
            
            ProxyTarget {
                uri: target_uri,
                host_value: host_value.clone(),
                x_forwarded_host: host_value,
                x_forwarded_proto: scheme_value,
            }
        };

        Ok(Self {
            config,
            rules,
            bot,
            task_store,
            server_secret,
            template,
            cowcat_image1,
            cowcat_image2,
            proxy_client,
            favicon_cache: Arc::new(tokio::sync::RwLock::new(None)),
            proxy_target,
        })
    }
}

fn build_server_secret(salt: &str) -> anyhow::Result<String> {
    let trimmed = salt.trim();
    if !trimmed.is_empty() {
        tracing::info!("secret(config): {}", trimmed);
        return Ok(pad_secret(trimmed, 32));
    }
    let rng = SystemRandom::new();
    let mut buf = vec![0u8; 16];
    rng.fill(&mut buf).map_err(|_| anyhow::anyhow!("failed to generate secret"))?;
    let encoded = hex::encode(buf);
    tracing::info!("secret(generated): {}", encoded);
    Ok(pad_secret(&encoded, 32))
}

fn pad_secret(value: &str, min_len: usize) -> String {
    if value.len() >= min_len {
        return value.to_string();
    }
    let mut out = String::with_capacity(min_len);
    out.push_str(value);
    while out.len() < min_len {
        out.push('0');
    }
    out
}

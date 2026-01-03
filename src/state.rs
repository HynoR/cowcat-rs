use std::sync::Arc;

use ring::rand::{SecureRandom, SystemRandom};

use crate::config::Config;
use crate::rules::RulesEngine;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;

use crate::storage::TaskStore;

pub struct AppState {
    pub config: Config,
    pub rules: RulesEngine,
    pub task_store: Arc<TaskStore>,
    pub server_secret: String,
    pub template: String,
    pub cowcat_image1: String,
    pub cowcat_image2: String,
    pub proxy_client: Client<HttpConnector, axum::body::Body>,
}

impl AppState {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        let rules = RulesEngine::from_config(&config.rules)?;
        let task_store = TaskStore::new();
        let server_secret = build_server_secret(&config.pow.salt)?;
        tracing::debug!("server secret: {}", server_secret);
        let (template, cowcat_image1, cowcat_image2) = crate::static_files::load_template_assets()?;

        let proxy_client = Client::builder(TokioExecutor::new()).build(HttpConnector::new());

        Ok(Self {
            config,
            rules,
            task_store,
            server_secret,
            template,
            cowcat_image1,
            cowcat_image2,
            proxy_client,
        })
    }
}

fn build_server_secret(salt: &str) -> anyhow::Result<String> {
    let trimmed = salt.trim();
    if !trimmed.is_empty() {
        return Ok(pad_secret(trimmed, 32));
    }
    let rng = SystemRandom::new();
    let mut buf = vec![0u8; 16];
    rng.fill(&mut buf).map_err(|_| anyhow::anyhow!("failed to generate secret"))?;
    let encoded = hex::encode(buf);
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

use std::fs;

use serde::Deserialize;

use crate::rules::RuleAction;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub pow: PowConfig,
    pub proxy: ProxyConfig,
    pub rules: RulesConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            pow: PowConfig::default(),
            proxy: ProxyConfig::default(),
            rules: RulesConfig::default(),
        }
    }
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let raw = match fs::read_to_string(path) {
            Ok(data) => data,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!(path, "config not found, using defaults");
                String::new()
            }
            Err(err) => {
                return Err(anyhow::anyhow!("failed to read config {path}: {err}"));
            }
        };
        let mut cfg: Config = if raw.trim().is_empty() {
            Config::default()
        } else {
            toml::from_str(&raw)
                .map_err(|err| anyhow::anyhow!("failed to parse config {path}: {err}"))?
        };
        cfg.apply_defaults();
        cfg.validate()?;
        Ok(cfg)
    }

    fn apply_defaults(&mut self) {
        let defaults = Config::default();
        if self.server.listen.trim().is_empty() {
            self.server.listen = defaults.server.listen;
        }
        if self.pow.difficulty == 0 {
            self.pow.difficulty = defaults.pow.difficulty;
        }
        if self.pow.cookie_expire_hours <= 0 {
            self.pow.cookie_expire_hours = defaults.pow.cookie_expire_hours;
        }
        if self.pow.workers <= 0 {
            self.pow.workers = defaults.pow.workers;
        }
        if self.pow.worker_type.trim().is_empty() {
            self.pow.worker_type = defaults.pow.worker_type;
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.pow.difficulty < 0 || self.pow.difficulty > 10 {
            anyhow::bail!("pow.difficulty must be within 0..=10");
        }
        if self.pow.workers < 1 || self.pow.workers > 8 {
            anyhow::bail!("pow.workers must be within 1..=8");
        }
        let worker = self.pow.worker_type.as_str();
        if worker != "wasm" && worker != "native" {
            anyhow::bail!("pow.worker_type must be wasm or native");
        }
        Ok(())
    }

    pub fn print_config(&self) {
        tracing::info!("config: {:?}", self);
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub listen: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen: "0.0.0.0:8080".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PowConfig {
    pub difficulty: i32,
    pub cookie_expire_hours: i64,
    pub salt: String,
    pub workers: i32,
    pub worker_type: String,
    pub ip_policy: IpPolicy,
    pub test_mode: bool,
}

impl Default for PowConfig {
    fn default() -> Self {
        Self {
            difficulty: 3,
            cookie_expire_hours: 24,
            salt: String::new(),
            workers: 4,
            worker_type: "wasm".to_string(),
            ip_policy: IpPolicy::None,
            test_mode: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ProxyConfig {
    pub target: String,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            target: "http://127.0.0.1:1234".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IpPolicy {
    None,
    Enable,
    Strict,
}

impl Default for IpPolicy {
    fn default() -> Self {
        IpPolicy::None
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RulesConfig {
    pub enabled: bool,
    pub default_action: RuleAction,
    pub rule: Vec<RuleConfig>,
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_action: RuleAction::Challenge,
            rule: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RuleConfig {
    pub name: Option<String>,
    pub action: RuleAction,
    pub difficulty_delta: Option<i32>,
    pub path_prefix: Option<String>,
    pub path_exact: Option<String>,
    pub header: Option<HeaderMatch>,
    pub ip_cidr: Option<Vec<String>>,
}

impl Default for RuleConfig {
    fn default() -> Self {
        Self {
            name: None,
            action: RuleAction::Challenge,
            difficulty_delta: None,
            path_prefix: None,
            path_exact: None,
            header: None,
            ip_cidr: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct HeaderMatch {
    pub name: String,
    pub equals: Option<String>,
    pub contains: Option<String>,
}

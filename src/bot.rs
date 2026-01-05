use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use anyhow::Context;
use ipnet::IpNet;
use tokio::sync::RwLock;
use trust_dns_resolver::config::{
    NameServerConfig, NameServerConfigGroup, Protocol, ResolverConfig, ResolverOpts,
};
use trust_dns_resolver::TokioAsyncResolver;

use crate::config::RulesConfig;

const BOT_UA_KEYWORD: &str = "googlebot";
const STRICT_SUFFIXES: [&str; 2] = [".googlebot.com", ".google.com"];
const DENY_TTL_SECS: u64 = 24 * 60 * 60;

const GOOGLEBOT_KEYWORD: &str = "Googlebot";
const GOOGLE_PTR_SUFFIX: [&str; 2] = [".googlebot.com", ".google.com"];

const BINGBOT_KEYWORD: &str = "bingbot";
const BING_PTR_SUFFIX: [&str; 2] = [".bingbot.com", ".msn.com"];

pub struct BotState {
    resolver: TokioAsyncResolver,
    allow_prefixes: Vec<IpNet>,
    allow_ips: RwLock<HashSet<IpAddr>>,
    deny_ips: RwLock<HashMap<IpAddr, Instant>>,
    deny_ttl: Duration,
}

impl BotState {
    pub fn new(cfg: &RulesConfig) -> anyhow::Result<Self> {
        let resolver = build_resolver(cfg.bot_dns.as_deref())?;
        let allow_prefixes = match cfg.bot_allowlist_file.as_deref() {
            Some(path) => load_allowlist_file(path)?,
            None => Vec::new(),
        };
        Ok(Self {
            resolver,
            allow_prefixes,
            allow_ips: RwLock::new(HashSet::new()),
            deny_ips: RwLock::new(HashMap::new()),
            deny_ttl: Duration::from_secs(DENY_TTL_SECS),
        })
    }

    pub fn ua_matches_bot(ua: &str) -> bool {
        let ua = ua.trim();
        if ua.is_empty() {
            return false;
        }
        ua.to_ascii_lowercase().contains(BOT_UA_KEYWORD)
    }

    pub async fn is_strict_bot(&self, ua: &str, ip: Option<IpAddr>) -> bool {
        if !Self::ua_matches_bot(ua) {
            return false;
        }
        let Some(ip) = ip else {
            return false;
        };
        if self.is_allowlisted(ip).await {
            return true;
        }
        if self.is_denylisted(ip).await {
            return false;
        }

        let verified = self.strict_dns_verify(ip).await;
        if verified {
            self.add_allow(ip).await;
        } else {
            self.add_deny(ip).await;
        }
        verified
    }

    async fn is_allowlisted(&self, ip: IpAddr) -> bool {
        if self.allow_prefixes.iter().any(|net| net.contains(&ip)) {
            return true;
        }
        let allow_ips = self.allow_ips.read().await;
        allow_ips.contains(&ip)
    }

    async fn add_allow(&self, ip: IpAddr) {
        let mut allow_ips = self.allow_ips.write().await;
        allow_ips.insert(ip);
    }

    async fn is_denylisted(&self, ip: IpAddr) -> bool {
        let mut deny_ips = self.deny_ips.write().await;
        if let Some(ts) = deny_ips.get(&ip) {
            if ts.elapsed() < self.deny_ttl {
                return true;
            }
            deny_ips.remove(&ip);
        }
        false
    }

    async fn add_deny(&self, ip: IpAddr) {
        let mut deny_ips = self.deny_ips.write().await;
        deny_ips.insert(ip, Instant::now());
    }

    async fn strict_dns_verify(&self, ip: IpAddr) -> bool {
        let lookup = match self.resolver.reverse_lookup(ip).await {
            Ok(lookup) => lookup,
            Err(err) => {
                tracing::debug!(error = %err, "bot strict reverse lookup failed");
                return false;
            }
        };
        let ptr_name = match lookup.iter().next() {
            Some(name) => name.to_utf8(),
            None => return false,
        };
        let ptr = normalize_ptr(&ptr_name);
        if !ptr_allowed(&ptr) {
            return false;
        }
        let forward = match self.resolver.lookup_ip(ptr.as_str()).await {
            Ok(lookup) => lookup,
            Err(err) => {
                tracing::debug!(error = %err, "bot strict forward lookup failed");
                return false;
            }
        };
        if !forward.iter().any(|addr| addr == ip) {
            return false;
        }
        true
    }
}

fn build_resolver(servers: Option<&[String]>) -> anyhow::Result<TokioAsyncResolver> {
    let Some(servers) = servers else {
        return TokioAsyncResolver::tokio_from_system_conf()
            .context("failed to load system DNS config");
    };
    let mut addrs = Vec::new();
    for raw in servers {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let addr = parse_dns_server(trimmed)
            .with_context(|| format!("invalid bot_dns entry: {trimmed}"))?;
        addrs.push(addr);
    }
    if addrs.is_empty() {
        return TokioAsyncResolver::tokio_from_system_conf()
            .context("failed to load system DNS config");
    }

    let mut name_servers = NameServerConfigGroup::new();
    for addr in addrs {
        name_servers.push(NameServerConfig {
            socket_addr: addr,
            protocol: Protocol::Udp,
            tls_dns_name: None,
            trust_negative_responses: false,
            bind_addr: None,
        });
        name_servers.push(NameServerConfig {
            socket_addr: addr,
            protocol: Protocol::Tcp,
            tls_dns_name: None,
            trust_negative_responses: false,
            bind_addr: None,
        });
    }
    let config = ResolverConfig::from_parts(None, vec![], name_servers);
    Ok(TokioAsyncResolver::tokio(config, ResolverOpts::default()))
}

fn parse_dns_server(raw: &str) -> anyhow::Result<SocketAddr> {
    if let Ok(addr) = raw.parse::<SocketAddr>() {
        return Ok(addr);
    }
    let ip = raw
        .parse::<IpAddr>()
        .with_context(|| format!("invalid DNS server address: {raw}"))?;
    Ok(SocketAddr::new(ip, 53))
}

fn load_allowlist_file(path: &str) -> anyhow::Result<Vec<IpNet>> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read bot allowlist file: {path}"))?;
    let mut nets = Vec::new();
    for (idx, line) in contents.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let net = parse_allowlist_entry(trimmed)
            .with_context(|| format!("invalid allowlist entry at line {}: {}", idx + 1, trimmed))?;
        nets.push(net);
    }
    Ok(nets)
}

fn parse_allowlist_entry(raw: &str) -> anyhow::Result<IpNet> {
    if let Ok(net) = raw.parse::<IpNet>() {
        return Ok(net);
    }
    let ip = raw.parse::<IpAddr>()?;
    let prefix = if ip.is_ipv4() { 32 } else { 128 };
    Ok(IpNet::new(ip, prefix)?)
}

fn normalize_ptr(ptr: &str) -> String {
    ptr.trim_end_matches('.').to_ascii_lowercase()
}

fn ptr_allowed(ptr: &str) -> bool {
    STRICT_SUFFIXES.iter().any(|suffix| ptr.ends_with(suffix))
}

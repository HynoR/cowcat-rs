use axum::http::HeaderMap;
use ipnet::IpNet;
use serde::Deserialize;
use std::net::IpAddr;

use crate::config::{HeaderMatch, RulesConfig};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Block,
    Challenge,
}

#[derive(Debug, Clone)]
pub struct RulesEngine {
    enabled: bool,
    default_action: RuleAction,
    rules: Vec<Rule>,
}

#[derive(Debug, Clone)]
struct Rule {
    name: Option<String>,
    action: RuleAction,
    difficulty_delta: i32,
    matcher: Matcher,
}

#[derive(Debug, Clone)]
struct Matcher {
    path_prefix: Option<String>,
    path_exact: Option<String>,
    header: Option<HeaderPredicate>,
    ip_nets: Vec<IpNet>,
}

#[derive(Debug, Clone)]
struct HeaderPredicate {
    name: String,
    equals: Option<String>,
    contains: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuleDecision {
    pub action: RuleAction,
    pub difficulty_delta: i32,
}

impl RulesEngine {
    pub fn from_config(cfg: &RulesConfig) -> anyhow::Result<Self> {
        let mut rules = Vec::new();
        for rule_cfg in &cfg.rule {
            let ip_nets = parse_ip_nets(rule_cfg.ip_cidr.as_deref().unwrap_or_default())?;
            let header = rule_cfg.header.as_ref().map(to_header_predicate).transpose()?;
            let matcher = Matcher {
                path_prefix: rule_cfg.path_prefix.clone(),
                path_exact: rule_cfg.path_exact.clone(),
                header,
                ip_nets,
            };
            let rule = Rule {
                name: rule_cfg.name.clone(),
                action: rule_cfg.action.clone(),
                difficulty_delta: rule_cfg.difficulty_delta.unwrap_or(0),
                matcher,
            };
            rules.push(rule);
        }
        Ok(Self {
            enabled: cfg.enabled,
            default_action: cfg.default_action.clone(),
            rules,
        })
    }

    pub fn evaluate(
        &self,
        path: &str,
        headers: &HeaderMap,
        client_ip: Option<IpAddr>,
    ) -> Option<RuleDecision> {
        if !self.enabled {
            return None;
        }
        for rule in &self.rules {
            if rule.matcher.is_match(path, headers, client_ip) {
                tracing::info!(rule = rule.name.as_deref().unwrap_or("unnamed"), "rule matched");
                return Some(RuleDecision {
                    action: rule.action.clone(),
                    difficulty_delta: rule.difficulty_delta,
                });
            }
        }
        Some(RuleDecision {
            action: self.default_action.clone(),
            difficulty_delta: 0,
        })
    }
}

impl Matcher {
    fn is_match(&self, path: &str, headers: &HeaderMap, client_ip: Option<IpAddr>) -> bool {
        if let Some(prefix) = &self.path_prefix {
            if !path.starts_with(prefix) {
                return false;
            }
        }
        if let Some(exact) = &self.path_exact {
            if path != exact {
                return false;
            }
        }
        if let Some(predicate) = &self.header {
            if !predicate.is_match(headers) {
                return false;
            }
        }
        if !self.ip_nets.is_empty() {
            let Some(ip) = client_ip else {
                return false;
            };
            if !self.ip_nets.iter().any(|net| net.contains(&ip)) {
                return false;
            }
        }
        if self.path_prefix.is_none()
            && self.path_exact.is_none()
            && self.header.is_none()
            && self.ip_nets.is_empty()
        {
            return true;
        }
        true
    }
}

impl HeaderPredicate {
    fn is_match(&self, headers: &HeaderMap) -> bool {
        let Some(value) = headers.get(&self.name) else {
            return false;
        };
        let Ok(value) = value.to_str() else {
            return false;
        };
        let value_lower = value.to_ascii_lowercase();
        if let Some(expected) = &self.equals {
            return value_lower == expected.to_ascii_lowercase();
        }
        if let Some(contains) = &self.contains {
            return value_lower.contains(&contains.to_ascii_lowercase());
        }
        true
    }
}

fn parse_ip_nets(values: &[String]) -> anyhow::Result<Vec<IpNet>> {
    let mut nets = Vec::new();
    for raw in values {
        let net: IpNet = raw
            .parse()
            .map_err(|err| anyhow::anyhow!("invalid ip_cidr {raw}: {err}"))?;
        nets.push(net);
    }
    Ok(nets)
}

fn to_header_predicate(match_cfg: &HeaderMatch) -> anyhow::Result<HeaderPredicate> {
    let name = match_cfg.name.trim();
    if name.is_empty() {
        anyhow::bail!("header.name must be set");
    }
    if match_cfg.equals.is_none() && match_cfg.contains.is_none() {
        anyhow::bail!("header must set equals or contains");
    }
    Ok(HeaderPredicate {
        name: name.to_string(),
        equals: match_cfg.equals.clone(),
        contains: match_cfg.contains.clone(),
    })
}

pub fn clamp_difficulty(value: i32) -> i32 {
    value.clamp(0, 10)
}

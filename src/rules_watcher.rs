use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::{EventKind, RecursiveMode, Watcher};

use crate::config::{Config, RulesConfig};
use crate::rules::RulesEngine;
use crate::state::AppState;

const RELOAD_COOLDOWN: Duration = Duration::from_secs(2);

pub fn start_rules_watcher(state: Arc<AppState>, config_path: String) {
    let (watch_path, is_external) = resolve_watch_path(&state.config, &config_path);
    let watch_dir = watch_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let watch_filename = match watch_path.file_name() {
        Some(f) => f.to_os_string(),
        None => {
            tracing::error!(
                "cannot determine filename to watch: {}",
                watch_path.display()
            );
            return;
        }
    };

    tracing::info!(
        path = %watch_path.display(),
        dir = %watch_dir.display(),
        "starting rules file watcher"
    );

    tokio::task::spawn_blocking(move || {
        let (tx, rx) = std::sync::mpsc::channel();

        let mut watcher = match notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "failed to create file watcher");
                return;
            }
        };

        if let Err(e) = watcher.watch(&watch_dir, RecursiveMode::NonRecursive) {
            tracing::error!(
                error = %e,
                dir = %watch_dir.display(),
                "failed to watch directory"
            );
            return;
        }

        tracing::info!("rules file watcher started");

        let mut last_reload = Instant::now() - RELOAD_COOLDOWN;

        loop {
            match rx.recv() {
                Ok(Ok(event)) => {
                    if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                        continue;
                    }

                    let is_our_file = event
                        .paths
                        .iter()
                        .any(|p| p.file_name() == Some(&watch_filename));
                    if !is_our_file {
                        continue;
                    }

                    if last_reload.elapsed() < RELOAD_COOLDOWN {
                        continue;
                    }

                    // Debounce: wait for the write to finish, then drain queued events
                    std::thread::sleep(Duration::from_millis(500));
                    while rx.try_recv().is_ok() {}

                    last_reload = Instant::now();
                    reload_rules(&state, &watch_path, is_external);
                }
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, "file watcher error");
                }
                Err(_) => {
                    tracing::info!("file watcher channel closed, stopping");
                    break;
                }
            }
        }
    });
}

fn resolve_watch_path(config: &Config, config_path: &str) -> (PathBuf, bool) {
    if let Some(ref rules_file) = config.rules.rules_file {
        if !rules_file.trim().is_empty() {
            let config_dir = Path::new(config_path)
                .parent()
                .unwrap_or(Path::new("."));
            return (config_dir.join(rules_file), true);
        }
    }
    (PathBuf::from(config_path), false)
}

fn reload_rules(state: &AppState, path: &Path, is_external: bool) {
    tracing::info!(path = %path.display(), "reloading rules");

    let rules_config = if is_external {
        load_external_rules(path)
    } else {
        load_inline_rules(path)
    };

    match rules_config {
        Ok(cfg) => match RulesEngine::from_config(&cfg) {
            Ok(engine) => {
                let enabled = cfg.get_enabled_rule_len();
                let total = cfg.get_rule_len();
                let default_action = format!("{:?}", cfg.default_action);
                let allow_wellknown = cfg.allow_wellknown;

                state.rules.store(Arc::new(engine));

                tracing::warn!(
                    enabled,
                    total,
                    default_action,
                    allow_wellknown,
                    "rules reloaded successfully"
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to build rules engine, keeping old rules");
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "failed to load rules file, keeping old rules");
        }
    }
}

fn load_external_rules(path: &Path) -> anyhow::Result<RulesConfig> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("read {}: {}", path.display(), e))?;
    let cfg: RulesConfig = toml::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("parse {}: {}", path.display(), e))?;
    Ok(cfg)
}

fn load_inline_rules(path: &Path) -> anyhow::Result<RulesConfig> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("read {}: {}", path.display(), e))?;
    let cfg: Config = toml::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("parse {}: {}", path.display(), e))?;
    Ok(cfg.rules)
}

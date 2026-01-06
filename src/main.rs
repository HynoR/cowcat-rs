mod config;
mod crypto;
mod handlers;
mod middleware;
mod protocol;
mod proxy;
mod rules;
mod state;
mod static_files;
mod storage;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use axum::Router;
use clap::Parser;
use tower_http::compression::predicate::{DefaultPredicate, NotForContentType, Predicate};
use tower_http::compression::CompressionLayer;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

use crate::config::Config;
use crate::handlers::favicon::favicon_handler;
use crate::handlers::pow::{challenge_page, health_ok, pow_task, pow_verify, serve_asset};
use crate::middleware::pow::pow_gate;
use crate::proxy::forward::proxy_handler;
use crate::state::AppState;

#[derive(Parser, Debug)]
#[command(name = "cowcat-rs", version, about = "CowCat PoW shield (Rust)")]
struct Args {
    #[arg(long, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy()
        )
        .init();

    let config = Config::load(&args.config)?;
    config.print_config();
    let state = Arc::new(AppState::new(config).await?);

    let pow_routes = Router::new()
        .route("/", get(challenge_page))
        .route("/ok", get(health_ok))
        .route("/assets/{*path}", get(serve_asset))
        .route("/task", post(pow_task))
        .route("/verify", post(pow_verify))
        .layer(
            CompressionLayer::new()
                .br(true)
                .gzip(true)
                .compress_when(
                    DefaultPredicate::new()
                        .and(NotForContentType::const_new("application/octet-stream")),
                  //      .and(NotForContentType::const_new("application/wasm")),
                ),
        );

    let listen = state.config.server.listen.clone();
    let app = Router::new()
        .route("/favicon.ico", get(favicon_handler))
        .nest("/__cowcatwaf", pow_routes)
        .fallback(proxy_handler)
        .layer(from_fn_with_state(state.clone(), pow_gate))
        .with_state(state);

    let addr: SocketAddr = listen
        .parse()
        .map_err(|err| anyhow::anyhow!("invalid listen address: {err}"))?;

    tracing::warn!(listen = %addr, "cowcat-rs starting");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;

    Ok(())
}

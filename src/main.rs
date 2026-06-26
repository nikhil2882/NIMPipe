mod cli;
mod config;
mod logging;
mod models;
mod proxy;
mod server;
mod transform;

use anyhow::{Context, Result};
use clap::Parser;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    // Ensure directories exist before we try to load anything.
    config::ensure_dirs()?;

    // Load config early so logging can use it.
    let cfg = config::load_config().context("Failed to load config")?;
    logging::init_logging(&cfg).context("Failed to initialize logging")?;

    cli::run_command(cli).await
}

/// Main server entry point used by the `start` command.
pub async fn server_main() -> Result<()> {
    let cfg = config::load_config().context("Failed to load config")?;
    let registry = models::load_registry().context("Failed to load model registry")?;

    let api_key = std::env::var("NIMPIPE_NVIDIA_API_KEY")
        .context("NIMPIPE_NVIDIA_API_KEY environment variable is not set")?;

    let proxy = proxy::ProxyClient::new(api_key, cfg.timeouts.clone())?;

    let state = server::AppState {
        config: Arc::new(RwLock::new(cfg.clone())),
        registry: Arc::new(RwLock::new(registry)),
        proxy: Arc::new(proxy),
        recent_events: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
    };

    let app = server::create_app(state);
    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("Failed to bind to {}", addr))?;

    info!("NIMPipe listening on http://{}", addr);
    info!("OpenAI API base URL: http://{}/v1", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

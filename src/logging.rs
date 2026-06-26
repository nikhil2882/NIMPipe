use crate::config::{AppConfig, log_dir};
use anyhow::Result;
use std::fs::OpenOptions;
use tracing_subscriber::{
    EnvFilter, Registry,
    fmt::{self},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

pub fn init_logging(config: &AppConfig) -> Result<()> {
    let log_dir = log_dir()?;
    std::fs::create_dir_all(&log_dir)?;

    let log_path = log_dir.join("nimpipe.log");
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.logging.level));

    let file_layer = fmt::layer()
        .json()
        .with_writer(move || file.try_clone().expect("Failed to clone log file"))
        .with_ansi(false);

    let stdout_layer = fmt::layer().pretty();

    let subscriber = Registry::default().with(env_filter);

    if config.logging.debug_mode {
        subscriber.with(file_layer).with(stdout_layer).init();
    } else {
        subscriber.with(file_layer).init();
    }

    Ok(())
}

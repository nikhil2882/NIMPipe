use crate::config::{AppConfig, log_dir};
use anyhow::Result;
use tracing_appender::rolling::Rotation;
use tracing_subscriber::{
    EnvFilter, Registry,
    fmt::{self},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

pub fn init_logging(config: &AppConfig) -> Result<()> {
    let dir = log_dir()?;
    std::fs::create_dir_all(&dir)?;

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.logging.level));

    // Daily rotating file appender: nimpipe-2026-06-30.log
    let file_appender = tracing_appender::rolling::Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix("nimpipe")
        .filename_suffix("log")
        .max_log_files(30)
        .build(&dir)
        .expect("Failed to create rolling file appender");

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // File layer: JSON format, no ANSI, all data
    let file_layer = fmt::layer()
        .json()
        .with_writer(non_blocking)
        .with_ansi(false);

    // Stdout layer: pretty format, compact (skip large payloads in console)
    let stdout_layer = fmt::layer()
        .pretty()
        .with_target(true);

    let subscriber = Registry::default()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer);

    subscriber.init();

    // Leak the guard so the non-blocking writer stays alive for the process lifetime
    std::mem::forget(_guard);

    Ok(())
}

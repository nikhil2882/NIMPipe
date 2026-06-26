use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub timeouts: TimeoutsConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutsConfig {
    pub request_seconds: u64,
    pub streaming_seconds: u64,
    pub max_poll_seconds: u64,
    pub poll_interval_start_ms: u64,
    pub poll_interval_max_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub debug_mode: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8787,
            },
            timeouts: TimeoutsConfig {
                request_seconds: 120,
                streaming_seconds: 300,
                max_poll_seconds: 300,
                poll_interval_start_ms: 1000,
                poll_interval_max_ms: 10000,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                debug_mode: false,
            },
        }
    }
}

pub fn config_dir() -> Result<PathBuf> {
    dirs::config_dir()
        .map(|d| d.join("nimpipe"))
        .context("Could not determine config directory")
}

pub fn data_dir() -> Result<PathBuf> {
    dirs::data_dir()
        .map(|d| d.join("nimpipe"))
        .context("Could not determine data directory")
}

pub fn log_dir() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .map(|d| d.join("Library/Logs/nimpipe"))
            .context("Could not determine log directory")
    }
    #[cfg(not(target_os = "macos"))]
    {
        dirs::state_dir()
            .or_else(dirs::data_dir)
            .map(|d| d.join("nimpipe/log"))
            .context("Could not determine log directory")
    }
}

pub fn ensure_dirs() -> Result<()> {
    std::fs::create_dir_all(config_dir()?)?;
    std::fs::create_dir_all(data_dir()?)?;
    std::fs::create_dir_all(log_dir()?)?;
    Ok(())
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

pub fn models_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("models.toml"))
}

pub fn load_config() -> Result<AppConfig> {
    let path = config_path()?;
    if !path.exists() {
        info!("Config not found at {:?}, writing default", path);
        let cfg = AppConfig::default();
        save_config(&cfg)?;
        return Ok(cfg);
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config from {:?}", path))?;
    let cfg: AppConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse config from {:?}", path))?;
    Ok(cfg)
}

pub fn save_config(cfg: &AppConfig) -> Result<()> {
    ensure_dirs()?;
    let path = config_path()?;
    let content = toml::to_string_pretty(cfg).context("Failed to serialize config")?;
    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write config to {:?}", path))?;
    Ok(())
}

pub fn load_raw_models() -> Result<String> {
    let path = models_path()?;
    if !path.exists() {
        info!("Models registry not found at {:?}, writing defaults", path);
        let defaults = crate::models::default_models_toml();
        std::fs::write(&path, &defaults)
            .with_context(|| format!("Failed to write default models to {:?}", path))?;
        return Ok(defaults);
    }
    Ok(std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read models from {:?}", path))?)
}

pub fn save_raw_models(content: &str) -> Result<()> {
    ensure_dirs()?;
    let path = models_path()?;
    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write models to {:?}", path))?;
    Ok(())
}

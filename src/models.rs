use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

/// Model entry in the registry.
/// `openai_id` is what clients see; `backend_id` is sent upstream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub openai_id: String,
    pub backend_id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub max_tokens_cap: Option<u32>,
    #[serde(default)]
    pub default_params: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub injected_params: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub strip_params: Vec<String>,
    #[serde(default = "default_true")]
    pub supports_streaming: bool,
    #[serde(default = "default_true")]
    pub supports_tools: bool,
    #[serde(default)]
    pub status_poll_path: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Registry container used for TOML serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistryFile {
    pub models: Vec<ModelEntry>,
}

/// In-memory registry with lookup helpers.
#[derive(Debug, Clone, Default)]
pub struct ModelRegistry {
    models: BTreeMap<String, ModelEntry>,
}

impl ModelRegistry {
    pub fn from_file(file: &ModelRegistryFile) -> Result<Self> {
        let mut models = BTreeMap::new();
        for entry in &file.models {
            if models.contains_key(&entry.openai_id) {
                anyhow::bail!("Duplicate openai_id in registry: {}", entry.openai_id);
            }
            models.insert(entry.openai_id.clone(), entry.clone());
        }
        Ok(Self { models })
    }

    pub fn to_file(&self) -> ModelRegistryFile {
        let mut models: Vec<_> = self.models.values().cloned().collect();
        models.sort_by(|a, b| a.openai_id.cmp(&b.openai_id));
        ModelRegistryFile { models }
    }

    pub fn get(&self, openai_id: &str) -> Option<&ModelEntry> {
        self.models.get(openai_id)
    }

    pub fn list(&self) -> Vec<&ModelEntry> {
        let mut v: Vec<_> = self.models.values().collect();
        v.sort_by(|a, b| a.openai_id.cmp(&b.openai_id));
        v
    }
}

pub type SharedRegistry = Arc<RwLock<ModelRegistry>>;

pub fn load_registry() -> Result<ModelRegistry> {
    let raw = crate::config::load_raw_models()?;
    let file: ModelRegistryFile = toml::from_str(&raw).context("Failed to parse models.toml")?;
    ModelRegistry::from_file(&file)
}

pub fn save_registry(registry: &ModelRegistry) -> Result<()> {
    let file = registry.to_file();
    let raw = toml::to_string_pretty(&file).context("Failed to serialize models registry")?;
    crate::config::save_raw_models(&raw)
}

/// Default models registry as TOML.
pub fn default_models_toml() -> String {
    include_str!("../assets/default_models.toml").to_string()
}

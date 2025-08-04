// src/config/mod.rs
mod models;

pub use models::*;

use anyhow::{Context, Result};
use std::path::Path;

/// Load configuration from a file (YAML or JSON)
pub async fn load_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let path = path.as_ref();
    let contents = tokio::fs::read_to_string(path)
        .await
        .context("Failed to read config file")?;
    
    let config: Config = if path.extension().and_then(|s| s.to_str()) == Some("yaml") 
        || path.extension().and_then(|s| s.to_str()) == Some("yml") {
        serde_yaml::from_str(&contents).context("Failed to parse YAML config")?
    } else {
        serde_json::from_str(&contents).context("Failed to parse JSON config")?
    };
    
    config.validate()?;
    Ok(config)
}

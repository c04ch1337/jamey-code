//! CLI configuration management
//! 
//! Handle configuration loading and management for the CLI

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use dirs::home_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    pub default_model: String,
    pub api_key: Option<String>,
    pub runtime_url: String,
    pub timeout_seconds: u64,
    pub verbose: bool,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            default_model: "claude-3-sonnet".to_string(),
            api_key: None,
            runtime_url: "http://localhost:3000".to_string(),
            timeout_seconds: 30,
            verbose: false,
        }
    }
}

impl CliConfig {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_file_path();
        
        if config_path.exists() {
            let content = std::fs::read_to_string(config_path)?;
            let config: Self = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }
    
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_file_path();
        
        // Create config directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = toml::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }
    
    fn config_file_path() -> PathBuf {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
            .join("jamey")
            .join("cli.toml")
    }
}
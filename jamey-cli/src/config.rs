//! CLI configuration management
//! 
//! Handle configuration loading and management for the CLI
//! 
//! SECURITY: Secrets are loaded from environment variables only.
//! Never store API keys or passwords in configuration files.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use dirs::home_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    pub default_model: String,
    #[serde(skip_serializing)] // Never serialize API keys to disk
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
    /// Load configuration from file and environment variables
    /// 
    /// SECURITY: API keys are ALWAYS loaded from environment variables,
    /// never from configuration files. This prevents accidental secret exposure.
    pub fn load() -> Result<Self> {
        let mut config = Self::default();
        let config_path = Self::config_file_path();
        
        // Load non-sensitive config from file if it exists
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;
            let file_config: CliConfigFile = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?;
            
            config.default_model = file_config.default_model;
            config.runtime_url = file_config.runtime_url;
            config.timeout_seconds = file_config.timeout_seconds;
            config.verbose = file_config.verbose;
        }
        
        // ALWAYS load API key from environment variable (never from file)
        config.api_key = std::env::var("JAMEY_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());
        
        // Also check for OpenRouter API key (common case)
        if config.api_key.is_none() {
            config.api_key = std::env::var("OPENROUTER_API_KEY")
                .ok()
                .filter(|s| !s.is_empty());
        }
        
        Ok(config)
    }
    
    /// Save configuration to file (excluding secrets)
    /// 
    /// SECURITY: API keys are never written to disk
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_file_path();
        
        // Create config directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
        }
        
        // Create file config without secrets
        let file_config = CliConfigFile {
            default_model: self.default_model.clone(),
            runtime_url: self.runtime_url.clone(),
            timeout_seconds: self.timeout_seconds,
            verbose: self.verbose,
        };
        
        let content = toml::to_string_pretty(&file_config)
            .context("Failed to serialize config")?;
        std::fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;
        
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

/// Configuration file structure (without secrets)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CliConfigFile {
    pub default_model: String,
    pub runtime_url: String,
    pub timeout_seconds: u64,
    pub verbose: bool,
}
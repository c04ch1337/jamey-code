use anyhow::Result;
use jamey_core::cache::CacheConfig;
use jamey_providers::openrouter::OpenRouterConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Missing required configuration: {0}")]
    MissingConfig(String),
    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),
    #[error("Environment error: {0}")]
    Environment(#[from] std::env::VarError),
    #[error("Configuration loading error: {0}")]
    Loading(#[from] config::ConfigError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub project_name: String,
    pub memory: MemoryConfig,
    pub cache: CacheConfig,
    pub llm: LlmConfig,
    pub api: ApiConfig,
    pub security: SecurityConfig,
    pub tools: ToolConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub postgres_host: String,
    pub postgres_port: u16,
    pub postgres_db: String,
    pub postgres_user: String,
    pub postgres_password: String,
    pub postgres_max_connections: u32,
    pub vector_dimension: usize,
    pub vector_similarity_threshold: f32,
    pub vector_index_type: String,
    pub max_memory_entries: usize,
    pub memory_retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub openrouter_api_key: String,
    pub openrouter_default_model: String,
    pub openrouter_allowed_models: Vec<String>,
    pub openrouter_timeout_seconds: u64,
    pub openrouter_max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub log_level: String,
    pub allowed_origins: Vec<String>,
    pub enable_cors: bool,
    pub metrics_port: Option<u16>,
    pub health_check_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub api_key_required: bool,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub enable_registry_tool: bool,
    pub backup_dir: PathBuf,
    pub process_tool_enabled: bool,
    pub process_tool_max_list: usize,
    pub self_modify_backup_count: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            project_name: "jamey".to_string(),
            memory: MemoryConfig::default(),
            cache: CacheConfig::default(),
            llm: LlmConfig::default(),
            api: ApiConfig::default(),
            security: SecurityConfig::default(),
            tools: ToolConfig::default(),
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            postgres_host: "localhost".to_string(),
            postgres_port: 5432,
            postgres_db: "jamey".to_string(),
            postgres_user: "jamey".to_string(),
            postgres_password: "change_me_in_production".to_string(),
            postgres_max_connections: 10,
            vector_dimension: 1536,
            vector_similarity_threshold: 0.8,
            vector_index_type: "ivfflat".to_string(),
            max_memory_entries: 1000,
            memory_retention_days: 30,
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            openrouter_api_key: String::new(),
            openrouter_default_model: "claude-3-sonnet".to_string(),
            openrouter_allowed_models: vec![
                "claude-3-sonnet".to_string(),
                "gpt-4".to_string(),
                "gpt-3.5-turbo".to_string(),
            ],
            openrouter_timeout_seconds: 30,
            openrouter_max_retries: 3,
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 3000,
            log_level: "info".to_string(),
            allowed_origins: vec!["http://localhost:3000".to_string()],
            enable_cors: true,
            metrics_port: Some(9090),
            health_check_port: Some(8081),
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            api_key_required: true,
            api_key: None,
        }
    }
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            enable_registry_tool: true,
            backup_dir: PathBuf::from("./backups"),
            process_tool_enabled: true,
            process_tool_max_list: 100,
            self_modify_backup_count: 5,
        }
    }
}

impl RuntimeConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        // Load .env file if it exists, but don't fail if it doesn't
        dotenv::dotenv().ok();

        // Load required environment variables
        let postgres_password = std::env::var("POSTGRES_PASSWORD")
            .map_err(|_| ConfigError::MissingConfig("POSTGRES_PASSWORD".to_string()))?;
        
        let openrouter_api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| ConfigError::MissingConfig("OPENROUTER_API_KEY".to_string()))?;

        let api_key = if std::env::var("API_KEY_REQUIRED").unwrap_or_else(|_| "true".to_string()) == "true" {
            Some(std::env::var("API_KEY")
                .map_err(|_| ConfigError::MissingConfig("API_KEY is required when API_KEY_REQUIRED=true".to_string()))?)
        } else {
            None
        };

        // Create base config from defaults
        let mut config = Self::default();

        // Update with required values
        config.memory.postgres_password = postgres_password;
        config.llm.openrouter_api_key = openrouter_api_key;
        config.security.api_key = api_key;
        config.security.api_key_required = std::env::var("API_KEY_REQUIRED")
            .map(|v| v == "true")
            .unwrap_or(true);

        // Load optional environment variables
        if let Ok(host) = std::env::var("POSTGRES_HOST") {
            config.memory.postgres_host = host;
        }
        if let Ok(port) = std::env::var("POSTGRES_PORT").and_then(|p| p.parse().map_err(|_| std::env::VarError::NotPresent)) {
            config.memory.postgres_port = port;
        }
        if let Ok(db) = std::env::var("POSTGRES_DB") {
            config.memory.postgres_db = db;
        }
        if let Ok(user) = std::env::var("POSTGRES_USER") {
            config.memory.postgres_user = user;
        }
        if let Ok(max_conn) = std::env::var("POSTGRES_MAX_CONNECTIONS").and_then(|m| m.parse().map_err(|_| std::env::VarError::NotPresent)) {
            config.memory.postgres_max_connections = max_conn;
        }

        // Validate the configuration
        config.validate()?;

        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate project name
        if self.project_name.is_empty() {
            return Err(ConfigError::MissingConfig("project_name".to_string()));
        }

        // Validate memory config
        if self.memory.postgres_password == "change_me_in_production" {
            return Err(ConfigError::InvalidValue(
                "Default postgres password must be changed".to_string(),
            ));
        }

        // Validate LLM config
        if self.llm.openrouter_api_key.is_empty() {
            return Err(ConfigError::MissingConfig("openrouter_api_key".to_string()));
        }

        // Validate security config
        if self.security.api_key_required && self.security.api_key.is_none() {
            return Err(ConfigError::MissingConfig(
                "API key required but not provided".to_string(),
            ));
        }

        Ok(())
    }

    pub fn into_openrouter_config(&self) -> Result<OpenRouterConfig, ConfigError> {
        Ok(OpenRouterConfig {
            api_key: self.llm.openrouter_api_key.clone(),
            api_base_url: url::Url::parse("https://openrouter.ai/api/v1")
                .map_err(|_| ConfigError::InvalidValue("Invalid OpenRouter API URL".to_string()))?,
            default_model: self.llm.openrouter_default_model.clone(),
            allowed_models: self.llm.openrouter_allowed_models.clone(),
            timeout_seconds: self.llm.openrouter_timeout_seconds,
            max_retries: self.llm.openrouter_max_retries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_config() {
        let config = RuntimeConfig::default();
        assert_eq!(config.project_name, "jamey");
        assert_eq!(config.memory.postgres_host, "localhost");
        assert!(config.api.enable_cors);
    }

    #[test]
    fn test_config_validation() {
        let mut config = RuntimeConfig::default();
        
        // Test invalid project name
        config.project_name = "".to_string();
        assert!(config.validate().is_err());

        // Test missing OpenRouter API key
        config.project_name = "jamey".to_string();
        assert!(config.validate().is_err());

        // Test valid config
        config.llm.openrouter_api_key = "test_key".to_string();
        config.memory.postgres_password = "secure_password".to_string();
        config.security.api_key = Some("api_key".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_env_override() {
        env::set_var("PROJECT_NAME", "test_project");
        env::set_var("MEMORY__POSTGRES_HOST", "test_host");
        
        let config = RuntimeConfig::from_env().unwrap();
        assert_eq!(config.project_name, "test_project");
        assert_eq!(config.memory.postgres_host, "test_host");
        
        env::remove_var("PROJECT_NAME");
        env::remove_var("MEMORY__POSTGRES_HOST");
    }
}
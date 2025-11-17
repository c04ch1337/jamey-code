use anyhow::Result;
use jamey_core::cache::CacheConfig;
use jamey_core::prelude::{SecretManager, redact_sensitive_data};
use jamey_providers::openrouter::OpenRouterConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;
use tracing_honeycomb::SensitiveValue;
use tracing;

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
    #[error("Secret management error: {0}")]
    Secret(#[from] jamey_core::prelude::SecretError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default = "default_project_name")]
    pub project_name: String,
    pub memory: MemoryConfig,
    pub cache: CacheConfig,
    pub llm: LlmConfig,
    pub api: ApiConfig,
    pub security: SecurityConfig,
    pub tools: ToolConfig,
}

fn default_project_name() -> String {
    "jamey".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_postgres_host")]
    pub postgres_host: String,
    #[serde(default = "default_postgres_port")]
    pub postgres_port: u16,
    #[serde(default = "default_postgres_db")]
    pub postgres_db: String,
    #[serde(default = "default_postgres_user")]
    pub postgres_user: String,
    pub postgres_password: SensitiveValue<String>,
    #[serde(default = "default_postgres_max_connections")]
    #[serde(validate(range(min = 1, max = 100)))]
    pub postgres_max_connections: u32,
    #[serde(default = "default_vector_dimension")]
    #[serde(validate(range(min = 1, max = 4096)))]
    pub vector_dimension: usize,
    #[serde(default = "default_vector_similarity_threshold")]
    #[serde(validate(range(min = 0.0, max = 1.0)))]
    pub vector_similarity_threshold: f32,
    #[serde(default = "default_vector_index_type")]
    pub vector_index_type: String,
    #[serde(default = "default_max_memory_entries")]
    #[serde(validate(range(min = 1, max = 10000)))]
    pub max_memory_entries: usize,
    #[serde(default = "default_memory_retention_days")]
    #[serde(validate(range(min = 1, max = 365)))]
    pub memory_retention_days: u32,
}

fn default_postgres_host() -> String { "localhost".to_string() }
fn default_postgres_port() -> u16 { 5432 }
fn default_postgres_db() -> String { "jamey".to_string() }
fn default_postgres_user() -> String { "jamey".to_string() }
fn default_postgres_max_connections() -> u32 { 10 }
fn default_vector_dimension() -> usize { 1536 }
fn default_vector_similarity_threshold() -> f32 { 0.8 }
fn default_vector_index_type() -> String { "ivfflat".to_string() }
fn default_max_memory_entries() -> usize { 1000 }
fn default_memory_retention_days() -> u32 { 30 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub openrouter_api_key: SensitiveValue<String>,
    pub openrouter_default_model: String,
    pub openrouter_allowed_models: Vec<String>,
    pub openrouter_timeout_seconds: u64,
    pub openrouter_max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub http_port: u16,
    pub https_port: u16,
    pub tls_cert_path: Option<PathBuf>,
    pub tls_key_path: Option<PathBuf>,
    pub tls_ca_cert_path: Option<PathBuf>,
    pub tls_min_version: String,
    pub enable_hsts: bool,
    pub hsts_max_age: u64,
    pub hsts_include_subdomains: bool,
    pub hsts_preload: bool,
    pub enable_https: bool,
    pub redirect_http_to_https: bool,
    pub log_level: String,
    pub allowed_origins: Vec<String>,
    pub enable_cors: bool,
    pub metrics_port: Option<u16>,
    pub health_check_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub api_key_required: bool,
    pub api_key: Option<SensitiveValue<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub enable_registry_tool: bool,
    pub backup_dir: PathBuf,
    pub process_tool_enabled: bool,
    pub process_tool_max_list: usize,
    pub self_modify_backup_count: usize,
    // Full access configuration
    pub download_dir: PathBuf,
    pub system_root: PathBuf,
    pub github_token: Option<String>,
    pub linkedin_token: Option<String>,
    pub web_search_api_key: Option<String>,
    pub mcp_server_url: Option<String>,
    pub enable_24_7: bool,
    pub scheduler_enabled: bool,
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
            postgres_password: SensitiveValue("change_me_in_production".to_string()),
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
            openrouter_api_key: SensitiveValue(String::new()),
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
            http_port: 3000,
            https_port: 3443,
            tls_cert_path: None,
            tls_key_path: None,
            tls_ca_cert_path: None,
            tls_min_version: "1.3".to_string(),
            enable_hsts: true,
            hsts_max_age: 31536000, // 1 year
            hsts_include_subdomains: true,
            hsts_preload: false,
            enable_https: false,
            redirect_http_to_https: false,
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
            download_dir: PathBuf::from("./downloads"),
            system_root: PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" }),
            github_token: None,
            linkedin_token: None,
            web_search_api_key: None,
            mcp_server_url: None,
            enable_24_7: false,
            scheduler_enabled: false,
        }
    }
}

impl RuntimeConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        // Load .env file if it exists, but don't fail if it doesn't
        dotenv::dotenv().ok();

        // Initialize secret manager
        let secret_manager = SecretManager::new("jamey_runtime");

        // Load required environment variables and store them securely
        let postgres_password = std::env::var("POSTGRES_PASSWORD")
            .map_err(|_| ConfigError::MissingConfig("POSTGRES_PASSWORD".to_string()))?;
        secret_manager.store_secret("postgres_password", &postgres_password)?;
        tracing::info!("Stored database credentials in secure keychain");
        
        let openrouter_api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| ConfigError::MissingConfig("OPENROUTER_API_KEY".to_string()))?;
        secret_manager.store_secret("openrouter_api_key", &openrouter_api_key)?;
        tracing::info!("Stored LLM provider credentials in secure keychain");

        let api_key = if std::env::var("API_KEY_REQUIRED").unwrap_or_else(|_| "true".to_string()) == "true" {
            let key = std::env::var("API_KEY")
                .map_err(|_| ConfigError::MissingConfig("API_KEY is required when API_KEY_REQUIRED=true".to_string()))?;
            secret_manager.store_secret("api_key", &key)?;
            tracing::info!("Stored API authentication credentials in secure keychain");
            Some(SensitiveValue(key))
        } else {
            None
        };


        // Create base config from defaults
        let mut config = Self::default();

        // Update with securely stored values
        config.memory.postgres_password = SensitiveValue(secret_manager.get_secret("postgres_password")?);
        config.llm.openrouter_api_key = SensitiveValue(secret_manager.get_secret("openrouter_api_key")?);
        if api_key.is_some() {
            config.security.api_key = Some(SensitiveValue(secret_manager.get_secret("api_key")?));
        }
        config.security.api_key_required = std::env::var("API_KEY_REQUIRED")
            .map(|v| v == "true")
            .unwrap_or(true);

        // Load optional environment variables
        if let Ok(host) = std::env::var("API_HOST") {
            config.api.host = host;
        }
        if let Ok(port) = std::env::var("API_HTTP_PORT").and_then(|p| p.parse().map_err(|_| std::env::VarError::NotPresent)) {
            config.api.http_port = port;
        }
        if let Ok(port) = std::env::var("API_HTTPS_PORT").and_then(|p| p.parse().map_err(|_| std::env::VarError::NotPresent)) {
            config.api.https_port = port;
        }
        if let Ok(cert_path) = std::env::var("API_TLS_CERT_PATH") {
            config.api.tls_cert_path = Some(PathBuf::from(cert_path));
        }
        if let Ok(key_path) = std::env::var("API_TLS_KEY_PATH") {
            config.api.tls_key_path = Some(PathBuf::from(key_path));
        }
        if let Ok(ca_cert_path) = std::env::var("API_TLS_CA_CERT_PATH") {
            config.api.tls_ca_cert_path = Some(PathBuf::from(ca_cert_path));
        }
        if let Ok(tls_version) = std::env::var("API_TLS_MIN_VERSION") {
            config.api.tls_min_version = tls_version;
        }
        if let Ok(enable_hsts) = std::env::var("API_ENABLE_HSTS") {
            config.api.enable_hsts = enable_hsts == "true" || enable_hsts == "1";
        }
        if let Ok(hsts_max_age) = std::env::var("API_HSTS_MAX_AGE").and_then(|m| m.parse().map_err(|_| std::env::VarError::NotPresent)) {
            config.api.hsts_max_age = hsts_max_age;
        }
        if let Ok(hsts_subdomains) = std::env::var("API_HSTS_INCLUDE_SUBDOMAINS") {
            config.api.hsts_include_subdomains = hsts_subdomains == "true" || hsts_subdomains == "1";
        }
        if let Ok(hsts_preload) = std::env::var("API_HSTS_PRELOAD") {
            config.api.hsts_preload = hsts_preload == "true" || hsts_preload == "1";
        }
        if let Ok(enable_https) = std::env::var("API_ENABLE_HTTPS") {
            config.api.enable_https = enable_https == "true" || enable_https == "1";
        }
        if let Ok(redirect_https) = std::env::var("API_REDIRECT_HTTP_TO_HTTPS") {
            config.api.redirect_http_to_https = redirect_https == "true" || redirect_https == "1";
        }
        
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

        // Load full access configuration
        if let Ok(download_dir) = std::env::var("DOWNLOAD_DIR") {
            config.tools.download_dir = PathBuf::from(download_dir);
        }
        if let Ok(system_root) = std::env::var("SYSTEM_ROOT") {
            config.tools.system_root = PathBuf::from(system_root);
        }
        if let Ok(github_token) = std::env::var("GITHUB_TOKEN") {
            config.tools.github_token = Some(github_token);
        }
        if let Ok(linkedin_token) = std::env::var("LINKEDIN_TOKEN") {
            config.tools.linkedin_token = Some(linkedin_token);
        }
        if let Ok(web_search_key) = std::env::var("WEB_SEARCH_API_KEY") {
            config.tools.web_search_api_key = Some(web_search_key);
        }
        if let Ok(mcp_url) = std::env::var("MCP_SERVER_URL") {
            config.tools.mcp_server_url = Some(mcp_url);
        }
        if let Ok(enable_24_7) = std::env::var("ENABLE_24_7") {
            config.tools.enable_24_7 = enable_24_7 == "true" || enable_24_7 == "1";
        }
        if let Ok(scheduler_enabled) = std::env::var("SCHEDULER_ENABLED") {
            config.tools.scheduler_enabled = scheduler_enabled == "true" || scheduler_enabled == "1";
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
        if self.project_name.len() > 64 {
            return Err(ConfigError::InvalidValue("project_name too long (max 64 chars)".to_string()));
        }

        // Validate memory config
        if self.memory.postgres_password == "change_me_in_production" {
            return Err(ConfigError::InvalidValue(
                "Default postgres password must be changed".to_string(),
            ));
        }
        if self.memory.postgres_host.len() > 255 {
            return Err(ConfigError::InvalidValue("postgres_host too long".to_string()));
        }
        if self.memory.postgres_db.len() > 64 {
            return Err(ConfigError::InvalidValue("postgres_db name too long".to_string()));
        }
        if self.memory.postgres_user.len() > 64 {
            return Err(ConfigError::InvalidValue("postgres_user name too long".to_string()));
        }
        if !["ivfflat", "hnsw"].contains(&self.memory.vector_index_type.as_str()) {
            return Err(ConfigError::InvalidValue("Invalid vector_index_type".to_string()));
        }

        // Validate LLM config
        if self.llm.openrouter_api_key.is_empty() {
            return Err(ConfigError::MissingConfig("openrouter_api_key".to_string()));
        }
        if self.llm.openrouter_timeout_seconds == 0 || self.llm.openrouter_timeout_seconds > 300 {
            return Err(ConfigError::InvalidValue("Invalid timeout value (1-300 seconds)".to_string()));
        }
        if self.llm.openrouter_max_retries > 10 {
            return Err(ConfigError::InvalidValue("max_retries too high (max 10)".to_string()));
        }

        // Validate security config
        if self.security.api_key_required && self.security.api_key.is_none() {
            return Err(ConfigError::MissingConfig(
                "API key required but not provided".to_string(),
            ));
        }

        // Validate API config
        if let Some(metrics_port) = self.api.metrics_port {
            if metrics_port == self.api.http_port || metrics_port == self.api.https_port {
                return Err(ConfigError::InvalidValue("metrics_port conflicts with other ports".to_string()));
            }
        }
        if let Some(health_port) = self.api.health_check_port {
            if health_port == self.api.http_port || health_port == self.api.https_port {
                return Err(ConfigError::InvalidValue("health_check_port conflicts with other ports".to_string()));
            }
        }

        // Validate TLS configuration

    /// Convert API configuration to TLS configuration
    pub fn into_tls_config(&self) -> Result<Option<crate::tls::TlsConfig>, ConfigError> {
        if !self.api.enable_https {
            return Ok(None);
        }

        let cert_path = self.api.tls_cert_path.clone()
            .ok_or_else(|| ConfigError::MissingConfig("TLS certificate path required".to_string()))?;
        let key_path = self.api.tls_key_path.clone()
            .ok_or_else(|| ConfigError::MissingConfig("TLS private key path required".to_string()))?;

        let tls_version = match self.api.tls_min_version.as_str() {
            "1.2" => crate::tls::TlsVersion::Tls12,
            "1.3" => crate::tls::TlsVersion::Tls13,
            _ => return Err(ConfigError::InvalidValue("Invalid TLS version".to_string())),
        };

        let mut tls_config = crate::tls::TlsConfig::new(cert_path, key_path)
            .with_min_tls_version(tls_version)
            .with_hsts(
                self.api.enable_hsts,
                self.api.hsts_max_age,
                self.api.hsts_include_subdomains,
                self.api.hsts_preload,
            );

        if let Some(ca_cert_path) = &self.api.tls_ca_cert_path {
            tls_config = tls_config.with_ca_cert(ca_cert_path.clone());
        }

        Ok(Some(tls_config))
    }
        if self.api.enable_https {
            if self.api.tls_cert_path.is_none() {
                return Err(ConfigError::MissingConfig("TLS certificate path required when HTTPS is enabled".to_string()));
            }
            if self.api.tls_key_path.is_none() {
                return Err(ConfigError::MissingConfig("TLS private key path required when HTTPS is enabled".to_string()));
            }
            if !["1.2", "1.3"].contains(&self.api.tls_min_version.as_str()) {
                return Err(ConfigError::InvalidValue("TLS version must be 1.2 or 1.3".to_string()));
            }
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
        config.llm.openrouter_api_key = SensitiveValue("test_key".to_string());
        config.memory.postgres_password = SensitiveValue("secure_password".to_string());
        config.security.api_key = Some(SensitiveValue("api_key".to_string()));
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
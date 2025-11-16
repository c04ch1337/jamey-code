//! Runtime engine for Digital Twin Jamey
//! 
//! This crate provides the runtime environment that coordinates all components,
//! including memory management, LLM providers, and system tools.

pub mod config;
pub mod state;

use anyhow::Result;
use config::{ConfigError, RuntimeConfig};
use metrics_exporter_prometheus::PrometheusBuilder;
use state::{RuntimeError, RuntimeState};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{debug, error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    #[error("Runtime error: {0}")]
    Runtime(#[from] RuntimeError),
    #[error("Initialization error: {0}")]
    Init(String),
}

/// Main runtime engine that coordinates all components
pub struct Runtime {
    state: Arc<RuntimeState>,
    shutdown_rx: broadcast::Receiver<()>,
}

impl Runtime {
    /// Create a new runtime instance with the provided configuration
    pub async fn new(config: RuntimeConfig) -> Result<Self, Error> {
        // Validate configuration
        config.validate()?;

        // Initialize logging
        let subscriber = FmtSubscriber::builder()
            .with_max_level(match config.api.log_level.as_str() {
                "debug" => Level::DEBUG,
                "info" => Level::INFO,
                "warn" => Level::WARN,
                "error" => Level::ERROR,
                _ => Level::INFO,
            })
            .with_target(false)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .pretty()
            .try_init()
            .map_err(|e| Error::Init(format!("Failed to initialize logging: {}", e)))?;

        // Initialize metrics
        let builder = PrometheusBuilder::new();
        builder
            .install()
            .map_err(|e| Error::Init(format!("Failed to initialize metrics: {}", e)))?;

        // Initialize runtime state
        let state = RuntimeState::new(config).await?;
        let shutdown_rx = state.shutdown_signal.subscribe();

        Ok(Self {
            state: Arc::new(state),
            shutdown_rx,
        })
    }

    /// Start the runtime and run until shutdown signal is received
    pub async fn run(&mut self) -> Result<(), Error> {
        info!("Starting Digital Twin Jamey runtime...");

        // Start session cleanup task
        let session_manager = self.state.session_manager.clone();
        let mut cleanup_interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes
        let mut shutdown_rx = self.shutdown_rx.resubscribe();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cleanup_interval.tick() => {
                        session_manager.cleanup_expired_sessions(
                            std::time::Duration::from_secs(3600) // 1 hour
                        );
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Shutting down session cleanup task");
                        break;
                    }
                }
            }
        });

        // Wait for shutdown signal
        let _ = self.shutdown_rx.recv().await;
        info!("Shutting down runtime...");
        self.state.shutdown().await;
        Ok(())
    }

    /// Get a reference to the runtime state
    pub fn state(&self) -> &RuntimeState {
        &self.state
    }

    /// Request runtime shutdown
    pub async fn shutdown(&self) {
        self.state.shutdown().await;
    }
}

/// Re-export common types
pub mod prelude {
    pub use super::config::{
        ApiConfig, ConfigError, LlmConfig, MemoryConfig, RuntimeConfig, SecurityConfig, ToolConfig,
    };
    pub use super::state::{RuntimeError, RuntimeState, Session, SessionManager, ToolRegistry};
    pub use super::{Error, Runtime};
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_runtime_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = RuntimeConfig::default();
        config.tools.backup_dir = temp_dir.path().to_path_buf();
        config.memory.postgres_password = "test_password".to_string();
        config.llm.openrouter_api_key = "test_key".to_string();

        // Create runtime
        let mut runtime = Runtime::new(config).await.unwrap();
        
        // Start runtime in background
        let runtime_handle = tokio::spawn({
            let mut runtime = runtime;
            async move {
                runtime.run().await.unwrap();
            }
        });

        // Let runtime initialize
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Test session creation
        let session_id = runtime.state().session_manager.create_session();
        assert!(runtime.state().session_manager.get_session(session_id).is_some());

        // Shutdown runtime
        runtime.shutdown().await;
        runtime_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_runtime_config_validation() {
        // Test invalid config
        let mut config = RuntimeConfig::default();
        config.project_name = "".to_string();
        assert!(Runtime::new(config).await.is_err());

        // Test valid config
        let temp_dir = TempDir::new().unwrap();
        let mut config = RuntimeConfig::default();
        config.tools.backup_dir = temp_dir.path().to_path_buf();
        config.memory.postgres_password = "test_password".to_string();
        config.llm.openrouter_api_key = "test_key".to_string();
        assert!(Runtime::new(config).await.is_ok());
    }
}
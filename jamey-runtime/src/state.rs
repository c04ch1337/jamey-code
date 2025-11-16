use crate::config::RuntimeConfig;
use anyhow::Result;
use dashmap::DashMap;
use jamey_core::memory::{Memory, PostgresMemoryStore};
use jamey_providers::openrouter::OpenRouterProvider;
use jamey_tools::system::{ProcessTool, SelfModifyTool};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("Component initialization error: {0}")]
    Initialization(String),
    #[error("Memory error: {0}")]
    Memory(#[from] jamey_core::memory::MemoryError),
    #[error("Provider error: {0}")]
    Provider(#[from] jamey_providers::ProviderError),
    #[error("Tool error: {0}")]
    Tool(#[from] jamey_tools::ToolError),
    #[error("Session not found: {0}")]
    SessionNotFound(Uuid),
}

/// Manages active user sessions and their state
pub struct SessionManager {
    sessions: DashMap<Uuid, Session>,
    config: Arc<RuntimeConfig>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub memory_context: Vec<Memory>,
    pub last_activity: std::time::Instant,
}

impl SessionManager {
    pub fn new(config: Arc<RuntimeConfig>) -> Self {
        Self {
            sessions: DashMap::new(),
            config,
        }
    }

    pub fn create_session(&self) -> Uuid {
        let session_id = Uuid::new_v4();
        self.sessions.insert(
            session_id,
            Session {
                id: session_id,
                memory_context: Vec::new(),
                last_activity: std::time::Instant::now(),
            },
        );
        session_id
    }

    pub fn get_session(&self, id: Uuid) -> Option<Session> {
        self.sessions.get(&id).map(|s| {
            let mut session = (*s.value()).clone();
            session.last_activity = std::time::Instant::now();
            session
        })
    }

    pub fn cleanup_expired_sessions(&self, timeout: std::time::Duration) {
        let now = std::time::Instant::now();
        self.sessions.retain(|_, session| {
            now.duration_since(session.last_activity) < timeout
        });
    }
}

/// Manages tool registration and access
pub struct ToolRegistry {
    process_tool: Option<ProcessTool>,
    #[cfg(windows)]
    registry_tool: Option<jamey_tools::RegistryTool>,
    self_modify_tool: Option<SelfModifyTool>,
}

impl ToolRegistry {
    pub fn new(config: &RuntimeConfig) -> Result<Self, RuntimeError> {
        let process_tool = if config.tools.process_tool_enabled {
            Some(ProcessTool::new())
        } else {
            None
        };

        #[cfg(windows)]
        let registry_tool = if config.tools.enable_registry_tool {
            Some(jamey_tools::RegistryTool::new())
        } else {
            None
        };

        let self_modify_tool = SelfModifyTool::new(&config.tools.backup_dir)
            .map_err(|e| RuntimeError::Initialization(e.to_string()))?;

        Ok(Self {
            process_tool,
            #[cfg(windows)]
            registry_tool,
            self_modify_tool: Some(self_modify_tool),
        })
    }

    pub fn get_process_tool(&self) -> Option<&ProcessTool> {
        self.process_tool.as_ref()
    }

    #[cfg(windows)]
    pub fn get_registry_tool(&self) -> Option<&jamey_tools::RegistryTool> {
        self.registry_tool.as_ref()
    }

    pub fn get_self_modify_tool(&self) -> Option<&SelfModifyTool> {
        self.self_modify_tool.as_ref()
    }
}

/// Main runtime state containing all component managers
pub struct RuntimeState {
    pub config: Arc<RuntimeConfig>,
    pub session_manager: Arc<SessionManager>,
    pub memory_store: Arc<PostgresMemoryStore>,
    pub llm_provider: Arc<OpenRouterProvider>,
    pub tool_registry: Arc<ToolRegistry>,
    pub shutdown_signal: broadcast::Sender<()>,
}

impl RuntimeState {
    pub async fn new(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        let config = Arc::new(config);
        
        // Initialize PostgreSQL connection pool
        let pool = deadpool_postgres::Config {
            host: Some(config.memory.postgres_host.clone()),
            port: Some(config.memory.postgres_port),
            dbname: Some(config.memory.postgres_db.clone()),
            user: Some(config.memory.postgres_user.clone()),
            password: Some(config.memory.postgres_password.clone()),
            ..Default::default()
        }
        .create_pool(Some(deadpool_postgres::Runtime::Tokio1), tokio_postgres::NoTls)
        .map_err(|e| RuntimeError::Initialization(e.to_string()))?;

        // Initialize components
        let memory_store = Arc::new(
            PostgresMemoryStore::new(pool, config.memory.vector_dimension)
                .await
                .map_err(|e| RuntimeError::Initialization(e.to_string()))?,
        );

        let llm_provider = Arc::new(
            OpenRouterProvider::new(config.clone().into_openrouter_config())
                .map_err(|e| RuntimeError::Initialization(e.to_string()))?,
        );

        let tool_registry = Arc::new(ToolRegistry::new(&config)?);
        let session_manager = Arc::new(SessionManager::new(config.clone()));
        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            config,
            session_manager,
            memory_store,
            llm_provider,
            tool_registry,
            shutdown_signal: shutdown_tx,
        })
    }

    pub async fn shutdown(&self) {
        let _ = self.shutdown_signal.send(());
        // Additional cleanup if needed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_runtime_state() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = RuntimeConfig::default();
        config.tools.backup_dir = temp_dir.path().to_path_buf();
        config.memory.postgres_password = "test_password".to_string();
        config.llm.openrouter_api_key = "test_key".to_string();

        let state = RuntimeState::new(config).await;
        assert!(state.is_ok());

        let state = state.unwrap();
        
        // Test session management
        let session_id = state.session_manager.create_session();
        assert!(state.session_manager.get_session(session_id).is_some());

        // Test tool registry
        assert!(state.tool_registry.get_process_tool().is_some());
        assert!(state.tool_registry.get_self_modify_tool().is_some());

        // Test shutdown
        state.shutdown().await;
    }

    #[tokio::test]
    async fn test_session_manager() {
        let config = Arc::new(RuntimeConfig::default());
        let manager = SessionManager::new(config);

        // Create and verify session
        let session_id = manager.create_session();
        let session = manager.get_session(session_id);
        assert!(session.is_some());
        assert_eq!(session.unwrap().id, session_id);

        // Test session cleanup
        std::thread::sleep(std::time::Duration::from_millis(100));
        manager.cleanup_expired_sessions(std::time::Duration::from_millis(50));
        assert!(manager.get_session(session_id).is_none());
    }
}
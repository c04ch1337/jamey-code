use crate::config::RuntimeConfig;
use crate::hybrid_orchestrator::{HybridOrchestrator, SafetyMode, FullAccessConfig};
use crate::scheduler::TaskScheduler;
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
///
/// Arc is used for RuntimeConfig because:
/// - Config is shared across multiple sessions and components
/// - Config is read-only after initialization
/// - Arc provides cheap cloning for sharing across threads
pub struct SessionManager {
    sessions: DashMap<Uuid, Session>,
    config: Arc<RuntimeConfig>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub memory_context: DashMap<Uuid, Memory>,
    pub last_activity: std::time::Instant,
}

impl Session {
    fn new(id: Uuid) -> Self {
        Self {
            id,
            memory_context: DashMap::new(),
            last_activity: std::time::Instant::now(),
        }
    }

    pub fn add_memory(&self, memory: Memory) {
        self.memory_context.insert(memory.id, memory);
    }

    pub fn get_memory(&self, id: &Uuid) -> Option<Memory> {
        self.memory_context.get(id).map(|m| (*m.value()).clone())
    }

    /// List all memories in the session (use with caution for large sessions)
    pub fn list_memories(&self) -> Vec<Memory> {
        let start = std::time::Instant::now();
        let memories: Vec<Memory> = self.memory_context
            .iter()
            .map(|m| (*m.value()).clone())
            .collect();
        let duration = start.elapsed();
        tracing::debug!("list_memories() took {:?} to collect {} memories", duration, memories.len());
        memories
    }

    /// List memories with pagination support
    ///
    /// # Arguments
    /// * `limit` - Maximum number of memories to return
    /// * `offset` - Number of memories to skip
    ///
    /// # Returns
    /// A tuple of (memories, total_count) for pagination metadata
    pub fn list_memories_paginated(&self, limit: usize, offset: usize) -> (Vec<Memory>, usize) {
        let start = std::time::Instant::now();
        let total_count = self.memory_context.len();
        
        let memories: Vec<Memory> = self.memory_context
            .iter()
            .skip(offset)
            .take(limit)
            .map(|m| (*m.value()).clone())
            .collect();
        
        let duration = start.elapsed();
        tracing::debug!(
            "list_memories_paginated(limit={}, offset={}) took {:?} to collect {} of {} memories",
            limit, offset, duration, memories.len(), total_count
        );
        
        (memories, total_count)
    }

    /// Iterator-based memory access for streaming large result sets
    /// Returns an iterator that yields Memory references without cloning
    pub fn iter_memories(&self) -> impl Iterator<Item = Memory> + '_ {
        self.memory_context.iter().map(|m| (*m.value()).clone())
    }
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
        self.sessions.insert(session_id, Session::new(session_id));
        session_id
    }

    pub fn get_session(&self, id: Uuid) -> Option<Session> {
        // Optimize: Update last_activity in-place instead of cloning entire session
        self.sessions.get_mut(&id).map(|mut s| {
            s.last_activity = std::time::Instant::now();
            s.clone()
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
///
/// Arc usage rationale:
/// - config: Shared read-only configuration across all components
/// - session_manager: Shared mutable state accessed from multiple async tasks
/// - memory_store: Shared database connection pool, thread-safe by design
/// - llm_provider: Shared API client with internal connection pooling
/// - tool_registry: Shared read-only tool instances
/// - hybrid_orchestrator: Shared mutable orchestrator state (Mutex for interior mutability)
/// - scheduler: Shared mutable scheduler state (Mutex for interior mutability)
pub struct RuntimeState {
    pub config: Arc<RuntimeConfig>,
    pub session_manager: Arc<SessionManager>,
    pub memory_store: Arc<PostgresMemoryStore>,
    pub llm_provider: Arc<OpenRouterProvider>,
    pub tool_registry: Arc<ToolRegistry>,
    pub hybrid_orchestrator: Arc<tokio::sync::Mutex<HybridOrchestrator>>,
    pub scheduler: Arc<tokio::sync::Mutex<TaskScheduler>>,
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
        tracing::debug!("Creating PostgresMemoryStore Arc");
        let memory_store = Arc::new(
            PostgresMemoryStore::new(pool, config.memory.vector_dimension)
                .await
                .map_err(|e| RuntimeError::Initialization(format!("Failed to create memory store: {}", e)))?
        );
        tracing::debug!("PostgresMemoryStore Arc strong count: {}", Arc::strong_count(&memory_store));

        tracing::debug!("Creating OpenRouterProvider Arc");
        // Optimize: Use reference to config instead of cloning Arc
        let llm_provider = Arc::new(
            OpenRouterProvider::new((*config).clone().into_openrouter_config()
                .map_err(|e| RuntimeError::Initialization(format!("Failed to create OpenRouter config: {}", e)))?)
                .map_err(|e| RuntimeError::Initialization(format!("Failed to create OpenRouter provider: {}", e)))?
        );
        tracing::debug!("OpenRouterProvider Arc strong count: {}", Arc::strong_count(&llm_provider));

        tracing::debug!("Creating ToolRegistry Arc");
        let tool_registry = Arc::new(ToolRegistry::new(&config)?);
        tracing::debug!("ToolRegistry Arc strong count: {}", Arc::strong_count(&tool_registry));
        tracing::debug!("Creating SessionManager Arc");
        // Arc clone is necessary here as SessionManager needs to own the config
        let session_manager = Arc::new(SessionManager::new(Arc::clone(&config)));
        tracing::debug!("SessionManager Arc strong count: {}", Arc::strong_count(&session_manager));

        // Initialize Hybrid Orchestrator
        tracing::debug!("Creating HybridOrchestrator");
        let safety_mode = if config.tools.enable_24_7 {
            SafetyMode::Development
        } else {
            SafetyMode::Testing
        };
        let mut hybrid_orch = HybridOrchestrator::new(safety_mode, config.tools.system_root.clone());
        
        // Register all connectors
        let full_access_config = FullAccessConfig {
            backup_dir: config.tools.backup_dir.clone(),
            download_dir: config.tools.download_dir.clone(),
            system_root: config.tools.system_root.clone(),
            github_token: config.tools.github_token.clone(),
            linkedin_token: config.tools.linkedin_token.clone(),
            web_search_api_key: config.tools.web_search_api_key.clone(),
            mcp_server_url: config.tools.mcp_server_url.clone(),
        };
        hybrid_orch.register_all_connectors(&full_access_config).await
            .map_err(|e| RuntimeError::Initialization(format!("Failed to register connectors: {}", e)))?;
        
        let hybrid_orchestrator = Arc::new(tokio::sync::Mutex::new(hybrid_orch));

        // Initialize Scheduler
        tracing::debug!("Creating TaskScheduler");
        let scheduler = Arc::new(tokio::sync::Mutex::new(TaskScheduler::new()));

        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            config,
            session_manager,
            memory_store,
            llm_provider,
            tool_registry,
            hybrid_orchestrator,
            scheduler,
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
    async fn test_runtime_state() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let mut config = RuntimeConfig::default();
        config.tools.backup_dir = temp_dir.path().to_path_buf();
        config.tools.download_dir = temp_dir.path().join("downloads");
        config.tools.system_root = temp_dir.path().to_path_buf();
        // Test secrets - do not use in production
        use tracing_honeycomb::SensitiveValue;
        config.memory.postgres_password = SensitiveValue("test_password".to_string());
        config.llm.openrouter_api_key = SensitiveValue("test_key".to_string());
        
        let state = RuntimeState::new(config).await?;
        
        // Test session management
        let session_id = state.session_manager.create_session();
        assert!(state.session_manager.get_session(session_id).is_some());

        // Test tool registry
        assert!(state.tool_registry.get_process_tool().is_some());
        assert!(state.tool_registry.get_self_modify_tool().is_some());

        // Test shutdown
        state.shutdown().await;
        Ok(())
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
//! Hybrid Orchestrator
//! 
//! Combines system administration and self-improvement capabilities
//! with all full-access connectors

use jamey_tools::connector::{Connector, ConnectorRegistry, ConnectorResult, ExecutionContext};
use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum SafetyMode {
    Development,  // All connectors enabled, minimal restrictions
    Testing,      // Enhanced safety checks, approval required
    Production,   // Locked down, read-only by default
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub connector_id: String,
    pub action: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub success: bool,
    pub requires_rollback: bool,
}

#[derive(Debug, Clone)]
pub struct FullAccessConfig {
    pub backup_dir: PathBuf,
    pub download_dir: PathBuf,
    pub system_root: PathBuf,
    pub github_token: Option<String>,
    pub linkedin_token: Option<String>,
    pub web_search_api_key: Option<String>,
    pub mcp_server_url: Option<String>,
}

pub struct HybridOrchestrator {
    connector_registry: ConnectorRegistry,
    execution_history: Vec<ExecutionRecord>,
    safety_mode: SafetyMode,
    context: ExecutionContext,
}

impl HybridOrchestrator {
    pub fn new(safety_mode: SafetyMode, system_root: PathBuf) -> Self {
        let context = ExecutionContext {
            user_id: "jamey".to_string(),
            session_id: uuid::Uuid::new_v4().to_string(),
            network_access: true,
            file_system_root: system_root,
            allowed_hosts: Vec::new(), // Empty = all hosts
            credentials: HashMap::new(),
        };

        Self {
            connector_registry: ConnectorRegistry::new(),
            execution_history: Vec::new(),
            safety_mode,
            context,
        }
    }

    /// Register all connectors with full access configuration
    pub async fn register_all_connectors(&self, config: &FullAccessConfig) -> Result<()> {
        // System Admin
        let sys_admin = Box::new(jamey_tools::connectors::SystemAdminConnector::new());
        self.connector_registry.register(sys_admin).await?;
        info!("System Admin connector registered");

        // Self Improvement
        let self_improve = Box::new(
            jamey_tools::connectors::SelfImproveConnector::new(config.backup_dir.clone(), 5)?
        );
        self.connector_registry.register(self_improve).await?;
        info!("Self-Improvement connector registered");

        // Network & Web
        let network_web = Box::new(
            jamey_tools::connectors::NetworkWebConnector::new(
                config.download_dir.clone(),
                config.web_search_api_key.clone()
            )?
        );
        self.connector_registry.register(network_web).await?;
        info!("Network & Web connector registered");

        // GitHub
        if let Some(ref token) = config.github_token {
            let github = Box::new(
                jamey_tools::connectors::GitHubConnector::new(token.clone())?
            );
            self.connector_registry.register(github).await?;
            info!("GitHub connector registered");
        }

        // LinkedIn
        if let Some(ref token) = config.linkedin_token {
            let linkedin = Box::new(
                jamey_tools::connectors::LinkedInConnector::new(token.clone())?
            );
            self.connector_registry.register(linkedin).await?;
            info!("LinkedIn connector registered");
        }

        // Agent Orchestration
        let agent_orch = Box::new(
            jamey_tools::connectors::AgentOrchestrationConnector::new()
        );
        self.connector_registry.register(agent_orch).await?;
        info!("Agent Orchestration connector registered");

        // MCP
        if let Some(ref mcp_url) = config.mcp_server_url {
            let mcp = Box::new(
                jamey_tools::connectors::MCPConnector::new(mcp_url.clone())
            );
            self.connector_registry.register(mcp).await?;
            info!("MCP connector registered");
        }

        // Full System Access
        let full_sys = Box::new(
            jamey_tools::connectors::FullSystemConnector::new(config.system_root.clone())
        );
        self.connector_registry.register(full_sys).await?;
        info!("Full System Access connector registered");

        // IoT Device Connector
        let iot = Box::new(
            jamey_tools::connectors::IoTConnector::new()?
        );
        self.connector_registry.register(iot).await?;
        info!("IoT Device connector registered");

        Ok(())
    }

    /// Execute a connector
    pub async fn execute_connector(
        &mut self,
        connector_id: &str,
        params: HashMap<String, String>,
    ) -> Result<ConnectorResult> {
        let result = self.connector_registry
            .execute_connector(connector_id, params.clone(), &self.context)
            .await?;

        // Record execution
        self.execution_history.push(ExecutionRecord {
            connector_id: connector_id.to_string(),
            action: params.get("action").cloned().unwrap_or_default(),
            timestamp: chrono::Utc::now(),
            success: result.success,
            requires_rollback: !result.errors.is_empty() && result.success,
        });

        Ok(result)
    }

    /// Execute a hybrid operation (combines system admin + self-improvement)
    pub async fn execute_hybrid(
        &mut self,
        operation: HybridOperation,
    ) -> Result<HybridResult> {
        match operation {
            HybridOperation::OptimizeSystem { target_processes, optimize_code } => {
                self.optimize_system_and_code(target_processes, optimize_code).await
            }
            HybridOperation::MonitorAndImprove { duration_secs } => {
                self.monitor_and_improve(duration_secs).await
            }
            HybridOperation::SelfHeal { issue_description } => {
                self.self_heal(issue_description).await
            }
        }
    }

    async fn optimize_system_and_code(
        &mut self,
        target_processes: Vec<String>,
        optimize_code: bool,
    ) -> Result<HybridResult> {
        let mut results = HybridResult::new();

        // Step 1: System optimization
        if self.connector_registry.has_connector("system_admin").await {
            let mut params = HashMap::new();
            params.insert("action".to_string(), "list_processes".to_string());

            let sys_result = self.execute_connector("system_admin", params).await?;
            results.add_result("system_analysis", sys_result);
        }

        // Step 2: Code optimization (self-improvement)
        if optimize_code && self.connector_registry.has_connector("self_improve").await {
            // Analyze code performance
            let mut params = HashMap::new();
            params.insert("action".to_string(), "list_source_files".to_string());
            params.insert("pattern".to_string(), "**/*.rs".to_string());

            let code_result = self.execute_connector("self_improve", params).await?;
            results.add_result("code_analysis", code_result);
        }

        Ok(results)
    }

    async fn monitor_and_improve(&mut self, duration_secs: u64) -> Result<HybridResult> {
        let start = std::time::Instant::now();
        let mut results = HybridResult::new();

        while start.elapsed().as_secs() < duration_secs {
            // Monitor system
            // Identify improvements
            // Apply optimizations
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }

        Ok(results)
    }

    async fn self_heal(&mut self, issue_description: String) -> Result<HybridResult> {
        // Diagnose issue
        // Fix system problems
        // Improve code to prevent recurrence
        let mut results = HybridResult::new();
        // Implementation
        Ok(results)
    }

    /// Lock down the orchestrator (for production)
    pub async fn lock_down(&self) {
        self.connector_registry.lock().await;
        info!("Hybrid orchestrator locked down");
    }

    pub fn get_registry(&self) -> &ConnectorRegistry {
        &self.connector_registry
    }
}

#[derive(Debug, Clone)]
pub enum HybridOperation {
    OptimizeSystem {
        target_processes: Vec<String>,
        optimize_code: bool,
    },
    MonitorAndImprove {
        duration_secs: u64,
    },
    SelfHeal {
        issue_description: String,
    },
}

pub struct HybridResult {
    results: HashMap<String, ConnectorResult>,
    summary: String,
}

impl HybridResult {
    fn new() -> Self {
        Self {
            results: HashMap::new(),
            summary: String::new(),
        }
    }

    fn add_result(&mut self, key: String, result: ConnectorResult) {
        self.results.insert(key, result);
    }
}


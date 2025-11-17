//! Connector architecture for extensible tool system
//! 
//! This module provides the base connector trait and registry for
//! all system capabilities including network, web, cloud services, and more.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

/// Connector capability levels for full access
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityLevel {
    ReadOnly,
    ReadWrite,
    SystemAdmin,
    SelfModify,
    NetworkAccess,
    WebAccess,
    CloudAccess,
    AgentOrchestration,
    FullAccess,
}

/// Connector metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub capability_level: CapabilityLevel,
    pub requires_approval: bool,
    pub safety_checks: Vec<String>,
}

/// Connector execution context with full system access
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub user_id: String,
    pub session_id: String,
    pub network_access: bool,
    pub file_system_root: PathBuf,
    pub allowed_hosts: Vec<String>, // Empty = all hosts allowed
    pub credentials: HashMap<String, String>, // Encrypted credentials
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            user_id: "jamey".to_string(),
            session_id: uuid::Uuid::new_v4().to_string(),
            network_access: true,
            file_system_root: PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" }),
            allowed_hosts: Vec::new(), // Empty = all hosts
            credentials: HashMap::new(),
        }
    }
}

/// Network request tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkRequest {
    pub url: String,
    pub method: String,
    pub status_code: Option<u16>,
    pub timestamp: DateTime<Utc>,
}

/// Connector execution result with full metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorResult {
    pub success: bool,
    pub output: String,
    pub metadata: HashMap<String, String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub network_requests: Vec<NetworkRequest>,
    pub files_accessed: Vec<String>,
    pub agents_contacted: Vec<String>,
}

impl ConnectorResult {
    pub fn new() -> Self {
        Self {
            success: false,
            output: String::new(),
            metadata: HashMap::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
            network_requests: Vec::new(),
            files_accessed: Vec::new(),
            agents_contacted: Vec::new(),
        }
    }
}

/// Base trait for all connectors
#[async_trait::async_trait]
pub trait Connector: Send + Sync {
    /// Get connector metadata
    fn metadata(&self) -> &ConnectorMetadata;
    
    /// Execute the connector with given parameters and context
    async fn execute(
        &self,
        params: HashMap<String, String>,
        context: &ExecutionContext,
    ) -> Result<ConnectorResult>;
    
    /// Validate parameters before execution
    fn validate(&self, params: &HashMap<String, String>) -> Result<()>;
    
    /// Get required parameters
    fn required_params(&self) -> Vec<String>;
    
    /// Check if connector is enabled
    fn is_enabled(&self) -> bool;
    
    /// Get safety checks for this connector
    fn safety_checks(&self) -> Vec<String>;
    
    /// Check if connector requires network access
    fn requires_network(&self) -> bool;
    
    /// Get required credential keys
    fn requires_credentials(&self) -> Vec<String>;
}

/// Connector registry for dynamic registration
pub struct ConnectorRegistry {
    connectors: Arc<RwLock<HashMap<String, Box<dyn Connector>>>>,
    enabled_connectors: Arc<RwLock<Vec<String>>>,
    locked: Arc<RwLock<bool>>,
}

impl ConnectorRegistry {
    pub fn new() -> Self {
        Self {
            connectors: Arc::new(RwLock::new(HashMap::new())),
            enabled_connectors: Arc::new(RwLock::new(Vec::new())),
            locked: Arc::new(RwLock::new(false)),
        }
    }
    
    pub async fn register(&self, connector: Box<dyn Connector>) -> Result<()> {
        let locked = *self.locked.read().await;
        if locked {
            return Err(anyhow::anyhow!("Registry is locked - cannot register new connectors"));
        }
        
        let id = connector.metadata().id.clone();
        let mut connectors = self.connectors.write().await;
        connectors.insert(id.clone(), connector);
        
        let mut enabled = self.enabled_connectors.write().await;
        enabled.push(id);
        
        Ok(())
    }
    
    pub async fn has_connector(&self, id: &str) -> bool {
        let connectors = self.connectors.read().await;
        connectors.contains_key(id)
    }
    
    pub async fn list(&self) -> Vec<ConnectorMetadata> {
        let connectors = self.connectors.read().await;
        connectors.values()
            .map(|c| c.metadata().clone())
            .collect()
    }
    
    pub async fn execute_connector(
        &self,
        id: &str,
        params: HashMap<String, String>,
        context: &ExecutionContext,
    ) -> Result<ConnectorResult> {
        let connectors = self.connectors.read().await;
        let connector = connectors.get(id)
            .ok_or_else(|| anyhow::anyhow!("Connector not found: {}", id))?;
        
        connector.validate(&params)?;
        connector.execute(params, context).await
    }
    
    pub async fn lock(&self) {
        let mut locked = self.locked.write().await;
        *locked = true;
    }
    
    pub async fn is_locked(&self) -> bool {
        *self.locked.read().await
    }
}

impl Default for ConnectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}


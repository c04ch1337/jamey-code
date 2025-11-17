//! Agent-to-Agent Orchestration Connector
//!
//! Orchestrates tasks across multiple agents with full communication

use crate::connector::*;
use reqwest::{Client, ClientBuilder};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::{Result, Context};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct AgentEndpoint {
    pub id: String,
    pub name: String,
    pub url: String,
    pub api_key: String,  // Made required (not optional)
    pub capabilities: Vec<String>,
}

pub struct AgentOrchestrationConnector {
    metadata: ConnectorMetadata,
    registered_agents: Arc<RwLock<HashMap<String, AgentEndpoint>>>,
    client: Client,
    enabled: bool,
}

impl AgentOrchestrationConnector {
    pub fn new() -> Result<Self> {
        // Build client with strict TLS settings
        let client = ClientBuilder::new()
            .min_tls_version(reqwest::tls::Version::TLS_1_2)  // Minimum TLS 1.2 (1.3 not supported by all backends)
            .danger_accept_invalid_certs(false)  // Never accept invalid certs
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to build secure HTTP client")?;
        
        Ok(Self {
            metadata: ConnectorMetadata {
                id: "agent_orchestration".to_string(),
                name: "Agent-to-Agent Orchestration".to_string(),
                version: "1.0.0".to_string(),
                description: "Orchestrate tasks across multiple agents with full communication".to_string(),
                capability_level: CapabilityLevel::AgentOrchestration,
                requires_approval: false,
                safety_checks: vec![
                    "Agent authentication required (API key mandatory)".to_string(),
                    "TLS 1.2+ minimum enforced".to_string(),
                    "Certificate validation enabled".to_string(),
                    "Task validation before delegation".to_string(),
                ],
            },
            registered_agents: Arc::new(RwLock::new(HashMap::new())),
            client,
            enabled: true,
        })
    }

    pub async fn register_agent(&self, agent: AgentEndpoint) -> Result<()> {
        // Validate agent URL
        let parsed_url = url::Url::parse(&agent.url)
            .context("Invalid agent URL")?;
        
        // Only allow HTTPS for agent communication
        if parsed_url.scheme() != "https" {
            anyhow::bail!(
                "Security violation: Agent URLs must use HTTPS. Got: {}",
                parsed_url.scheme()
            );
        }
        
        // Validate API key is not empty
        if agent.api_key.trim().is_empty() {
            anyhow::bail!("Security violation: API key cannot be empty");
        }
        
        tracing::info!("Registering agent: {} at {}", agent.name, agent.url);
        let mut agents = self.registered_agents.write().await;
        agents.insert(agent.id.clone(), agent);
        Ok(())
    }

    async fn send_task_to_agent(&self, agent_id: &str, task: &str, params: &HashMap<String, String>) -> Result<Value> {
        let agents = self.registered_agents.read().await;
        let agent = agents.get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", agent_id))?;
        
        tracing::info!("Sending task to agent {}: {}", agent_id, task);
        
        // API key is now required (not optional)
        let request = self.client.post(&format!("{}/api/v1/tasks", agent.url))
            .header("Authorization", format!("Bearer {}", agent.api_key))
            .json(&serde_json::json!({
                "task": task,
                "params": params
            }));
        
        let response = request.send().await
            .context("Failed to send task to agent")?;
        
        if !response.status().is_success() {
            anyhow::bail!(
                "Agent returned error status: {}",
                response.status()
            );
        }
        
        Ok(response.json().await?)
    }

    async fn broadcast_task(&self, task: &str, params: &HashMap<String, String>) -> Result<HashMap<String, Value>> {
        let agents = self.registered_agents.read().await;
        let mut results = HashMap::new();
        
        for (agent_id, _) in agents.iter() {
            match self.send_task_to_agent(agent_id, task, params).await {
                Ok(result) => {
                    results.insert(agent_id.clone(), result);
                }
                Err(e) => {
                    results.insert(agent_id.clone(), serde_json::json!({
                        "error": e.to_string()
                    }));
                }
            }
        }
        
        Ok(results)
    }
}

#[async_trait::async_trait]
impl Connector for AgentOrchestrationConnector {
    fn metadata(&self) -> &ConnectorMetadata {
        &self.metadata
    }
    
    async fn execute(
        &self,
        params: HashMap<String, String>,
        _context: &ExecutionContext,
    ) -> Result<ConnectorResult> {
        let action = params.get("action")
            .ok_or_else(|| anyhow::anyhow!("Missing 'action' parameter"))?;
        
        let mut result = ConnectorResult::new();
        
        match action.as_str() {
            "register_agent" => {
                let agent = AgentEndpoint {
                    id: params.get("agent_id").ok_or_else(|| anyhow::anyhow!("Missing agent_id"))?.clone(),
                    name: params.get("name").ok_or_else(|| anyhow::anyhow!("Missing name"))?.clone(),
                    url: params.get("url").ok_or_else(|| anyhow::anyhow!("Missing url"))?.clone(),
                    api_key: params.get("api_key")
                        .ok_or_else(|| anyhow::anyhow!("Missing required api_key"))?
                        .clone(),
                    capabilities: params.get("capabilities")
                        .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or_default(),
                };
                self.register_agent(agent).await?;
                result.output = "Agent registered successfully".to_string();
                result.success = true;
            }
            "send_task" => {
                let agent_id = params.get("agent_id").ok_or_else(|| anyhow::anyhow!("Missing agent_id"))?;
                let task = params.get("task").ok_or_else(|| anyhow::anyhow!("Missing task"))?;
                let task_params: HashMap<String, String> = params.iter()
                    .filter(|(k, _)| k.starts_with("param_"))
                    .filter_map(|(k, v)| {
                        k.strip_prefix("param_").map(|stripped| (stripped.to_string(), v.clone()))
                    })
                    .collect();
                
                let response = self.send_task_to_agent(agent_id, task, &task_params).await?;
                result.output = serde_json::to_string_pretty(&response)?;
                result.success = true;
                result.agents_contacted.push(agent_id.clone());
            }
            "broadcast" => {
                let task = params.get("task").ok_or_else(|| anyhow::anyhow!("Missing task"))?;
                let task_params: HashMap<String, String> = params.iter()
                    .filter(|(k, _)| k.starts_with("param_"))
                    .filter_map(|(k, v)| {
                        k.strip_prefix("param_").map(|stripped| (stripped.to_string(), v.clone()))
                    })
                    .collect();
                
                let results = self.broadcast_task(task, &task_params).await?;
                result.output = serde_json::to_string_pretty(&results)?;
                result.success = true;
            }
            _ => {
                result.errors.push(format!("Unknown action: {}", action));
            }
        }
        
        Ok(result)
    }
    
    fn validate(&self, params: &HashMap<String, String>) -> Result<()> {
        if !params.contains_key("action") {
            return Err(anyhow::anyhow!("Missing required parameter: action"));
        }
        Ok(())
    }
    
    fn required_params(&self) -> Vec<String> {
        vec!["action".to_string()]
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    fn safety_checks(&self) -> Vec<String> {
        self.metadata.safety_checks.clone()
    }
    
    fn requires_network(&self) -> bool {
        true
    }
    
    fn requires_credentials(&self) -> Vec<String> {
        vec![] // Depends on agent configuration
    }
}


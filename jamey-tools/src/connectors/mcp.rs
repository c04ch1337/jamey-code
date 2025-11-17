//! MCP (Model Context Protocol) Connector
//! 
//! Provides MCP protocol support for connecting to MCP servers

use crate::connector::*;
use reqwest::Client;
use std::collections::HashMap;
use anyhow::Result;
use serde_json::Value;

pub struct MCPConnector {
    metadata: ConnectorMetadata,
    mcp_server_url: String,
    client: Client,
    enabled: bool,
}

impl MCPConnector {
    pub fn new(mcp_server_url: String) -> Self {
        Self {
            metadata: ConnectorMetadata {
                id: "mcp".to_string(),
                name: "Model Context Protocol".to_string(),
                version: "1.0.0".to_string(),
                description: "MCP protocol support for connecting to MCP servers".to_string(),
                capability_level: CapabilityLevel::FullAccess,
                requires_approval: false,
                safety_checks: vec![
                    "MCP server authentication".to_string(),
                    "Protocol version validation".to_string(),
                ],
            },
            mcp_server_url,
            client: Client::new(),
            enabled: true,
        }
    }

    async fn mcp_request(&self, method: &str, params: &Value) -> Result<Value> {
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });
        
        let response = self.client
            .post(&self.mcp_server_url)
            .json(&payload)
            .send()
            .await?;
        
        Ok(response.json().await?)
    }

    async fn list_resources(&self) -> Result<Value> {
        self.mcp_request("resources/list", &serde_json::json!({})).await
    }

    async fn read_resource(&self, uri: &str) -> Result<Value> {
        self.mcp_request("resources/read", &serde_json::json!({
            "uri": uri
        })).await
    }

    async fn call_tool(&self, name: &str, arguments: &Value) -> Result<Value> {
        self.mcp_request("tools/call", &serde_json::json!({
            "name": name,
            "arguments": arguments
        })).await
    }
}

#[async_trait::async_trait]
impl Connector for MCPConnector {
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
            "list_resources" => {
                let resources = self.list_resources().await?;
                result.output = serde_json::to_string_pretty(&resources)?;
                result.success = true;
            }
            "read_resource" => {
                let uri = params.get("uri").ok_or_else(|| anyhow::anyhow!("Missing uri"))?;
                let resource = self.read_resource(uri).await?;
                result.output = serde_json::to_string_pretty(&resource)?;
                result.success = true;
            }
            "call_tool" => {
                let name = params.get("name").ok_or_else(|| anyhow::anyhow!("Missing name"))?;
                let args_json = params.get("arguments")
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or(serde_json::json!({}));
                let tool_result = self.call_tool(name, &args_json).await?;
                result.output = serde_json::to_string_pretty(&tool_result)?;
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
        vec![] // Depends on MCP server configuration
    }
}


//! System Administration Connector
//!
//! Provides process management, system monitoring, and resource control

use crate::connector::*;
use crate::system::ProcessTool;
use std::collections::HashMap;
use anyhow::Result;

/// List of protected process names that cannot be terminated
/// These are critical system processes that should never be killed
const PROTECTED_PROCESS_NAMES: &[&str] = &[
    // Windows critical processes
    "System", "csrss.exe", "wininit.exe", "services.exe", "lsass.exe",
    "winlogon.exe", "smss.exe", "svchost.exe", "explorer.exe",
    // Linux/Unix critical processes
    "init", "systemd", "launchd", "kernel", "kthreadd",
    // Database processes
    "postgres", "mysqld", "mongod", "redis-server",
    // Security processes
    "antivirus", "defender", "firewall",
];

/// Checks if a process is protected and should not be terminated
///
/// # Security
/// Prevents termination of critical system processes
///
/// # Examples
/// ```
/// assert!(is_protected_process("csrss.exe"));
/// assert!(!is_protected_process("notepad.exe"));
/// ```
fn is_protected_process(process_name: &str) -> bool {
    let lower_name = process_name.to_lowercase();
    
    PROTECTED_PROCESS_NAMES.iter().any(|protected| {
        lower_name.contains(&protected.to_lowercase())
    })
}

pub struct SystemAdminConnector {
    metadata: ConnectorMetadata,
    #[cfg(windows)]
    registry_tool: Option<crate::system::RegistryTool>,
    enabled: bool,
}

impl SystemAdminConnector {
    pub fn new() -> Self {
        Self {
            metadata: ConnectorMetadata {
                id: "system_admin".to_string(),
                name: "System Administration".to_string(),
                version: "1.0.0".to_string(),
                description: "Process management, system monitoring, and resource control".to_string(),
                capability_level: CapabilityLevel::SystemAdmin,
                requires_approval: true,
                safety_checks: vec![
                    "Process kill operations require confirmation".to_string(),
                    "System resource limits enforced".to_string(),
                ],
            },
            #[cfg(windows)]
            registry_tool: Some(crate::system::RegistryTool::new()),
            enabled: true,
        }
    }
}

#[async_trait::async_trait]
impl Connector for SystemAdminConnector {
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
            "list_processes" => {
                let mut tool = ProcessTool::new();
                let processes = tool.list_processes();
                result.output = serde_json::to_string_pretty(&processes)?;
                result.success = true;
                result.metadata.insert("process_count".to_string(), processes.len().to_string());
            }
            "kill_process" => {
                let pid = params.get("pid")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'pid' parameter"))?
                    .parse::<u32>()?;
                
                // Safety check: require confirmation
                if !params.contains_key("confirmed") {
                    result.errors.push("Process kill requires confirmation".to_string());
                    return Ok(result);
                }
                
                // Get process info to check if it's protected
                let mut tool = ProcessTool::new();
                let process_info = tool.get_process_info(pid)
                    .map_err(|e| anyhow::anyhow!("Failed to get process info: {}", e))?;
                
                // Check if process is protected
                if is_protected_process(&process_info.name) {
                    anyhow::bail!(
                        "Security violation: Cannot terminate protected process '{}' (PID: {}). \
                        This is a critical system process.",
                        process_info.name,
                        pid
                    );
                }
                
                tracing::warn!("Terminating process: {} (PID: {})", process_info.name, pid);
                
                tool.kill_process(pid)
                    .map_err(|e| anyhow::anyhow!("Failed to kill process: {}", e))?;
                result.success = true;
                result.output = format!("Process {} ({}) terminated", pid, process_info.name);
            }
            "get_process_info" => {
                let pid = params.get("pid")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'pid' parameter"))?
                    .parse::<u32>()?;
                let mut tool = ProcessTool::new();
                let info = tool.get_process_info(pid)
                    .map_err(|e| anyhow::anyhow!("Failed to get process info: {}", e))?;
                result.output = serde_json::to_string_pretty(&info)?;
                result.success = true;
            }
            #[cfg(windows)]
            "read_registry" => {
                let key = params.get("key")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'key' parameter"))?;
                let value = params.get("value")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'value' parameter"))?;
                
                if let Some(ref reg_tool) = self.registry_tool {
                    let reg_value = reg_tool.read_value(key, value)
                        .map_err(|e| anyhow::anyhow!("Registry read failed: {}", e))?;
                    result.output = reg_value;
                    result.success = true;
                } else {
                    result.errors.push("Registry tool not available".to_string());
                }
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
        false
    }
    
    fn requires_credentials(&self) -> Vec<String> {
        vec![]
    }
}


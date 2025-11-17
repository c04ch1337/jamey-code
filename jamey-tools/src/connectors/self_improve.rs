//! Self-Improvement Connector
//! 
//! Provides read and modify source code capabilities with automatic backups

use crate::connector::*;
use crate::system::SelfModifyTool;
use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;

pub struct SelfImproveConnector {
    metadata: ConnectorMetadata,
    modify_tool: SelfModifyTool,
    enabled: bool,
    backup_count: usize,
}

impl SelfImproveConnector {
    pub fn new(backup_dir: PathBuf, backup_count: usize) -> Result<Self> {
        Ok(Self {
            metadata: ConnectorMetadata {
                id: "self_improve".to_string(),
                name: "Self-Improvement".to_string(),
                version: "1.0.0".to_string(),
                description: "Read and modify source code with automatic backups".to_string(),
                capability_level: CapabilityLevel::SelfModify,
                requires_approval: true,
                safety_checks: vec![
                    "Automatic backup before modification".to_string(),
                    "Source file validation".to_string(),
                    "Rollback capability".to_string(),
                ],
            },
            modify_tool: SelfModifyTool::new(&backup_dir)?,
            enabled: true,
            backup_count,
        })
    }
}

#[async_trait::async_trait]
impl Connector for SelfImproveConnector {
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
            "read_file" => {
                let file_path = params.get("file_path")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'file_path' parameter"))?;
                let path = PathBuf::from(file_path);
                
                // Validate it's a source file
                if !path.extension().map_or(false, |ext| ext == "rs" || ext == "toml" || ext == "md") {
                    result.warnings.push("File is not a recognized source file".to_string());
                }
                
                let content = tokio::fs::read_to_string(&path).await?;
                result.output = content;
                result.success = true;
                result.metadata.insert("file_path".to_string(), file_path.clone());
                result.files_accessed.push(path.to_string_lossy().to_string());
            }
            "modify_file" => {
                let file_path = params.get("file_path")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'file_path' parameter"))?;
                let new_content = params.get("content")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'content' parameter"))?;
                
                // Safety: Require confirmation for self-modification
                if !params.contains_key("confirmed") {
                    result.errors.push("Self-modification requires explicit confirmation".to_string());
                    return Ok(result);
                }
                
                let path = PathBuf::from(file_path);
                let backup = self.modify_tool.modify_file(&path, new_content)?;
                
                result.success = true;
                result.output = format!("File modified successfully. Backup: {:?}", backup.backup_path);
                result.metadata.insert("backup_path".to_string(), backup.backup_path.to_string_lossy().to_string());
                result.metadata.insert("backup_timestamp".to_string(), backup.timestamp.to_rfc3339());
                result.files_accessed.push(path.to_string_lossy().to_string());
            }
            "list_source_files" => {
                let default_pattern = "**/*.rs".to_string();
                let pattern = params.get("pattern").unwrap_or(&default_pattern);
                let files = self.modify_tool.list_source_files(pattern)?;
                result.output = serde_json::to_string_pretty(&files)?;
                result.success = true;
                result.metadata.insert("file_count".to_string(), files.len().to_string());
            }
            "restore_backup" => {
                let _backup_path = params.get("backup_path")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'backup_path' parameter"))?;
                
                // This would require loading backup metadata
                result.errors.push("Restore functionality requires backup metadata - use modify_tool directly".to_string());
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


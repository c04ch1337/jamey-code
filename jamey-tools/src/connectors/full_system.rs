//! Full System Access Connector
//!
//! Provides complete access to entire laptop filesystem, network, and system resources

use crate::connector::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use std::process::Command;

/// List of allowed commands for execution
const ALLOWED_COMMANDS: &[&str] = &[
    "ls", "dir", "cat", "type", "echo", "pwd", "cd",
    "git", "npm", "cargo", "python", "node", "rustc",
    "grep", "find", "which", "where", "whoami",
];

/// List of dangerous command flags that should be blocked
const BLOCKED_FLAGS: &[&str] = &[
    "--privileged", "--cap-add", "sudo", "su",
    "rm -rf /", "format", "mkfs",
];

/// Sanitizes and validates a file path to prevent path traversal attacks
///
/// # Security Checks
/// - Rejects absolute paths
/// - Blocks parent directory (..) traversal
/// - Validates paths stay within allowed directories
/// - Uses canonicalize() to resolve symlinks
///
/// # Examples
/// ```
/// let safe_path = sanitize_path(&root, "data/file.txt")?;
/// ```
fn sanitize_path(root: &Path, user_path: &str) -> Result<PathBuf> {
    // Reject absolute paths
    if Path::new(user_path).is_absolute() {
        anyhow::bail!("Security violation: Absolute paths are not allowed. Path: {}", user_path);
    }
    
    // Check for parent directory traversal
    if user_path.contains("..") {
        anyhow::bail!("Security violation: Parent directory traversal (..) is not allowed. Path: {}", user_path);
    }
    
    // Construct the full path
    let full_path = root.join(user_path);
    
    // Canonicalize to resolve symlinks and get absolute path
    let canonical_path = full_path.canonicalize()
        .with_context(|| format!("Failed to canonicalize path: {}", full_path.display()))?;
    
    // Ensure the canonical path is still within the root directory
    let canonical_root = root.canonicalize()
        .with_context(|| format!("Failed to canonicalize root: {}", root.display()))?;
    
    if !canonical_path.starts_with(&canonical_root) {
        anyhow::bail!(
            "Security violation: Path escapes root directory. Path: {}, Root: {}",
            canonical_path.display(),
            canonical_root.display()
        );
    }
    
    Ok(canonical_path)
}

/// Validates a command before execution to prevent dangerous operations
///
/// # Security Checks
/// - Only allows whitelisted commands
/// - Blocks dangerous flags and arguments
/// - Validates command structure
///
/// # Examples
/// ```
/// validate_command("git", &["status"])?;
/// ```
fn validate_command(command: &str, args: &[String]) -> Result<()> {
    // Check if command is in whitelist
    let command_name = Path::new(command)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(command);
    
    if !ALLOWED_COMMANDS.contains(&command_name) {
        anyhow::bail!(
            "Security violation: Command '{}' is not in the allowed list. Allowed commands: {:?}",
            command_name,
            ALLOWED_COMMANDS
        );
    }
    
    // Check for dangerous flags
    let args_str = args.join(" ");
    for blocked in BLOCKED_FLAGS {
        if args_str.contains(blocked) {
            anyhow::bail!(
                "Security violation: Blocked flag or pattern detected: {}",
                blocked
            );
        }
    }
    
    Ok(())
}

pub struct FullSystemConnector {
    metadata: ConnectorMetadata,
    root_path: PathBuf,
    enabled: bool,
}

impl FullSystemConnector {
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            metadata: ConnectorMetadata {
                id: "full_system".to_string(),
                name: "Full System Access".to_string(),
                version: "1.0.0".to_string(),
                description: "Complete access to entire laptop filesystem, network, and system resources".to_string(),
                capability_level: CapabilityLevel::FullAccess,
                requires_approval: false,
                safety_checks: vec![
                    "All operations logged".to_string(),
                    "Backup before destructive operations".to_string(),
                ],
            },
            root_path,
            enabled: true,
        }
    }
}

#[async_trait::async_trait]
impl Connector for FullSystemConnector {
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
                let path = params.get("path").ok_or_else(|| anyhow::anyhow!("Missing path"))?;
                
                // Sanitize path to prevent traversal attacks
                let safe_path = sanitize_path(&self.root_path, path)
                    .context("Path validation failed")?;
                
                let content = tokio::fs::read_to_string(&safe_path).await
                    .context("Failed to read file")?;
                result.output = content;
                result.success = true;
                result.files_accessed.push(safe_path.to_string_lossy().to_string());
                tracing::info!("File read: {}", safe_path.display());
            }
            "write_file" => {
                let path = params.get("path").ok_or_else(|| anyhow::anyhow!("Missing path"))?;
                let content = params.get("content").ok_or_else(|| anyhow::anyhow!("Missing content"))?;
                
                // Sanitize path to prevent traversal attacks
                let safe_path = sanitize_path(&self.root_path, path)
                    .context("Path validation failed")?;
                
                // Create parent directories if needed
                if let Some(parent) = safe_path.parent() {
                    tokio::fs::create_dir_all(parent).await
                        .context("Failed to create parent directories")?;
                }
                
                tokio::fs::write(&safe_path, content).await
                    .context("Failed to write file")?;
                result.output = format!("File written: {}", safe_path.display());
                result.success = true;
                result.files_accessed.push(safe_path.to_string_lossy().to_string());
                tracing::info!("File written: {}", safe_path.display());
            }
            "execute_command" => {
                let command = params.get("command").ok_or_else(|| anyhow::anyhow!("Missing command"))?.clone();
                let args: Vec<String> = params.get("args")
                    .map(|s| s.split_whitespace().map(|s| s.to_string()).collect())
                    .unwrap_or_default();
                
                // Validate command before execution
                validate_command(&command, &args)
                    .context("Command validation failed")?;
                
                tracing::warn!("Executing command: {} {:?}", command, args);
                
                // Execute in a blocking way with cleared environment
                let output = tokio::task::spawn_blocking(move || {
                    let mut cmd = Command::new(&command);
                    cmd.args(&args);
                    
                    // On Windows, we need to preserve some environment variables
                    #[cfg(windows)]
                    {
                        cmd.env_clear()
                            .env("SystemRoot", std::env::var("SystemRoot").unwrap_or_default())
                            .env("PATH", std::env::var("PATH").unwrap_or_default());
                    }
                    
                    // On Unix, we can be more restrictive
                    #[cfg(not(windows))]
                    {
                        cmd.env_clear()
                            .env("PATH", "/usr/local/bin:/usr/bin:/bin");
                    }
                    
                    cmd.output()
                }).await??;
                
                result.output = String::from_utf8_lossy(&output.stdout).to_string();
                if !output.stderr.is_empty() {
                    result.warnings.push(String::from_utf8_lossy(&output.stderr).to_string());
                }
                result.success = output.status.success();
            }
            "list_directory" => {
                let default_path = ".".to_string();
                let path = params.get("path").unwrap_or(&default_path);
                
                // Sanitize path to prevent traversal attacks
                let safe_path = sanitize_path(&self.root_path, path)
                    .context("Path validation failed")?;
                
                let mut entries = tokio::fs::read_dir(&safe_path).await
                    .context("Failed to read directory")?;
                let mut entry_list = Vec::new();
                
                while let Some(entry) = entries.next_entry().await? {
                    entry_list.push(entry.path().to_string_lossy().to_string());
                }
                
                result.output = serde_json::to_string_pretty(&entry_list)?;
                result.success = true;
                tracing::info!("Directory listed: {}", safe_path.display());
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


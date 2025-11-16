//! System tools implementation for Digital Twin Jamey
//! 
//! This crate provides system-level tools for process management,
//! Windows registry access, and self-modification capabilities.

pub mod system;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error(transparent)]
    System(#[from] system::SystemToolError),
    #[error("Tool execution error: {0}")]
    Execution(String),
}

/// Common traits and types used across tools
pub mod prelude {
    pub use super::system::{
        FileBackup, ProcessInfo, ProcessTool, SelfModifyTool,
    };
    #[cfg(windows)]
    pub use super::system::RegistryTool;
    pub use super::ToolError;
}

/// Re-export main tool implementations
pub use system::{
    FileBackup, ProcessInfo, ProcessTool, SelfModifyTool,
};
#[cfg(windows)]
pub use system::RegistryTool;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_process_management() {
        let mut tool = ProcessTool::new();
        
        // List processes
        let processes = tool.list_processes();
        assert!(!processes.is_empty());

        // Get info for current process
        let current_pid = std::process::id();
        let process_info = tool.get_process_info(current_pid);
        assert!(process_info.is_ok());
    }

    #[test]
    fn test_self_modification() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let tool = SelfModifyTool::new(&backup_dir).unwrap();

        // Create and modify a test file
        let test_file = temp_dir.path().join("test.rs");
        let original_content = "// Original content";
        let mut file = File::create(&test_file).unwrap();
        file.write_all(original_content.as_bytes()).unwrap();

        // Test file modification with backup
        let new_content = "// Modified content";
        let backup = tool.modify_file(&test_file, new_content).unwrap();

        // Verify modification
        let modified_content = std::fs::read_to_string(&test_file).unwrap();
        assert_eq!(modified_content, new_content);

        // Verify backup exists
        assert!(backup.backup_path.exists());

        // Test restoration
        tool.restore_backup(&backup).unwrap();
        let restored_content = std::fs::read_to_string(&test_file).unwrap();
        assert_eq!(restored_content, original_content);
    }

    #[cfg(windows)]
    #[test]
    fn test_registry_access() {
        let tool = RegistryTool::new();
        
        // Test reading a known Windows registry value
        let result = tool.read_value(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "SystemRoot",
        );
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }
}
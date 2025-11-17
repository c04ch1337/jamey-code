use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sysinfo::{PidExt, ProcessExt, System, SystemExt};
use thiserror::Error;
use tracing::error;
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug, Error)]
pub enum SystemToolError {
    #[error("Process not found: {0}")]
    ProcessNotFound(u32),
    #[error("Registry error: {0}")]
    Registry(String),
    #[error("File operation error: {0}")]
    FileOperation(String),
    #[error("Backup error: {0}")]
    Backup(String),
}

// Process Management

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub start_time: DateTime<Utc>,
}

pub struct ProcessTool {
    system: System,
}

impl ProcessTool {
    pub fn new() -> Self {
        let mut system = System::new();
        system.refresh_all();
        Self { system }
    }

    pub fn list_processes(&mut self) -> Vec<ProcessInfo> {
        self.system.refresh_all();
        self.system
            .processes()
            .values()
            .map(|process| {
                let name = process.name();
                ProcessInfo {
                    pid: process.pid().as_u32(),
                    name: name.into(),
                    cpu_usage: process.cpu_usage(),
                    memory_usage: process.memory(),
                    start_time: DateTime::from(SystemTime::now()), // Placeholder - actual start time if available
                }
            })
            .collect()
    }

    pub fn kill_process(&mut self, pid: u32) -> Result<(), SystemToolError> {
        self.system.refresh_all();
        if let Some(process) = self.system.process(sysinfo::Pid::from(pid as usize)) {
            if process.kill() {
                Ok(())
            } else {
                Err(SystemToolError::ProcessNotFound(pid))
            }
        } else {
            Err(SystemToolError::ProcessNotFound(pid))
        }
    }

    pub fn get_process_info(&mut self, pid: u32) -> Result<ProcessInfo, SystemToolError> {
        self.system.refresh_all();
        if let Some(process) = self.system.process(sysinfo::Pid::from(pid as usize)) {
            Ok(ProcessInfo {
                pid: process.pid().as_u32(),
                name: process.name().to_string(),
                cpu_usage: process.cpu_usage(),
                memory_usage: process.memory(),
                start_time: DateTime::from(SystemTime::now()), // Placeholder
            })
        } else {
            Err(SystemToolError::ProcessNotFound(pid))
        }
    }
}

// Windows Registry Access

#[cfg(windows)]
pub struct RegistryTool;

#[cfg(windows)]
impl RegistryTool {
    pub fn new() -> Self {
        Self
    }

    pub fn read_value(&self, key: &str, value_name: &str) -> Result<String, SystemToolError> {
        use windows::Win32::System::Registry::*;
        use windows::Win32::Foundation::WIN32_ERROR;

        unsafe {
            let mut key_handle = HKEY::default();
            let result = RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                &windows::core::HSTRING::from(key),
                0,
                KEY_READ,
                &mut key_handle,
            );

            if result != WIN32_ERROR(0) {
                return Err(SystemToolError::Registry(format!("Failed to open registry key: {:#x}", result.0)));
            }

            let mut buffer = [0u16; 1024];
            let mut size = (buffer.len() * 2) as u32;

            let value_name_hstring = windows::core::HSTRING::from(value_name);
            use windows::core::PCWSTR;
            let value_name_pcwstr = PCWSTR::from_raw(value_name_hstring.as_ptr());
            let query_result = RegQueryValueExW(
                key_handle,
                value_name_pcwstr,
                None,
                None,
                Some(buffer.as_mut_ptr() as *mut u8),
                Some(&mut size),
            );

            RegCloseKey(key_handle);

            if query_result != WIN32_ERROR(0) {
                return Err(SystemToolError::Registry(format!("Failed to query registry value: {:#x}", query_result.0)));
            }

            let string_len = (size as usize / 2).min(buffer.len());
            Ok(String::from_utf16_lossy(&buffer[..string_len]).trim_end_matches('\0').to_string())
        }
    }
}

// Self Modification Tool

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBackup {
    pub original_path: PathBuf,
    pub backup_path: PathBuf,
    pub timestamp: DateTime<Utc>,
}

pub struct SelfModifyTool {
    backup_dir: PathBuf,
}

impl SelfModifyTool {
    pub fn new<P: AsRef<Path>>(backup_dir: P) -> Result<Self> {
        let backup_dir = backup_dir.as_ref().to_path_buf();
        fs::create_dir_all(&backup_dir)?;
        Ok(Self { backup_dir })
    }

    pub fn create_backup<P: AsRef<Path>>(&self, file_path: P) -> Result<FileBackup, SystemToolError> {
        let file_path = file_path.as_ref();
        let timestamp = Utc::now();
        let file_name = file_path
            .file_name()
            .ok_or_else(|| SystemToolError::FileOperation("Invalid file path".to_string()))?;

        let backup_name = {
            let file_str = file_name.to_string_lossy();
            let time_str = timestamp.format("%Y%m%d_%H%M%S");
            format!("{file_str}.{time_str}.bak")
        };
        let backup_path = self.backup_dir.join(backup_name);

        fs::copy(file_path, &backup_path).map_err(|e| {
            SystemToolError::Backup(format!("Failed to create backup: {e}"))
        })?;

        Ok(FileBackup {
            original_path: file_path.to_path_buf(),
            backup_path,
            timestamp,
        })
    }

    pub fn modify_file<P: AsRef<Path>>(
        &self,
        file_path: P,
        new_content: &str,
    ) -> Result<FileBackup, SystemToolError> {
        let backup = self.create_backup(&file_path)?;

        fs::write(&file_path, new_content).map_err(|e| {
            SystemToolError::FileOperation(format!("Failed to write file: {e}"))
        })?;

        Ok(backup)
    }

    pub fn restore_backup(&self, backup: &FileBackup) -> Result<(), SystemToolError> {
        fs::copy(&backup.backup_path, &backup.original_path).map_err(|e| {
            SystemToolError::Backup(format!("Failed to restore backup: {e}"))
        })?;

        Ok(())
    }

    pub fn list_source_files(&self, pattern: &str) -> Result<Vec<PathBuf>, SystemToolError> {
        glob::glob(pattern)
            .map_err(|e| SystemToolError::FileOperation(format!("Invalid glob pattern: {e}")))?
            .filter_map(Result::ok)
            .collect::<Vec<_>>()
            .into_iter()
            .filter(|p| p.extension().map_or(false, |ext| ext == "rs"))
            .map(Ok)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_process_tool() {
        let mut tool = ProcessTool::new();
        let processes = tool.list_processes();
        assert!(!processes.is_empty());
    }

    #[test]
    fn test_self_modify_tool() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let tool = SelfModifyTool::new(&backup_dir).unwrap();

        // Create a test file
        let test_file = temp_dir.path().join("test.rs");
        let mut file = File::create(&test_file).unwrap();
        file.write_all(b"original content").unwrap();

        // Test backup creation
        let backup = tool.create_backup(&test_file).unwrap();
        assert!(backup.backup_path.exists());

        // Test file modification
        let new_content = "modified content";
        let backup = tool.modify_file(&test_file, new_content).unwrap();
        let modified_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(modified_content, new_content);

        // Test backup restoration
        tool.restore_backup(&backup).unwrap();
        let restored_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(restored_content, "original content");
    }

    #[cfg(windows)]
    #[test]
    fn test_registry_tool() {
        let tool = RegistryTool::new();
        // Test reading a known Windows registry value
        let result = tool.read_value(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "SystemRoot",
        );
        assert!(result.is_ok());
    }
}
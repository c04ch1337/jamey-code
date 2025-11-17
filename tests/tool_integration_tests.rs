mod fixtures;
mod helpers;
mod mocks;
mod utils;

use anyhow::Result;
use jamey_core::{Memory, MemoryType};
use jamey_runtime::{Runtime, RuntimeConfig};
use jamey_tools::{
    system::ProcessInfo,
    ToolRegistry,
    ToolError,
};
use std::{path::PathBuf, sync::Arc};
use tempfile::TempDir;
use utils::{assert_memories_equal, wait_for_condition};

async fn setup_test_environment() -> Result<(Runtime, TempDir)> {
    let temp_dir = TempDir::new()?;
    let mut config = RuntimeConfig::default();
    
    config.project_name = "tool_test".to_string();
    config.memory.postgres_password = "test_password".to_string();
    config.llm.openrouter_api_key = "test_key".to_string();
    config.tools.backup_dir = temp_dir.path().to_path_buf();
    config.security.api_key_required = false;

    let runtime = Runtime::new(config).await?;
    Ok((runtime, temp_dir))
}

#[tokio::test]
async fn test_process_tool_memory_integration() -> Result<()> {
    let (runtime, _temp_dir) = setup_test_environment().await?;
    let state = runtime.state();

    // Get process information
    let process_tool = state.tool_registry.get_process_tool().unwrap();
    let processes = process_tool.list_processes();
    
    // Store process information as memory
    let memory = Memory {
        id: uuid::Uuid::new_v4(),
        memory_type: MemoryType::System,
        content: serde_json::to_string(&processes)?,
        embedding: vec![0.1; 1536],
        metadata: serde_json::json!({
            "tool": "process",
            "process_count": processes.len()
        }),
        created_at: chrono::Utc::now(),
        last_accessed: chrono::Utc::now(),
    };

    let memory_id = state.memory_store.store(memory).await?;

    // Search for process-related memories
    let similar_memories = state.memory_store.search(vec![0.1; 1536], 5).await?;
    assert!(similar_memories.iter().any(|m| m.id == memory_id));

    Ok(())
}

#[tokio::test]
async fn test_self_modify_tool_backup() -> Result<()> {
    let (runtime, temp_dir) = setup_test_environment().await?;
    let state = runtime.state();

    // Create test file
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "original content")?;

    // Create backup using self-modify tool
    let self_modify_tool = state.tool_registry.get_self_modify_tool().unwrap();
    let backup = self_modify_tool.create_backup(&test_file)?;

    // Verify backup was created
    assert!(backup.backup_path.exists());
    let backup_content = std::fs::read_to_string(&backup.backup_path)?;
    assert_eq!(backup_content, "original content");

    // Store backup information as memory
    let memory = Memory {
        id: uuid::Uuid::new_v4(),
        memory_type: MemoryType::System,
        content: format!("Backup created for {}", test_file.display()),
        embedding: vec![0.1; 1536],
        metadata: serde_json::json!({
            "tool": "self_modify",
            "original_path": test_file.to_str(),
            "backup_path": backup.backup_path.to_str()
        }),
        created_at: chrono::Utc::now(),
        last_accessed: chrono::Utc::now(),
    };

    state.memory_store.store(memory).await?;

    Ok(())
}

#[tokio::test]
async fn test_tool_error_handling() -> Result<()> {
    let (runtime, temp_dir) = setup_test_environment().await?;
    let state = runtime.state();

    // Test process tool with invalid PID
    let process_tool = state.tool_registry.get_process_tool().unwrap();
    let result = process_tool.get_process_info(std::u32::MAX);
    assert!(matches!(result, Err(ToolError::ProcessNotFound(_))));

    // Test self-modify tool with non-existent file
    let self_modify_tool = state.tool_registry.get_self_modify_tool().unwrap();
    let non_existent = temp_dir.path().join("non_existent.txt");
    let result = self_modify_tool.create_backup(&non_existent);
    assert!(matches!(result, Err(ToolError::FileNotFound(_))));

    Ok(())
}

#[tokio::test]
async fn test_tool_concurrent_access() -> Result<()> {
    let (runtime, temp_dir) = setup_test_environment().await?;
    let state = Arc::new(runtime.state());

    let mut handles = Vec::new();
    
    // Spawn multiple tasks using tools concurrently
    for i in 0..5 {
        let state = state.clone();
        let temp_dir = temp_dir.path().to_path_buf();
        
        handles.push(tokio::spawn(async move {
            // Use process tool
            let process_tool = state.tool_registry.get_process_tool().unwrap();
            let processes = process_tool.list_processes();
            
            // Use self-modify tool
            let self_modify_tool = state.tool_registry.get_self_modify_tool().unwrap();
            let test_file = temp_dir.join(format!("test_{}.txt", i));
            std::fs::write(&test_file, format!("content {}", i))?;
            let backup = self_modify_tool.create_backup(&test_file)?;
            
            // Store tool results as memories
            let memory = Memory {
                id: uuid::Uuid::new_v4(),
                memory_type: MemoryType::System,
                content: format!("Tool test {}", i),
                embedding: vec![0.1; 1536],
                metadata: serde_json::json!({
                    "process_count": processes.len(),
                    "backup_path": backup.backup_path.to_str()
                }),
                created_at: chrono::Utc::now(),
                last_accessed: chrono::Utc::now(),
            };
            
            state.memory_store.store(memory).await?;
            
            Ok::<_, anyhow::Error>(())
        }));
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await??;
    }

    Ok(())
}

#[tokio::test]
async fn test_tool_session_integration() -> Result<()> {
    let (runtime, _temp_dir) = setup_test_environment().await?;
    let state = runtime.state();

    // Create session
    let session_id = state.session_manager.create_session();
    let mut session = state.session_manager.get_session(session_id).unwrap();

    // Use process tool and store result in session context
    let process_tool = state.tool_registry.get_process_tool().unwrap();
    let processes = process_tool.list_processes();
    
    let memory = Memory {
        id: uuid::Uuid::new_v4(),
        memory_type: MemoryType::System,
        content: serde_json::to_string(&processes)?,
        embedding: vec![0.1; 1536],
        metadata: serde_json::json!({
            "tool": "process",
            "session_id": session_id
        }),
        created_at: chrono::Utc::now(),
        last_accessed: chrono::Utc::now(),
    };

    let memory_id = state.memory_store.store(memory).await?;
    let stored_memory = state.memory_store.retrieve(memory_id).await?;
    session.memory_context.push(stored_memory);

    // Verify session context
    assert_eq!(session.memory_context.len(), 1);
    assert_eq!(
        session.memory_context[0].metadata["session_id"].as_str().unwrap(),
        session_id
    );

    Ok(())
}

#[tokio::test]
async fn test_tool_registry_lifecycle() -> Result<()> {
    let (mut runtime, _temp_dir) = setup_test_environment().await?;
    let state = runtime.state();

    // Start runtime
    let runtime_handle = tokio::spawn({
        let mut runtime = runtime;
        async move {
            runtime.run().await.unwrap();
        }
    });

    // Use tools while runtime is active
    let process_tool = state.tool_registry.get_process_tool().unwrap();
    let processes = process_tool.list_processes();
    assert!(!processes.is_empty());

    // Shutdown runtime
    runtime.shutdown().await;
    runtime_handle.await?;

    Ok(())
}
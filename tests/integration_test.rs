use anyhow::Result;
use jamey_core::memory::{Memory, MemoryType};
use jamey_providers::openrouter::{ChatRequest, Message};
use jamey_runtime::prelude::*;
use jamey_tools::system::ProcessInfo;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio;
use uuid::Uuid;

async fn setup_test_runtime() -> Result<(Runtime, TempDir)> {
    let temp_dir = TempDir::new()?;
    let mut config = RuntimeConfig::default();
    
    // Configure for testing
    config.project_name = "jamey_test".to_string();
    config.memory.postgres_password = "test_password".to_string();
    config.llm.openrouter_api_key = "test_key".to_string();
    config.tools.backup_dir = temp_dir.path().to_path_buf();
    config.memory.postgres_db = "jamey_test".to_string();
    config.security.api_key_required = false;

    let runtime = Runtime::new(config).await?;
    Ok((runtime, temp_dir))
}

#[tokio::test]
async fn test_memory_llm_integration() -> Result<()> {
    let (runtime, _temp_dir) = setup_test_runtime().await?;
    let state = runtime.state();

    // Create a memory entry
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Conversation,
        content: "Test conversation memory".to_string(),
        embedding: vec![0.1; 1536],
        metadata: serde_json::json!({"source": "integration_test"}),
        created_at: chrono::Utc::now(),
        last_accessed: chrono::Utc::now(),
    };

    let memory_id = state.memory_store.store(memory.clone()).await?;

    // Use LLM to process the memory
    let chat_request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: format!("Process this memory: {}", memory.content),
        }],
        tools: None,
        tool_choice: None,
        temperature: Some(0.0),
        max_tokens: None,
    };

    let response = state.llm_provider.chat(chat_request).await?;
    assert!(!response.choices.is_empty());

    // Verify memory retrieval
    let retrieved = state.memory_store.retrieve(memory_id).await?;
    assert_eq!(retrieved.content, memory.content);

    Ok(())
}

#[tokio::test]
async fn test_tool_integration() -> Result<()> {
    let (runtime, temp_dir) = setup_test_runtime().await?;
    let state = runtime.state();

    // Test process tool
    if let Some(process_tool) = state.tool_registry.get_process_tool() {
        let processes = process_tool.list_processes();
        assert!(!processes.is_empty());
    }

    // Test self-modify tool
    if let Some(self_modify_tool) = state.tool_registry.get_self_modify_tool() {
        let test_file = temp_dir.path().join("test.rs");
        std::fs::write(&test_file, "// Test content")?;

        let backup = self_modify_tool.create_backup(&test_file)?;
        assert!(backup.backup_path.exists());
    }

    // Test Windows registry tool if available
    #[cfg(windows)]
    if let Some(registry_tool) = state.tool_registry.get_registry_tool() {
        let result = registry_tool.read_value(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "SystemRoot",
        );
        assert!(result.is_ok());
    }

    Ok(())
}

#[tokio::test]
async fn test_session_management() -> Result<()> {
    let (runtime, _temp_dir) = setup_test_runtime().await?;
    let state = runtime.state();

    // Create session
    let session_id = state.session_manager.create_session();
    let session = state.session_manager.get_session(session_id);
    assert!(session.is_some());

    // Add memory to session context
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Conversation,
        content: "Session test memory".to_string(),
        embedding: vec![0.1; 1536],
        metadata: serde_json::json!({}),
        created_at: chrono::Utc::now(),
        last_accessed: chrono::Utc::now(),
    };

    let memory_id = state.memory_store.store(memory).await?;
    let mut session = session.unwrap();
    session.memory_context.push(
        state.memory_store.retrieve(memory_id).await?
    );

    // Test session cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    state.session_manager.cleanup_expired_sessions(
        std::time::Duration::from_millis(50)
    );
    assert!(state.session_manager.get_session(session_id).is_none());

    Ok(())
}

#[tokio::test]
async fn test_runtime_lifecycle() -> Result<()> {
    let (mut runtime, _temp_dir) = setup_test_runtime().await?;

    // Start runtime in background
    let runtime_handle = tokio::spawn({
        let mut runtime = runtime;
        async move {
            runtime.run().await.unwrap();
        }
    });

    // Let runtime initialize
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Test basic operations
    let state = runtime.state();
    let session_id = state.session_manager.create_session();
    assert!(state.session_manager.get_session(session_id).is_some());

    // Shutdown runtime
    runtime.shutdown().await;
    runtime_handle.await?;

    Ok(())
}
mod fixtures;
mod helpers;
mod mocks;
mod utils;

use anyhow::Result;
use jamey_core::{Memory, MemoryType};
use jamey_providers::openrouter::{ChatRequest, Message, Role};
use jamey_runtime::{Runtime, RuntimeConfig, State};
use jamey_tools::system::ProcessInfo;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tempfile::TempDir;
use tokio::time::sleep;
use utils::{assert_memories_equal, wait_for_condition};

async fn setup_test_environment() -> Result<(Runtime, TempDir)> {
    let temp_dir = TempDir::new()?;
    let mut config = RuntimeConfig::default();
    
    config.project_name = "workflow_test".to_string();
    config.memory.postgres_password = "test_password".to_string();
    config.llm.openrouter_api_key = "test_key".to_string();
    config.tools.backup_dir = temp_dir.path().to_path_buf();
    config.security.api_key_required = false;

    let runtime = Runtime::new(config).await?;
    Ok((runtime, temp_dir))
}

#[tokio::test]
async fn test_conversation_workflow() -> Result<()> {
    let (runtime, _temp_dir) = setup_test_environment().await?;
    let state = runtime.state();

    // 1. Create a new session
    let session_id = state.session_manager.create_session();
    let mut session = state.session_manager.get_session(session_id).unwrap();

    // 2. Start conversation with LLM
    let initial_request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![
            Message {
                role: Role::System,
                content: "You are a helpful assistant.".to_string(),
            },
            Message {
                role: Role::User,
                content: "What can you tell me about Rust programming?".to_string(),
            },
        ],
        tools: None,
        tool_choice: None,
        temperature: Some(0.0),
        max_tokens: None,
    };

    let response = state.llm_provider.chat(initial_request).await?;
    
    // 3. Store conversation in memory
    let memory = Memory {
        id: uuid::Uuid::new_v4(),
        memory_type: MemoryType::Conversation,
        content: response.choices[0].message.content.clone(),
        embedding: vec![0.1; 1536],
        metadata: serde_json::json!({
            "session_id": session_id,
            "role": "assistant"
        }),
        created_at: chrono::Utc::now(),
        last_accessed: chrono::Utc::now(),
    };

    let memory_id = state.memory_store.store(memory).await?;
    let stored_memory = state.memory_store.retrieve(memory_id).await?;
    session.memory_context.push(stored_memory);

    // 4. Follow-up interaction
    let follow_up_request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![
            Message {
                role: Role::Assistant,
                content: response.choices[0].message.content.clone(),
            },
            Message {
                role: Role::User,
                content: "Can you provide an example?".to_string(),
            },
        ],
        tools: None,
        tool_choice: None,
        temperature: Some(0.0),
        max_tokens: None,
    };

    let follow_up_response = state.llm_provider.chat(follow_up_request).await?;
    
    // 5. Store follow-up in memory
    let follow_up_memory = Memory {
        id: uuid::Uuid::new_v4(),
        memory_type: MemoryType::Conversation,
        content: follow_up_response.choices[0].message.content.clone(),
        embedding: vec![0.1; 1536],
        metadata: serde_json::json!({
            "session_id": session_id,
            "role": "assistant",
            "previous_memory_id": memory_id
        }),
        created_at: chrono::Utc::now(),
        last_accessed: chrono::Utc::now(),
    };

    state.memory_store.store(follow_up_memory).await?;

    Ok(())
}

#[tokio::test]
async fn test_system_monitoring_workflow() -> Result<()> {
    let (runtime, _temp_dir) = setup_test_environment().await?;
    let state = runtime.state();

    // 1. Get system information using process tool
    let process_tool = state.tool_registry.get_process_tool().unwrap();
    let processes = process_tool.list_processes();

    // 2. Store system information in memory
    let system_memory = Memory {
        id: uuid::Uuid::new_v4(),
        memory_type: MemoryType::System,
        content: serde_json::to_string(&processes)?,
        embedding: vec![0.1; 1536],
        metadata: serde_json::json!({
            "source": "process_tool",
            "process_count": processes.len()
        }),
        created_at: chrono::Utc::now(),
        last_accessed: chrono::Utc::now(),
    };

    let memory_id = state.memory_store.store(system_memory).await?;

    // 3. Analyze system information with LLM
    let analysis_request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: format!(
                "Analyze these system processes and identify any potential issues: {}",
                serde_json::to_string(&processes)?
            ),
        }],
        tools: None,
        tool_choice: None,
        temperature: Some(0.0),
        max_tokens: None,
    };

    let analysis_response = state.llm_provider.chat(analysis_request).await?;

    // 4. Store analysis in memory
    let analysis_memory = Memory {
        id: uuid::Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: analysis_response.choices[0].message.content.clone(),
        embedding: vec![0.1; 1536],
        metadata: serde_json::json!({
            "type": "system_analysis",
            "source_memory_id": memory_id
        }),
        created_at: chrono::Utc::now(),
        last_accessed: chrono::Utc::now(),
    };

    state.memory_store.store(analysis_memory).await?;

    Ok(())
}

#[tokio::test]
async fn test_self_modification_workflow() -> Result<()> {
    let (runtime, temp_dir) = setup_test_environment().await?;
    let state = runtime.state();

    // 1. Create test file
    let test_file = temp_dir.path().join("test_code.rs");
    std::fs::write(&test_file, "fn main() {\n    println!(\"Hello, world!\");\n}")?;

    // 2. Create backup using self-modify tool
    let self_modify_tool = state.tool_registry.get_self_modify_tool().unwrap();
    let backup = self_modify_tool.create_backup(&test_file)?;

    // 3. Store backup information in memory
    let backup_memory = Memory {
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

    let memory_id = state.memory_store.store(backup_memory).await?;

    // 4. Modify file
    std::fs::write(&test_file, "fn main() {\n    println!(\"Modified!\");\n}")?;

    // 5. Store modification in memory
    let modification_memory = Memory {
        id: uuid::Uuid::new_v4(),
        memory_type: MemoryType::System,
        content: "File modified".to_string(),
        embedding: vec![0.1; 1536],
        metadata: serde_json::json!({
            "tool": "self_modify",
            "action": "modify",
            "file": test_file.to_str(),
            "backup_memory_id": memory_id
        }),
        created_at: chrono::Utc::now(),
        last_accessed: chrono::Utc::now(),
    };

    state.memory_store.store(modification_memory).await?;

    Ok(())
}

#[tokio::test]
async fn test_complete_runtime_workflow() -> Result<()> {
    let (mut runtime, _temp_dir) = setup_test_environment().await?;
    let state = runtime.state();

    // 1. Start runtime in background
    let runtime_handle = tokio::spawn({
        let mut runtime = runtime;
        async move {
            runtime.run().await.unwrap();
        }
    });

    // 2. Create session and initial context
    let session_id = state.session_manager.create_session();
    let mut session = state.session_manager.get_session(session_id).unwrap();

    // 3. Get system information
    let process_tool = state.tool_registry.get_process_tool().unwrap();
    let processes = process_tool.list_processes();

    // 4. Store system info and add to session context
    let system_memory = Memory {
        id: uuid::Uuid::new_v4(),
        memory_type: MemoryType::System,
        content: serde_json::to_string(&processes)?,
        embedding: vec![0.1; 1536],
        metadata: serde_json::json!({
            "session_id": session_id,
            "source": "process_tool"
        }),
        created_at: chrono::Utc::now(),
        last_accessed: chrono::Utc::now(),
    };

    let memory_id = state.memory_store.store(system_memory.clone()).await?;
    session.memory_context.push(system_memory);

    // 5. Process with LLM
    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: format!(
                "Analyze these processes and suggest optimizations: {}",
                serde_json::to_string(&processes)?
            ),
        }],
        tools: None,
        tool_choice: None,
        temperature: Some(0.0),
        max_tokens: None,
    };

    let response = state.llm_provider.chat(request).await?;

    // 6. Store analysis
    let analysis_memory = Memory {
        id: uuid::Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: response.choices[0].message.content,
        embedding: vec![0.1; 1536],
        metadata: serde_json::json!({
            "session_id": session_id,
            "type": "analysis",
            "source_memory_id": memory_id
        }),
        created_at: chrono::Utc::now(),
        last_accessed: chrono::Utc::now(),
    };

    state.memory_store.store(analysis_memory).await?;

    // 7. Clean up
    state.session_manager.remove_session(session_id);
    runtime.shutdown().await;
    runtime_handle.await?;

    Ok(())
}
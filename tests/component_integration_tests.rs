mod fixtures;
mod helpers;
mod mocks;
mod utils;

use anyhow::Result;
use jamey_core::{Memory, MemoryType};
use jamey_providers::openrouter::{ChatRequest, Message, OpenRouterProvider};
use jamey_runtime::{Runtime, RuntimeConfig, State};
use jamey_tools::system::ProcessInfo;
use std::sync::Arc;
use tokio::sync::RwLock;
use utils::{assert_memories_equal, wait_for_condition};

async fn setup_test_components() -> Result<(Runtime, Arc<State>)> {
    let mut config = RuntimeConfig::default();
    config.project_name = "integration_test".to_string();
    config.memory.postgres_password = "test_password".to_string();
    config.llm.openrouter_api_key = "test_key".to_string();
    config.security.api_key_required = false;

    let runtime = Runtime::new(config).await?;
    let state = runtime.state();
    Ok((runtime, state))
}

#[tokio::test]
async fn test_memory_llm_interaction() -> Result<()> {
    let (_runtime, state) = setup_test_components().await?;
    let memories = fixtures::TestMemories::default();

    // Store a conversation memory
    let memory_id = state.memory_store.store(memories.conversation.clone()).await?;

    // Use LLM to process the memory
    let chat_request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: format!("Process this memory: {}", memories.conversation.content),
        }],
        tools: None,
        tool_choice: None,
        temperature: Some(0.0),
        max_tokens: None,
    };

    let response = state.llm_provider.chat(chat_request).await?;
    assert!(!response.choices.is_empty());

    // Store LLM response as a new memory
    let llm_memory = Memory {
        id: uuid::Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: response.choices[0].message.content.clone(),
        embedding: vec![0.1; 1536], // Simplified for testing
        metadata: serde_json::json!({
            "source": "llm_processing",
            "original_memory_id": memory_id
        }),
        created_at: chrono::Utc::now(),
        last_accessed: chrono::Utc::now(),
    };

    let llm_memory_id = state.memory_store.store(llm_memory.clone()).await?;

    // Verify both memories can be retrieved and are linked
    let original = state.memory_store.retrieve(memory_id).await?;
    let processed = state.memory_store.retrieve(llm_memory_id).await?;

    assert_eq!(original.content, memories.conversation.content);
    assert_eq!(
        processed.metadata["original_memory_id"].as_str().unwrap(),
        memory_id.to_string()
    );

    Ok(())
}

#[tokio::test]
async fn test_memory_session_interaction() -> Result<()> {
    let (_runtime, state) = setup_test_components().await?;
    let memories = fixtures::TestMemories::default();

    // Create session and store memories
    let session_id = state.session_manager.create_session();
    let mut session = state.session_manager.get_session(session_id).unwrap();

    let memory_ids = vec![
        state.memory_store.store(memories.knowledge.clone()).await?,
        state.memory_store.store(memories.conversation.clone()).await?,
    ];

    // Add memories to session context
    for id in &memory_ids {
        let memory = state.memory_store.retrieve(*id).await?;
        session.memory_context.push(memory);
    }

    // Verify session context
    assert_eq!(session.memory_context.len(), 2);
    assert_memories_equal(&session.memory_context[0], &memories.knowledge);
    assert_memories_equal(&session.memory_context[1], &memories.conversation);

    Ok(())
}

#[tokio::test]
async fn test_tool_memory_interaction() -> Result<()> {
    let (_runtime, state) = setup_test_components().await?;
    
    // Use process tool to get system info
    if let Some(process_tool) = state.tool_registry.get_process_tool() {
        let processes = process_tool.list_processes();
        
        // Store process information as system memory
        let memory = Memory {
            id: uuid::Uuid::new_v4(),
            memory_type: MemoryType::System,
            content: serde_json::to_string(&processes)?,
            embedding: vec![0.1; 1536],
            metadata: serde_json::json!({
                "source": "process_tool",
                "timestamp": chrono::Utc::now()
            }),
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
        };

        let memory_id = state.memory_store.store(memory.clone()).await?;
        
        // Verify memory was stored
        let retrieved = state.memory_store.retrieve(memory_id).await?;
        assert_eq!(retrieved.memory_type, MemoryType::System);
        
        // Verify process data can be deserialized
        let _: Vec<ProcessInfo> = serde_json::from_str(&retrieved.content)?;
    }

    Ok(())
}

#[tokio::test]
async fn test_runtime_component_lifecycle() -> Result<()> {
    let (mut runtime, state) = setup_test_components().await?;

    // Start runtime in background
    let runtime_handle = tokio::spawn({
        let mut runtime = runtime;
        async move {
            runtime.run().await.unwrap();
        }
    });

    // Let runtime initialize
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Test component interactions during runtime
    let session_id = state.session_manager.create_session();
    let memories = fixtures::TestMemories::default();

    // Store memory
    let memory_id = state.memory_store.store(memories.knowledge.clone()).await?;

    // Process with LLM
    let chat_request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Test message".to_string(),
        }],
        tools: None,
        tool_choice: None,
        temperature: Some(0.0),
        max_tokens: None,
    };

    let response = state.llm_provider.chat(chat_request).await?;
    assert!(!response.choices.is_empty());

    // Use tool
    if let Some(process_tool) = state.tool_registry.get_process_tool() {
        let _processes = process_tool.list_processes();
    }

    // Clean up
    state.memory_store.delete(memory_id).await?;
    state.session_manager.remove_session(session_id);

    // Shutdown runtime
    runtime.shutdown().await;
    runtime_handle.await?;

    Ok(())
}

#[tokio::test]
async fn test_concurrent_component_access() -> Result<()> {
    let (_runtime, state) = setup_test_components().await?;
    let memories = fixtures::TestMemories::default();
    let state = Arc::new(state);

    // Spawn multiple tasks that interact with different components
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let state = state.clone();
        let memory = memories.knowledge.clone();
        
        handles.push(tokio::spawn(async move {
            // Create session
            let session_id = state.session_manager.create_session();
            
            // Store memory
            let mut mem = memory;
            mem.content = format!("Test memory {}", i);
            let memory_id = state.memory_store.store(mem).await?;
            
            // Process with LLM
            let chat_request = ChatRequest {
                model: "claude-3-sonnet".to_string(),
                messages: vec![Message {
                    role: "user".to_string(),
                    content: format!("Test message {}", i),
                }],
                tools: None,
                tool_choice: None,
                temperature: Some(0.0),
                max_tokens: None,
            };
            
            let response = state.llm_provider.chat(chat_request).await?;
            assert!(!response.choices.is_empty());
            
            // Clean up
            state.memory_store.delete(memory_id).await?;
            state.session_manager.remove_session(session_id);
            
            Ok::<_, anyhow::Error>(())
        }));
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await??;
    }

    Ok(())
}
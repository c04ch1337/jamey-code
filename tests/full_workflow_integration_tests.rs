//! Full workflow integration tests
//! Tests complete end-to-end scenarios across multiple components

use chrono::Utc;
use jamey_core::{Memory, MemoryStore, MemoryType, PostgresMemoryStore};
use jamey_providers::{ChatRequest, LlmProvider, Message, OpenRouterConfig, OpenRouterProvider};
use serde_json::json;
use uuid::Uuid;
use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

// ============================================================================
// Test Helpers
// ============================================================================

async fn create_test_memory_store() -> PostgresMemoryStore {
    let pool = create_test_pool().await;
    PostgresMemoryStore::new(pool, 1536).await.unwrap()
}

async fn create_test_pool() -> deadpool_postgres::Pool {
    use deadpool_postgres::{Config, Runtime};
    use tokio_postgres::NoTls;

    let mut cfg = Config::new();
    cfg.host = Some("localhost".to_string());
    cfg.dbname = Some("jamey_test".to_string());
    cfg.user = Some("postgres".to_string());
    cfg.password = Some("test_password".to_string());
    
    cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap()
}

async fn create_mock_provider(mock_server: &MockServer) -> OpenRouterProvider {
    let config = OpenRouterConfig {
        api_key: "test_key".to_string(),
        api_base_url: url::Url::parse(&mock_server.uri()).unwrap(),
        ..Default::default()
    };
    
    OpenRouterProvider::new(config).unwrap()
}

// ============================================================================
// End-to-End Chat Workflow Tests
// ============================================================================

#[tokio::test]
async fn test_complete_chat_workflow() {
    // Setup mock LLM provider
    let mock_server = MockServer::start().await;
    
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chat_response_1",
            "model": "claude-3-sonnet",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello! I'm Jamey, your digital assistant. How can I help you today?"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 15,
                "completion_tokens": 20,
                "total_tokens": 35
            }
        })))
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{
                "object": "embedding",
                "embedding": vec![0.1; 1536],
                "index": 0
            }],
            "model": "text-embedding-ada-002",
            "usage": {"prompt_tokens": 5, "total_tokens": 5}
        })))
        .mount(&mock_server)
        .await;

    let provider = create_mock_provider(&mock_server).await;
    let store = create_test_memory_store().await;

    // Step 1: User sends a message
    let user_message = "Hello, who are you?";
    
    // Step 2: Generate embedding for user message
    let user_embedding = provider.get_embedding(user_message).await.unwrap();
    assert_eq!(user_embedding.len(), 1536);

    // Step 3: Store user message in memory
    let user_memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Conversation,
        content: user_message.to_string(),
        embedding: user_embedding.clone(),
        metadata: json!({"role": "user", "timestamp": Utc::now().to_rfc3339()}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };
    
    let user_memory_id = store.store(user_memory).await.unwrap();

    // Step 4: Send to LLM provider
    let chat_request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: user_message.to_string(),
        }],
        tools: None,
        tool_choice: None,
        temperature: Some(0.7),
        max_tokens: Some(500),
    };

    let response = provider.chat(chat_request).await.unwrap();
    assert!(!response.choices.is_empty());
    
    let assistant_message = &response.choices[0].message.content;

    // Step 5: Generate embedding for assistant response
    let assistant_embedding = provider.get_embedding(assistant_message).await.unwrap();

    // Step 6: Store assistant response in memory
    let assistant_memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Conversation,
        content: assistant_message.clone(),
        embedding: assistant_embedding,
        metadata: json!({
            "role": "assistant",
            "timestamp": Utc::now().to_rfc3339(),
            "model": response.model,
            "tokens": response.usage.total_tokens
        }),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };
    
    let assistant_memory_id = store.store(assistant_memory).await.unwrap();

    // Step 7: Verify both memories can be retrieved
    let retrieved_user = store.retrieve(user_memory_id).await.unwrap();
    let retrieved_assistant = store.retrieve(assistant_memory_id).await.unwrap();

    assert_eq!(retrieved_user.content, user_message);
    assert_eq!(retrieved_assistant.content, *assistant_message);

    // Step 8: Test similarity search
    let similar_memories = store.search(&user_embedding, 5).await.unwrap();
    assert!(!similar_memories.is_empty());

    // Cleanup
    store.delete(user_memory_id).await.unwrap();
    store.delete(assistant_memory_id).await.unwrap();
}

// ============================================================================
// Memory Management Workflow Tests
// ============================================================================

#[tokio::test]
async fn test_memory_lifecycle_workflow() {
    let store = create_test_memory_store().await;

    // Step 1: Create and store multiple memories
    let mut memory_ids = Vec::new();
    
    for i in 0..5 {
        let memory = Memory {
            id: Uuid::new_v4(),
            memory_type: MemoryType::Knowledge,
            content: format!("Knowledge item {}", i),
            embedding: vec![0.1 * (i as f32); 1536],
            metadata: json!({"index": i, "category": "test"}),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };
        
        let id = store.store(memory).await.unwrap();
        memory_ids.push(id);
    }

    // Step 2: Retrieve and verify all memories
    for id in &memory_ids {
        let memory = store.retrieve(*id).await.unwrap();
        assert!(memory.content.starts_with("Knowledge item"));
    }

    // Step 3: Update a memory
    let update_id = memory_ids[2];
    store.update(
        update_id,
        "Updated knowledge item",
        &vec![0.5; 1536]
    ).await.unwrap();

    let updated = store.retrieve(update_id).await.unwrap();
    assert_eq!(updated.content, "Updated knowledge item");

    // Step 4: Search for similar memories
    let query_embedding = vec![0.2; 1536];
    let results = store.search(&query_embedding, 3).await.unwrap();
    assert!(!results.is_empty());
    assert!(results.len() <= 3);

    // Step 5: Paginate through memories
    let (page1, total) = store.list_paginated(2, 0).await.unwrap();
    assert_eq!(page1.len(), 2);
    assert!(total >= 5);

    let (page2, _) = store.list_paginated(2, 2).await.unwrap();
    assert!(!page2.is_empty());

    // Step 6: Delete all test memories
    for id in memory_ids {
        store.delete(id).await.unwrap();
    }

    // Step 7: Verify deletion
    for id in &memory_ids {
        let result = store.retrieve(*id).await;
        assert!(result.is_err());
    }
}

// ============================================================================
// Multi-Turn Conversation Workflow Tests
// ============================================================================

#[tokio::test]
async fn test_multi_turn_conversation_workflow() {
    let mock_server = MockServer::start().await;
    
    // Mock multiple conversation turns
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "response",
            "model": "claude-3-sonnet",
            "choices": [{
                "message": {"role": "assistant", "content": "Response"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        })))
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{
                "object": "embedding",
                "embedding": vec![0.1; 1536],
                "index": 0
            }],
            "model": "text-embedding-ada-002",
            "usage": {"prompt_tokens": 5, "total_tokens": 5}
        })))
        .mount(&mock_server)
        .await;

    let provider = create_mock_provider(&mock_server).await;
    let store = create_test_memory_store().await;

    let conversation_turns = vec![
        ("Hello!", "user"),
        ("Hi there!", "assistant"),
        ("How are you?", "user"),
        ("I'm doing well, thanks!", "assistant"),
        ("What can you do?", "user"),
        ("I can help with many tasks!", "assistant"),
    ];

    let mut memory_ids = Vec::new();

    // Process each conversation turn
    for (content, role) in conversation_turns {
        // Generate embedding
        let embedding = provider.get_embedding(content).await.unwrap();

        // Store in memory
        let memory = Memory {
            id: Uuid::new_v4(),
            memory_type: MemoryType::Conversation,
            content: content.to_string(),
            embedding,
            metadata: json!({"role": role}),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };

        let id = store.store(memory).await.unwrap();
        memory_ids.push(id);
    }

    // Verify conversation history
    assert_eq!(memory_ids.len(), 6);

    // Retrieve conversation in order
    for (i, id) in memory_ids.iter().enumerate() {
        let memory = store.retrieve(*id).await.unwrap();
        assert_eq!(memory.content, conversation_turns[i].0);
    }

    // Search for relevant context
    let query = "greeting";
    let query_embedding = provider.get_embedding(query).await.unwrap();
    let relevant = store.search(&query_embedding, 3).await.unwrap();
    assert!(!relevant.is_empty());

    // Cleanup
    for id in memory_ids {
        store.delete(id).await.unwrap();
    }
}

// ============================================================================
// Error Recovery Workflow Tests
// ============================================================================

#[tokio::test]
async fn test_error_recovery_workflow() {
    let mock_server = MockServer::start().await;
    
    // First request fails with rate limit
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(429))
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;

    // Subsequent requests succeed
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "success",
            "model": "claude-3-sonnet",
            "choices": [{
                "message": {"role": "assistant", "content": "Success after retry"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        })))
        .mount(&mock_server)
        .await;

    let provider = create_mock_provider(&mock_server).await;

    let request = ChatRequest {
        model: "claude-3-sonnet".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Test retry".to_string(),
        }],
        tools: None,
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    };

    // Should succeed after retry
    let result = provider.chat(request).await;
    assert!(result.is_ok());
}

// ============================================================================
// Concurrent Operations Workflow Tests
// ============================================================================

#[tokio::test]
async fn test_concurrent_operations_workflow() {
    let store = std::sync::Arc::new(create_test_memory_store().await);

    // Spawn multiple concurrent tasks
    let mut handles = vec![];

    for i in 0..10 {
        let store_clone = store.clone();
        let handle = tokio::spawn(async move {
            // Each task stores, retrieves, updates, and deletes a memory
            let memory = Memory {
                id: Uuid::new_v4(),
                memory_type: MemoryType::Knowledge,
                content: format!("Concurrent test {}", i),
                embedding: vec![0.1; 1536],
                metadata: json!({"thread": i}),
                created_at: Utc::now(),
                last_accessed: Utc::now(),
            };

            let id = store_clone.store(memory).await.unwrap();
            let retrieved = store_clone.retrieve(id).await.unwrap();
            
            store_clone.update(
                id,
                &format!("Updated {}", i),
                &vec![0.2; 1536]
            ).await.unwrap();

            store_clone.delete(id).await.unwrap();
            
            retrieved.content
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let results = futures::future::join_all(handles).await;

    // All tasks should succeed
    for (i, result) in results.iter().enumerate() {
        assert!(result.is_ok());
        let content = result.as_ref().unwrap();
        assert_eq!(content, &format!("Concurrent test {}", i));
    }
}

// ============================================================================
// Cache Integration Workflow Tests
// ============================================================================

#[tokio::test]
async fn test_cache_integration_workflow() {
    let store = create_test_memory_store().await;

    // Store a memory
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Cached content".to_string(),
        embedding: vec![0.1; 1536],
        metadata: json!({"cached": true}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let id = store.store(memory).await.unwrap();

    // First retrieval (cache miss)
    let start1 = std::time::Instant::now();
    let retrieved1 = store.retrieve(id).await.unwrap();
    let duration1 = start1.elapsed();

    // Second retrieval (potential cache hit)
    let start2 = std::time::Instant::now();
    let retrieved2 = store.retrieve(id).await.unwrap();
    let duration2 = start2.elapsed();

    // Verify content is the same
    assert_eq!(retrieved1.content, retrieved2.content);

    // Update should invalidate cache
    store.update(id, "Updated cached content", &vec![0.2; 1536]).await.unwrap();

    // Retrieval after update
    let retrieved3 = store.retrieve(id).await.unwrap();
    assert_eq!(retrieved3.content, "Updated cached content");

    // Cleanup
    store.delete(id).await.unwrap();
}
//! Comprehensive unit tests for memory module
//! Tests error scenarios, edge cases, boundary conditions, and validation

use chrono::Utc;
use jamey_core::{Memory, MemoryError, MemoryStore, MemoryType, PostgresMemoryStore};
use serde_json::json;
use uuid::Uuid;

mod helpers;
mod fixtures;

// ============================================================================
// Memory Type Tests
// ============================================================================

#[test]
fn test_memory_type_try_from_valid() {
    let test_cases = vec![
        ("conversation", MemoryType::Conversation),
        ("Conversation", MemoryType::Conversation),
        ("CONVERSATION", MemoryType::Conversation),
        ("knowledge", MemoryType::Knowledge),
        ("experience", MemoryType::Experience),
        ("skill", MemoryType::Skill),
        ("preference", MemoryType::Preference),
    ];

    for (input, expected) in test_cases {
        let result = MemoryType::try_from(input).unwrap();
        assert_eq!(result, expected, "Failed for input: {}", input);
    }
}

#[test]
fn test_memory_type_try_from_invalid() {
    let invalid_inputs = vec![
        "",
        "invalid",
        "memory",
        "unknown",
        "123",
        "conversation ",
        " conversation",
    ];

    for input in invalid_inputs {
        let result = MemoryType::try_from(input);
        assert!(result.is_err(), "Should fail for input: {}", input);
    }
}

#[test]
fn test_memory_type_display() {
    assert_eq!(MemoryType::Conversation.to_string(), "Conversation");
    assert_eq!(MemoryType::Knowledge.to_string(), "Knowledge");
    assert_eq!(MemoryType::Experience.to_string(), "Experience");
    assert_eq!(MemoryType::Skill.to_string(), "Skill");
    assert_eq!(MemoryType::Preference.to_string(), "Preference");
}

// ============================================================================
// Memory Validation Tests
// ============================================================================

#[test]
fn test_validate_embedding_empty() {
    use validator::Validate;
    
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Test".to_string(),
        embedding: vec![], // Empty embedding
        metadata: json!({}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let result = memory.validate();
    assert!(result.is_err());
}

#[test]
fn test_validate_embedding_too_large() {
    use validator::Validate;
    
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Test".to_string(),
        embedding: vec![0.1; 5000], // Too large (>4096)
        metadata: json!({}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let result = memory.validate();
    assert!(result.is_err());
}

#[test]
fn test_validate_embedding_nan_values() {
    use validator::Validate;
    
    let mut embedding = vec![0.1; 1536];
    embedding[100] = f32::NAN;
    
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Test".to_string(),
        embedding,
        metadata: json!({}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let result = memory.validate();
    assert!(result.is_err());
}

#[test]
fn test_validate_embedding_infinite_values() {
    use validator::Validate;
    
    let mut embedding = vec![0.1; 1536];
    embedding[100] = f32::INFINITY;
    
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Test".to_string(),
        embedding,
        metadata: json!({}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let result = memory.validate();
    assert!(result.is_err());
}

#[test]
fn test_validate_content_empty() {
    use validator::Validate;
    
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "".to_string(), // Empty content
        embedding: vec![0.1; 1536],
        metadata: json!({}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let result = memory.validate();
    assert!(result.is_err());
}

#[test]
fn test_validate_content_too_large() {
    use validator::Validate;
    
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "x".repeat(40000), // Exceeds 32768 limit
        embedding: vec![0.1; 1536],
        metadata: json!({}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let result = memory.validate();
    assert!(result.is_err());
}

#[test]
fn test_validate_metadata_too_large() {
    use validator::Validate;
    
    // Create metadata that exceeds 16384 bytes when serialized
    let large_value = "x".repeat(20000);
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Test".to_string(),
        embedding: vec![0.1; 1536],
        metadata: json!({"large": large_value}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let result = memory.validate();
    assert!(result.is_err());
}

#[test]
fn test_validate_metadata_too_many_fields() {
    use validator::Validate;
    
    // Create metadata with more than 50 fields
    let mut obj = serde_json::Map::new();
    for i in 0..60 {
        obj.insert(format!("field_{}", i), json!("value"));
    }
    
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Test".to_string(),
        embedding: vec![0.1; 1536],
        metadata: json!(obj),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let result = memory.validate();
    assert!(result.is_err());
}

#[test]
fn test_validate_metadata_key_too_long() {
    use validator::Validate;
    
    let long_key = "x".repeat(100); // Exceeds 64 character limit
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Test".to_string(),
        embedding: vec![0.1; 1536],
        metadata: json!({long_key: "value"}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let result = memory.validate();
    assert!(result.is_err());
}

#[test]
fn test_validate_metadata_value_too_long() {
    use validator::Validate;
    
    let long_value = "x".repeat(2000); // Exceeds 1024 character limit
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Test".to_string(),
        embedding: vec![0.1; 1536],
        metadata: json!({"key": long_value}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let result = memory.validate();
    assert!(result.is_err());
}

// ============================================================================
// PostgresMemoryStore Error Tests
// ============================================================================

#[tokio::test]
async fn test_store_with_invalid_vector_dimension() {
    let pool = helpers::create_test_pool().await;
    let store = PostgresMemoryStore::new(pool, 1536).await.unwrap();

    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Test".to_string(),
        embedding: vec![0.1; 512], // Wrong dimension
        metadata: json!({}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let result = store.store(memory).await;
    assert!(result.is_err());
    
    if let Err(e) = result {
        let error_msg = e.to_string();
        assert!(error_msg.contains("dimension"));
    }
}

#[tokio::test]
async fn test_store_with_empty_content() {
    let pool = helpers::create_test_pool().await;
    let store = PostgresMemoryStore::new(pool, 1536).await.unwrap();

    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "".to_string(),
        embedding: vec![0.1; 1536],
        metadata: json!({}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let result = store.store(memory).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_store_with_control_characters() {
    let pool = helpers::create_test_pool().await;
    let store = PostgresMemoryStore::new(pool, 1536).await.unwrap();

    // Content with control characters (should be sanitized)
    let content = "Test\x00\x01\x02content\x03";
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: content.to_string(),
        embedding: vec![0.1; 1536],
        metadata: json!({}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let id = store.store(memory).await.unwrap();
    let retrieved = store.retrieve(id).await.unwrap();
    
    // Control characters should be filtered out
    assert!(!retrieved.content.contains('\x00'));
    assert!(!retrieved.content.contains('\x01'));
}

#[tokio::test]
async fn test_retrieve_nonexistent_memory() {
    let pool = helpers::create_test_pool().await;
    let store = PostgresMemoryStore::new(pool, 1536).await.unwrap();

    let nonexistent_id = Uuid::new_v4();
    let result = store.retrieve(nonexistent_id).await;
    
    assert!(result.is_err());
}

#[tokio::test]
async fn test_update_nonexistent_memory() {
    let pool = helpers::create_test_pool().await;
    let store = PostgresMemoryStore::new(pool, 1536).await.unwrap();

    let nonexistent_id = Uuid::new_v4();
    let result = store.update(
        nonexistent_id,
        "Updated content",
        &vec![0.1; 1536]
    ).await;
    
    assert!(result.is_err());
}

#[tokio::test]
async fn test_delete_nonexistent_memory() {
    let pool = helpers::create_test_pool().await;
    let store = PostgresMemoryStore::new(pool, 1536).await.unwrap();

    let nonexistent_id = Uuid::new_v4();
    let result = store.delete(nonexistent_id).await;
    
    assert!(result.is_err());
}

#[tokio::test]
async fn test_search_with_invalid_dimension() {
    let pool = helpers::create_test_pool().await;
    let store = PostgresMemoryStore::new(pool, 1536).await.unwrap();

    let query = vec![0.1; 512]; // Wrong dimension
    let result = store.search(&query, 10).await;
    
    assert!(result.is_err());
}

#[tokio::test]
async fn test_search_with_empty_embedding() {
    let pool = helpers::create_test_pool().await;
    let store = PostgresMemoryStore::new(pool, 1536).await.unwrap();

    let query: Vec<f32> = vec![];
    let result = store.search(&query, 10).await;
    
    assert!(result.is_err());
}

// ============================================================================
// Boundary Condition Tests
// ============================================================================

#[tokio::test]
async fn test_pagination_boundary_conditions() {
    let pool = helpers::create_test_pool().await;
    let store = PostgresMemoryStore::new(pool, 1536).await.unwrap();

    // Store some test memories
    for i in 0..5 {
        let memory = Memory {
            id: Uuid::new_v4(),
            memory_type: MemoryType::Knowledge,
            content: format!("Test memory {}", i),
            embedding: vec![0.1; 1536],
            metadata: json!({"index": i}),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };
        store.store(memory).await.unwrap();
    }

    // Test limit = 0
    let (results, _) = store.list_paginated(0, 0).await.unwrap();
    assert_eq!(results.len(), 0);

    // Test offset beyond available records
    let (results, _) = store.list_paginated(10, 100).await.unwrap();
    assert_eq!(results.len(), 0);

    // Test large limit
    let (results, total) = store.list_paginated(1000, 0).await.unwrap();
    assert!(results.len() <= total as usize);
}

#[tokio::test]
async fn test_content_at_max_length() {
    let pool = helpers::create_test_pool().await;
    let store = PostgresMemoryStore::new(pool, 1536).await.unwrap();

    let max_content = "x".repeat(32768);
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: max_content.clone(),
        embedding: vec![0.1; 1536],
        metadata: json!({}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let id = store.store(memory).await.unwrap();
    let retrieved = store.retrieve(id).await.unwrap();
    
    assert_eq!(retrieved.content.len(), 32768);
}

#[tokio::test]
async fn test_embedding_at_max_dimension() {
    let pool = helpers::create_test_pool().await;
    let store = PostgresMemoryStore::new(pool, 4096).await.unwrap();

    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Test".to_string(),
        embedding: vec![0.1; 4096], // Max allowed dimension
        metadata: json!({}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };

    let id = store.store(memory).await.unwrap();
    let retrieved = store.retrieve(id).await.unwrap();
    
    assert_eq!(retrieved.embedding.len(), 4096);
}

// ============================================================================
// Concurrency Tests
// ============================================================================

#[tokio::test]
async fn test_concurrent_stores() {
    let pool = helpers::create_test_pool().await;
    let store = std::sync::Arc::new(
        PostgresMemoryStore::new(pool, 1536).await.unwrap()
    );

    let mut handles = vec![];
    
    for i in 0..10 {
        let store_clone = store.clone();
        let handle = tokio::spawn(async move {
            let memory = Memory {
                id: Uuid::new_v4(),
                memory_type: MemoryType::Knowledge,
                content: format!("Concurrent test {}", i),
                embedding: vec![0.1; 1536],
                metadata: json!({"thread": i}),
                created_at: Utc::now(),
                last_accessed: Utc::now(),
            };
            store_clone.store(memory).await
        });
        handles.push(handle);
    }

    let results: Vec<_> = futures::future::join_all(handles).await;
    
    // All stores should succeed
    for result in results {
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());
    }
}

#[tokio::test]
async fn test_concurrent_reads() {
    let pool = helpers::create_test_pool().await;
    let store = std::sync::Arc::new(
        PostgresMemoryStore::new(pool, 1536).await.unwrap()
    );

    // Store a memory first
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Shared memory".to_string(),
        embedding: vec![0.1; 1536],
        metadata: json!({}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };
    let id = store.store(memory).await.unwrap();

    // Concurrent reads
    let mut handles = vec![];
    for _ in 0..10 {
        let store_clone = store.clone();
        let id_clone = id;
        let handle = tokio::spawn(async move {
            store_clone.retrieve(id_clone).await
        });
        handles.push(handle);
    }

    let results: Vec<_> = futures::future::join_all(handles).await;
    
    // All reads should succeed
    for result in results {
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());
    }
}
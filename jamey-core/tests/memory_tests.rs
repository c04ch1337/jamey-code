mod fixtures;
mod helpers;
mod mocks;
mod utils;

use chrono::Utc;
use jamey_core::{Memory, MemoryType};
use serde_json::json;
use uuid::Uuid;
use utils::assert_memories_equal;

#[test]
fn test_memory_creation() {
    let id = Uuid::new_v4();
    let now = Utc::now();
    
    let memory = Memory {
        id,
        memory_type: MemoryType::Knowledge,
        content: "Test content".to_string(),
        embedding: vec![0.1; 1536],
        metadata: json!({"test": true}),
        created_at: now,
        last_accessed: now,
    };
    
    assert_eq!(memory.id, id);
    assert_eq!(memory.memory_type, MemoryType::Knowledge);
    assert_eq!(memory.content, "Test content");
    assert_eq!(memory.embedding.len(), 1536);
    assert!(memory.metadata["test"].as_bool().unwrap());
    assert_eq!(memory.created_at, now);
    assert_eq!(memory.last_accessed, now);
}

#[test]
fn test_memory_type_serialization() {
    let types = vec![
        MemoryType::Knowledge,
        MemoryType::Conversation,
        MemoryType::System,
    ];
    
    for memory_type in types {
        let serialized = serde_json::to_string(&memory_type).unwrap();
        let deserialized: MemoryType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(memory_type, deserialized);
    }
}

#[test]
fn test_memory_clone() {
    let original = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Original content".to_string(),
        embedding: vec![0.1; 1536],
        metadata: json!({"original": true}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };
    
    let cloned = original.clone();
    assert_memories_equal(&original, &cloned);
}

#[test]
fn test_memory_metadata_manipulation() {
    let mut memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::System,
        content: "Test content".to_string(),
        embedding: vec![0.1; 1536],
        metadata: json!({
            "tags": ["test"],
            "priority": 1,
            "nested": {
                "field": "value"
            }
        }),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };
    
    // Test metadata access
    assert!(memory.metadata["tags"].as_array().unwrap().contains(&json!("test")));
    assert_eq!(memory.metadata["priority"].as_i64().unwrap(), 1);
    assert_eq!(
        memory.metadata["nested"]["field"].as_str().unwrap(),
        "value"
    );
    
    // Test metadata modification
    memory.metadata = json!({
        "tags": ["test", "updated"],
        "priority": 2,
        "nested": {
            "field": "new_value"
        }
    });
    
    assert!(memory.metadata["tags"].as_array().unwrap().contains(&json!("updated")));
    assert_eq!(memory.metadata["priority"].as_i64().unwrap(), 2);
    assert_eq!(
        memory.metadata["nested"]["field"].as_str().unwrap(),
        "new_value"
    );
}

#[test]
fn test_memory_embedding_operations() {
    let memory = Memory {
        id: Uuid::new_v4(),
        memory_type: MemoryType::Knowledge,
        content: "Test content".to_string(),
        embedding: vec![0.1; 1536],
        metadata: json!({}),
        created_at: Utc::now(),
        last_accessed: Utc::now(),
    };
    
    // Test embedding dimension
    assert_eq!(memory.embedding.len(), 1536);
    
    // Test embedding values
    assert!(memory.embedding.iter().all(|&x| x == 0.1));
    
    // Test embedding normalization
    let norm: f32 = memory.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - (1536.0_f32 * 0.1_f32 * 0.1_f32).sqrt()).abs() < 1e-6);
}

#[tokio::test]
async fn test_memory_store_operations() {
    let context = helpers::TestContext::new().await.unwrap();
    let store = mocks::MockMemoryStore::new();
    
    // Test store operation
    let memory = context.create_test_memory();
    let id = store.store(memory.clone()).await.unwrap();
    
    // Test retrieve operation
    let retrieved = store.retrieve(id).await.unwrap();
    assert_memories_equal(&memory, &retrieved);
    
    // Test update operation
    let new_content = "Updated content".to_string();
    let new_embedding = vec![0.2; 1536];
    store.update(id, new_content.clone(), new_embedding.clone()).await.unwrap();
    
    let updated = store.retrieve(id).await.unwrap();
    assert_eq!(updated.content, new_content);
    assert_eq!(updated.embedding, new_embedding);
    
    // Test search operation
    let results = store.search(memory.embedding.clone(), 5).await.unwrap();
    assert!(!results.is_empty());
    
    // Test delete operation
    store.delete(id).await.unwrap();
    assert!(store.retrieve(id).await.is_err());
}
mod fixtures;
mod helpers;
mod mocks;
mod utils;

use jamey_core::{
    cache::CacheConfig,
    CachedMemoryStore,
    Memory,
    MemoryType,
};
use std::time::Duration;
use utils::{assert_memories_equal, wait_for_condition};

#[tokio::test]
async fn test_cached_memory_store_creation() {
    let context = helpers::TestContext::new().await.unwrap();
    let store = context.store;
    
    // Verify store is initialized properly
    let test_memory = context.create_test_memory();
    let id = store.store(test_memory.clone()).await.unwrap();
    let retrieved = store.retrieve(id).await.unwrap();
    assert_memories_equal(&test_memory, &retrieved);
}

#[tokio::test]
async fn test_cache_hit_and_miss() {
    let context = helpers::TestContext::new().await.unwrap();
    let store = context.store;
    let test_memory = context.create_test_memory();
    
    // Store memory
    let id = store.store(test_memory.clone()).await.unwrap();
    
    // First retrieval (should be cache miss)
    let first_retrieval = store.retrieve(id).await.unwrap();
    assert_memories_equal(&test_memory, &first_retrieval);
    
    // Second retrieval (should be cache hit)
    let second_retrieval = store.retrieve(id).await.unwrap();
    assert_memories_equal(&test_memory, &second_retrieval);
    
    // Delete from cache and verify miss
    store.delete(id).await.unwrap();
    assert!(store.retrieve(id).await.is_err());
}

#[tokio::test]
async fn test_cache_ttl() {
    let context = helpers::TestContext::new().await.unwrap();
    let config = CacheConfig {
        redis_url: Some(context.pools.redis.clone()),
        memory_capacity: 1000,
        default_ttl_seconds: 1, // Short TTL for testing
        enable_fallback: true,
    };
    
    let store = CachedMemoryStore::new(
        mocks::MockMemoryStore::new(),
        config
    ).await.unwrap();
    
    let test_memory = context.create_test_memory();
    let id = store.store(test_memory.clone()).await.unwrap();
    
    // Initial retrieval should succeed
    let retrieved = store.retrieve(id).await.unwrap();
    assert_memories_equal(&test_memory, &retrieved);
    
    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Verify item is no longer in cache
    assert!(store.retrieve(id).await.is_err());
}

#[tokio::test]
async fn test_cache_capacity() {
    let context = helpers::TestContext::new().await.unwrap();
    let config = CacheConfig {
        redis_url: Some(context.pools.redis.clone()),
        memory_capacity: 2, // Small capacity for testing
        default_ttl_seconds: 300,
        enable_fallback: true,
    };
    
    let store = CachedMemoryStore::new(
        mocks::MockMemoryStore::new(),
        config
    ).await.unwrap();
    
    // Create and store multiple memories
    let mut ids = Vec::new();
    for i in 0..3 {
        let mut memory = context.create_test_memory();
        memory.content = format!("Memory {}", i);
        let id = store.store(memory).await.unwrap();
        ids.push(id);
    }
    
    // Verify oldest item was evicted
    assert!(store.retrieve(ids[0]).await.is_err());
    
    // Verify newer items are still available
    assert!(store.retrieve(ids[1]).await.is_ok());
    assert!(store.retrieve(ids[2]).await.is_ok());
}

#[tokio::test]
async fn test_cache_fallback() {
    let context = helpers::TestContext::new().await.unwrap();
    let config = CacheConfig {
        redis_url: None, // No Redis for testing fallback
        memory_capacity: 1000,
        default_ttl_seconds: 300,
        enable_fallback: true,
    };
    
    let store = CachedMemoryStore::new(
        mocks::MockMemoryStore::new(),
        config
    ).await.unwrap();
    
    let test_memory = context.create_test_memory();
    let id = store.store(test_memory.clone()).await.unwrap();
    
    // Verify memory operations still work with fallback
    let retrieved = store.retrieve(id).await.unwrap();
    assert_memories_equal(&test_memory, &retrieved);
}

#[tokio::test]
async fn test_concurrent_cache_access() {
    let context = helpers::TestContext::new().await.unwrap();
    let store = context.store;
    let test_memory = context.create_test_memory();
    let id = store.store(test_memory.clone()).await.unwrap();
    
    // Spawn multiple concurrent retrievals
    let mut handles = Vec::new();
    for _ in 0..10 {
        let store = store.clone();
        let id = id;
        handles.push(tokio::spawn(async move {
            store.retrieve(id).await.unwrap()
        }));
    }
    
    // Wait for all retrievals and verify
    for handle in handles {
        let retrieved = handle.await.unwrap();
        assert_memories_equal(&test_memory, &retrieved);
    }
}

#[tokio::test]
async fn test_cache_update_propagation() {
    let context = helpers::TestContext::new().await.unwrap();
    let store = context.store;
    let test_memory = context.create_test_memory();
    
    // Store initial memory
    let id = store.store(test_memory.clone()).await.unwrap();
    
    // Update memory
    let new_content = "Updated content".to_string();
    let new_embedding = vec![0.2; 1536];
    store.update(id, new_content.clone(), new_embedding.clone()).await.unwrap();
    
    // Verify update is reflected in cache
    let updated = store.retrieve(id).await.unwrap();
    assert_eq!(updated.content, new_content);
    assert_eq!(updated.embedding, new_embedding);
}

#[tokio::test]
async fn test_cache_search() {
    let context = helpers::TestContext::new().await.unwrap();
    let store = context.store;
    let memories = fixtures::TestMemories::default();
    
    // Store multiple memories
    let ids = vec![
        store.store(memories.knowledge.clone()).await.unwrap(),
        store.store(memories.conversation.clone()).await.unwrap(),
        store.store(memories.system.clone()).await.unwrap(),
    ];
    
    // Search with different embeddings
    let results = store.search(memories.knowledge.embedding.clone(), 2).await.unwrap();
    assert_eq!(results.len(), 2);
    
    // Verify search results are ordered by similarity
    assert_eq!(results[0].id, ids[0]); // Most similar should be the knowledge memory
    
    // Clean up
    for id in ids {
        store.delete(id).await.unwrap();
    }
}
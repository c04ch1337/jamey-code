mod fixtures;
mod helpers;
mod mocks;
mod utils;

use anyhow::Result;
use jamey_core::{
    Memory, MemoryType,
    ConnectionPools, PoolConfig,
    PostgresPoolConfig, RedisPoolConfig,
    PostgresMemoryStore, CachedMemoryStore,
    cache::CacheConfig,
};
use std::time::Duration;
use tokio::time::sleep;
use utils::{assert_memories_equal, wait_for_condition};

async fn setup_test_pools() -> Result<ConnectionPools> {
    let config = PoolConfig {
        postgres: PostgresPoolConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "jamey_test".to_string(),
            user: "jamey".to_string(),
            password: "test_password".to_string(),
            max_connections: 20,
            min_connections: 5,
            connect_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(300),
        },
        redis: RedisPoolConfig {
            url: "redis://localhost".to_string(),
            max_connections: 20,
            min_connections: 5,
            connect_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(300),
        },
    };

    Ok(ConnectionPools::new(config).await?)
}

#[tokio::test]
async fn test_postgres_basic_operations() -> Result<()> {
    let pools = setup_test_pools().await?;
    let store = PostgresMemoryStore::new(pools.postgres.clone(), 1536).await?;
    let memories = fixtures::TestMemories::default();

    // Test insert
    let id = store.store(memories.knowledge.clone()).await?;
    
    // Test retrieve
    let retrieved = store.retrieve(id).await?;
    assert_memories_equal(&memories.knowledge, &retrieved);
    
    // Test update
    let new_content = "Updated content".to_string();
    let new_embedding = vec![0.2; 1536];
    store.update(id, new_content.clone(), new_embedding.clone()).await?;
    
    let updated = store.retrieve(id).await?;
    assert_eq!(updated.content, new_content);
    assert_eq!(updated.embedding, new_embedding);
    
    // Test delete
    store.delete(id).await?;
    assert!(store.retrieve(id).await.is_err());

    Ok(())
}

#[tokio::test]
async fn test_postgres_vector_search() -> Result<()> {
    let pools = setup_test_pools().await?;
    let store = PostgresMemoryStore::new(pools.postgres.clone(), 1536).await?;
    let memories = fixtures::TestMemories::default();

    // Store multiple memories with different embeddings
    let ids = vec![
        store.store(memories.knowledge.clone()).await?,
        store.store(memories.conversation.clone()).await?,
        store.store(memories.system.clone()).await?,
    ];

    // Test vector similarity search
    let results = store.search(memories.knowledge.embedding.clone(), 2).await?;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, ids[0]); // Most similar should be first

    // Clean up
    for id in ids {
        store.delete(id).await?;
    }

    Ok(())
}

#[tokio::test]
async fn test_postgres_concurrent_operations() -> Result<()> {
    let pools = setup_test_pools().await?;
    let store = PostgresMemoryStore::new(pools.postgres.clone(), 1536).await?;
    let memories = fixtures::TestMemories::default();
    
    let mut handles = Vec::new();
    
    // Spawn multiple concurrent operations
    for i in 0..10 {
        let store = store.clone();
        let mut memory = memories.knowledge.clone();
        memory.content = format!("Memory {}", i);
        
        handles.push(tokio::spawn(async move {
            // Store memory
            let id = store.store(memory.clone()).await?;
            
            // Retrieve memory
            let retrieved = store.retrieve(id).await?;
            assert_eq!(retrieved.content, format!("Memory {}", i));
            
            // Update memory
            store.update(
                id,
                format!("Updated memory {}", i),
                vec![0.2; 1536]
            ).await?;
            
            // Delete memory
            store.delete(id).await?;
            
            Ok::<_, anyhow::Error>(())
        }));
    }
    
    // Wait for all operations to complete
    for handle in handles {
        handle.await??;
    }

    Ok(())
}

#[tokio::test]
async fn test_redis_cache_operations() -> Result<()> {
    let pools = setup_test_pools().await?;
    let postgres_store = PostgresMemoryStore::new(pools.postgres.clone(), 1536).await?;
    
    let cache_config = CacheConfig {
        redis_url: Some(pools.redis.clone()),
        memory_capacity: 1000,
        default_ttl_seconds: 1, // Short TTL for testing
        enable_fallback: true,
    };
    
    let store = CachedMemoryStore::new(postgres_store, cache_config).await?;
    let memories = fixtures::TestMemories::default();

    // Store memory
    let id = store.store(memories.knowledge.clone()).await?;
    
    // First retrieval (cache miss, loads from Postgres)
    let first_retrieval = store.retrieve(id).await?;
    assert_memories_equal(&memories.knowledge, &first_retrieval);
    
    // Second retrieval (cache hit, loads from Redis)
    let second_retrieval = store.retrieve(id).await?;
    assert_memories_equal(&memories.knowledge, &second_retrieval);
    
    // Wait for TTL to expire
    sleep(Duration::from_secs(2)).await;
    
    // Third retrieval (cache miss after TTL, loads from Postgres)
    let third_retrieval = store.retrieve(id).await?;
    assert_memories_equal(&memories.knowledge, &third_retrieval);
    
    // Clean up
    store.delete(id).await?;

    Ok(())
}

#[tokio::test]
async fn test_redis_cache_fallback() -> Result<()> {
    let pools = setup_test_pools().await?;
    let postgres_store = PostgresMemoryStore::new(pools.postgres.clone(), 1536).await?;
    
    // Configure cache with invalid Redis URL to test fallback
    let cache_config = CacheConfig {
        redis_url: None, // No Redis connection
        memory_capacity: 1000,
        default_ttl_seconds: 300,
        enable_fallback: true,
    };
    
    let store = CachedMemoryStore::new(postgres_store, cache_config).await?;
    let memories = fixtures::TestMemories::default();

    // Store should still work using in-memory cache
    let id = store.store(memories.knowledge.clone()).await?;
    
    // Retrieval should work from in-memory cache
    let retrieved = store.retrieve(id).await?;
    assert_memories_equal(&memories.knowledge, &retrieved);
    
    // Clean up
    store.delete(id).await?;

    Ok(())
}

#[tokio::test]
async fn test_database_error_handling() -> Result<()> {
    // Test with invalid PostgreSQL connection
    let config = PoolConfig {
        postgres: PostgresPoolConfig {
            host: "invalid-host".to_string(),
            port: 5432,
            database: "invalid-db".to_string(),
            user: "invalid-user".to_string(),
            password: "invalid-password".to_string(),
            max_connections: 5,
            min_connections: 1,
            connect_timeout: Duration::from_secs(1),
            idle_timeout: Duration::from_secs(300),
        },
        redis: RedisPoolConfig {
            url: "redis://localhost".to_string(),
            max_connections: 5,
            min_connections: 1,
            connect_timeout: Duration::from_secs(1),
            idle_timeout: Duration::from_secs(300),
        },
    };

    // Connection should fail
    let result = ConnectionPools::new(config).await;
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_database_transaction_handling() -> Result<()> {
    let pools = setup_test_pools().await?;
    let mut client = pools.postgres.get().await?;
    let memories = fixtures::TestMemories::default();

    // Start transaction
    let tx = client.transaction().await?;

    // Insert memory
    tx.execute(
        "INSERT INTO memories (id, memory_type, content, embedding, metadata, created_at, last_accessed) 
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        &[
            &memories.knowledge.id,
            &memories.knowledge.memory_type,
            &memories.knowledge.content,
            &memories.knowledge.embedding,
            &memories.knowledge.metadata,
            &memories.knowledge.created_at,
            &memories.knowledge.last_accessed,
        ],
    ).await?;

    // Rollback transaction
    tx.rollback().await?;

    // Verify memory was not inserted
    let result = client
        .query_opt(
            "SELECT id FROM memories WHERE id = $1",
            &[&memories.knowledge.id],
        )
        .await?;
    assert!(result.is_none());

    Ok(())
}
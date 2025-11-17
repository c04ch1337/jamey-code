use jamey_core::{
    Memory, MemoryType, PostgresMemoryStore, CachedMemoryStore, ConnectionPools, PoolConfig,
    PostgresPoolConfig, RedisPoolConfig, cache::CacheConfig,
};
use std::time::Duration;
use uuid::Uuid;
use chrono::Utc;

pub struct TestContext {
    pub pools: ConnectionPools,
    pub store: CachedMemoryStore,
}

impl TestContext {
    pub async fn new() -> anyhow::Result<Self> {
        let pools = setup_test_pools().await?;
        let store = setup_test_store(&pools).await?;
        Ok(Self { pools, store })
    }

    pub fn create_test_memory(&self) -> Memory {
        Memory {
            id: Uuid::new_v4(),
            memory_type: MemoryType::Knowledge,
            content: "Test memory content".to_string(),
            embedding: vec![0.1; 1536],
            metadata: serde_json::json!({"test": true}),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        }
    }
}

pub async fn setup_test_pools() -> anyhow::Result<ConnectionPools> {
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

pub async fn setup_test_store(pools: &ConnectionPools) -> anyhow::Result<CachedMemoryStore> {
    let postgres_store = PostgresMemoryStore::new(pools.postgres.clone(), 1536).await?;
    
    let cache_config = CacheConfig {
        redis_url: Some(pools.redis.clone()),
        memory_capacity: 1000,
        default_ttl_seconds: 300,
        enable_fallback: true,
    };
    
    Ok(CachedMemoryStore::new(postgres_store, cache_config).await?)
}

pub async fn cleanup_test_data(store: &CachedMemoryStore) -> anyhow::Result<()> {
    // Clean up test memories
    let test_memories = store.search(vec![0.1; 1536], 100).await?;
    for memory in test_memories {
        store.delete(memory.id).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_context_creation() {
        let context = TestContext::new().await.unwrap();
        let memory = context.create_test_memory();
        assert_eq!(memory.embedding.len(), 1536);
        assert!(memory.metadata.get("test").unwrap().as_bool().unwrap());
    }
}
//! Core functionality for Digital Twin Jamey
//! 
//! This crate provides the fundamental building blocks for Jamey's memory and data management.
//! It includes PostgreSQL-backed vector storage with similarity search capabilities.

pub mod memory;
pub mod cache;
pub mod cached_memory;
pub mod pool;

pub use memory::{Memory, MemoryError, MemoryStore, MemoryType, PostgresMemoryStore};
pub use cache::{CacheManager, CacheConfig, CacheError, CacheBackend, RedisCache, MemoryCache, HybridCache};
pub use cached_memory::{CachedMemoryStore, AdvancedCachedMemoryStore, CacheStats, InvalidationStrategy};
pub use pool::{ConnectionPools, PoolConfig, PostgresPoolConfig, RedisPoolConfig, HealthStatus, PoolStatus};

/// Re-export common types used throughout the crate
pub mod prelude {
    pub use super::memory::{Memory, MemoryError, MemoryStore, MemoryType, PostgresMemoryStore};
    pub use super::pool::{ConnectionPools, PoolConfig, PostgresPoolConfig, RedisPoolConfig};
    pub use chrono::{DateTime, Utc};
    pub use uuid::Uuid;
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory::PostgresMemoryStore;
    use deadpool_postgres::{Config, Runtime};
    use tokio_postgres::NoTls;

    async fn setup_test_db() -> Result<PostgresMemoryStore, Box<dyn std::error::Error>> {
        let mut cfg = Config::new();
        cfg.host = Some("localhost".to_string());
        cfg.dbname = Some("jamey_test".to_string());
        cfg.user = Some("jamey".to_string());
        cfg.password = Some("test_password".to_string());

        let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)
            .map_err(|e| format!("Failed to create connection pool: {}", e))?;
        let store = PostgresMemoryStore::new(pool, 1536).await?;
        Ok(store)
    }

    #[tokio::test]
    async fn test_memory_integration() -> Result<(), Box<dyn std::error::Error>> {
        let store = setup_test_db().await?;
        
        // Test full memory lifecycle
        let memory = Memory {
            id: uuid::Uuid::new_v4(),
            memory_type: MemoryType::Knowledge,
            content: "Integration test memory".to_string(),
            embedding: vec![0.1; 1536],
            metadata: serde_json::json!({
                "source": "integration_test",
                "confidence": 0.95
            }),
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
        };

        // Store
        let id = store.store(memory.clone()).await?;

        // Retrieve
        let retrieved = store.retrieve(id).await?;
        assert_eq!(retrieved.content, memory.content);
        assert_eq!(retrieved.memory_type, memory.memory_type);

        // Search
        let results = store.search(vec![0.1; 1536], 1).await?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);

        // Update
        store.update(id, "Updated content".to_string(), vec![0.2; 1536]).await?;
        let updated = store.retrieve(id).await?;
        assert_eq!(updated.content, "Updated content");

        // Delete
        store.delete(id).await?;
        assert!(store.retrieve(id).await.is_err());
        Ok(())
    }
}
//! Cached memory store implementation
//! 
//! Combines PostgreSQL persistence with Redis/memory caching
//! for improved performance and reduced database load.

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::cache::CacheManager;
use crate::memory::{Memory, MemoryStore, PostgresMemoryStore};

/// Cached memory store that wraps PostgreSQL with caching
pub struct CachedMemoryStore {
    postgres_store: Arc<PostgresMemoryStore>,
    cache: Arc<CacheManager>,
}

impl CachedMemoryStore {
    pub async fn new(
        postgres_store: PostgresMemoryStore,
        cache_config: crate::cache::CacheConfig,
    ) -> Result<Self> {
        let cache = Arc::new(CacheManager::new(cache_config).await?);
        let postgres_store = Arc::new(postgres_store);
        
        info!("Initialized cached memory store with Redis fallback");
        
        Ok(Self {
            postgres_store,
            cache,
        })
    }

    /// Invalidate cache for a specific memory entry
    pub async fn invalidate_cache(&self, id: Uuid) -> Result<()> {
        debug!("Invalidating cache for memory: {}", id);
        self.cache.invalidate_memory(id).await?;
        Ok(())
    }

    /// Warm up cache with frequently accessed memories
    pub async fn warm_cache(&self, limit: usize) -> Result<usize> {
        info!("Warming up cache with {} recent memories", limit);
        
        let dummy_embedding = vec![0.0; 1536];
        let recent_memories = self.postgres_store.search(dummy_embedding, limit).await?;
        
        let mut cached_count = 0;
        for memory in recent_memories {
            if self.cache.cache_memory(&memory).await.is_ok() {
                cached_count += 1;
            }
        }
        
        info!("Successfully cached {} memories", cached_count);
        Ok(cached_count)
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> Result<CacheStats> {
        let stats = self.cache.get_stats().await?;
        Ok(CacheStats {
            memory_entries: stats.entries,
            search_entries: stats.search_entries,
            hit_rate: stats.hit_rate,
            memory_usage_mb: stats.memory_usage_mb,
        })
    }
}

#[async_trait]
impl MemoryStore for CachedMemoryStore {
    async fn store(&self, memory: Memory) -> Result<Uuid> {
        debug!("Storing memory with caching: {}", memory.id);
        
        // Store in PostgreSQL first
        let id = self.postgres_store.store(memory.clone()).await?;
        
        // Cache the stored memory
        if let Err(e) = self.cache.cache_memory(&memory).await {
            warn!("Failed to cache memory {}: {}", id, e);
        }
        
        Ok(id)
    }

    async fn retrieve(&self, id: Uuid) -> Result<Memory> {
        debug!("Retrieving memory with caching: {}", id);
        
        // Try cache first
        match self.cache.get_cached_memory(id).await {
            Ok(Some(memory)) => {
                debug!("Cache hit for memory: {}", id);
                return Ok(memory);
            }
            Ok(None) => {
                debug!("Cache miss for memory: {}", id);
            }
            Err(e) => {
                warn!("Cache error for memory {}: {}, falling back to database", id, e);
            }
        }
        
        // Fallback to PostgreSQL
        let memory = self.postgres_store.retrieve(id).await?;
        
        // Cache the retrieved memory for future requests
        if let Err(e) = self.cache.cache_memory(&memory).await {
            warn!("Failed to cache retrieved memory {}: {}", id, e);
        }
        
        Ok(memory)
    }

    async fn search(&self, query_embedding: Vec<f32>, limit: usize) -> Result<Vec<Memory>> {
        debug!("Searching memories with caching, limit: {}", limit);
        
        // Create a more robust cache key using multiple vector segments
        let query_key = {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            for chunk in query_embedding.chunks(32) {
                let chunk_hash = chunk.iter()
                    .map(|x| x.to_bits() as u64)
                    .fold(0, |acc, x| acc.wrapping_add(x));
                hasher.write_u64(chunk_hash);
            }
            format!("search:{}:{}", hasher.finish(), limit)
        };
        
        // Try cache first
        match self.cache.get_cached_search_results::<Vec<Memory>>(&query_key).await {
            Ok(Some(results)) => {
                debug!("Cache hit for search query");
                return Ok(results);
            }
            Ok(None) => {
                debug!("Cache miss for search query");
            }
            Err(e) => {
                warn!("Cache search error: {}, falling back to database", e);
            }
        }
        
        // Fallback to PostgreSQL
        let results = self.postgres_store.search(query_embedding, limit).await?;
        
        // Cache the search results (shorter TTL for search results)
        if let Err(e) = self.cache.cache_search_results(&query_key, &results).await {
            warn!("Failed to cache search results: {}", e);
        }
        
        Ok(results)
    }

    async fn update(&self, id: Uuid, content: String, embedding: Vec<f32>) -> Result<()> {
        debug!("Updating memory with cache invalidation: {}", id);
        
        // Update in PostgreSQL
        self.postgres_store.update(id, content.clone(), embedding.clone()).await?;
        
        // Retrieve updated memory and update cache immediately
        match self.postgres_store.retrieve(id).await {
            Ok(updated_memory) => {
                if let Err(e) = self.cache.cache_memory(&updated_memory).await {
                    warn!("Failed to update cache for memory {}: {}", id, e);
                }
            }
            Err(e) => {
                warn!("Failed to retrieve updated memory for caching {}: {}", id, e);
                // Fallback to cache invalidation
                if let Err(e) = self.invalidate_cache(id).await {
                    warn!("Failed to invalidate cache for memory {}: {}", id, e);
                }
            }
        }
        
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        debug!("Deleting memory with cache invalidation: {}", id);
        
        // Delete from PostgreSQL
        self.postgres_store.delete(id).await?;
        
        // Remove from cache
        if let Err(e) = self.invalidate_cache(id).await {
            warn!("Failed to remove memory {} from cache: {}", id, e);
        }
        
        Ok(())
    }
}

/// Cache statistics for monitoring
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub memory_entries: usize,
    pub search_entries: usize,
    pub hit_rate: f64,
    pub memory_usage_mb: f64,
}

/// Cache invalidation strategies
#[derive(Debug, Clone)]
pub enum InvalidationStrategy {
    /// Invalidate on write (immediate consistency)
    Immediate,
    /// Invalidate after a delay (eventual consistency)
    Delayed(std::time::Duration),
    /// Invalidate based on access patterns
    Adaptive,
    /// Manual invalidation only
    Manual,
}

impl Default for InvalidationStrategy {
    fn default() -> Self {
        Self::Immediate
    }
}

/// Advanced cached memory store with configurable invalidation
pub struct AdvancedCachedMemoryStore {
    inner: CachedMemoryStore,
    invalidation_strategy: InvalidationStrategy,
}

impl AdvancedCachedMemoryStore {
    pub async fn new(
        postgres_store: PostgresMemoryStore,
        cache_config: crate::cache::CacheConfig,
        invalidation_strategy: InvalidationStrategy,
    ) -> Result<Self> {
        let inner = CachedMemoryStore::new(postgres_store, cache_config).await?;
        
        Ok(Self {
            inner,
            invalidation_strategy,
        })
    }

    async fn invalidate_with_strategy(&self, id: Uuid) -> Result<()> {
        match self.invalidation_strategy {
            InvalidationStrategy::Immediate => {
                self.inner.invalidate_cache(id).await?;
            }
            InvalidationStrategy::Delayed(delay) => {
                let cache = self.inner.cache.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(delay).await;
                    if let Err(e) = cache.invalidate_memory(id).await {
                        warn!("Delayed cache invalidation failed for {}: {}", id, e);
                    }
                });
            }
            InvalidationStrategy::Adaptive => {
                let cache = self.inner.cache.clone();
                let access_count = cache.get_access_count(id).await.unwrap_or(0);
                
                if access_count > 10 {
                    // Frequently accessed items - update instead of invalidate
                    if let Ok(memory) = self.inner.postgres_store.retrieve(id).await {
                        if let Err(e) = cache.cache_memory(&memory).await {
                            warn!("Failed to update frequently accessed memory {}: {}", id, e);
                            // Fallback to invalidation
                            self.inner.invalidate_cache(id).await?;
                        }
                    }
                } else {
                    // Less frequently accessed - just invalidate
                    self.inner.invalidate_cache(id).await?;
                }
            }
            InvalidationStrategy::Manual => {
                // Don't invalidate automatically
                debug!("Skipping automatic cache invalidation for manual strategy");
            }
        }
        
        Ok(())
    }

    /// Manual cache invalidation for Manual strategy
    pub async fn manual_invalidate(&self, id: Uuid) -> Result<()> {
        self.inner.invalidate_cache(id).await
    }

    /// Clear all cache entries
    pub async fn clear_all_cache(&self) -> Result<()> {
        self.inner.cache.clear_all().await.map_err(|e| anyhow::anyhow!("Cache error: {}", e))
    }
}

#[async_trait]
impl MemoryStore for AdvancedCachedMemoryStore {
    async fn store(&self, memory: Memory) -> Result<Uuid> {
        self.inner.store(memory).await
    }

    async fn retrieve(&self, id: Uuid) -> Result<Memory> {
        self.inner.retrieve(id).await
    }

    async fn search(&self, query_embedding: Vec<f32>, limit: usize) -> Result<Vec<Memory>> {
        self.inner.search(query_embedding, limit).await
    }

    async fn update(&self, id: Uuid, content: String, embedding: Vec<f32>) -> Result<()> {
        // Update in database first
        self.inner.postgres_store.update(id, content, embedding).await?;
        
        // Invalidate cache according to strategy
        self.invalidate_with_strategy(id).await?;
        
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        // Delete from database first
        self.inner.postgres_store.delete(id).await?;
        
        // Invalidate cache according to strategy
        self.invalidate_with_strategy(id).await?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryType, PostgresMemoryStore};
    use deadpool_postgres::{Config, Runtime};
    use tokio_postgres::NoTls;
    use chrono::Utc;

    async fn create_test_store() -> Result<AdvancedCachedMemoryStore, Box<dyn std::error::Error>> {
        let mut cfg = Config::new();
        cfg.host = Some("localhost".to_string());
        cfg.dbname = Some("jamey_test".to_string());
        cfg.user = Some("jamey".to_string());
        cfg.password = Some("test_password".to_string());

        let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)
            .map_err(|e| format!("Failed to create connection pool: {}", e))?;
        let postgres_store = PostgresMemoryStore::new(pool, 1536).await?;
        
        let cache_config = crate::cache::CacheConfig {
            redis_url: None,
            memory_capacity: 100,
            default_ttl_seconds: 60,
            enable_fallback: false,
        };

        let store = AdvancedCachedMemoryStore::new(
            postgres_store,
            cache_config,
            InvalidationStrategy::Immediate,
        ).await?;
        
        Ok(store)
    }

    #[tokio::test]
    async fn test_cached_memory_store() -> Result<(), Box<dyn std::error::Error>> {
        let store = create_test_store().await?;
        
        let memory = Memory {
            id: Uuid::new_v4(),
            memory_type: MemoryType::Conversation,
            content: "Test cached memory".to_string(),
            embedding: vec![0.1; 1536],
            metadata: serde_json::json!({"test": true}),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };

        // Store memory
        let id = store.store(memory.clone()).await?;
        
        // Retrieve (should hit cache on second call)
        let retrieved1 = store.retrieve(id).await?;
        assert_eq!(retrieved1.content, memory.content);
        
        let retrieved2 = store.retrieve(id).await?;
        assert_eq!(retrieved2.content, memory.content);
        
        // Update memory
        store.update(id, "Updated content".to_string(), vec![0.2; 1536]).await?;
        
        // Retrieve updated memory
        let updated = store.retrieve(id).await?;
        assert_eq!(updated.content, "Updated content");
        
        // Delete memory
        store.delete(id).await?;
        assert!(store.retrieve(id).await.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn test_invalidation_strategies() -> Result<(), Box<dyn std::error::Error>> {
        let store = create_test_store().await?;
        
        let memory = Memory {
            id: Uuid::new_v4(),
            memory_type: MemoryType::Conversation,
            content: "Test memory".to_string(),
            embedding: vec![0.1; 1536],
            metadata: serde_json::json!({"test": true}),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };

        let id = store.store(memory.clone()).await?;
        
        // Test manual invalidation
        store.manual_invalidate(id).await?;
        
        // Clear all cache
        store.clear_all_cache().await?;
        Ok(())
    }
}
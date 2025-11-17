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
use crate::memory::{Memory, MemoryStore, PostgresMemoryStore, MemoryError};

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
        let recent_memories = self.postgres_store.search(&dummy_embedding, limit).await?;
        
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

    fn validate_search_results(results: &[Memory]) -> Result<()> {
        if results.len() > 1000 {
            return Err(MemoryError::InvalidRequest("Too many search results".to_string()).into());
        }
        
        for memory in results {
            // Validate content
            if memory.content.is_empty() || memory.content.len() > 32768 {
                return Err(MemoryError::InvalidRequest("Invalid content length in search results".to_string()).into());
            }
            
            // Validate embedding
            if memory.embedding.is_empty() || memory.embedding.len() > 4096 {
                return Err(MemoryError::InvalidRequest("Invalid embedding dimension in search results".to_string()).into());
            }
            if memory.embedding.iter().any(|x| x.is_nan() || x.is_infinite()) {
                return Err(MemoryError::InvalidRequest("Invalid embedding values in search results".to_string()).into());
            }
            
            // Validate metadata
            let metadata_str = serde_json::to_string(&memory.metadata)
                .map_err(|e| MemoryError::InvalidRequest(format!("Invalid metadata JSON: {}", e)))?;
            if metadata_str.len() > 16384 {
                return Err(MemoryError::InvalidRequest("Metadata too large in search results".to_string()).into());
            }
        }
        
        Ok(())
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

    async fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<Memory>> {
        debug!("Searching memories with caching, limit: {}", limit);
        
        // Validate input parameters
        if query_embedding.is_empty() {
            return Err(MemoryError::InvalidRequest("Query embedding cannot be empty".to_string()).into());
        }
        if query_embedding.iter().any(|x| x.is_nan() || x.is_infinite()) {
            return Err(MemoryError::InvalidRequest("Query embedding contains invalid values".to_string()).into());
        }
        if limit == 0 || limit > 1000 {
            return Err(MemoryError::InvalidRequest("Invalid limit: must be between 1 and 1000".to_string()).into());
        }

        // Create a secure cache key using multiple vector segments
        let query_key = {
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            
            // Add embedding chunks to hash
            for chunk in query_embedding.chunks(32) {
                let chunk_bytes: Vec<u8> = chunk.iter()
                    .flat_map(|x| x.to_le_bytes().to_vec())
                    .collect();
                hasher.update(&chunk_bytes);
            }
            
            // Add limit to hash
            hasher.update(&limit.to_le_bytes());
            
            // Create final key with prefix
            format!("search:{:x}:{}", hasher.finalize(), limit)
        };
        
        // Try cache first with validation
        match self.cache.get_cached_search_results::<Vec<Memory>>(&query_key).await {
            Ok(Some(results)) => {
                debug!("Cache hit for search query");
                // Validate cached results
                if let Err(e) = Self::validate_search_results(&results) {
                    warn!("Invalid cached results: {}, falling back to database", e);
                } else {
                    return Ok(results);
                }
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
        
        // Validate results before caching
        Self::validate_search_results(&results)?;
        
        // Cache the search results (shorter TTL for search results)
        if let Err(e) = self.cache.cache_search_results(&query_key, &results).await {
            warn!("Failed to cache search results: {}", e);
        }
        
        Ok(results)
    }

    async fn update(&self, id: Uuid, content: &str, embedding: &[f32]) -> Result<()> {
        debug!("Updating memory with cache invalidation: {}", id);
        
        // Validate input
        if content.is_empty() {
            return Err(MemoryError::InvalidRequest("Content cannot be empty".to_string()).into());
        }
        if content.len() > 32768 {
            return Err(MemoryError::InvalidRequest("Content too long".to_string()).into());
        }
        if embedding.is_empty() {
            return Err(MemoryError::InvalidRequest("Embedding cannot be empty".to_string()).into());
        }
        if embedding.iter().any(|x| x.is_nan() || x.is_infinite()) {
            return Err(MemoryError::InvalidRequest("Embedding contains invalid values".to_string()).into());
        }
        
        // Update in PostgreSQL
        self.postgres_store.update(id, content, embedding).await?;
        
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

    async fn list_paginated(&self, limit: usize, offset: usize) -> Result<(Vec<Memory>, i64)> {
        debug!("Listing memories with pagination (cached): limit={}, offset={}", limit, offset);
        
        // Pagination results are not cached as they change frequently
        // and caching would require complex invalidation logic
        self.postgres_store.list_paginated(limit, offset).await
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

    fn validate_search_results(results: &[Memory]) -> Result<()> {
        if results.len() > 1000 {
            return Err(MemoryError::InvalidRequest("Too many search results".to_string()).into());
        }
        
        for memory in results {
            // Validate content
            if memory.content.is_empty() || memory.content.len() > 32768 {
                return Err(MemoryError::InvalidRequest("Invalid content length in search results".to_string()).into());
            }
            
            // Validate embedding
            if memory.embedding.is_empty() || memory.embedding.len() > 4096 {
                return Err(MemoryError::InvalidRequest("Invalid embedding dimension in search results".to_string()).into());
            }
            if memory.embedding.iter().any(|x| x.is_nan() || x.is_infinite()) {
                return Err(MemoryError::InvalidRequest("Invalid embedding values in search results".to_string()).into());
            }
            
            // Validate metadata
            let metadata_str = serde_json::to_string(&memory.metadata)
                .map_err(|e| MemoryError::InvalidRequest(format!("Invalid metadata JSON: {}", e)))?;
            if metadata_str.len() > 16384 {
                return Err(MemoryError::InvalidRequest("Metadata too large in search results".to_string()).into());
            }
        }
        
        Ok(())
    }

    async fn invalidate_with_strategy(&self, id: Uuid) -> Result<()> {
        // Validate UUID
        if id.is_nil() {
            return Err(MemoryError::InvalidRequest("Invalid UUID".to_string()).into());
        }

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

    async fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<Memory>> {
        self.inner.search(query_embedding, limit).await
    }

    async fn update(&self, id: Uuid, content: &str, embedding: &[f32]) -> Result<()> {
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

    async fn list_paginated(&self, limit: usize, offset: usize) -> Result<(Vec<Memory>, i64)> {
        self.inner.list_paginated(limit, offset).await
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
        log::info!("Connecting to test database with configured credentials");
        
        let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)
            .map_err(|e| format!("Failed to create connection pool: {}", e))?;
        let postgres_store = PostgresMemoryStore::new(pool, 1536).await?;
        
        let cache_config = crate::cache::CacheConfig {
            redis_url: None,
            memory_capacity: 100,
            default_ttl_seconds: 60,
            enable_fallback: false,
            key_prefix: "test".to_string(),
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
        store.update(id, "Updated content", &vec![0.2; 1536]).await?;
        
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
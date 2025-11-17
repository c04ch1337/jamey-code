//! Caching layer for Jamey's memory and data
//! 
//! Provides Redis-backed caching with in-memory fallback
//! for improved performance and scalability.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Cache miss for key: {0}")]
    Miss(String),
    #[error("Cache connection error: {0}")]
    Connection(String),
}

/// Cache backend trait for different storage implementations
#[async_trait]
pub trait CacheBackend: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError>;
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<(), CacheError>;
    async fn delete(&self, key: &str) -> Result<bool, CacheError>;
    async fn clear(&self) -> Result<(), CacheError>;
    async fn exists(&self, key: &str) -> Result<bool, CacheError>;
}

/// Redis cache backend implementation
pub struct RedisCache {
    client: redis::aio::ConnectionManager,
    key_prefix: String,
}

impl RedisCache {
    pub async fn new(redis_url: &str, key_prefix: &str) -> Result<Self, CacheError> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        
        let conn = client.get_connection_manager().await?;
        
        info!("Connected to Redis cache with prefix: {}", key_prefix);
        
        Ok(Self {
            client: conn,
            key_prefix: key_prefix.to_string(),
        })
    }

    fn format_key(&self, key: &str) -> String {
        format!("{}:{}", self.key_prefix, key)
    }
}

#[async_trait]
impl CacheBackend for RedisCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        let formatted_key = self.format_key(key);
        debug!("Getting cache key: {}", formatted_key);
        
        let result: Option<Vec<u8>> = redis::cmd("GET")
            .arg(&formatted_key)
            .query_async(&mut self.client.clone())
            .await?;
            
        Ok(result)
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<(), CacheError> {
        let formatted_key = self.format_key(key);
        debug!("Setting cache key: {} with TTL: {:?}", formatted_key, ttl);
        
        if let Some(ttl) = ttl {
            let ttl_secs = ttl.as_secs();
            let mut cmd = redis::cmd("SETEX");
            cmd.arg(&formatted_key)
               .arg(ttl_secs)
               .arg(&value);
            cmd.query_async(&mut self.client.clone()).await?;
        } else {
            let mut cmd = redis::cmd("SET");
            cmd.arg(&formatted_key)
               .arg(&value);
            cmd.query_async(&mut self.client.clone()).await?;
        }
        
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        let formatted_key = self.format_key(key);
        debug!("Deleting cache key: {}", formatted_key);
        
        let deleted: i32 = redis::cmd("DEL")
            .arg(&formatted_key)
            .query_async(&mut self.client.clone())
            .await?;
            
        Ok(deleted > 0)
    }

    async fn clear(&self) -> Result<(), CacheError> {
        warn!("Clearing all cache entries with prefix: {}", self.key_prefix);
        
        let pattern = format!("{}:*", self.key_prefix);
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut self.client.clone())
            .await?;
            
        if !keys.is_empty() {
            let mut cmd = redis::cmd("DEL");
            cmd.arg(keys);
            cmd.query_async(&mut self.client.clone()).await?;
        }
        
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        let formatted_key = self.format_key(key);
        
        let exists: bool = redis::cmd("EXISTS")
            .arg(&formatted_key)
            .query_async(&mut self.client.clone())
            .await?;
            
        Ok(exists)
    }
}

/// In-memory LRU cache backend for fallback
pub struct MemoryCache {
    cache: tokio::sync::RwLock<lru::LruCache<String, (Vec<u8>, Option<std::time::Instant>)>>,
    default_ttl: Duration,
}

impl MemoryCache {
    pub fn new(capacity: usize, default_ttl: Duration) -> Result<Self, CacheError> {
        info!("Creating in-memory cache with capacity: {} and TTL: {:?}", capacity, default_ttl);
        
        let non_zero_capacity = std::num::NonZeroUsize::new(capacity)
            .ok_or_else(|| CacheError::Connection("Cache capacity must be greater than zero".to_string()))?;
        
        Ok(Self {
            cache: tokio::sync::RwLock::new(lru::LruCache::new(non_zero_capacity)),
            default_ttl,
        })
    }

    async fn cleanup_expired(&self) {
        let mut cache = self.cache.write().await;
        let now = std::time::Instant::now();
        let mut keys_to_remove = Vec::new();
        
        // Collect expired keys
        for (key, (_, expiry)) in cache.iter() {
            if let Some(expiry) = expiry {
                if now >= *expiry {
                    keys_to_remove.push(key.clone());
                }
            }
        }
        
        // Remove expired keys
        for key in keys_to_remove {
            cache.pop(&key);
        }
    }
}

#[async_trait]
impl CacheBackend for MemoryCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        self.cleanup_expired().await;
        
        let mut cache = self.cache.write().await;
        
        if let Some((value, expiry)) = cache.get(key) {
            let now = std::time::Instant::now();
            match expiry {
                Some(expiry) if now >= *expiry => {
                    cache.pop(key);
                    Ok(None)
                }
                _ => Ok(Some(value.clone())),
            }
        } else {
            Ok(None)
        }
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<(), CacheError> {
        let expiry = ttl
            .or(Some(self.default_ttl))
            .map(|duration| std::time::Instant::now() + duration);
        
        let mut cache = self.cache.write().await;
        cache.put(key.to_string(), (value, expiry));
        
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        let mut cache = self.cache.write().await;
        Ok(cache.pop(key).is_some())
    }

    async fn clear(&self) -> Result<(), CacheError> {
        let mut cache = self.cache.write().await;
        cache.clear();
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        self.cleanup_expired().await;
        
        let cache = self.cache.read().await;
        Ok(cache.contains(key))
    }
}

/// Hybrid cache with Redis primary and memory fallback
pub struct HybridCache {
    redis: Option<RedisCache>,
    memory: MemoryCache,
    fallback_enabled: bool,
}

impl HybridCache {
    pub async fn new(
        redis_url: Option<&str>,
        memory_capacity: usize,
        default_ttl: Duration,
        key_prefix: &str,
    ) -> Result<Self, CacheError> {
        let redis = if let Some(url) = redis_url {
            match RedisCache::new(url, key_prefix).await {
                Ok(cache) => {
                    info!("Redis cache initialized successfully");
                    Some(cache)
                }
                Err(e) => {
                    warn!("Failed to initialize Redis cache, falling back to memory-only: {}", e);
                    None
                }
            }
        } else {
            info!("No Redis URL provided, using memory-only cache");
            None
        };

        let memory = MemoryCache::new(memory_capacity, default_ttl)?;
        let fallback_enabled = redis.is_some();

        Ok(Self {
            redis,
            memory,
            fallback_enabled,
        })
    }

    pub async fn get_with_fallback<T>(&self, key: &str) -> Result<Option<T>, CacheError>
    where
        T: for<'de> Deserialize<'de> + Serialize,
    {
        // Try Redis first if available
        if let Some(redis) = &self.redis {
            match redis.get(key).await {
                Ok(Some(data)) => {
                    let value: T = serde_json::from_slice(&data)?;
                    debug!("Cache hit from Redis for key: {}", key);
                    return Ok(Some(value));
                }
                Ok(None) => {
                    debug!("Cache miss from Redis for key: {}", key);
                }
                Err(e) => {
                    warn!("Redis cache error for key {}: {}, trying memory", key, e);
                }
            }
        }

        // Fallback to memory cache
        match self.memory.get(key).await {
            Ok(Some(data)) => {
                let value: T = serde_json::from_slice(&data)?;
                debug!("Cache hit from memory for key: {}", key);
                
                // If we have Redis but it failed, try to repopulate it
                if self.redis.is_some() && self.fallback_enabled {
                    if let Err(e) = self.set_without_fallback::<T>(key, &value, None).await {
                        warn!("Failed to repopulate Redis cache for key {}: {}", key, e);
                    }
                }
                
                Ok(Some(value))
            }
            Ok(None) => {
                debug!("Cache miss from memory for key: {}", key);
                Ok(None)
            }
            Err(e) => {
                error!("Memory cache error for key {}: {}", key, e);
                Err(e)
            }
        }
    }

    pub async fn set_with_fallback<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<(), CacheError>
    where
        T: Serialize,
    {
        let data = serde_json::to_vec(value)?;
        
        // Set in Redis if available
        if let Some(redis) = &self.redis {
            if let Err(e) = redis.set(key, data.clone(), ttl).await {
                warn!("Failed to set in Redis cache for key {}: {}", key, e);
            }
        }

        // Always set in memory cache
        self.memory.set(key, data, ttl).await?;
        debug!("Set cache key: {} in memory cache", key);
        
        Ok(())
    }

    async fn set_without_fallback<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<(), CacheError>
    where
        T: Serialize,
    {
        let data = serde_json::to_vec(value)?;
        
        if let Some(redis) = &self.redis {
            redis.set(key, data, ttl).await?;
        }
        
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        let mut deleted = false;
        
        if let Some(redis) = &self.redis {
            if redis.delete(key).await? {
                deleted = true;
            }
        }
        
        if self.memory.delete(key).await? {
            deleted = true;
        }
        
        Ok(deleted)
    }

    pub async fn clear(&self) -> Result<(), CacheError> {
        if let Some(redis) = &self.redis {
            redis.clear().await?;
        }
        
        self.memory.clear().await?;
        Ok(())
    }
}

/// Cache statistics
#[derive(Debug)]
pub struct CacheStats {
    pub entries: usize,
    pub search_entries: usize,
    pub hit_rate: f64,
    pub memory_usage_mb: f64,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub redis_url: Option<String>,
    pub key_prefix: String,
    pub memory_capacity: usize,
    pub default_ttl_seconds: u64,
    pub enable_fallback: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            redis_url: None,
            key_prefix: "jamey".to_string(),
            memory_capacity: 1000,
            default_ttl_seconds: 3600, // 1 hour
            enable_fallback: true,
        }
    }
}

/// Cache manager for Jamey
pub struct CacheManager {
    cache: HybridCache,
    config: CacheConfig,
}

impl CacheManager {
    pub async fn new(config: CacheConfig) -> Result<Self, CacheError> {
        let cache = HybridCache::new(
            config.redis_url.as_deref(),
            config.memory_capacity,
            Duration::from_secs(config.default_ttl_seconds),
            &config.key_prefix,
        ).await?;

        Ok(Self { cache, config })
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> Result<CacheStats, CacheError> {
        Ok(CacheStats {
            entries: 0, // TODO: Implement actual stats
            search_entries: 0,
            hit_rate: 0.0,
            memory_usage_mb: 0.0,
        })
    }

    /// Get access count for a memory entry
    pub async fn get_access_count(&self, _id: Uuid) -> Result<usize, CacheError> {
        Ok(0) // TODO: Implement actual access counting
    }


    /// Cache a memory entry
    pub async fn cache_memory(&self, memory: &crate::memory::Memory) -> Result<(), CacheError> {
        let key = format!("memory:{}", memory.id);
        let ttl = Some(Duration::from_secs(self.config.default_ttl_seconds));
        self.cache.set_with_fallback(&key, memory, ttl).await
    }

    /// Get cached memory entry
    pub async fn get_cached_memory(&self, id: Uuid) -> Result<Option<crate::memory::Memory>, CacheError> {
        let key = format!("memory:{}", id);
        self.cache.get_with_fallback(&key).await
    }

    /// Cache search results
    pub async fn cache_search_results<T>(&self, query: &str, results: &T) -> Result<(), CacheError>
    where
        T: Serialize,
    {
        let key = format!("search:{}", query);
        let ttl = Some(Duration::from_secs(300)); // 5 minutes for search results
        self.cache.set_with_fallback(&key, results, ttl).await
    }

    /// Get cached search results
    pub async fn get_cached_search_results<T>(&self, query: &str) -> Result<Option<T>, CacheError>
    where
        T: for<'de> Deserialize<'de> + Serialize,
    {
        let key = format!("search:{}", query);
        self.cache.get_with_fallback(&key).await
    }

    /// Invalidate memory cache
    pub async fn invalidate_memory(&self, id: Uuid) -> Result<bool, CacheError> {
        let key = format!("memory:{}", id);
        self.cache.delete(&key).await
    }

    /// Clear all cache
    pub async fn clear_all(&self) -> Result<(), CacheError> {
        self.cache.clear().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Memory, MemoryType};
    use chrono::Utc;

    #[tokio::test]
    async fn test_memory_cache() {
        let config = CacheConfig {
            redis_url: None,
            memory_capacity: 10,
            default_ttl_seconds: 60,
            enable_fallback: false,
        };

        let cache = CacheManager::new(config).await.unwrap();

        let memory = Memory {
            id: Uuid::new_v4(),
            memory_type: MemoryType::Conversation,
            content: "Test memory".to_string(),
            embedding: vec![0.1; 1536],
            metadata: serde_json::json!({"test": true}),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };

        // Cache the memory
        cache.cache_memory(&memory).await.unwrap();

        // Retrieve from cache
        let cached = cache.get_cached_memory(memory.id).await.unwrap();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().content, "Test memory");
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let config = CacheConfig::default();
        let cache = CacheManager::new(config).await.unwrap();

        let memory_id = Uuid::new_v4();
        let memory = Memory {
            id: memory_id,
            memory_type: MemoryType::Conversation,
            content: "Test memory".to_string(),
            embedding: vec![0.1; 1536],
            metadata: serde_json::json!({"test": true}),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };

        // Cache the memory
        cache.cache_memory(&memory).await.unwrap();

        // Verify it's cached
        let cached = cache.get_cached_memory(memory_id).await.unwrap();
        assert!(cached.is_some());

        // Invalidate
        let invalidated = cache.invalidate_memory(memory_id).await.unwrap();
        assert!(invalidated);

        // Verify it's gone
        let cached = cache.get_cached_memory(memory_id).await.unwrap();
        assert!(cached.is_none());
    }
}
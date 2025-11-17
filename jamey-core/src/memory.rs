use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{error, instrument};
use uuid::Uuid;
use validator::{Validate, ValidationError};
use crate::profiling::TimingGuard;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("Database error: {0}")]
    Database(#[from] tokio_postgres::Error),
    #[error("Pool error: {0}")]
    Pool(#[from] deadpool_postgres::PoolError),
    #[error("Vector dimension mismatch: expected {expected}, got {actual}")]
    VectorDimension { expected: usize, actual: usize },
    #[error("Memory not found: {0}")]
    NotFound(Uuid),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Validation error: {0}")]
    Validation(#[from] validator::ValidationError),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    Conversation,
    Knowledge,
    Experience,
    Skill,
    Preference,
}

impl TryFrom<&str> for MemoryType {
    type Error = anyhow::Error;
    
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "conversation" => Ok(MemoryType::Conversation),
            "knowledge" => Ok(MemoryType::Knowledge),
            "experience" => Ok(MemoryType::Experience),
            "skill" => Ok(MemoryType::Skill),
            "preference" => Ok(MemoryType::Preference),
            _ => Err(anyhow::anyhow!("Invalid memory type: {}", s))
        }
    }
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryType::Conversation => write!(f, "Conversation"),
            MemoryType::Knowledge => write!(f, "Knowledge"),
            MemoryType::Experience => write!(f, "Experience"),
            MemoryType::Skill => write!(f, "Skill"),
            MemoryType::Preference => write!(f, "Preference"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Memory {
    pub id: Uuid,
    pub memory_type: MemoryType,
    #[validate(length(min = 1, max = 32768))]
    pub content: String,
    #[validate(custom(function = "validate_embedding"))]
    pub embedding: Vec<f32>,
    #[validate(custom(function = "validate_metadata"))]
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
}

fn validate_embedding(embedding: &[f32]) -> Result<(), ValidationError> {
    if embedding.is_empty() {
        return Err(ValidationError::new("embedding_empty"));
    }
    if embedding.len() > 4096 {
        return Err(ValidationError::new("embedding_too_large"));
    }
    if embedding.iter().any(|x| x.is_nan() || x.is_infinite()) {
        return Err(ValidationError::new("invalid_embedding_values"));
    }
    Ok(())
}

fn validate_metadata(metadata: &serde_json::Value) -> Result<(), ValidationError> {
    let serialized = serde_json::to_string(metadata)
        .map_err(|_| ValidationError::new("invalid_json"))?;
    
    if serialized.len() > 16384 {
        return Err(ValidationError::new("metadata_too_large"));
    }
    
    if let Some(obj) = metadata.as_object() {
        if obj.len() > 50 {
            return Err(ValidationError::new("too_many_fields"));
        }
        for (key, value) in obj {
            if key.len() > 64 {
                return Err(ValidationError::new("key_too_long"));
            }
            if let Some(s) = value.as_str() {
                if s.len() > 1024 {
                    return Err(ValidationError::new("value_too_long"));
                }
            }
        }
    }
    Ok(())
}

#[async_trait]
pub trait MemoryStore {
    async fn store(&self, memory: Memory) -> Result<Uuid>;
    async fn retrieve(&self, id: Uuid) -> Result<Memory>;
    async fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<Memory>>;
    async fn update(&self, id: Uuid, content: &str, embedding: &[f32]) -> Result<()>;
    async fn delete(&self, id: Uuid) -> Result<()>;
    async fn list_paginated(&self, limit: usize, offset: usize) -> Result<(Vec<Memory>, i64)>;
}

pub struct PostgresMemoryStore {
    pool: Pool,
    vector_dim: usize,
}

impl PostgresMemoryStore {
    pub async fn new(pool: Pool, vector_dim: usize) -> Result<Self> {
        let client = pool.get().await?;
        
        // Ensure pgvector extension is installed first
        client
            .execute("CREATE EXTENSION IF NOT EXISTS vector", &[])
            .await?;
        
        // Create the memories table if it doesn't exist
        client
            .execute(
                "CREATE TABLE IF NOT EXISTS memories (
                    id UUID PRIMARY KEY,
                    memory_type TEXT NOT NULL,
                    content TEXT NOT NULL,
                    embedding vector(1536) NOT NULL,
                    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    last_accessed TIMESTAMPTZ NOT NULL DEFAULT NOW()
                )",
                &[],
            )
            .await?;

        // Create an index for vector similarity search
        client
            .execute(
                "CREATE INDEX IF NOT EXISTS memories_embedding_idx ON memories 
                 USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100)",
                &[],
            )
            .await?;

        Ok(Self { pool, vector_dim })
    }

    fn validate_vector_dimension(&self, embedding: &[f32]) -> Result<(), MemoryError> {
        if embedding.is_empty() {
            return Err(MemoryError::VectorDimension {
                expected: self.vector_dim,
                actual: 0,
            });
        }
        if embedding.len() != self.vector_dim {
            return Err(MemoryError::VectorDimension {
                expected: self.vector_dim,
                actual: embedding.len(),
            });
        }
        if embedding.iter().any(|x| x.is_nan() || x.is_infinite()) {
            return Err(MemoryError::InvalidRequest("Embedding contains invalid values".to_string()));
        }
        Ok(())
    }

    fn sanitize_content(content: &str) -> String {
        content.chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
            .take(32768)
            .collect()
    }

    fn validate_metadata(metadata: &serde_json::Value) -> Result<(), MemoryError> {
        validate_metadata(metadata).map_err(MemoryError::Validation)
    }
}

#[async_trait]
impl MemoryStore for PostgresMemoryStore {
    #[instrument(skip(self, memory), fields(memory_type = %memory.memory_type))]
    async fn store(&self, mut memory: Memory) -> Result<Uuid> {
        let _timer = TimingGuard::new("memory_store");
        
        self.validate_vector_dimension(&memory.embedding)?;
        Self::validate_metadata(&memory.metadata)?;
        
        // Sanitize content
        memory.content = Self::sanitize_content(&memory.content);
        if memory.content.is_empty() {
            return Err(MemoryError::InvalidRequest("Content cannot be empty".to_string()).into());
        }

        let client = self.pool.get().await?;
        let id = Uuid::new_v4();
        let memory_type_str = memory.memory_type.to_string();
        let metadata_json = serde_json::to_value(&memory.metadata)?;
        
        // Convert vector to string format for PostgreSQL
        let embedding_str = format!("[{}]",
            memory.embedding.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        
        client
            .execute(
                "INSERT INTO memories (id, memory_type, content, embedding, metadata)
                 VALUES ($1::uuid, $2, $3, $4::vector, $5::jsonb)",
                &[
                    &id,
                    &memory_type_str,
                    &memory.content,
                    &embedding_str,
                    &metadata_json,
                ],
            )
            .await?;

        Ok(id)
    }

    #[instrument(skip(self), fields(memory_id = %id))]
    async fn retrieve(&self, id: Uuid) -> Result<Memory> {
        let _timer = TimingGuard::new("memory_retrieve");
        let client = self.pool.get().await?;

        let row = client
            .query_one(
                "UPDATE memories 
                 SET last_accessed = NOW()
                 WHERE id = $1
                 RETURNING id, memory_type, content, embedding, metadata, created_at, last_accessed",
                &[&id],
            )
            .await?;

        // Get embedding as string and parse it
        let embedding_str: String = row.get("embedding");
        let embedding: Vec<f32> = embedding_str
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(|s| s.trim().parse::<f32>().map_err(|e|
                anyhow::anyhow!("Failed to parse embedding value: {}", e)
            ))
            .collect::<Result<Vec<f32>>>()?;

        let memory_type_str: String = row.get("memory_type");
        let memory_type = MemoryType::try_from(memory_type_str.as_str())
            .map_err(|e| MemoryError::InvalidRequest(format!("Invalid memory type: {}", e)))?;

        Ok(Memory {
            id: row.get("id"),
            memory_type,
            content: row.get("content"),
            embedding,
            metadata: row.get("metadata"),
            created_at: row.get("created_at"),
            last_accessed: row.get("last_accessed"),
        })
    }

    #[instrument(skip(self, query_embedding), fields(limit = limit))]
    async fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<Memory>> {
        let _timer = TimingGuard::new("memory_search");
        self.validate_vector_dimension(query_embedding)?;
        let client = self.pool.get().await?;

        // Convert query embedding to string format
        let query_embedding_str = format!("[{}]",
            query_embedding.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );
        
        let rows = client
            .query(
                "SELECT id, memory_type, content, embedding, metadata, created_at, last_accessed,
                        embedding <=> $1::vector as distance
                 FROM memories
                 ORDER BY distance
                 LIMIT $2",
                &[&query_embedding_str, &(limit as i64)],
            )
            .await?;

        let mut memories = Vec::with_capacity(rows.len());
        for row in rows {
            // Get embedding as string and parse it
            let embedding_str: String = row.get("embedding");
            let embedding: Vec<f32> = embedding_str
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|s| s.trim().parse::<f32>().map_err(|e|
                    anyhow::anyhow!("Failed to parse embedding value: {}", e)
                ))
                .collect::<Result<Vec<f32>>>()?;
            
            let memory_type_str: String = row.get("memory_type");
            let memory_type = MemoryType::try_from(memory_type_str.as_str())
                .map_err(|e| MemoryError::InvalidRequest(format!("Invalid memory type: {}", e)))?;
            
            memories.push(Memory {
                id: row.get("id"),
                memory_type,
                content: row.get("content"),
                embedding,
                metadata: row.get("metadata"),
                created_at: row.get("created_at"),
                last_accessed: row.get("last_accessed"),
            });
        }

        Ok(memories)
    }

    #[instrument(skip(self, content, embedding), fields(memory_id = %id))]
    async fn update(&self, id: Uuid, content: &str, embedding: &[f32]) -> Result<()> {
        let _timer = TimingGuard::new("memory_update");
        self.validate_vector_dimension(embedding)?;
        
        // Sanitize content
        let content = Self::sanitize_content(content);
        if content.is_empty() {
            return Err(MemoryError::InvalidRequest("Content cannot be empty".to_string()).into());
        }

        let client = self.pool.get().await?;

        // Convert embedding to string format
        let embedding_str = format!("[{}]",
            embedding.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );
        
        let rows_affected = client
            .execute(
                "UPDATE memories 
                 SET content = $2, embedding = $3::vector, last_accessed = NOW()
                 WHERE id = $1",
                &[&id, &content, &embedding_str],
            )
            .await?;

        if rows_affected == 0 {
            return Err(MemoryError::NotFound(id).into());
        }

        Ok(())
    }

    #[instrument(skip(self), fields(memory_id = %id))]
    async fn delete(&self, id: Uuid) -> Result<()> {
        let _timer = TimingGuard::new("memory_delete");
        let client = self.pool.get().await?;

        let rows_affected = client
            .execute("DELETE FROM memories WHERE id = $1", &[&id])
            .await?;

        if rows_affected == 0 {
            return Err(MemoryError::NotFound(id).into());
        }

        Ok(())
    }

    /// List memories with pagination support
    ///
    /// # Arguments
    /// * `limit` - Maximum number of memories to return
    /// * `offset` - Number of memories to skip
    ///
    /// # Returns
    /// A tuple of (memories, total_count) for pagination metadata
    #[instrument(skip(self), fields(limit = limit, offset = offset))]
    async fn list_paginated(&self, limit: usize, offset: usize) -> Result<(Vec<Memory>, i64)> {
        let _timer = TimingGuard::new("memory_list_paginated");
        let client = self.pool.get().await?;

        // Get total count
        let count_row = client
            .query_one("SELECT COUNT(*) as count FROM memories", &[])
            .await?;
        let total_count: i64 = count_row.get("count");

        // Get paginated results
        let rows = client
            .query(
                "SELECT id, memory_type, content, embedding, metadata, created_at, last_accessed
                 FROM memories
                 ORDER BY created_at DESC
                 LIMIT $1 OFFSET $2",
                &[&(limit as i64), &(offset as i64)],
            )
            .await?;

        let mut memories = Vec::with_capacity(rows.len());
        for row in rows {
            // Get embedding as string and parse it
            let embedding_str: String = row.get("embedding");
            let embedding: Vec<f32> = embedding_str
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|s| s.trim().parse::<f32>().map_err(|e|
                    anyhow::anyhow!("Failed to parse embedding value: {}", e)
                ))
                .collect::<Result<Vec<f32>>>()?;
            
            let memory_type_str: String = row.get("memory_type");
            let memory_type = MemoryType::try_from(memory_type_str.as_str())
                .map_err(|e| MemoryError::InvalidRequest(format!("Invalid memory type: {}", e)))?;
            
            memories.push(Memory {
                id: row.get("id"),
                memory_type,
                content: row.get("content"),
                embedding,
                metadata: row.get("metadata"),
                created_at: row.get("created_at"),
                last_accessed: row.get("last_accessed"),
            });
        }

        Ok((memories, total_count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadpool_postgres::{Config, Runtime};
    use tokio_postgres::NoTls;

    async fn create_test_pool() -> Pool {
        let mut cfg = Config::new();
        cfg.host = Some("localhost".to_string());
        cfg.dbname = Some("jamey_test".to_string());
        cfg.user = Some("jamey".to_string());
        cfg.password = Some("test_password".to_string());
        log::info!("Connecting to test database with configured credentials");
        cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap()
    }

    #[tokio::test]
    async fn test_memory_store() {
        let pool = create_test_pool().await;
        let store = PostgresMemoryStore::new(pool, 1536).await.unwrap();

        // Test storing and retrieving a memory
        let memory = Memory {
            id: Uuid::new_v4(),
            memory_type: MemoryType::Conversation,
            content: "Test memory".to_string(),
            embedding: vec![0.0; 1536],
            metadata: serde_json::json!({"test": true}),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
        };

        let id = store.store(memory.clone()).await.unwrap();
        let retrieved = store.retrieve(id).await.unwrap();

        assert_eq!(retrieved.content, memory.content);
        assert_eq!(retrieved.embedding.len(), memory.embedding.len());

        // Test searching
        let results = store.search(&vec![0.0; 1536], 1).await.unwrap();
        assert!(!results.is_empty());

        // Test updating
        store
            .update(id, "Updated content", &vec![0.0; 1536])
            .await
            .unwrap();

        // Test deleting
        store.delete(id).await.unwrap();
    }
}